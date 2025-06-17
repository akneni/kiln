use crate::{header_gen, lexer_c};

use anyhow::{anyhow, Result};
use std::{
    collections::HashMap,
    env,
    fmt::Debug,
};

/// This checks if unsafe functions exist within a line using general string parsing
/// This is messy and prone to false positives.
/// TODO: Create a lexer to better perform static analysis
struct FunctionMap {
    map: HashMap<String, String>,
}

impl FunctionMap {
    fn new() -> Self {
        let map: HashMap<String, String> = [
            // String Functions
            ("strcpy".to_string(), "strncpy".to_string()),
            ("strcat".to_string(), "strncat".to_string()),
            ("strtok".to_string(), "strtok_r".to_string()),
            ("vsprintf".to_string(), "vsnprintf".to_string()),
            // I/O Functions
            ("gets".to_string(), "fgets".to_string()),
            ("sprintf".to_string(), "snprintf".to_string()),
            // DType conversions
            ("atoi".to_string(), "strtol".to_string()),
            ("atol".to_string(), "strtol".to_string()),
            ("atoll".to_string(), "strtoll".to_string()),
            ("atof".to_string(), "strtof".to_string()),
            // Time related functions
            ("gmtime".to_string(), "gmtime_r".to_string()),
            ("localtime".to_string(), "localtime_r".to_string()),
            ("ctime".to_string(), "ctime_r".to_string()),
            ("asctime".to_string(), "asctime_r".to_string()),
        ]
        .into_iter()
        .collect();

        Self { map }
    }
}

#[derive(Debug)]
pub enum WarningType {
    UnsafeFunction,
}

#[derive(Debug)]
pub struct Warning {
    pub msg: String,
    pub filename: String,
    pub line: usize,
    pub warning_type: WarningType,
}

pub fn check_files(source_type: &str, proj_tokens: &header_gen::ProjTokens) -> Result<Vec<Warning>> {
    let mut warnings = vec![];
    let mut source_dir = env::current_dir()?;
    source_dir.push("src");

    if !source_dir.exists() {
        return Err(anyhow!("src/ does not exist."));
    } else if !source_dir.is_dir() {
        return Err(anyhow!("src is not a directory."));
    }

    let func_map = FunctionMap::new();

    for (name, source_code) in proj_tokens.iter_source_code() {
        if !name.ends_with(source_type) {
            continue;
        }

        let mut curr_warnings = scan_file(&name, &source_code, &func_map);
        warnings.append(&mut curr_warnings);
    }

    Ok(warnings)
}

fn scan_file(filename: &str, source_code: &str, func_map: &FunctionMap) -> Vec<Warning> {
    let mut warnings = vec![];

    let tokens = lexer_c::tokenize(source_code)
        .unwrap();

    for (token_num, token) in tokens.iter().enumerate() {
        if tokens[token_num..].len() < 3 {
            continue;
        }

        if let header_gen::Token::Object(obj) = token {
            if tokens[token_num + 1] != header_gen::Token::OpenParen {
                continue;
            }
            if let Some(safe_fn) = func_map.map.get(*obj) {
                let warning = Warning {
                    warning_type: WarningType::UnsafeFunction,
                    msg: format!(
                        "{}() is an unsafe function. Consuder using {}() instead",
                        obj, safe_fn
                    ),
                    filename: filename.to_string(),
                    line: token_num + 1,
                };

                warnings.push(warning);
            }
        }
        
    }

    warnings
}