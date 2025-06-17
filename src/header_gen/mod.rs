pub mod lexer_c;

use std::{collections::{HashMap, HashSet}, fs};

use anyhow::{anyhow, Result};
use ouroboros::self_referencing;

use crate::config;


// Maps character's ascii codes to their token
const TOKEN_MAPPING: [Option<Token>; 128] = [
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Token::Tab),
    Some(Token::NewLine),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Token::Space),
    Some(Token::Exclamation),
    None,
    Some(Token::HashTag),
    Some(Token::DollarSign),
    Some(Token::ModOperator),
    Some(Token::Ampersand),
    None,
    Some(Token::OpenParen),
    Some(Token::CloseParen),
    Some(Token::Asterisk),
    Some(Token::Plus),
    Some(Token::Comma),
    Some(Token::Minus),
    Some(Token::Period),
    Some(Token::ForwardSlash),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Token::Colon),
    Some(Token::Semicolon),
    Some(Token::LessThan),
    Some(Token::Equal),
    Some(Token::GreaterThan),
    Some(Token::QuestionMark),
    Some(Token::At),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Token::OpenSquareBracket),
    Some(Token::BackSlash),
    Some(Token::CloseSquareBracket),
    Some(Token::Carrot),
    None,
    Some(Token::Tick),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Token::OpenCurlyBrace),
    Some(Token::Pipe),
    Some(Token::CloseCurlyBrace),
    Some(Token::Tilda),
    None,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token<'a> {
    Object(&'a str),
    Literal(&'a str),
    Comment(&'a str),
    HashTag,
    GreaterThan,
    LessThan,
    Equal,
    Exclamation,
    Period,
    OpenParen,
    CloseParen,
    OpenCurlyBrace,
    CloseCurlyBrace,
    OpenSquareBracket,
    CloseSquareBracket,
    Semicolon,
    Comma,
    Asterisk,
    Plus,
    Minus,
    ForwardSlash,
    BackSlash,
    Pipe,
    Ampersand,
    ModOperator,
    Carrot,
    Colon,
    At,
    DollarSign,
    Tilda,
    Tick,
    QuestionMark,
    NewLine,
    Space,
    Tab,
}

#[derive(Default)]
pub struct ProjTokens<'self_> {
    // Maps full file paths to its source code
    code_files: HashMap<String, Option<String>>,

    // Maps fill file paths to the lazily loaded tokens
    tokens: HashMap<String, Vec<Token<'self_>>>,

    // Option<(does a file with int main() exist? , "filename if it does exist")>
    main_file: Option<(bool, String)>,
}

impl<'a> Token<'a> {
    pub fn tokens_to_string(tokens: &[Token]) -> String {
        let mut string = String::new();

        for &t in tokens.iter() {
            if let Token::Object(s) = t {
                string.push_str(s);
            }
            else if let Token::Literal(s) = t {
                string.push_str(s);
            }
            else if let Token::Comment(c) = t {
                string.push_str(c);
            } 
            else {
                for i in 0..TOKEN_MAPPING.len() {
                    if let Some(c) = TOKEN_MAPPING[i] {
                        if c == t {
                            string.push((i as u8) as char);
                        }
                    }
                }
            }
        }
        string
    }
}


impl<'self_> ProjTokens<'self_> {
    fn from_proj(config: &config::Config) {
        let mut proj_tokens = Self::default();

        let dir_groups = &[
            &config.project.src_dirs,
            &config.project.include_dirs,
        ];

        for &dir_group in dir_groups {
            for src_dir in dir_group {
                for src_file in fs::read_dir(src_dir).unwrap() {
                    if let Ok(src_file) = src_file {
                        if !src_file.file_type().unwrap().is_file() {
                            continue;
                        }

                        let src_file_str = src_file.path()
                            .to_str()
                            .unwrap()
                            .to_string();
    
                        let valid_files = &[
                            config.project.language_ext(),
                            config.project.header_ext(),
                        ];

                        if valid_files.iter().any(|&ext| src_dir.ends_with(ext)) {
                            proj_tokens.code_files.insert(src_file_str.clone(), None);
                        }
                    }
                }
            }
        }
    }


    /// Returns None if there is no file in the project direcotry with the name specified
    fn get_tokens<'a>(&'a mut self, filepath: &str) -> Option<&'a Vec<Token<'self_>>> 
        where 'a: 'self_
    {
        if let Some(tokens) = self.tokens.get(filepath) {
            return Some(tokens);
        }

        let code_text = match self.code_files.get(filepath) {
            Some(code_text) => {
                match code_text {
                    Some(code_text) => code_text,
                    None => {
                        let code_text = fs::read_to_string(filepath).unwrap();
                        unsafe {
                            let code_files_ptr = &self.code_files as *const HashMap<String, Option<String>>;
                            let code_files_ptr = code_files_ptr as *mut HashMap<String, Option<String>>;
                            (*code_files_ptr).insert(filepath.to_string(), Some(code_text));
                        }

                        self.code_files.get(filepath).unwrap().as_ref().unwrap()
                    }
                }
            }
            None => {
                return None;
            }
        };

        let tokens = lexer_c::tokenize(&code_text).unwrap();
        unsafe {
            let tokens_ptr= &self.tokens as *const HashMap<String, Vec<Token<'self_>>>;
            let tokens_ptr = tokens_ptr as *mut HashMap<String, Vec<Token<'self_>>>;
            (*tokens_ptr).insert(filepath.to_string(), tokens);
        }

        self.tokens.get(filepath)
    }

    fn iter_tokens(&mut self) {

    }

}

struct ProjTokensIter<'a> {
    proj_tokens: &'a mut ProjTokens<'a>,
    filepaths: Vec<String>,
    idx: usize,
}

impl<'a> Iterator for ProjTokensIter<'a> {
    type Item = &'a Vec<Token<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.filepaths.len() {
            return None;
        }

        let idx = self.idx;
        self.idx += 1;

        let tokens_ptr = unsafe {
            let proj_tokens_ptr = self.proj_tokens as *mut ProjTokens<'a>;
            (*proj_tokens_ptr).get_tokens(&self.filepaths[idx]).unwrap()
        };

        unsafe {
            let tokens_ptr: &'a Vec<Token<'a>> = std::mem::transmute(tokens_ptr);
            Some(tokens_ptr)
        }
    }
}

/// Returns an error if there are any duplicate definitions
/// Otherwise, adds all definitions in `src` to `dst`
pub fn merge_defines<'a>(
    dst: &mut Vec<&'a [Token<'a>]>,
    src: &[&'a [Token<'a>]],
) -> Result<()> {
    let mut dst_set = HashSet::new();

    for &tokens in dst.iter() {
        let s = lexer_c::get_define_name(tokens);
        dst_set.insert(s);
    }

    for &tokens in src.iter() {
        let s = lexer_c::get_define_name(tokens);
        if dst_set.contains(&s) {
            return Err(anyhow!("Duplicate #define definitions for {}", s));
        }
    }

    dst.extend_from_slice(src);

    Ok(())
}

/// Returns an error if there are any duplicate definitions
/// Otherwise, adds all definitions in `src` to `dst`
pub fn merge_includes<'a>(
    dst: &mut Vec<&'a [Token<'a>]>,
    src: &[&'a [Token<'a>]],
) {
    let mut dst_set = HashSet::new();

    for &tokens in dst.iter() {
        let s = lexer_c::get_include_name(tokens);
        dst_set.insert(s);
    }

    for &tokens in src.iter() {
        let s = lexer_c::get_include_name(tokens);
        if !dst_set.contains(&s) {
            dst.push(tokens);
        }
    }
}

/// Returns an error if there are any duplicate definitions
/// Otherwise, adds all definitions in `src` to `dst`
pub fn merge_udts<'a>(
    dst: &mut Vec<&'a [Token<'a>]>,
    src: &[&'a [Token<'a>]],
) -> Result<()> {
    let mut dst_set = HashSet::new();

    for &tokens in dst.iter() {
        let s = lexer_c::get_udt_name(tokens);
        dst_set.insert(s);
    }

    for &tokens in src.iter() {
        let s = lexer_c::get_udt_name(tokens);
        if dst_set.contains(&s) {
            return Err(anyhow!("Duplicate struct definitions for {}", s));
        }
    }

    dst.extend_from_slice(src);

    Ok(())
}

/// Expects raw source code and an include path (in the form `"../include/filename.h"`)
/// This will do nothing and return `code` if the include statement already exists, otherwise
/// it will insert it at the end of all the include statements
pub fn insert_self_include(code: String, include: &str) -> String {
    let mut code_lines: Vec<&str> = code.lines().collect();

    let contains_include = code_lines.iter().any(|&line| {
        line.trim().starts_with("#") && line.contains("include") && line.contains(include)
    });

    if contains_include {
        return code;
    }

    let mut line_idx: usize = 0;

    for (i, &line) in code_lines.iter().enumerate() {
        let is_include_statement = line.trim().starts_with("#")
            && line.contains("include")
            && (line.contains("<") || line.contains("\""));

        if is_include_statement {
            line_idx = i;
        }
    }

    let include_line = format!("#include {}", include);

    if code_lines.len() == 0 {
        code_lines.push(&include_line);
    } else {
        code_lines.insert(line_idx + 1, &include_line);
    }

    code_lines.join("\n")
}

/// Filters out `#include "XXX.h"` where `file_name` is `"XXX"`
pub fn filter_out_includes<'a>(
    includes: &Vec<&'a [Token]>,
    file_name: &str,
) -> Vec<&'a [Token<'a>]> {
    let include_str_name = [
        format!("{}.h\"", file_name),
        format!("/{}.h\"", file_name),
        format!("\\{}.h\"", file_name),
    ];

    includes
        .clone()
        .into_iter()
        .filter(|&x| {
            if let Some(Token::Literal(s)) = x.last() {
                if s == &include_str_name[0] {
                    return false;
                } else if include_str_name[1..]
                    .iter()
                    .any(|inc_name| s.ends_with(inc_name))
                {
                    return false;
                }
            }

            true
        })
        .collect()
}
