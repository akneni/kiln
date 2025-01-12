use crate::utils;
use crate::utils::Language;
use crate::{config::Config, constants::CONFIG_FILE};

use anyhow::{anyhow, Result};
use std::{
    fs,
    path::Path,
};

pub fn create_project(path: &Path, lang: Language) -> Result<()> {
    let toml_path = path.join(CONFIG_FILE);
    if toml_path.exists() {
        return Err(anyhow!("directory is already a Kiln project."));
    }

    let dir_name = path.file_name().unwrap().to_str().unwrap();

    let mut config = Config::new(dir_name);
    if lang == Language::Cpp {
        config.project.language = "cpp".to_string();
    }

    let config_str = toml::to_string(&config)?;

    fs::write(&toml_path, config_str)?;

    let source_dir = path.join("src");
    fs::create_dir(&path.join("src"))?;

    match lang {
        Language::C => {
            let starter_code = "#include <stdio.h>\n\nint main() {\n\tprintf(\"Welcome to Kiln!\\n\");\n\treturn 0;\n}";
            fs::write(&source_dir.join("main.c"), starter_code)?;
        }
        Language::Cpp => {
            let starter_code = "#include <iostream>\n\nint main() {\n\tstd::cout << \"Welcome to Kiln!\\n\";\n\treturn 0;\n}";
            fs::write(&source_dir.join("main.cpp"), starter_code)?;
        }
    }
    fs::create_dir(path.join("include"))?;

    Ok(())
}

/// Links all the files in the project together
pub fn link_files(config: &Config, proj_dir: &Path, language: Language) -> Result<Vec<String>> {
    let source_dir = proj_dir.join(config.get_src_dir());

    let mut c_files = vec![];
    let source_dir_iter = fs::read_dir(&source_dir)
        .map_err(|err| anyhow!("Failed to iterate over source dir {:?}c: {}", source_dir, err))?;

    for file in source_dir_iter {
        if let Ok(file) = file {
            let file = file.file_name();
            let filename = file.to_str().unwrap();
            if filename.ends_with(language.file_ext()) {
                c_files.push(format!("src/{}", filename));
            }
        }
    }

    Ok(c_files)
}

pub fn link_lib(path: &Path) -> Vec<String> {
    let c_lib_mappings = [
        ("<math.h>", "-lm"),                // Math library
        ("<omp.h>", "-fopenmp"),            // OpenMP library
        ("<pthread.h>", "-pthread"),        // POSIX threads
        ("<zlib.h>", "-lz"),                // Compression library (zlib)
        ("<curl/curl.h>", "-lcurl"),        // cURL library for network operations
        ("<ssl.h>", "-lssl"),               // SSL/TLS library
        ("<crypto.h>", "-lcrypto"),         // Cryptography library
        ("<ncurses.h>", "-lncurses"),       // Ncurses for terminal handling
        ("<mariadb/mysql.h>", "-lmariadb"), // MySQL/MariaDB client library
        ("<sqlite3.h>", "-lsqlite3"),       // SQLite library
        ("<GL/gl.h>", "-lGL"),              // OpenGL library
        ("<GL/glut.h>", "-lglut"),          // GLUT library for OpenGL
        ("<X11/Xlib.h>", "-lX11"),          // X11 library for X Window System
        ("<immintrin.h>", "-mavx"),         // AVX instructions
        ("<liburing.h>", "-luring"),        // liburing library for asynchronous I/O
        ("<arm_neon.h>", "-mfpu=neon"),     // NEON support for ARM
    ];

    let includes = utils::extract_include_statements(path);

    let mut libs = vec![];
    for (incl, link) in c_lib_mappings {
        if includes.contains(&incl.to_string()) {
            libs.push(link.to_string())
        }
    }

    libs
}

pub fn opt_flags(profile: &str, config: &Config) -> Result<Vec<String>> {
    let profile = &profile[2..];

    if let Some(prof) = config.profile.get(profile) {
        return Ok(prof.flags.clone());
    }
    Err(anyhow!(
        "profile `--{}` does not exist. Choose a different profile or declare it in Kiln.toml",
        profile
    ))
}

pub fn full_compilation_cmd(
    config: &Config,
    profile: &str,
    link_file: &Vec<String>,
    link_lib: &Vec<String>,
    flags: &Vec<String>,
) -> Result<Vec<String>> {

    let compiler = config.get_compiler_path();
    let standard = config.get_standard();

    let mut command = vec![compiler.to_string()];
    if let Some(standard) = standard {
        command.push(format!("-std={}", standard));
    }

    command.extend_from_slice(flags);

    let profile = &profile[2..];
    let build_path = format!("build/{}/{}", profile, &config.project.name);

    command.extend_from_slice(&["-o".to_string(), build_path]);

    // Links all the files in the current project
    command.extend_from_slice(link_file);

    let main_filepath = config.get_main_filepath();
    if !main_filepath.starts_with(&config.get_src_dir()) {
        command.push(main_filepath);
    }

    // Link all the libraries (shared objects like -lm, -lpthread, etc)
    command.extend_from_slice(link_lib);

    Ok(command)
}

pub fn validate_proj_repo(path: &Path) -> Result<()>{
    let config = path.join(CONFIG_FILE);
    if !config.exists() {
        return Err(anyhow!("Invalid Project Directory: config file `{}` doesn't exist.", CONFIG_FILE));
    }
    else if !config.is_file() {
        return Err(anyhow!("Invalid Project Directory: `{}` is not a file.", CONFIG_FILE));
    } 
    
    let source_dir = path.join("src");
    if !source_dir.exists() {
        return Err(anyhow!("Invalid Project Directory: source code directory `src/` doesn't exist."));
    }
    else if !source_dir.is_dir() {
        return Err(anyhow!("Invalid Project Directory: `src` is not a directory."));
    }
    Ok(())
}