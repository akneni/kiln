pub mod lexer_c;

use std::collections::HashSet;

use anyhow::{anyhow, Result};

/// Returns an error if there are any duplicate definitions
/// Otherwise, adds all definitions in `src` to `dst`
pub fn merge_defines<'a>(
    dst: &mut Vec<&'a [lexer_c::Token<'a>]>,
    src: &[&'a [lexer_c::Token<'a>]],
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
    dst: &mut Vec<&'a [lexer_c::Token<'a>]>,
    src: &[&'a [lexer_c::Token<'a>]],
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
    dst: &mut Vec<&'a [lexer_c::Token<'a>]>,
    src: &[&'a [lexer_c::Token<'a>]],
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
    includes: &Vec<&'a [lexer_c::Token]>,
    file_name: &str,
) -> Vec<&'a [lexer_c::Token<'a>]> {
    let include_str_name = [
        format!("{}.h\"", file_name),
        format!("/{}.h\"", file_name),
        format!("\\{}.h\"", file_name),
    ];

    includes
        .clone()
        .into_iter()
        .filter(|&x| {
            if let Some(lexer_c::Token::Literal(s)) = x.last() {
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
