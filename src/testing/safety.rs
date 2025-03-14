use crate::testing::valgrind::VgOutput;
use crate::{constants::VALGRIND_OUT, lexer_c, utils};

use anyhow::{anyhow, Result};
use std::{
    collections::HashMap,
    env,
    fmt::Debug,
    fs,
    process::{self, Command},
    sync::{Arc, Mutex},
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

pub fn check_files(source_type: &str) -> Result<Vec<Warning>> {
    let mut warnings = vec![];
    let mut source_dir = env::current_dir()?;
    source_dir.push("src");

    if !source_dir.exists() {
        return Err(anyhow!("src/ does not exist."));
    } else if !source_dir.is_dir() {
        return Err(anyhow!("src is not a directory."));
    }

    let func_map = FunctionMap::new();

    for path in fs::read_dir(source_dir)? {
        if let Ok(path) = path {
            let path = path.path();
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            if !name.ends_with(source_type) {
                continue;
            }

            let source_code = fs::read_to_string(path)?;
            let mut curr_warnings = scan_file(&name, &source_code, &func_map);

            warnings.append(&mut curr_warnings);
        }
    }

    Ok(warnings)
}

#[allow(unused)]
pub fn check_files_threaded(source_type: &str, warn_buff: Arc<Mutex<Vec<Warning>>>) -> Result<()> {
    let mut warnings = check_files(source_type)?;

    let mut lock = warn_buff.lock().unwrap();
    lock.append(&mut warnings);

    Ok(())
}

fn scan_file(filename: &str, source_code: &str, func_map: &FunctionMap) -> Vec<Warning> {
    let mut warnings = vec![];

    for (line_num, line) in source_code.split('\n').enumerate() {
        if line.trim().len() == 0 {
            continue;
        }

        let tokens = match lexer_c::tokenize(line) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if tokens.len() < 3 {
            continue;
        }

        for i in 0..(tokens.len() - 1) {
            if let lexer_c::Token::Object(obj) = tokens[i] {
                if tokens[i + 1] != lexer_c::Token::OpenParen {
                    continue;
                }
                if let Some(safe_fn) = func_map.map.get(obj) {
                    let warning = Warning {
                        warning_type: WarningType::UnsafeFunction,
                        msg: format!(
                            "{}() is an unsafe function. Consuder using {}() instead",
                            obj, safe_fn
                        ),
                        filename: filename.to_string(),
                        line: line_num + 1,
                    };

                    warnings.push(warning);
                }
            }
        }
    }

    warnings
}

/// Executes a binary (merges Stdio to the current process) and
/// returns a string of Valgrind's results and the number of lines
/// output by the underlying program
pub fn exec_w_valgrind(bin_path: &str, passthough_args: &Vec<String>) -> Result<VgOutput> {
    let log_file = format!("--xml-file={}", VALGRIND_OUT);
    let mut valgrind_args = vec![
        &log_file,
        "--leak-check=full",
        "--track-origins=yes",
        "--xml=yes",
        bin_path,
    ];

    for arg in passthough_args {
        valgrind_args.push(arg.as_str());
    }

    let output = Command::new("valgrind")
        .args(valgrind_args)
        .stdin(process::Stdio::inherit())
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|e| anyhow!("Failed to run valgrind binary: {}", e));

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            println!("Error spawning valgrind process: {}", e);
            println!("Make sure you have valgrind installed");
            println!("Debian Based  => sudo apt-get install valgrind");
            println!("Arch Based    => sudo dnf install valgrind");
            println!("Fedora Based  => sudo pacman -S valgrind");
            std::process::exit(1);
        }
    };
    if !output.status.success() {
        let code = output.status.code().unwrap_or(1);
        process::exit(code);
    }

    let valgrind_out = fs::read_to_string(VALGRIND_OUT)
        .map_err(|err| anyhow!("Error reading from file: {}", err))?;

    if fs::exists(VALGRIND_OUT).map_err(|err| anyhow!("Error checking if file exists: {}", err))? {
        fs::remove_file(VALGRIND_OUT).map_err(|err| anyhow!("Error removing file: {}", err))?;
    }

    VgOutput::from_str(&valgrind_out).map_err(|err| anyhow!("Error parsing Valgrind: {}", err))
}

/// Returns a vector of human readable error messages
/// meant for the end user of Kiln to see
pub fn print_vg_errors(vg_output: &VgOutput) {
    for err in &vg_output.errors {
        let mut filename = "UKNOWN".to_string();
        let mut line = "??".to_string();

        for stack in err.stack.frames.iter() {
            for frame in stack {
                if let Some(file) = frame.file.as_ref() {
                    filename = file.clone();
                }
                if let Some(&line_no) = frame.line.as_ref() {
                    line = format!("{}", line_no);
                }
            }
        }

        utils::print_warning("Valgrind", &filename, &line, &err.kind, &err.xwhat.text);
    }
}
