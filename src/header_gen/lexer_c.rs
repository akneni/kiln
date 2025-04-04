use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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


pub fn tokenize_unclean(code: &str) -> Result<(Vec<Token>, Vec<usize>)> {
    let code_bytes = code.as_bytes();
    let mut tokens = Vec::with_capacity(4096);
    let mut byte_idx = Vec::with_capacity(4096);

    let mut idx: usize = 0;
    while idx < code.len() {
        if code[idx..].starts_with("//") {
            idx += code[idx..].find('\n').unwrap_or(code.len()) + 1;
            continue;
        } else if code[idx..].starts_with("/*") {
            idx += code[idx..].find("*/").unwrap_or(code.len()) + 2;
            continue;
        }

        if code_bytes[idx] == ' ' as u8 || code_bytes[idx] == '\t' as u8 {
            idx += 1;
            continue;
        }
        if let Some(sym) = is_symbol(&code[idx..]) {
            tokens.push(sym);
            byte_idx.push(idx);
            idx += 1;
            continue;
        }
        if code_bytes[idx] == '"' as u8 {
            let len = find_len_string_literal(&code_bytes[idx..])?;
            let val = &code[idx..(idx + len)];
            let tok = Token::Literal(val);
            tokens.push(tok);
            byte_idx.push(idx);
            idx += len;
            continue;
        }
        let new_idx = find_len_object(code_bytes, idx);
        let val = &code[idx..new_idx];
        let tok = Token::Object(val);
        tokens.push(tok);
        byte_idx.push(idx);
        idx = new_idx;
    }

    Ok((tokens, byte_idx))
}

pub fn tokenize(code: &str) -> Result<Vec<Token>> {
    let code_bytes = code.as_bytes();
    let mut tokens = Vec::with_capacity(4096);

    let mut idx: usize = 0;
    while idx < code.len() {
        match code_bytes[idx] as char {
            ' ' => {
                tokens.push(Token::Space);
                idx += 1;
                continue;
            }
            '\t' => {
                tokens.push(Token::Tab);
                idx += 1;
                continue;
            }
            '\n' => {
                tokens.push(Token::NewLine);
                idx += 1;
                continue;
            }
            '"' => {
                let len = find_len_string_literal(&code_bytes[idx..])?;
                let val = &code[idx..(idx + len)];
                let tok = Token::Literal(val);
                tokens.push(tok);
                idx += len;
                continue;
            }
            '/' => {
                if matches!(code_bytes[idx+1] as char, '*' | '/') {
                    let len = find_len_comment(&code_bytes[idx..]);
                    let val = &code[idx..(idx + len)];
                    let tok = Token::Comment(val);
                    tokens.push(tok);
                    idx += len;
                    continue;
                }
            }
            _ => {}
        }

        if let Some(sym) = is_symbol(&code[idx..]) {
            tokens.push(sym);
            idx += 1;
            continue;
        }
        let new_idx = find_len_object(code_bytes, idx);
        let val = &code[idx..new_idx];
        let tok = Token::Object(val);
        tokens.push(tok);
        idx = new_idx;
    }

    Ok(tokens)
}

#[inline]
fn is_symbol(code: &str) -> Option<Token> {
    let char = code.chars().next();
    if let Some(char) = char {
        let char_code = char as usize;
        if char_code > TOKEN_MAPPING.len() {
            return None;
        }
        return TOKEN_MAPPING[char_code];
    }
    None
}

fn find_len_object(code_bytes: &[u8], mut curr_idx: usize) -> usize {
    curr_idx += 1;
    while curr_idx < code_bytes.len() {
        let ascii_char = code_bytes[curr_idx] as usize;
        if ascii_char < TOKEN_MAPPING.len() {
            if TOKEN_MAPPING[ascii_char].is_some() || ascii_char == ' ' as usize {
                return curr_idx;
            }
        }
        curr_idx += 1;
    }
    return curr_idx;
}

/// `code_bytes` must be a slice such that the start of the slice is the same as the start of the string (first character must be a `"`)
fn find_len_string_literal(code_bytes: &[u8]) -> Result<usize> {
    let mut idx: usize = 1;
    while idx < code_bytes.len() {
        if code_bytes[idx] == '\n' as u8 {
            break;
        }
        if code_bytes[idx] == '"' as u8 {
            if code_bytes[idx] != '\\' as u8 {
                idx += 1;
                return Ok(idx);
            }
        }
        idx += 1;
    }
    Err(anyhow!("String literal not closed"))
}

/// `code_bytes` must be a slice such that the start of the slice is the same as the start of the comment (first characters must be `//` or `/*`)
fn find_len_comment(code_bytes: &[u8]) -> usize {
    #[cfg(debug_assertions)] {
        if code_bytes[0] != '/' as u8 || !(matches!(code_bytes[1] as char, '*' | '/')){
            panic!("Not a comment");
        }    
    }

    let mut idx = 2;
    match code_bytes[1] as char {
        '*' => {
            while idx < code_bytes.len() {
                if code_bytes[idx] == '*' as u8 && code_bytes[idx+1] == '/' as u8 {
                    idx += 2;
                    break;
                }
                idx += 1;
            }
        }
        '/' => {
            while idx < code_bytes.len() && code_bytes[idx] != '\n' as u8 {
                idx += 1;
            }
        }
        _ => unsafe { std::hint::unreachable_unchecked() },
    }

    idx
}

/// Gets the ranges (as [start, end] byte offsets) from the `byte_idx` vector to keep,
/// excluding the token slices in `exclude_tokens`.
///
/// Both `tokens` and `byte_idx` are assumed to be parallel; i.e. the ith element of `byte_idx`
/// gives the starting offset of the ith token in `tokens`. In an ideal setup, `byte_idx` would have
/// one extra element (the file length) to mark the end of the last token.
pub fn get_inclusion_ranges(
    tokens: &Vec<Token>,
    byte_idx: &Vec<usize>,
    exclude_tokens: &[&[Token]],
) -> Vec<[usize; 2]> {
    let mut inclusion_ranges = Vec::new();
    // current_inclusion_start marks the index (in tokens) where the current “keep” region began.
    let mut current_inclusion_start: usize = 0;
    let mut i = 0;

    while i < tokens.len() {
        let mut matched_exclusion = None;
        // See if any of the exclusion slices match starting at token index i.
        for &excl in exclude_tokens {
            if excl.is_empty() {
                continue;
            }
            // If there are enough tokens left and the slice matches...
            if i + excl.len() <= tokens.len() && &tokens[i..(i + excl.len())] == excl {
                matched_exclusion = Some(excl.len());
                break;
            }
        }

        if let Some(skip_len) = matched_exclusion {
            // End the current inclusion region (if nonempty) at the beginning of the exclusion.
            if current_inclusion_start < i {
                inclusion_ranges.push([byte_idx[current_inclusion_start], byte_idx[i]]);
            }
            // Skip over the excluded tokens.
            i += skip_len;
            current_inclusion_start = i;
        } else {
            // No exclusion match here; move on.
            i += 1;
        }
    }

    // If there is any trailing inclusion region after the last exclusion, add it.
    if current_inclusion_start < tokens.len() {
        // For the end offset, we try to use the next byte offset if available.
        // (Ideally, byte_idx has length tokens.len() + 1.)
        let end = if tokens.len() < byte_idx.len() {
            byte_idx[tokens.len()]
        } else {
            // Fallback: use the last token's start offset.
            *byte_idx.last().unwrap()
        };
        inclusion_ranges.push([byte_idx[current_inclusion_start], end]);
    }

    inclusion_ranges
}

pub fn merge_inclusion_ranges(code: &str, inclusion_ranges: &Vec<[usize; 2]>) -> String {
    let mut new_code = "".to_string();
    if inclusion_ranges.len() == 0 {
        return new_code;
    }

    for range in inclusion_ranges[..inclusion_ranges.len() - 1].iter() {
        new_code.push_str(&code[range[0]..range[1]]);
    }

    if let Some(&r) = inclusion_ranges.last() {
        let mut r = r;
        if r[1] == code.len() - 1 {
            r[1] += 1;
        }
        new_code.push_str(&code[r[0]..r[1]]);
    }

    new_code
}

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

const RESTRICTED_KWARGS: &[&str] = &["for", "while", "if"];

pub fn get_fn_def<'a>(tokens: &'a Vec<Token>) -> Vec<&'a [Token<'a>]> {
    let mut fn_defs = vec![];

    let mut conditions: [bool; 3];

    let mut idx: usize = 0;
    while idx < tokens.len() {
        conditions = [
            false, // Starts with at least two objects
            false, // Has open paren
            false, // Has close paren
        ];

        if let Token::Object(obj) = tokens[idx] {
            if RESTRICTED_KWARGS.contains(&obj) {
                skip_to(tokens, Token::CloseParen, &mut idx);
                continue;
            } else if obj == "include" {
                skip_to_oneof(tokens, &[Token::GreaterThan, Token::Literal("\"")], &mut idx);
                continue;
            } else if obj == "define" {
                skip_to(tokens, Token::NewLine, &mut idx);
                continue;
            } else if matches!(obj, "return" | "if") {
                idx += 1;
                continue;
            }

            let mut j = idx + 1;
            while j < tokens.len() {
                if let Token::Object(obj_2) = tokens[j] {
                    if RESTRICTED_KWARGS.contains(&obj_2) || obj_2 == "main" {
                        break;
                    }
                    conditions[0] = true;
                } else if let Token::OpenParen = tokens[j] {
                    conditions[1] = true;
                } else if let Token::CloseParen = tokens[j] {
                    conditions[2] = true;
                } else if let Token::OpenCurlyBrace = tokens[j] {
                    if conditions.iter().all(|&i| i) {
                        fn_defs.push(&tokens[idx..j]);
                    }
                    break;
                } else if let Token::Semicolon = tokens[j] {
                    break;
                } else if let Token::Equal = tokens[j] {
                    break;
                }
                j += 1;
            }
            idx = j + 1;
            continue;
        }
        idx += 1;
    }

    fn_defs
}

pub fn get_includes<'a>(tokens: &'a Vec<Token>) -> Vec<&'a [Token<'a>]> {
    let mut includes = vec![];

    let mut idx: usize = 0;
    while idx < tokens.len() {
        if let Token::HashTag = tokens[idx] {
            if tokens[idx + 1] != Token::Object("include") {
                idx += 1;
                continue;
            }
            let mut end = idx;
            skip_to_oneof(
                tokens,
                &[Token::GreaterThan, Token::Literal("\"")],
                &mut end,
            );
            includes.push(&tokens[idx..(end + 1)]);
            idx = end;
            continue;
        }
        idx += 1;
    }

    includes
}

/// Extracts the user defined types (UDTs)
pub fn get_udts<'a>(tokens: &'a Vec<Token>) -> Vec<&'a [Token<'a>]> {
    let mut udts = vec![];
    if tokens.len() < 3 {
        return udts;
    }

    let mut idx: usize = 0;
    while idx < tokens.len() - 2 {
        if let Token::Object(obj) = tokens[idx] {
            if !matches!(obj, "typedef" | "struct" | "union" | "enum") {
                idx += 1;
                continue;
            } 

            let next_idx = if obj == "typedef" {
                let x = idx + next_non_whitespace_token(&tokens[idx..]);
                if x >= tokens.len() {
                    unreachable!();
                }
                x
            }
            else {
                idx
            };

            match tokens[next_idx] {
                Token::Object("struct") |
                Token::Object("enum") |
                Token::Object("union") => {
                    let start_idx = idx;
                    idx = next_idx;
                    let mut curlybrace_stack = 0;

                    while idx < tokens.len() {
                        match tokens[idx] {
                            Token::OpenCurlyBrace => curlybrace_stack += 1,
                            Token::CloseCurlyBrace => {
                                if curlybrace_stack == 0 {
                                    unreachable!();
                                }

                                curlybrace_stack -= 1;
                            }
                            Token::Semicolon => {
                                if curlybrace_stack == 0 {
                                    let x = &tokens[start_idx..=idx];
                                    udts.push(x);
                                    break;
                                }
                            }
                            _ => {},
                        }
                        idx += 1;
                    }
                }
                _ => {
                    idx = next_idx;
                }
            }
        }
        else {
            idx += 1;
        }
    }

    udts
}

pub fn get_defines<'a>(tokens: &'a Vec<Token>) -> Vec<&'a [Token<'a>]> {
    let mut defines = vec![];

    let mut idx: usize = 0;

    while idx < tokens.len() {
        if tokens[idx] != Token::HashTag {
            skip_to(tokens, Token::HashTag, &mut idx);
        }

        let start_idx = idx;

        if idx + 1 >= tokens.len() || tokens[idx + 1] != Token::Object("define") {
            idx += 2;
            continue;
        }
        idx += 1;

        skip_to(tokens, Token::NewLine, &mut idx);
        while idx < tokens.len() && tokens[idx - 1] == Token::BackSlash {
            skip_to(tokens, Token::NewLine, &mut idx);
        }

        defines.push(&tokens[start_idx..idx]);
    }

    defines
}

/// Gets the name of the struct
/// Ex) for `struct Point {...}`, this would return "Point"
pub fn get_udt_name<'a>(tokens: &'a [Token]) -> &'a str {
    if tokens.len() < 3 {
        unreachable!("Token string is not a valid user defined type definition");
    }

    let mut idx = 0;
    let mut num_unclosed_braces = 0;
    
    while idx < tokens.len() {
        match tokens[idx] {
            Token::Object("struct") |
            Token::Object("enum") |
            Token::Object("union") => {
                let next_idx = idx + next_non_whitespace_token(&tokens[idx..]);

                if next_idx + 1 >= tokens.len() {
                    unreachable!("Invalid UDT (1)");
                }
                if let Token::Object(obj) = tokens[next_idx] {
                    return obj;
                }
            }
            Token::OpenCurlyBrace => num_unclosed_braces += 1,
            Token::CloseCurlyBrace => {
                if num_unclosed_braces == 0 {
                    unreachable!("Invalid UDT (unmatched close curly brace)");
                }

                num_unclosed_braces -= 1;

                if num_unclosed_braces == 0 {
                    let next_idx = idx + next_non_whitespace_token(&tokens[idx..]);
                    if next_idx + 1 >= tokens.len() {
                        unreachable!("Invalid UDT (2)");
                    }

                    if let Token::Object(obj) = tokens[next_idx] {
                        return obj;
                    }
                }
            }
            _ => {}
        }
        idx += 1;
    }

    unreachable!("Invalid UDT (end)");
}

/// Gets the name of the define statement
/// Ex) for `#define FOO 42`, this would return "FOO"
pub fn get_define_name<'a>(tokens: &'a [Token]) -> &'a str {
    if tokens.len() < 5 || tokens[0] != Token::HashTag {
        unreachable!("Token string is not a valid define macro (1)");
    }

    let mut define_seen = false;

    for &t in &tokens[1..] {
        match t {
            Token::Object("define") => {
                if define_seen {
                    unreachable!("Token string is not a valid define macro (2)");
                }
                define_seen = true;
            }
            Token::Object(obj) => {
                if define_seen {
                    return obj;
                }
                else {
                    unreachable!("Token string is not a valid define macro (3)");
                }
            }
            _ => {}
        }
    }


    unreachable!("Token string is not a valid define macro (4)");
}

/// Updates `idx` to point to the next token specified. If the
/// token does not exist, `idx` will be set equal to tokens.len()
fn skip_to(tokens: &[Token], target: Token, idx: &mut usize) {
    for i in (*idx + 1)..tokens.len() {
        *idx = i;
        if tokens[i] == target {
            return;
        }
    }

    // If the for loop ends, we haven't found it, so set idx appropriately
    *idx = tokens.len();
}

/// Ignores values inside the targets, it just skips to the next token
/// that's one of the target variants
fn skip_to_oneof(tokens: &[Token], targets: &[Token], idx: &mut usize) {
    for i in (*idx + 1)..tokens.len() {
        *idx = i;
        for target in targets {
            if std::mem::discriminant(&tokens[i]) == std::mem::discriminant(target) {
                return;
            }
        }
    }
}


/// Passing the below list to this function would return `3` (gets the next token, not the current token)
/// `[object-token-curr, whitespace, whitespace, object-token-next]`
#[inline]
fn next_non_whitespace_token(tokens: &[Token]) -> usize {
    let mut idx = 1;
    while idx < tokens.len() && matches!(tokens[idx], Token::Space | Token::Tab | Token::NewLine | Token::Comment(_)) {
        idx += 1;
    }

    idx
}

#[cfg(test)]
mod lexer_tests {
    use std::fs;

    use super::*;

    #[test]
    fn test_get_defines() {
        let s = fs::read_to_string("tests/lexer-define.c").unwrap();
        let tokens = tokenize(&s).unwrap();

        let defines = get_defines(&tokens);

        let mut log_dump = "".to_string();
        for &def in &defines {
            let x = format!("{:?}\n\n", def);
            log_dump.push_str(&x);
        }

        fs::write("tests/lexer.test_get_defines.log", format!("{}", log_dump)).unwrap();

        assert_eq!(
            defines.len(),
            s.split("#define").collect::<Vec<&str>>().len() - 1
        );
    }

    #[test]
    fn test_get_udts() {
        let s = fs::read_to_string("tests/lexer-UDT.c").unwrap();
        let tokens = tokenize(&s).unwrap();

        let defines = get_udts(&tokens);

        let mut log_dump = "".to_string();
        for &def in &defines {
            let x = format!("{:?}\n\n", def);
            log_dump.push_str(&x);
        }

        fs::write("tests/lexer.test_get_udts.log", format!("{}", log_dump)).unwrap();
    }

    #[test]
    fn test_get_define_name() {
        let s = fs::read_to_string("tests/lexer-define.c").unwrap();
        let tokens = tokenize(&s).unwrap();

        let defines = get_defines(&tokens);

        let mut names = vec![];
        for &d in &defines {
            names.push(get_define_name(d));
        }

        assert_eq!(
            defines.len(),
            s.split("#define").collect::<Vec<&str>>().len() - 1
        );

        fs::write(
            "tests/lexer.test_get_define_name.log",
            format!("{:#?}", names),
        )
        .unwrap();
    }

    #[test]
    fn test_get_udt_name() {
        let s = fs::read_to_string("tests/lexer-UDT.c").unwrap();
        let tokens = tokenize(&s).unwrap();

        let structs = get_udts(&tokens);

        let mut names = vec![];
        for &d in &structs {
            names.push(get_udt_name(d));
        }

        let mut dump = "".to_string();

        for (i, n) in names.into_iter().enumerate() {
            dump.push_str(&format!("{}) {}\n", i + 1, n));
        }

        fs::write("tests/lexer.test_get_udt_name.log", format!("{}", dump)).unwrap();
    }
}
