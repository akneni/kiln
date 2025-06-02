mod build_sys;
mod cli;
mod config;
mod constants;
mod header_gen;
mod local_dev;
mod packaging;
mod testing;
mod utils;

use anyhow::{anyhow, Result};
use clap::Parser;
use config::Config;
use constants::{CONFIG_FILE, DEV_ENV_CFG_FILE, PACKAGE_DIR, SEPARATOR};
use header_gen::lexer_c;
use local_dev::{dev_env_config, editors};
use packaging::package_manager::{self, PkgError};
use std::{env, fs, io::Write, path::Path, process, time};
use strum::IntoEnumIterator;
use testing::safety;
use utils::Language;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli_args: cli::CliCommand;
    let raw_cli_args = std::env::args().collect::<Vec<String>>();
    if raw_cli_args.len() < 2 {
        // Let the program fail and have Clap display it's help message
        cli_args = cli::CliCommand::parse();
    } 
    else if matches!(raw_cli_args[1].as_str(), "run" | "build" | "build-trace") {
        let mut profile = "--debug".to_string();
        let mut args = vec![];
        if raw_cli_args.len() >= 3
            && raw_cli_args[2].starts_with("--")
            && raw_cli_args[2].len() > 2
        {
            // Extracts compilation profile
            profile = raw_cli_args[2].clone();
        }
        if let Some(idx) = raw_cli_args.iter().position(|i| i == "--") {
            // Extracts passthrough CLI arguments (kiln run)
            assert!([2_usize, 3_usize].contains(&idx));
            args = raw_cli_args[(idx + 1)..].to_vec();
        } 
        else {
            // verify structure of CLI arguments
            if !(raw_cli_args.len() <= 3) {
                println!("Invalid CLI arguments");
                process::exit(1);
            }
        }
        cli_args = cli::CliCommand {
            command: cli::Commands::new(&raw_cli_args[1], &profile, args),
        }
    } 
    else {
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
        cli::Commands::GenHeaders { args } => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let config = config.unwrap();
            if config.project.language != "c" {
                println!("Unfortunately, generating header files is only available for C.");
                println!("Stay tuned!! C++/CUDA support coming soon!");
                process::exit(0);
            }

            if let Err(err) = handle_gen_headers(&config, args) {
                println!("An error occurred while generating header files:\n{}", err);
                process::exit(1);
            }
        }
        cli::Commands::Add { dep_uri } => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let mut config = config.unwrap();

            let (owner, proj_name) = package_manager::parse_github_uri(&dep_uri).unwrap();
            let res = package_manager::resolve_adding_package(&mut config, owner, proj_name, None);

            if let Err(err) = res.await {
                match &err {
                    PkgError::Reqwest(e) => {
                        let e_str = format!("{}", e);
                        if e_str.contains("TimedOut") {
                            dbg!(e);
                            eprintln!("Request timed out, please check internet connection");
                        } else {
                            eprintln!("An unknown error occurred:\n{}", err);
                        }
                    }
                    _ => {
                        eprintln!("An unknown error occurred:\n{}", err);
                    }
                }
                std::process::exit(1);
            }

            config.to_disk(Path::new(constants::CONFIG_FILE));

            let cwd = env::current_dir().unwrap();
            editors::handle_editor_includes(&config, &cwd).unwrap();
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
            handle_check_installs(&config).await;

            if let Err(e) = handle_warnings(&config) {
                eprintln!("An error occurred during static analysis:\n{}", e);
                process::exit(1);
            }

            for &b_type in config.project.build_type.iter() {
                if let Err(e) = handle_build(&profile, &config, b_type) {
                    eprintln!(
                        "An error occurred while building the project (build mode {:?}):\n{}",
                        b_type, e
                    );
                    process::exit(1);
                }
            }
        }
        cli::Commands::Run {
            profile,
            args,
        } => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }

            let config = config.unwrap();
            if !config.project.build_type.contains(&config::BuildType::Exe) {
                eprintln!("Cannot run a non executable project");
                process::exit(1);
            }

            handle_check_installs(&config).await;

            if let Err(e) = handle_warnings(&config) {
                eprintln!("An error occurred during static analysis:\n{}", e);
                process::exit(1);
            }
            if let Err(e) = handle_build(&profile, &config, config::BuildType::Exe) {
                eprintln!("An error occurred while building the project:\n{}", e);
                process::exit(1);
            }

            let err = handle_execution(&profile, &config, &cwd, &args);
            if let Err(e) = err {
                eprintln!("Code build successfully, but failed to execute:\n{}", e);
                process::exit(1);
            }
            
        }
        cli::Commands::BuildTrace { profile } => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let config = config.unwrap();
            handle_check_installs(&config).await;

            if let Err(e) = handle_warnings(&config) {
                eprintln!("An error occurred during static analysis:\n{}", e);
                process::exit(1);
            }

            for &b_type in config.project.build_type.iter() {
                println!("BuildType: {:?}", b_type);

                let comp_cmd = build_compilation_cmd(&profile, &config, b_type);

                match comp_cmd {
                    Ok(v) => {
                        println!("{}\n", v.join(" "));
                    }
                    Err(e) => {
                        eprintln!(
                            "An error occurred while building the project (build mode {:?}):\n{}",
                            b_type, e
                        );
                        process::exit(1);
                    }
                }
            }

        }
        cli::Commands::Test { tests } => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let config = config.unwrap();

            let mut files_to_test = vec![];

            if let Some(tests) = tests.as_ref() {
                files_to_test.extend_from_slice(&tests);
            } 
            else if let Ok(test_dir) = Path::new("tests").read_dir() {
                for file in test_dir {
                    if let Ok(file) = file {                           
                        let filepath = file.path();
                        let filepath = filepath.to_str()
                            .unwrap();
                        files_to_test.push(filepath.to_string());
                    }
                }
            } 
            else {
                eprintln!("unable to read test directory");
                process::exit(1);
            }

            let seperator = "=".repeat(40);
            println!("\n\n");

            for file in &files_to_test {
                println!("{a}\n{b:?}\n{a}", a=seperator, b=file);

                let res = handle_tests("--debug", &config, file);
                if let Err(err) = res {
                    println!("{}", err);
                }

                println!("{}\n\n\n", seperator);
            }
        }
        cli::Commands::LocalDev { subcommand } => match subcommand {
            cli::LocalDevSubCmd::SetEditor => {
                if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                    println!("{}", e);
                    process::exit(1);
                }
                let config = config.unwrap();

                let cwd = env::current_dir().unwrap();
                let editor_types: Vec<dev_env_config::EditorType> =
                    dev_env_config::EditorType::iter().collect();

                for (i, e) in editor_types.iter().enumerate() {
                    println!("{}) {:?}", i, e);
                }
                println!("------------------");
                println!("Choose an editor: ");
                std::io::stdout().flush().unwrap();

                let mut s_in = "".to_string();
                std::io::stdin().read_line(&mut s_in).unwrap();

                if let Ok(editor_num) = s_in.trim().parse::<usize>() {
                    if editor_num >= editor_types.len() {
                        eprintln!("Index `{}` doesn't match any editor", s_in);
                        process::exit(0);
                    }

                    let local_config = dev_env_config::DevEnvConfig {
                        editor: Some(editor_types[editor_num]),
                    };

                    let local_config_str = toml::to_string(&local_config).unwrap();

                    let dev_env_f = cwd.join(DEV_ENV_CFG_FILE);

                    fs::write(&dev_env_f, &local_config_str).unwrap();
                } else {
                    eprintln!("Error parsing `{}`", s_in);
                    process::exit(0);
                }

                editors::handle_editor_includes(&config, &cwd).unwrap();
            }
            cli::LocalDevSubCmd::UpdateEditorInc => {
                if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                    println!("{}", e);
                    process::exit(1);
                }
                let config = config.unwrap();
                let cwd = env::current_dir().unwrap();

                editors::handle_editor_includes(&config, cwd).unwrap();
            }
        },
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
        println!("{}", *SEPARATOR);
    }

    Ok(warnings)
}

fn build_compilation_cmd(profile: &str, config: &Config, build_type: config::BuildType) -> Result<Vec<String>> {
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
    build_sys::link_dep_files(&config, lang, &mut link_file)?;
    build_sys::link_proj_files(&config, &cwd, lang, &mut link_file)
        .map_err(|err| anyhow!("Failed to link source files: {}", err))?;

    let link_lib = build_sys::link_sys_lib(&cwd);
    let opt_flags = build_sys::opt_flags(&profile, config).unwrap();

    let header_dirs = build_sys::link_dep_headers(&config)?;
    let so_dir = build_sys::link_dep_shared_obj(&cwd)?;

    let compilation_cmd = build_sys::full_compilation_cmd(
        config,
        &profile,
        &link_file,
        &link_lib,
        &header_dirs,
        &so_dir,
        &opt_flags,
        build_type,
    )?;

    Ok(compilation_cmd)
}

fn handle_build(profile: &str, config: &Config, build_type: config::BuildType) -> Result<()> {
    let compilation_cmd = build_compilation_cmd(profile, config, build_type)?;

    #[cfg(debug_assertions)]
    {
        println!("{}\n\n", compilation_cmd.join(" "));
    }

    let build_dir = match build_type {
        config::BuildType::StaticLibrary => env::current_dir()
            .unwrap()
            .join("build")
            .join(&profile[2..])
            .join("obj"),
        _ => env::current_dir().unwrap(),
    };

    let command = compilation_cmd.join(" ");
    let (shell, flag) = if cfg!(target_os = "windows") {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };

    let child = process::Command::new(shell)
        .arg(flag)
        .arg(&command)
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .stdin(process::Stdio::inherit())
        .current_dir(&build_dir)
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
    passthrough_args: &[String],
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
        .args(passthrough_args)
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

fn handle_gen_headers(config: &Config, mut files: Option<Vec<String>>) -> Result<()> {
    let cwd = env::current_dir()?;
    let src_dir = config.get_src_dir();
    let inc_dir = config.get_include_dir();

    let src_dir = cwd.join(src_dir);
    let inc_dir = cwd.join(inc_dir);

    files.as_mut().map(|v| {
        for i in 0..v.len() {
            let idx = v[i].rfind('/');
            if let Some(idx) = idx {
                v[i] = v[i][(idx+1)..].to_string();
            }
            
        }
    });   

    for file in fs::read_dir(&src_dir).unwrap() {
        if let Ok(file) = file {
            let raw_name = file.file_name();
            let (raw_name, file_ext) = raw_name.to_str().unwrap().rsplit_once(".").unwrap();
            if raw_name == "main" || file_ext != "c" {
                continue;
            }

            if let Some(files) = &files {
                let full_name = format!("{}.{}", raw_name, file_ext);
                if !files.contains(&full_name) {
                    continue;
                }
            }

            let header_name = format!("{}.h", raw_name);

            let code = fs::read_to_string(file.path())?;
            let tokens = lexer_c::tokenize(&code)?;

            let code_h = fs::read_to_string(inc_dir.join(&header_name)).unwrap_or("".to_string());
            let tokens_h = lexer_c::tokenize(&code_h)?;

            let mut defines_h = lexer_c::get_defines(&tokens_h);
            let mut udts_h = lexer_c::get_udts(&tokens_h);
            let mut includes_h = lexer_c::get_includes(&tokens_h);

            let fn_defs = lexer_c::get_fn_def(&tokens);
            let includes = lexer_c::get_includes(&tokens);
            let defines = lexer_c::get_defines(&tokens);
            let udts = lexer_c::get_udts(&tokens);

            // Ensure headerfiles don't include themselves
            let includes = header_gen::filter_out_includes(&includes, raw_name);

            // Skip the first definition to skip the #ifndef NAME_H #define NAME_H
            if defines_h.len() > 0 {
                defines_h.remove(0);
            }

            let res = header_gen::merge_defines(&mut defines_h, &defines);
            if let Err(e) = res {
                eprintln!("Error: {}", e);
                process::exit(1);
            }

            let res = header_gen::merge_udts(&mut udts_h, &udts);
            if let Err(e) = res {
                eprintln!("Error: {}", e);
                process::exit(1);
            }

            header_gen::merge_includes(&mut includes_h, &includes);

            let mut headers = String::new();

            headers.push_str(&format!("#ifndef {}_H\n", raw_name.to_uppercase()));
            headers.push_str(&format!("#define {}_H\n\n", raw_name.to_uppercase()));

            for &inc in &includes {
                let s = lexer_c::Token::tokens_to_string(inc);
                headers.push_str(s.trim());
                headers.push('\n');
            }
            headers.push('\n');

            for &def in &defines_h {
                let s = lexer_c::Token::tokens_to_string(def);
                headers.push_str(&s);
                headers.push('\n');
            }
            headers.push('\n');

            for &struc in &udts_h {
                headers.push_str(&lexer_c::Token::tokens_to_string(struc).trim());
                headers.push_str("\n\n");
            }
            headers.push('\n');

            for &func in &fn_defs {
                // turn `inline void XXX() {}` in .c into `extern inline void XXX();` in .h
                if let lexer_c::Token::Object("inline") = func[0] {
                    headers.push_str("extern ");
                }

                let s = lexer_c::Token::tokens_to_string(func);
                headers.push_str(s.trim());
                headers.push_str(";\n\n");
            }
            headers.push('\n');
            headers.push_str(&format!("#endif // {}_H", raw_name.to_uppercase()));

            fs::write(inc_dir.join(&header_name), headers)?;

            // Remove definitions from original C file to avoid duplicates
            let mut exclude_tokens = udts;
            exclude_tokens.extend_from_slice(&defines);

            let mut new_code = lexer_c::reconstruct_source(&tokens, &exclude_tokens);

            let header_inc_path = format!("\"../include/{}\"", &header_name);

            new_code = header_gen::insert_self_include(new_code, &header_inc_path);

            // let new_file = format!("{}.c.tmp", raw_name);
            let new_file = format!("{}.c", raw_name);
            let new_filepath = src_dir.join(&new_file);

            fs::write(new_filepath, new_code).unwrap();
        }
    }
    Ok(())
}

/// Checks the deps listed in Kiln.Toml config for any that aren't installed globally.
/// If it fins finds any such packages, it installs them.
async fn handle_check_installs(config: &Config) {
    let timer = time::Instant::now();

    let mut config = config.clone();
    let not_installed = package_manager::check_pkgs(&config);

    #[cfg(debug_assertions)]
    if not_installed.len() > 1 {
        dbg!(&not_installed);
    }

    for i in not_installed {
        package_manager::resolve_adding_package(&mut config, &i[0], &i[1], Some(&i[2]))
            .await
            .unwrap();
    }

    #[cfg(debug_assertions)]
    dbg!(timer.elapsed());
}

fn handle_tests(profile: &str, config: &Config, test_file: &str) -> Result<()> {
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
    build_sys::link_dep_files(&config, lang, &mut link_file)?;
    build_sys::link_proj_files(&config, &cwd, lang, &mut link_file)
        .map_err(|err| anyhow!("Failed to link source files: {}", err))?;

    let main_file = config.get_main_filepath();
    link_file = link_file
        .into_iter()
        .filter(|f| !f.ends_with(&main_file))
        .collect();

    link_file.push(test_file.to_string());

    let link_lib = build_sys::link_sys_lib(&cwd);
    let opt_flags = build_sys::opt_flags(&profile, config).unwrap();

    let header_dirs = build_sys::link_dep_headers(&config)?;
    let so_dir = build_sys::link_dep_shared_obj(&cwd)?;

    let compilation_cmd = build_sys::full_compilation_cmd(
        config,
        &profile,
        &link_file,
        &link_lib,
        &header_dirs,
        &so_dir,
        &opt_flags,
        config::BuildType::Exe,
    )?;

    let output = process::Command::new(&compilation_cmd[0])
        .args(&compilation_cmd[1..])
        .stdout(process::Stdio::piped())
        .stdin(process::Stdio::null())
        .stderr(process::Stdio::piped())
        .output()?;

    if !output.status.success() {
        let msg = String::from_utf8(output.stderr).unwrap_or("unknown stderr".to_string());
        return Err(anyhow!("Compilation failed for `{}`:\n{}", test_file, msg));
    }

    let bin_path = cwd
        .join("build")
        .join(&profile[2..])
        .join(&config.project.name);

    if !bin_path.exists() {
        return Err(anyhow!("Binary {:?} does not exist", bin_path));
    }

    let _output = process::Command::new(&bin_path)
        .stdin(process::Stdio::inherit())
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|e| anyhow!("Failed to run {:?} binary: {}", bin_path, e))?;

    fs::remove_file(bin_path)?;
    
    Ok(())
}