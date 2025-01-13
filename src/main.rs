mod build_sys;
mod cli;
mod config;
mod constants;
mod kiln_package;
mod lexer;
mod package_manager;
mod safety;
mod utils;
mod valgrind;

use anyhow::{anyhow, Result};
use clap::Parser;
use config::Config;
use constants::{CONFIG_FILE, PACKAGE_DIR, SEPETATOR};
use std::{env, fs, path::Path, process};
use utils::Language;
use valgrind::VgOutput;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli_args: cli::CliCommand;
    let raw_cli_args = std::env::args().collect::<Vec<String>>();
    if raw_cli_args.len() < 2 {
        // Let the program fail and have Clap display it's help message
        cli_args = cli::CliCommand::parse();
    } else if raw_cli_args[1] == "run" || raw_cli_args[1] == "build" {
        let mut profile = "--debug".to_string();
        let mut args = vec![];
        if raw_cli_args.len() >= 3
            && raw_cli_args[2].starts_with("--")
            && raw_cli_args[2].len() > 2
            && raw_cli_args[2] != "--valgrind"
        {
            // Extracts compilation profile

            profile = raw_cli_args[2].clone();
        }
        if let Some(idx) = raw_cli_args.iter().position(|i| i == "--") {
            // Extracts passthough CLI arguments (kiln run)
            assert!([2_usize, 3_usize].contains(&idx));
            args = raw_cli_args[(idx + 1)..].to_vec();
        } else {
            // verify structure of CLI arguments
            if !(raw_cli_args.len() <= 3) {
                println!("Invalid CLI arguments");
                process::exit(1);
            }
        }
        let valgrind = raw_cli_args.contains(&"--valgrind".to_string());
        cli_args = cli::CliCommand {
            command: cli::Commands::new(&raw_cli_args[1], &profile, args, valgrind),
        }
    } else {
        cli_args = cli::CliCommand::parse();
    }

    let cwd = env::current_dir().unwrap();
    let config = Config::from(&cwd.join(CONFIG_FILE));

    match cli_args.command {
        cli::Commands::Init { language } => {
            let cwd = env::current_dir().unwrap();

            if let Err(e) = build_sys::create_project(&cwd, language) {
                println!("An error occurred while creating the project:\n{}", e);
                process::exit(1);
            }
        }
        cli::Commands::New {
            proj_name,
            language,
        } => {
            let mut target_dir = env::current_dir().unwrap();
            target_dir.push(proj_name);
            if target_dir.exists() {
                println!("Error: file of directory already exists");
                process::exit(1);
            }
            fs::create_dir(&target_dir).unwrap();

            if let Err(e) = build_sys::create_project(&target_dir, language) {
                println!("An error occurred while creating the project:\n{}", e);
            }
        }
        cli::Commands::GenHeaders => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let config = config.unwrap();
            if config.project.language != "c" {
                println!("Unfortunately, generating header files is only avilalbe for C.");
                println!("Stay tuned!! C++/CUDA support coming soon!");
                process::exit(0);
            }

            if let Err(err) = handle_headers(&config) {
                println!("An error occured while generating header files:\n{}", err);
                process::exit(1);
            }
        }
        cli::Commands::Add { dep } => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let mut config = config.unwrap();
            let (owner, proj_name) = package_manager::parse_github_uri(&dep).unwrap();

            let res = package_manager::resolve_adding_package(&mut config, owner, proj_name, None);

            res.await.unwrap();

            config.to_disk(Path::new(constants::CONFIG_FILE));
        }
        cli::Commands::PurgeGlobalInstalls => {
            let pkg_dir = (*PACKAGE_DIR).clone();

            fs::remove_dir_all(&pkg_dir).unwrap();
            fs::create_dir(&pkg_dir).unwrap();
        }
        cli::Commands::Build { profile } => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let config = config.unwrap();

            if let Err(e) = handle_warnings(&config) {
                eprintln!("An error occured during static analysis:\n{}", e);
                process::exit(1);
            }
            if let Err(e) = handle_build(&profile, &config) {
                eprintln!("An error occured while building the project:\n{}", e);
                process::exit(1);
            }
        }
        cli::Commands::Run {
            profile,
            args,
            valgrind,
        } => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let config = config.unwrap();

            if let Err(e) = handle_warnings(&config) {
                eprintln!("An error occured during static analysis:\n{}", e);
                process::exit(1);
            }
            if let Err(e) = handle_build(&profile, &config) {
                eprintln!("An error occured while building the project:\n{}", e);
                process::exit(1);
            }

            if valgrind {
                if env::consts::OS != "linux" {
                    eprintln!("Valgrind is only supported for linux.");
                    std::process::exit(1);
                }
                let exe_path = cwd
                    .join("build")
                    .join(&profile[2..])
                    .join(config.project.name);

                let bin = exe_path.to_str().unwrap();
                let valgrind_out = match safety::exec_w_valgrind(bin, &args) {
                    Ok(vg) => vg,
                    Err(e) => {
                        if !e
                            .to_string()
                            .to_lowercase()
                            .contains("error parsing valgrind")
                        {
                            eprintln!("Error executing with valgrind");
                            process::exit(1);
                        }
                        VgOutput::default()
                    }
                };

                if valgrind_out.errors.len() > 0 {
                    println!("{}\n", *SEPETATOR);
                    safety::print_vg_errors(&valgrind_out);
                }
            } else {
                // If use_valgrind = false
                let err = handle_execution(&profile, &config, &cwd, &args);
                if let Err(e) = err {
                    eprintln!("Code build sucessfully, but failed to execute:\n{}", e);
                    process::exit(1);
                }
            }
        }
    }
}

/// Returns true if there were warnings and false if there was no warnings.
fn handle_warnings(config: &Config) -> Result<Vec<safety::Warning>> {
    if !config.get_kiln_static_analysis() {
        return Ok(vec![]);
    }

    let warnings = safety::check_files(&config.project.language)?;

    for w in &warnings {
        utils::print_warning(
            "Kiln",
            &w.filename,
            &format!("{}", w.line),
            &format!("{:?}", w.warning_type),
            &w.msg,
        );
    }
    if warnings.len() > 0 {
        println!("{}", *SEPETATOR);
    }

    Ok(warnings)
}

fn handle_build(profile: &str, config: &Config) -> Result<()> {
    if !profile.starts_with("--") {
        eprintln!("Error: profile must start with `--`");
        process::exit(1);
    }
    let cwd = env::current_dir().unwrap();

    let build_dir = cwd.join("build").join(&profile[2..]);
    if !build_dir.exists() {
        fs::create_dir_all(&build_dir).unwrap();
    }

    let lang = Language::new(&config.project.language).unwrap();
    let mut link_file = vec![];
    build_sys::link_dep_files(&cwd, lang, &mut link_file)?;
    build_sys::link_proj_files(&config, &cwd, lang, &mut link_file)
        .map_err(|err| anyhow!("Failed to link source files: {}", err))?;

    let link_lib = build_sys::link_sys_lib(&cwd);
    let opt_flags = build_sys::opt_flags(&profile, config).unwrap();

    let header_dir = build_sys::link_dep_headers(&cwd)?;
    let so_dir = build_sys::link_dep_shared_obj(&cwd)?;

    let compilation_cmd =
        build_sys::full_compilation_cmd(
            config, 
            &profile, 
            &link_file, 
            &link_lib,
            &header_dir,
            &so_dir,
            &opt_flags
        )?;

    dbg!(compilation_cmd.join(" "));

    let child = process::Command::new(&compilation_cmd[0])
        .args(&compilation_cmd[1..])
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .stdin(process::Stdio::inherit())
        .output()?;

    if !child.status.success() {
        return Err(anyhow!(
            "Compilation command exited with non-zero exit code"
        ));
    }

    Ok(())
}

fn handle_execution(
    profile: &str,
    config: &Config,
    project_dir: &Path,
    passthough_args: &[String],
) -> Result<()> {
    if !profile.starts_with("--") {
        return Err(anyhow!("Error: profile must start with `--`"));
    }

    let bin_path = project_dir
        .join("build")
        .join(&profile[2..])
        .join(&config.project.name);

    if !bin_path.exists() {
        return Err(anyhow!("Binary {:?} does not exist", bin_path));
    }

    let output = process::Command::new(&bin_path)
        .args(passthough_args)
        .stdin(process::Stdio::inherit())
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|e| anyhow!("Failed to run {:?} binary: {}", bin_path, e))?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(1);
        process::exit(code);
    }

    Ok(())
}

fn handle_headers(config: &Config) -> Result<()> {
    let cwd = env::current_dir()?;
    let src_dir = config.get_src_dir();
    let inc_dir = config.get_include_dir();

    let src_dir = cwd.join(src_dir);
    let inc_dir = cwd.join(inc_dir);

    for file in fs::read_dir(src_dir).unwrap() {
        if let Ok(file) = file {
            let raw_name = file.file_name();
            let raw_name = raw_name.to_str().unwrap().rsplit_once(".").unwrap().0;
            if raw_name == "main" {
                continue;
            }
            let header_name = format!("{}.h", raw_name);

            let mut code = fs::read_to_string(file.path())?;
            code = lexer::clean_source_code(code);
            let tokens = lexer::tokenize(&code)?;

            let mut code_h =
                fs::read_to_string(inc_dir.join(&header_name)).unwrap_or("".to_string());
            code_h = lexer::clean_source_code(code_h);
            let tokens_h = lexer::tokenize(&code_h)?;

            let fn_defs = lexer::get_fn_def(&tokens);
            let includes = lexer::get_includes(&tokens);
            let structs = lexer::get_structs(&tokens_h);

            let mut headers = String::new();

            headers.push_str(&format!("#ifndef {}_H\n", raw_name.to_uppercase()));
            headers.push_str(&format!("#define {}_H\n\n", raw_name.to_uppercase()));

            for &inc in &includes {
                let s = lexer::Token::tokens_to_string(inc);
                headers.push_str(&s);
                headers.push('\n');
            }
            headers.push('\n');
            for &struc in &structs {
                headers.push_str(&lexer::Token::struct_tokens_to_string(struc).trim());
                headers.push_str("\n\n");
            }
            for &func in &fn_defs {
                let s = lexer::Token::tokens_to_string(func);
                headers.push_str(&s);
                headers.push_str(";\n");
            }
            headers.push('\n');
            headers.push_str(&format!("#endif // {}_H", raw_name.to_uppercase()));

            fs::write(inc_dir.join(header_name), headers)?;
        }
    }
    Ok(())
}
