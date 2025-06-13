use crate::config::{self, KilnIngot};
use crate::constants::PACKAGE_CONFIG_FILE;
use crate::packaging::ingot::{IngotMetadata, Metadata};
use crate::{constants, utils};
use crate::utils::Language;
use crate::{config::Config, constants::CONFIG_FILE};

use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::{env, process};
use std::{fs, path::Path};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildProfile  {
    Debug,
    Release,
}

impl BuildProfile {
    pub fn from(s: &str) -> Self {
        match s {
            "debug" | "--debug" => Self::Debug,
            "release" | "--release" => Self::Release,
            _ => panic!("Invalid build profile"),
        }
    }

    pub fn to_str(&self, include_dashes: bool) -> &'static str {
        match (include_dashes, self) {
            (true, Self::Release) => "--release",
            (false, Self::Release) => "release",
            (true, Self::Debug) => "--debug",
            (false, Self::Debug) => "debug",
        }
    }
}

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

pub fn link_sys_lib(path: &Path) -> Vec<&'static str> {
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

    let mut libs = vec![];

    // TODO: Get thing working
    // let includes = utils::extract_include_statements(path);
    // for (incl, link) in c_lib_mappings {
    //     if includes.contains(&incl.to_string()) {
    //         libs.push(link)
    //     }
    // }

    libs
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


#[derive(Debug)]
pub struct ProjBuilder<'a> {
    config: &'a Config,
    ingots: HashSet<String>,
    pub compile_cmd: CompileCmdBuilder,
}

#[derive(Debug, Default)]
pub struct CompileCmdBuilder {
    pub source_files: HashSet<String>,
    include_dirs: HashSet<String>,
    static_libs: HashSet<String>,
    dynamic_libs: HashSet<String>,
    sys_libs: HashSet<String>,
    compiler: String,
    output_filename: Option<String>,
    compiler_flags: HashSet<String>,
}

impl<'a> ProjBuilder<'a> {
    pub fn new(config: &'a Config) -> Self {
        let mut compile_cmd = CompileCmdBuilder {
            compiler: config.get_compiler_path(),
            ..CompileCmdBuilder::default()
        };

        for src_dir in &config.project.src_dirs {
            for file in fs::read_dir(src_dir).unwrap() {
                let file = file.unwrap();
                if !file.file_type().unwrap().is_file() {
                    continue;
                }

                if !file.file_name().to_str().unwrap().ends_with(config.project.language_ext()) {
                    continue;
                }

                let filepath = file.path();
                let filepath = filepath.to_str().unwrap().to_string();

                compile_cmd.source_files.insert(filepath);
            }
        }

        if let Some(staticlib_dirs) = &config.project.staticlib_dirs  {
            for static_lib in staticlib_dirs {
                for file in fs::read_dir(static_lib).unwrap() {
                    let file = file.unwrap();
                    if !file.file_type().unwrap().is_file() {
                        continue;
                    }

                    if !file.file_name().to_str().unwrap().ends_with(constants::STATIC_LIB_FE) {
                        continue;
                    }

                    let filepath = file.path();
                    let filepath = filepath.to_str().unwrap().to_string();

                    compile_cmd.static_libs.insert(filepath);
                }
            }
        }

        for include_dir in &config.project.include_dirs {
            compile_cmd.include_dirs.insert(include_dir.clone());
        }

        Self {
            config,
            ingots: HashSet::new(),
            compile_cmd,
        }
    }

    pub fn attach_ingot(&mut self, ingot: &KilnIngot) {
        let path_buf = ingot.get_global_path();
        let path = path_buf.to_str()
            .unwrap()
            .to_string();

        if !self.ingots.insert(path.clone()) {
            // Runs if the path already exists
            return;
        }

        // Add source files and static libraries to compile comand
        let ingot_dir = path_buf.join("build").join("ingot");
        for file in fs::read_dir(&ingot_dir).unwrap() {
            match file {
                Ok(file) => {
                    if !file.file_type().unwrap().is_file() {
                        continue;
                    }

                    let filename = file.file_name();
                    let filename = filename.to_str().unwrap();
                    
                    let target_f = file.path().to_str().unwrap().to_string();

                    if filename.ends_with(self.config.project.language_ext()) {
                        self.compile_cmd.source_files.insert(target_f);
                    }
                    else if filename.ends_with(constants::STATIC_LIB_FE) {
                        self.compile_cmd.static_libs.insert(target_f);
                    }
                }
                Err(e) => {
                    eprintln!("WARNING: Error scanning dir {:?}:\n{}", ingot_dir, e);
                }
            }
        }

        // Add include & dynamic library directories to compile command. 
        let ingot_dir_s = ingot_dir.to_str()
            .unwrap()
            .to_string();
        self.compile_cmd.dynamic_libs.insert(format!("-L{}", ingot_dir_s));
        self.compile_cmd.include_dirs.insert(ingot_dir_s);

        let ingot_md_path = ingot_dir.join(PACKAGE_CONFIG_FILE);
        let ingot_md: IngotMetadata = IngotMetadata::from(&ingot_md_path).unwrap();

        // Add syslibs to compile command
        for sys_lib in &ingot_md.metadata.sys_libs {
            self.compile_cmd.sys_libs.insert(sys_lib.clone());
        }

        // Recursively does the same for all the other ingots. 
        for upstream_ingot in &ingot_md.metadata.ingot_deps {
            self.attach_ingot(upstream_ingot);
        }
    }

    pub fn build_exe(&mut self, build_prof: BuildProfile) -> Result<()> {
        let mut output_file = self.config.project.name.to_string();
        output_file.push_str(constants::EXECUTABLE_FE);

        let output_filepath = Path::new("build")
            .join(build_prof.to_str(false))
            .join(output_file);
        
        self.compile_cmd.output_filename = Some(output_filepath.to_str().unwrap().to_string());

        let (shell, flag) = if cfg!(target_os = "windows") {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };

        let compile_cmd = self.compile_cmd.generate_compile_cmd(config::BuildType::exe).join(" ");

        let cmd = process::Command::new(shell)
            .arg(flag)
            .arg(&compile_cmd)
            .stdout(process::Stdio::inherit())
            .stderr(process::Stdio::inherit())
            .stdin(process::Stdio::inherit())
            .output()?;

        if !cmd.status.success() {
            // If the compile command failed, terminate this parent process
            process::exit(1);
        }

        Ok(())
    }

    pub fn build_ingot(&self) {
        let ingot_dir = Path::new("build").join("ingot");
        if fs::exists(&ingot_dir).unwrap() {
            fs::remove_dir_all(&ingot_dir).unwrap();
        }

        fs::create_dir_all(&ingot_dir).unwrap();
        
        for src_file in &self.compile_cmd.source_files {
            let src_filename = utils::extract_filename(src_file);
            fs::copy(&src_file, ingot_dir.join(src_filename)).unwrap();
        }

        for static_lib in &self.compile_cmd.static_libs {
            let static_lib_name = utils::extract_filename(static_lib);
            fs::copy(&static_lib, ingot_dir.join(static_lib_name)).unwrap();
        }

        for include_dir in &self.compile_cmd.include_dirs {
            for header_file in fs::read_dir(include_dir).unwrap() {
                let header_file = header_file.unwrap();
                if !header_file.file_type().unwrap().is_file() {
                    continue;
                }
                let headerfile_name = header_file.file_name()
                    .into_string()
                    .unwrap();

                fs::copy( header_file.path(), ingot_dir.join(headerfile_name)).unwrap();
            }
        }

        let mut ingot_deps = vec![];
        if let Some(v) = &self.config.dependency {
            ingot_deps = v.clone();
        }

        let ingot_md = IngotMetadata {
            metadata: Metadata {
                ingot_deps,
                sys_libs: vec![], // TODO -> Fill this out properly
                staticlib_support: false,
                source_support: true,
            }
        };

        let ingot_md_str = toml::to_string_pretty(&ingot_md).unwrap();
        fs::write(ingot_dir.join(constants::PACKAGE_CONFIG_FILE), &ingot_md_str)
            .unwrap();
    }

}

impl CompileCmdBuilder {
    pub fn generate_compile_cmd(&self, build_type: config::BuildType) -> Vec<String> {
        let mut compile_cmd = vec![
            self.compiler.clone(),
        ];

        if let config::BuildType::dynamic_library = build_type {
            compile_cmd.push("-shared".to_string());
        }

        compile_cmd.push(format!("\"{}\"", self.output_filename.clone().unwrap()));
        compile_cmd.push("-o".to_string());

        for static_lib in &self.static_libs {
            compile_cmd.push(format!("\"{}\"", static_lib));
        }
        for source_file in &self.source_files {
            compile_cmd.push(format!("\"{}\"", source_file));
        }
        for include_dir in &self.include_dirs {
            compile_cmd.push(format!("\"-I{}\"", include_dir));
        }
        for dynamic_lib in &self.dynamic_libs {
            compile_cmd.push(format!("\"-L{}\"", dynamic_lib));
        }
        for sys_lib in &self.sys_libs {
            compile_cmd.push(format!("\"{}\"", sys_lib));
        }

        match build_type {
            config::BuildType::exe => {
                // Nothing additional is required
            }
            config::BuildType::static_library => {
                // Already taken care of above
            }
            config::BuildType::dynamic_library => {
                unimplemented!();
            }
            config::BuildType::ingot => {
                unreachable!("You should not be calling this function to build an ingot (if you are a user, please file a github issue)");
            }
        }

        compile_cmd
    }
}