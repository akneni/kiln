use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use colored::*;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, clap::ValueEnum)]
pub enum Language {
    C,
    Cpp,
    Cuda,
}

impl Language {
    pub fn new(mut s: &str) -> Result<Self> {
        if s.starts_with("--") {
            s = &s[2..];
        } else if s.starts_with(".") {
            s = &s[1..];
        }
        let s = s.to_lowercase();
        match s.as_str() {
            "c" => Ok(Language::C),
            "cpp" => Ok(Language::Cpp),
            _ => Err(anyhow!("string not valid")),
        }
    }

    pub fn file_ext(&self) -> &'static str {
        match self {
            Self::C => ".c",
            Self::Cpp => ".cpp",
            Self::Cuda => ".cu",
        }
    }
}

#[allow(unused)]
pub fn expand_user(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home_dir) = std::env::var_os("HOME") {
            let path_without_tilde = &path[2..]; // Remove "~/" prefix
            return Path::new(&home_dir)
                .join(path_without_tilde)
                .to_str()
                .unwrap()
                .to_string();
        }
    }
    path.to_string()
}

/// Prints a warning message in a standardized way
/// This is used to print warnings related to static analysis
pub fn print_warning(
    warning_source: &str,
    filename: &str,
    line: &str,
    warning_type: &str,
    msg: &str,
) {
    let err_msg = format!(
        "{} {} [src/{} | Line {} ]: {:?}\n{}",
        warning_source.red().bold(),
        "Warning".red().bold(),
        filename,
        line,
        warning_type,
        msg,
    );
    println!("{}\n", err_msg);
}

pub fn join_rel_path(abs_path: impl AsRef<Path>, rel_path: &str) -> PathBuf {
    let path = abs_path.as_ref();
    match rel_path {
        "" | "." | "./" => path.to_path_buf(),
        _ => path.join(rel_path),
    }
}

pub fn extract_filename<'a>(filepath: &'a str) -> &'a str {
    let delimiter = if cfg!(target_os = "windows") {
        "\\"
    } else {
        "/"
    };
    
    match filepath.rsplit_once(delimiter) {
        Some((_, filename)) => filename,
        None => filepath
    }
}
