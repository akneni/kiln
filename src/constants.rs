use crate::utils::expand_user;
use colored::*;
use once_cell::sync::Lazy;
use std::{
    env::consts::OS,
    path::{Path, PathBuf},
};

pub const CONFIG_FILE: &str = "Kiln.toml";
pub const PACKAGE_CONFIG_FILE: &str = "kiln-package.toml";
pub const VALGRIND_OUT: &str = "kiln-valgrind-output.xml";

pub static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let paths = [
        ("linux", "/usr/share/kiln/", "~/.local/share/kiln/"),
        (
            "macos",
            "/Library/Application Support/kiln/",
            "~/Library/Application Support/kiln/",
        ),
        (
            "windows",
            "C:\\ProgramData\\kiln\\",
            "C:\\Users\\%USERNAME%\\AppData\\Local\\kiln\\",
        ),
    ];

    for (os, sys_path, user_path) in paths {
        if OS == os {
            let user_path_s = expand_user(&user_path);
            if Path::new(sys_path).exists() {
                return Path::new(sys_path).to_path_buf();
            } else if Path::new(&user_path_s).exists() {
                return Path::new(&user_path_s).to_path_buf();
            }
            panic!(
                "Error, no app data directory found. Please create the directory {} (system) or {} (user)",
                sys_path,
                user_path,
            );
        }
    }

    panic!("OS `{}` not supported", OS);
});

pub static PACKAGE_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let data_dir = (*DATA_DIR).clone();
    data_dir.join("packages")
});

pub static SEPETATOR: Lazy<ColoredString> = Lazy::new(|| {
    "✦ ═════════════════════════════════ ⚔ ═════════════════════════════════ ✦"
        .to_string()
        .blue()
        .bold()
});
