
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use toml;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project: Project,
    pub build_options: BuildOptions,
    pub profile: HashMap<String, Profile>,
}

impl Config {

    #[allow(unused)]
    pub fn main_filepath(&self) -> String {
        let filename = match self.project.name.as_str() {
            "c" => "main.c".to_string(),
            "cpp" => "main.cpp".to_string(),
            "cuda" => "main.cu".to_string(),
            _ => "main.unknown".to_string(),
        };
        format!("{}/{}", self.get_src_dir(), filename)
    }

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
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub version: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildOptions {
    pub compiler_path: String,
    pub src_dir: Option<String>,
    pub include_dir: Option<String>,
    pub standard: Option<String>,
    pub kiln_static_analysis: Option<bool>,
}

impl BuildOptions {
    fn from(project: &Project) -> Result<Self> {
        let mut b_config = BuildOptions {
            standard: None,
            compiler_path: "gcc".to_string(),
            src_dir: None,
            include_dir: None,
            kiln_static_analysis: None,
        };

        match project.language.as_str() {
            "c" => {
                b_config.compiler_path = "gcc".to_string();
            },
            "cpp" => {
                b_config.compiler_path = "g++".to_string();
            },
            "cuda" => {
                b_config.compiler_path = "nvcc".to_string();
            }
            _ => return Err(anyhow!("language {} not supported", project.language)),
        }
        Ok(b_config)
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub flags: Vec<String>,
}

impl Config {
    const REQUIRED_PROFILES: [&str; 2] = ["debug", "release"];

    pub fn new(proj_name: &str) -> Self {
        let project = Project {
            name: proj_name.to_string(),
            version: "0.0.1".to_string(),
            language: "c".to_string(),
        };

        // No posibility of this failing
        let build_options = BuildOptions::from(&project)
            .unwrap();

        let mut profile = HashMap::new();

        profile.insert(
            "debug".to_string(),
            Profile {
                flags: vec![
                    "-g".to_string(),
                    "-O0".to_string(),
                    "-Wall".to_string(),
                    "-fsanitize=undefined".to_string(),
                ],
            },
        );

        profile.insert(
            "release".to_string(),
            Profile {
                flags: vec![
                    "-Wall".to_string(),
                    "-O3".to_string(),
                ],
            },
        );

        Config {
            project,
            build_options,
            profile,
        }
    }

    pub fn from(path: &Path) -> Result<Self> {
        let toml_str = fs::read_to_string(path)?;

        let config: Config = toml::from_str(&toml_str)?;
        config.validate_profiles()?;

        Ok(config)
    }

    #[allow(unused)]
    pub fn to_disk(&self, path: &Path) {
        let s = toml::to_string(&self).unwrap();
        fs::write(path, s).unwrap();
    }

    pub fn validate_profiles(&self) -> Result<()> {
        for k in self.profile.keys() {
            if !Self::REQUIRED_PROFILES.contains(&k.as_str()) {
                return Err(anyhow!("Missing required profile `{}`", k));
            }
        }

        Ok(())
    }
}
