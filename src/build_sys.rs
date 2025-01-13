use crate::utils;
use crate::utils::Language;
use crate::{config::Config, constants::CONFIG_FILE};

use anyhow::{anyhow, Result};
use std::{fs, path::Path};

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
        Language::Cpp | Language::Cuda => {
            let starter_code = "#include <iostream>\n\nint main() {\n\tstd::cout << \"Welcome to Kiln!\\n\";\n\treturn 0;\n}";
            fs::write(&source_dir.join("main.cpp"), starter_code)?;
        }
    }
    fs::create_dir(path.join("include"))?;

    Ok(())
}

/// Links all the files in the project together
pub fn link_proj_files(
    config: &Config, 
    proj_dir: &Path, 
    language: Language,
    out_buffer: &mut Vec<String>
) -> Result<()> {

    let source_dir = proj_dir.join(config.get_src_dir());

    let source_dir_iter = source_dir.read_dir().map_err(|err| {
        anyhow!(
            "Failed to iterate over source dir {:?}c: {}",
            source_dir,
            err
        )
    })?;

    for file in source_dir_iter {
        if let Ok(file) = file {
            let file = file.file_name();
            let filename = file.to_str().unwrap();
            if filename.ends_with(language.file_ext()) {
                let filepath = source_dir.join(filename);
                let filepath = filepath
                    .to_str()
                    .unwrap()
                    .to_string();

                out_buffer.push(filepath);
            }
        }
    }

    Ok(())
}

pub fn link_sys_lib(path: &Path) -> Vec<String> {
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

pub(super) fn link_dep_files(
    proj_dir: &Path,
    language: Language,
    out_buffer: &mut Vec<String>,
) -> Result<()> {
    
    let code_deps = proj_dir.join("dependencies").join("source_code");
    if !code_deps.exists() {
        return Ok(());
    }

    for f_name in code_deps.read_dir()? {
        let f_name = match f_name {
            Ok(f) => f,
            Err(e) => {
                dbg!(e);
                continue;
            }
        };

        if !f_name.file_type()?.is_file() {
            continue;
        }

        let f_osstr = f_name.file_name();
        let f_str = f_osstr.to_str().unwrap();

        let valid_ext = match language {
            Language::C => [".c"].as_slice(),
            Language::Cpp => [".c", ".cpp"].as_slice(),
            Language::Cuda => [".c", ".cpp", ".cu"].as_slice(),
        };

        if !valid_ext.iter().any(|&ext| f_str.ends_with(ext)) {
            continue;
        }

        let f_path = code_deps.join(f_str);

        let f_path_str = f_path.to_str().unwrap().to_string();
        out_buffer.push(f_path_str);
    }

    Ok(())
}

pub(super) fn link_dep_headers(proj_dir: &Path) -> Result<Option<String>> {
    let headers_dir = proj_dir.join("dependencies").join("header_files");
    if !headers_dir.exists() {
        return Ok(None);
    }
    let headers_path = headers_dir
        .to_str()
        .unwrap()
        .to_string();
    Ok(Some(headers_path))
}

pub(super) fn link_dep_shared_obj(proj_dir: &Path) -> Result<Option<String>> {
    let headers_dir = proj_dir.join("dependencies").join("shared_objects");
    if !headers_dir.exists() {
        return Ok(None);
    }
    let headers_path = headers_dir
        .to_str()
        .unwrap()
        .to_string();
    Ok(Some(headers_path))
}

pub fn opt_flags(profile: &str, config: &Config) -> Result<Vec<String>> {
    let profile = &profile[2..];

    if let Some(prof) = config.get_flags(profile) {
        return Ok(prof.clone());
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
    header_dir: &Option<String>,
    shared_obj_dir: &Option<String>,
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

    if let Some(header_dir) = header_dir {
        let tmp = format!("-I{}", header_dir);
        command.push(tmp);
    }
    if let Some(shared_obj_dir) = shared_obj_dir {
        let tmp = format!("-L{}", shared_obj_dir);
        command.push(tmp);
    }

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

pub fn validate_proj_repo(path: &Path) -> Result<()> {
    let config = path.join(CONFIG_FILE);
    if !config.exists() {
        return Err(anyhow!(
            "Invalid Project Directory: config file `{}` doesn't exist.",
            CONFIG_FILE
        ));
    } else if !config.is_file() {
        return Err(anyhow!(
            "Invalid Project Directory: `{}` is not a file.",
            CONFIG_FILE
        ));
    }

    let source_dir = path.join("src");
    if !source_dir.exists() {
        return Err(anyhow!(
            "Invalid Project Directory: source code directory `src/` doesn't exist."
        ));
    } else if !source_dir.is_dir() {
        return Err(anyhow!(
            "Invalid Project Directory: `src` is not a directory."
        ));
    }
    Ok(())
}
