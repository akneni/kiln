use std::collections::HashSet;

use crate::lexer;
use anyhow::{Result, anyhow};


/// Returns an error if there are any duplicate definitions
/// Otherwise, adds all definitions in `src` to `dst`
pub(super) fn merge_defines<'a>(dst: &mut Vec<&'a [lexer::Token<'a>]>, src: &[&'a [lexer::Token<'a>]]) -> Result<()> {
    let mut dst_set = HashSet::new();

    for &tokens in dst.iter() {
        let s = lexer::get_define_name(tokens);
        dst_set.insert(s);
    }

    for &tokens in src.iter() {
        let s = lexer::get_define_name(tokens);
        if dst_set.contains(&s) {
            return Err(anyhow!("Duplicate #define definitions for {}", s));
        }
    }

    dst.extend_from_slice(src);

    Ok(())
}

/// Returns an error if there are any duplicate definitions
/// Otherwise, adds all definitions in `src` to `dst`
pub(super) fn merge_structs<'a>(dst: &mut Vec<&'a [lexer::Token<'a>]>, src: &[&'a [lexer::Token<'a>]]) -> Result<()> {
    let mut dst_set = HashSet::new();

    for &tokens in dst.iter() {
        let s = lexer::get_struct_name(tokens);
        dst_set.insert(s);
    }

    for &tokens in src.iter() {
        let s = lexer::get_struct_name(tokens);
        if dst_set.contains(&s) {
            return Err(anyhow!("Duplicate struct definitions for {}", s));
        }
    }

    dst.extend_from_slice(src);

    Ok(())
}