use crate::config::{self, Dependency};
use crate::utils;
use crate::utils::Language;
use crate::{config::Config, constants::CONFIG_FILE};

use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::{env, process};
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
    out_buffer: &mut Vec<String>,
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
                let filepath = filepath.to_str().unwrap().to_string();

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
        ("<immintrin.h>", "-march=native"), // AVX instructions
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

pub fn link_dep_files(
    config: &Config,
    language: Language,
    out_buffer: &mut Vec<String>,
) -> Result<()> {
    let deps = match &config.dependency {
        Some(d) => d.clone(),
        None => return Ok(()),
    };

    // {key: uri, value: version}
    let mut packages: HashMap<String, String> = HashMap::new();

    // {key: filename, value: uri file is from}
    let mut filenames: HashMap<String, String> = HashMap::new();

    for dep in deps {
        link_dep_files_h(&dep, language, out_buffer, &mut packages, &mut filenames)?;
    }

    Ok(())
}

pub fn link_dep_headers(config: &Config) -> Result<Vec<String>> {
    let mut header_dirs = vec![];

    let mut packages: HashSet<String> = HashSet::new();

    if let Some(deps) = &config.dependency {
        for dep in deps {
            link_dep_headers_h(dep, &mut header_dirs, &mut packages)?;
        }
    }

    Ok(header_dirs)
}

pub fn link_dep_shared_obj(proj_dir: &Path) -> Result<Option<String>> {
    let headers_dir = proj_dir.join("dependencies").join("shared_objects");
    if !headers_dir.exists() {
        return Ok(None);
    }
    let headers_path = headers_dir.to_str().unwrap().to_string();
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

/// if `build_type` is `BuildType::StaticLibrary`, then we will need to run the build command in the
/// `build/XXX/obj` directory
pub fn full_compilation_cmd(
    config: &Config,
    profile: &str,
    link_file: &Vec<String>,
    link_lib: &Vec<String>,
    header_dirs: &Vec<String>,
    shared_obj_dir: &Option<String>,
    flags: &Vec<String>,
    build_type: config::BuildType,
) -> Result<Vec<String>> {
    let compiler = config.get_compiler_path();
    let standard = config.get_standard();

    let mut command = vec![compiler.to_string()];
    if let Some(standard) = standard {
        command.push(format!("-std={}", standard));
    }

    command.extend_from_slice(flags);

    let cwd = env::current_dir()?;
    let cwd = cwd.as_os_str();
    let cwd = cwd.to_str().unwrap();

    let profile = &profile[2..];
    let mut build_path = format!("{}/build/{}/{}", cwd, profile, &config.project.name);

    match build_type {
        config::BuildType::DynamicLibrary => {
            let file_ext = match env::consts::OS {
                "linux" => ".so",
                "windows" => ".dll",
                "mac" => ".dylib",
                _ => {
                    eprintln!("OS {} not supported", env::consts::OS);
                    process::exit(1);
                }
            };
            build_path.push_str(file_ext);

            command.extend_from_slice(&["-shared".to_string(), "-fPIC".to_string()]);
        }
        config::BuildType::StaticLibrary => {
            command.push("-c".to_string());

            let _ = fs::create_dir_all(&build_path);
        }
        _ => {}
    }

    if build_type != config::BuildType::StaticLibrary {
        command.extend_from_slice(&["-o".to_string(), build_path]);
    }

    for header_dir in header_dirs {
        let tmp: String = format!("-I{}", header_dir);
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

    if build_type == config::BuildType::StaticLibrary {
        let output_file = format!("{}/build/{}/{}.a", cwd, profile, &config.project.name);
        let object_dir = format!("{}/build/{}/obj/*.o", cwd, profile);

        let second_cmd = [
            "&&".to_string(),
            "ar".to_string(),
            "rcs".to_string(),
            output_file,
            object_dir,
        ];

        command.extend_from_slice(&second_cmd);
    }

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

/// Helper function that recursivly links all the source code files
fn link_dep_files_h(
    dep: &Dependency,
    language: Language,
    out_buffer: &mut Vec<String>,
    packages: &mut HashMap<String, String>,
    filenames: &mut HashMap<String, String>,
) -> Result<()> {
    let dep_id = format!("{}/{}", dep.owner(), dep.repo_name());

    if let Some(_) = packages.get(&dep_id) {
        // TODO: Handle version mismatches
        return Ok(());
    }

    let source_dir = match dep.get_source_dir()? {
        Some(s) => s,
        None => return Err(anyhow!("{} has an ambiguous source dir", &dep.uri)),
    };

    packages.insert(dep_id, dep.version.clone());

    let valid_ext = match language {
        Language::C => [".c"].as_slice(),
        Language::Cpp => [".c", ".cpp"].as_slice(),
        Language::Cuda => [".c", ".cpp", ".cu"].as_slice(),
    };

    for file in source_dir.read_dir()? {
        let file = match file {
            Ok(r) => r,
            Err(err) => {
                dbg!(err);
                continue;
            }
        };
        if !file.file_type()?.is_file() {
            continue;
        }

        let filename = file.file_name().to_str().unwrap().to_string();

        if !valid_ext.iter().any(|&ext| filename.ends_with(ext)) {
            continue;
        }

        if let Some(other_uri) = filenames.get(&filename) {
            eprintln!("Fatal Error: Multiple dependencies have files of the same name:");
            eprintln!("{}", dep.uri);
            eprintln!("{}", other_uri);
            eprintln!("Common File: {}", &filename);
            std::process::exit(1);
        }

        filenames.insert(filename.clone(), dep.uri.clone());

        let filepath = source_dir.join(&filename);
        let filepath = filepath.to_str().unwrap().to_string();

        out_buffer.push(filepath);
    }

    // Recursivley handle chain dependnecies
    if let Some(kiln_cfg) = dep.get_kiln_cfg()? {
        if let Some(chain_deps) = kiln_cfg.dependency {
            for cd in &chain_deps {
                link_dep_files_h(cd, language, out_buffer, packages, filenames)?;
            }
        }
    }

    Ok(())
}

/// Helper function that recursivly links all the header file directories
fn link_dep_headers_h(
    dep: &Dependency,
    out_buffer: &mut Vec<String>,
    packages: &mut HashSet<String>,
) -> Result<()> {
    let dep_id = format!("{}/{}", dep.owner(), dep.repo_name());

    if packages.contains(&dep_id) {
        return Ok(());
    } else {
        packages.insert(dep_id);
    }

    let include_dir = match dep.get_include_dir()? {
        Some(s) => s,
        None => return Err(anyhow!("{} has an ambiguous include dir", &dep.uri)),
    };

    let inc_dir_string = include_dir.to_str().unwrap().to_string();

    if !include_dir.is_dir() {
        eprintln!("Path to include directory points to a non-directory");
        eprintln!("[{}]'s include_dir points to {}", &dep.uri, &inc_dir_string);
        eprintln!("Change/add the `include_dir = \"relative/path/to/include\"` fild in Kiln.Toml to fix this");
        std::process::exit(1);
    }

    out_buffer.push(inc_dir_string);

    // Recursivley handle chain dependnecies
    if let Some(kiln_cfg) = dep.get_kiln_cfg()? {
        if let Some(chain_deps) = kiln_cfg.dependency {
            for cd in &chain_deps {
                link_dep_headers_h(cd, out_buffer, packages)?;
            }
        }
    }

    Ok(())
}

pub fn validate_build_dir() -> Result<()> {
    let a = Path::new("build/release");
    let b = Path::new("build/debug");

    if !a.exists() {
        fs::create_dir_all(a)?;
    }
    if !b.exists() {
        fs::create_dir_all(b)?;
    }

    Ok(())
}
