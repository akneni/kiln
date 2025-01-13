use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use toml;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project: Project,
    pub build_options: BuildOptions,
    pub dependnecy: Option<Vec<Dependnecy>>,
}

impl Config {
    pub fn new(proj_name: &str) -> Self {
        let project = Project {
            name: proj_name.to_string(),
            version: "0.0.1".to_string(),
            language: "c".to_string(),
        };

        // No posibility of this failing
        let build_options = BuildOptions::from(&project).unwrap();

        Config {
            project,
            build_options,
            dependnecy: None,
        }
    }

    pub fn from(path: &Path) -> Result<Self> {
        let toml_str = fs::read_to_string(path)?;

        let config: Config = toml::from_str(&toml_str)?;

        Ok(config)
    }

    #[allow(unused)]
    pub fn to_disk(&self, path: &Path) {
        let s = toml::to_string(&self).unwrap();
        fs::write(path, s).unwrap();
    }

    // ========== Getter methods for the build options ==================

    pub fn get_compiler_path(&self) -> String {
        self.build_options.compiler_path.clone()
    }

    pub fn get_src_dir(&self) -> String {
        let default = "src".to_string();
        self.build_options.src_dir.clone().unwrap_or(default)
    }

    pub fn get_include_dir(&self) -> String {
        let default = "include".to_string();
        self.build_options.include_dir.clone().unwrap_or(default)
    }

    pub fn get_kiln_static_analysis(&self) -> bool {
        self.build_options.kiln_static_analysis.unwrap_or(true)
    }

    pub fn get_standard(&self) -> Option<String> {
        self.build_options.standard.clone()
    }

    pub fn get_main_filepath(&self) -> String {
        if let Some(file) = self.build_options.main_filepath.as_ref() {
            return file.clone();
        }
        let filename = match self.project.language.as_str() {
            "c" => "main.c".to_string(),
            "cpp" => "main.cpp".to_string(),
            "cuda" => "main.cu".to_string(),
            _ => {
                eprintln!("`{}` is not a supported language", self.project.language);
                std::process::exit(1);
            }
        };
        format!("{}/{}", self.get_src_dir(), filename)
    }

    pub fn get_flags(&self, compilation_profile: &str) -> Option<&Vec<String>> {
        if compilation_profile == "debug" {
            return Some(&self.build_options.debug_flags);
        } else if compilation_profile == "release" {
            return Some(&self.build_options.release_flags);
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub version: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildOptions {
    compiler_path: String,
    debug_flags: Vec<String>,
    release_flags: Vec<String>,
    src_dir: Option<String>,
    include_dir: Option<String>,
    standard: Option<String>,
    kiln_static_analysis: Option<bool>,
    main_filepath: Option<String>,
}

impl BuildOptions {
    fn from(project: &Project) -> Result<Self> {
        let debug_flags = vec![
            "-g".to_string(),
            "-O0".to_string(),
            "-Wall".to_string(),
            "-fsanitize=undefined".to_string(),
        ];
        let release_flags = vec!["-Wall".to_string(), "-O3".to_string()];

        let mut b_config = BuildOptions {
            standard: None,
            debug_flags,
            release_flags,
            compiler_path: "placeholder".to_string(),
            src_dir: None,
            include_dir: None,
            kiln_static_analysis: None,
            main_filepath: None,
        };

        match project.language.as_str() {
            "c" => {
                b_config.compiler_path = "gcc".to_string();
            }
            "cpp" => {
                b_config.compiler_path = "g++".to_string();
            }
            "cuda" => {
                b_config.compiler_path = "nvcc".to_string();
            }
            _ => return Err(anyhow!("language {} not supported", project.language)),
        }
        Ok(b_config)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Dependnecy {
    pub uri: String,
    pub version: String,
    pub include_dir: Option<String>,
    pub source_dir: Option<String>,
    pub shared_object_dir: Option<String>,
    pub static_lib_dir: Option<String>,
}
