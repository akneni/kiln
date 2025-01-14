use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};
use toml;

use crate::{constants::{CONFIG_FILE, PACKAGE_CONFIG_FILE, PACKAGE_DIR}, package_manager::{self, DepType}, utils};
use crate::kiln_package::KilnPackageConfig;

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
pub(super) struct Dependnecy {
    pub uri: String,
    pub version: String,
    pub include_dir: Option<String>,
    pub source_dir: Option<String>,
    pub shared_object_dir: Option<String>,
    pub static_lib_dir: Option<String>,
}

impl Dependnecy {
    pub(super) fn new(owner: &str, repo_name: &str, version: &str) -> Self {
        Dependnecy {
            uri: format!("https://github.com/{}/{}.git", owner, repo_name),
            version: version.to_string(),
            ..Dependnecy::default()
        }
    }

    pub(super) fn owner(&self) -> &str {
        let (owner, _repo) = package_manager::parse_github_uri(&self.uri).unwrap();
        owner
    }

    pub(super) fn repo_name(&self) -> &str {
        let (_owner, repo) = package_manager::parse_github_uri(&self.uri).unwrap();
        repo
    }

    pub(super) fn get_global_path(&self) -> PathBuf {
        let (owner, repo) = package_manager::parse_github_uri(&self.uri).unwrap();

        (*PACKAGE_DIR)
            .join(owner)
            .join(repo)
            .join(&self.version)
    }

    pub(super) fn get_kiln_cfg(&self) -> Result<Option<Config>> {
        let cfg_file = self.get_global_path().join(CONFIG_FILE);
        if !cfg_file.exists() {
            return Ok(None);
        }

        let cfg = Config::from(&cfg_file)?;
        Ok(Some(cfg))
    }

    pub(super) fn get_include_dir(&self) -> Result<Option<PathBuf>> {
        let p = self.get_global_path();

        if let Some(include_dir) = &self.include_dir {
            return Ok(Some(utils::join_rel_path(&p, &include_dir)));
        }

        let config_path = p.join(CONFIG_FILE);
        if config_path.exists() {
            let config = Config::from(&config_path)?;
            return Ok(Some(p.join(&config.get_include_dir())));
        }

        let pgk_path = p.join(PACKAGE_CONFIG_FILE);
        if pgk_path.exists() {
            let pkg_cfg = KilnPackageConfig::from(&pgk_path)?;
            return Ok(Some(p.join(&pkg_cfg.metadata.include_dir)));
        }

        Ok(None)
    }

    pub(super) fn get_source_dir(&self) -> Result<Option<PathBuf>> {
        let p = self.get_global_path();

        if let Some(source_dir) = &self.source_dir {
            return Ok(Some(utils::join_rel_path(&p, &source_dir)));
        }

        let config_path = p.join(CONFIG_FILE);
        if config_path.exists() {
            let config = Config::from(&config_path)?;
            return Ok(Some(p.join(&config.get_src_dir())));
        }

        let pgk_path = p.join(PACKAGE_CONFIG_FILE);
        if pgk_path.exists() {
            let pkg_cfg = KilnPackageConfig::from(&pgk_path)?;
            return Ok(Some(p.join(&pkg_cfg.metadata.source_dir)));
        }

        Ok(None)
    }

    pub(super) fn get_shared_obj_dir(&self) -> Result<Option<PathBuf>> {
        let p = self.get_global_path();
        let so_path = Path::new("build").join("release");

        let config_path = p.join(CONFIG_FILE);
        if config_path.exists() {
            return Ok(Some(p.join(so_path)));
        }

        Ok(None)
    }

    pub(super) fn get_static_lib_dir(&self) -> Result<Option<PathBuf>> {
        let p = self.get_global_path();
        let sl_path = Path::new("build").join("release");

        let config_path = p.join(CONFIG_FILE);
        if config_path.exists() {
            return Ok(Some(p.join(sl_path)));
        }

        Ok(None)
    }


    /// Adds a depdendnecy if it doesn't already exist
    /// Returns true if the dependency already exists
    pub(super) fn add_dependency(deps: &mut Vec<Dependnecy>, new_dep: Dependnecy) -> bool {
        for dep in deps.iter() {
            if *dep == new_dep {
                return true;
            }
        }
        deps.push(new_dep);
        false
    }

    fn get_dep_dir(&self, dep_type: DepType) -> Result<Option<PathBuf>> {
        match dep_type {
            DepType::SourceCode => self.get_source_dir(),
            DepType::HeaderFile => self.get_include_dir(),
            DepType::SharedObject => self.get_shared_obj_dir(),
            DepType::StaticLibrary => self.get_static_lib_dir(),
        }
    }

}



/// Computes weak equality. Evaluates to true if the github uri has the same
/// project name and owner
impl PartialEq for Dependnecy {
    fn eq(&self, other: &Self) -> bool {
        self.owner() == other.owner() &&
        self.repo_name() == other.repo_name()
    }
}