use std::path::Path;

use colored::*;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, clap::ValueEnum)]
pub enum Language {
    C,
    Cpp,
    Cuda,
}

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
