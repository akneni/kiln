use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet, fs, path::{Path, PathBuf}, process
};
use toml;

use crate::constants::{CONFIG_FILE, PACKAGE_DIR};
use crate::package_manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project: Project,
    pub build_options: BuildOptions,
    pub dependency: Option<Vec<KilnIngot>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub version: String,
    pub language: String,
    pub build_type: Vec<BuildType>,
    pub src_dirs: Vec<String>,
    pub include_dirs: Vec<String>,
    pub staticlib_dirs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildOptions {
    pub debug_flags: Vec<String>,
    pub release_flags: Vec<String>,
    pub shared_flags: Vec<String>,
    compiler: Option<String>,
    standard: Option<String>,
    kiln_static_analysis: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KilnIngot {
    pub url: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BuildType {
    #[allow(non_camel_case_types)]
    exe,

    #[allow(non_camel_case_types)]
    ingot,

    #[allow(non_camel_case_types)]
    static_library,

    #[allow(non_camel_case_types)]
    dynamic_library,
}

impl Config {
    pub fn new(proj_name: &str) -> Self {
        let project = Project {
            name: proj_name.to_string(),
            version: "0.0.1".to_string(),
            language: "c".to_string(),
            build_type: vec![BuildType::exe],
            src_dirs: vec!["src".to_string()],
            include_dirs: vec!["include".to_string()],
            staticlib_dirs: None,
        };

        let build_options = BuildOptions::default();

        Config {
            project,
            build_options,
            dependency: None,
        }
    }

    pub fn from(path: &Path) -> Result<Self> {
        let toml_str = fs::read_to_string(path)?;

        let config: Config = toml::from_str(&toml_str)?;

        let build_types = &config.project.build_type;

        if build_types.len() == 0 {
            return Err(anyhow!("Project must have a build type"));
        }
        if build_types.contains(&BuildType::exe) && build_types.len() > 1 {
            return Err(anyhow!(
                "Project cannot be executable in addition to other types"
            ));
        }

        Ok(config)
    }

    pub fn to_disk(&self, path: impl AsRef<Path>) {
        let self_str = toml::to_string_pretty(self).unwrap();
        fs::write(path, &self_str).unwrap();
    }

    pub fn get_compiler_path(&self) -> String {
        match &self.build_options.compiler {
            Some(p) => p.clone(),
            None => match self.project.language.as_str() {
                "c" => "gcc".to_string(),
                "c++" => "g++".to_string(),
                "cuda" => "nvcc".to_string(),
                _ => {
                    eprintln!(
                        "Language `{}` is not supported",
                        self.project.language.as_str()
                    );
                    process::exit(1);
                }
            },
        }
    }

    pub fn kiln_static_analysis(&self) -> bool {
        self.build_options.kiln_static_analysis.unwrap_or(true)
    }

    pub fn get_standard(&self) -> Option<String> {
        self.build_options.standard.clone()
    }
}

impl Project {
    /// Returns the file exension with the `.` in front
    pub fn language_ext(&self) -> &'static str {
        match self.language.as_str() {
            "c" => ".c",
            "cpp" => ".cpp",
            "cuda" => ".cu",
            _ => panic!("Language not supported"),
        }
    }

    /// Returns the file exension with the `.` in front
    pub fn header_ext(&self) -> &'static str {
        match self.language.as_str() {
            "c" => ".h",
            "cpp" => ".hpp",
            "cuda" => ".cuh",
            _ => panic!("Language not supported"),
        }
    }
}

impl Default for BuildOptions {
    fn default() -> Self {
        let debug_flags = vec![
            "-g".to_string(),
            "-O0".to_string(),
            "-fsanitize=undefined".to_string(),
        ];
        let release_flags = vec!["-O3".to_string()];
        let shared_flags = vec!["-Wall".to_string()];

        BuildOptions {
            standard: None,
            debug_flags,
            release_flags,
            shared_flags,
            compiler: None,
            kiln_static_analysis: None,
        }
    }
}

impl KilnIngot {
    pub fn new(owner: &str, repo_name: &str, version: &str) -> Self {
        KilnIngot {
            url: format!("https://github.com/{}/{}.git", owner, repo_name),
            version: version.to_string(),
        }
    }

    pub fn owner(&self) -> &str {
        let (owner, _repo) = package_manager::parse_github_uri(&self.url).unwrap();
        owner
    }

    pub fn repo_name(&self) -> &str {
        let (_owner, repo) = package_manager::parse_github_uri(&self.url).unwrap();
        repo
    }

    /// Returns the path to the root directory of the project.
    pub fn get_global_path(&self) -> PathBuf {
        let (owner, repo) = package_manager::parse_github_uri(&self.url).unwrap();

        (*PACKAGE_DIR).join(owner).join(repo).join(&self.version)
    }

    pub fn get_kiln_cfg(&self) -> Result<Option<Config>> {
        let cfg_file = self.get_global_path().join(CONFIG_FILE);
        if !cfg_file.exists() {
            return Ok(None);
        }

        let cfg = Config::from(&cfg_file)?;
        Ok(Some(cfg))
    }

    /// Adds a dependency if it doesn't already exist
    /// Returns true if the dependency already exists
    pub fn add_dependency(deps: &mut Vec<KilnIngot>, new_dep: KilnIngot) -> bool {
        for dep in deps.iter() {
            if *dep == new_dep {
                return true;
            }
        }
        deps.push(new_dep);
        false
    }
}

/// Computes weak equality. Evaluates to true if the github uri has the same
/// project name and owner
impl PartialEq for KilnIngot {
    fn eq(&self, other: &Self) -> bool {
        self.owner() == other.owner() && self.repo_name() == other.repo_name()
    }
}


#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LockFile {
    // Here, the `version` field for each KilnIngot is guarenteed to be a specific version
    // (as opposed to something like "<=1.0.0,>2.0.0")
    pub ingots: Vec<KilnIngot>,
    pub ingot_url: HashSet<String>
}

impl LockFile {
    pub fn from(path: impl AsRef<Path>) -> Result<Self> {
        if !path.as_ref().exists() {
            return Ok(Self::default());
        }

        let lock_file_bin = fs::read(&path)?;
        
        match bincode::deserialize(&lock_file_bin) {
            Ok(r) => Ok(r),
            Err(e) => {
                #[cfg(debug_assertions)] dbg!(e);
                fs::remove_file(&path)?;
                Ok(Self::default())
            }   
        }
    }

    pub fn to_disk(&self, path: impl AsRef<Path>) -> Result<()> {
        let binary_lock = bincode::serialize(&self)?;
        fs::write(path, &binary_lock);
        Ok(())
    }

    /// Selects the correct versions of each of the ingots and adds them to 
    /// the self lockfile structure. 
    pub fn add_ingots(&mut self, ingots: &[(String, &[package_manager::Tag])]) {
        // the first element of string is the github url (same as the url field in kiln ingot)
    }
}

