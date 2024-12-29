#![allow(unused)]

mod build_sys;
mod cli;
mod config;
mod constants;
mod safety;
mod utils;
mod valgrind;
mod lexer;

use anyhow::{anyhow, Result};
use clap::Parser;
use config::Config;
use constants::{CONFIG_FILE, SEPETATOR};
use lexer::{clean_source_code, Token};
use std::{env, fs, io::{stdout, Write}, path::Path, process};
use utils::Language;
use valgrind::VgOutput;


fn temp() {
    let mut code = fs::read_to_string("example.c")
        .unwrap();

    code = lexer::clean_source_code(code);
    let tokens = lexer::tokenize(&code)
        .unwrap();
    // for t in tokens{
    //     println!("{:?}", t);
    // }

    let includes = lexer::get_includes(&tokens);

    for inc in includes {
        println!("{:?}\n\n", inc);
    }


    std::process::exit(0);
}

fn main() {
    // temp();


    let cli_args: cli::CliCommand;
    let raw_cli_args = std::env::args().collect::<Vec<String>>();
    if raw_cli_args.len() < 2 {
        // Let the program fail and have Clap display it's help message
        cli_args = cli::CliCommand::parse();
    } else if raw_cli_args[1] == "run" || raw_cli_args[1] == "build" {
        let mut profile = "--dev".to_string();
        let mut args = vec![];
        if (
            raw_cli_args.len() >= 3 && 
            raw_cli_args[2].starts_with("--") && 
            raw_cli_args[2].len() > 2 &&
            raw_cli_args[2] != "--valgrind"
        ) {
            // Extracts compilation profile

            profile = raw_cli_args[2].clone();
        }
        if let Some(idx) = raw_cli_args.iter().position(|i| i == "--") {
            // Extracts passthough CLI arguments (trufc run)
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
        cli::Commands::Build { profile } => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let config = config.unwrap();

            handle_warnings(&config).unwrap();
            handle_build(&profile, &config).unwrap();
        }
        cli::Commands::Run { profile, args, valgrind } => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let config = config.unwrap();

            handle_warnings(&config).unwrap();
            if let Err(e) = handle_build(&profile, &config) {
                println!("Compilation Failed: {}", e);
                process::exit(1);
            }
            
            if valgrind {
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
            }
            else {
                // If use_valgrind = false
                handle_execution(&profile, &config, &cwd, &args).unwrap();
            }
        }
        cli::Commands::GenHeaders => {
            if let Err(e) = build_sys::validate_proj_repo(cwd.as_path()) {
                println!("{}", e);
                process::exit(1);
            }
            let config = config.unwrap();

            let cwd = env::current_dir()
                .unwrap();
            let src_dir = cwd.join("src");
            let inc_dir = cwd.join("include");

            for file in fs::read_dir(src_dir).unwrap() {
                if let Ok(file) = file {
                    if file.file_name() == "main.c" {
                        continue;
                    }

                    let mut code = fs::read_to_string(file.path())
                        .unwrap();
                    code = lexer::clean_source_code(code);
                    let tokens = lexer::tokenize(&code)
                        .unwrap();

                    let fn_defs = lexer::get_fn_def(&tokens);
                    let includes = lexer::get_includes(&tokens);

                    let raw_name = file
                        .file_name();
                    let raw_name = raw_name
                        .to_str()
                        .unwrap()
                        .rsplit_once(".")
                        .unwrap()
                        .0;

                    let mut headers = String::new();

                    headers.push_str(&format!("#ifndef {}_H\n", raw_name.to_uppercase()));
                    headers.push_str(&format!("#define {}_H\n\n", raw_name.to_uppercase()));

                    for &inc in &includes {
                        let s = lexer::Token::tokens_to_string(inc);
                        headers.push_str(&s);
                        headers.push('\n');
                    }
                    headers.push('\n');
                    for &func in &fn_defs {
                        let s = lexer::Token::tokens_to_string(func);
                        headers.push_str(&s);
                        headers.push_str(";\n");
                    }
                    headers.push('\n');
                    headers.push_str(&format!("#endif // {}_H", raw_name.to_uppercase()));

                    let header_name = format!("{}.h", raw_name);

                    fs::write(inc_dir.join(header_name), headers)
                        .unwrap();
                
                }   
            }
        }
    }
}

/// Returns true if there were warnings and false if there was no warnings.
fn handle_warnings(config: &Config) -> Result<Vec<safety::Warning>> {
    if !config.get_kiln_static_analysis() {
        return Ok(vec![])
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
        println!("Error: profile must start with `--`");
        process::exit(1);
    }

    let mut cwd = env::current_dir().unwrap();
    cwd.push("build");
    cwd.push(&profile[2..]);
    if !cwd.exists() {
        fs::create_dir_all(&cwd).unwrap();
    }
    cwd.pop();
    cwd.pop();

    let lang = Language::new(&config.project.language).unwrap();
    let link_file = build_sys::link_files(&cwd, lang)
        .map_err(|err| anyhow!("Failed to link source files: {}", err))?;
    let link_lib = build_sys::link_lib(&cwd);
    let opt_flags = build_sys::opt_flags(&profile, config).unwrap();

    let compilation_cmd =
        build_sys::full_compilation_cmd(config, &profile, &link_file, &link_lib, &opt_flags)
            .unwrap();

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
    passthough_args: &[String]
) -> Result<()> {
    if !profile.starts_with("--") {
        println!("Error: profile must start with `--`");
        process::exit(1);
    }

    let bin_path = project_dir
        .join("build")
        .join(&profile[2..])
        .join(&config.project.name);

    if !bin_path.exists() {
        eprintln!("Binary does not exist.");
        process::exit(1);
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
