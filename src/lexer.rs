use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token<'a> {
    Object(&'a str),
    Literal(&'a str),
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
}

impl<'a> Token<'a> {
    pub fn tokens_to_string(tokens: &[Token]) -> String {
        if tokens.len() >= 2 && (tokens[0] == Self::Object("struct") || tokens[1] == Self::Object("struct")) {
            return Self::struct_tokens_to_string(tokens);
        }

        let space_after = [Token::Comma, Token::Asterisk];
        let mut string = String::new();

        for (i, &t) in tokens.iter().enumerate() {
            if let Token::Object(s) = t {
                if i != 0 {
                    if let Token::Object(_) = tokens[i - 1] {
                        string.push(' ');
                    } else if space_after.contains(&tokens[i - 1]) {
                        string.push(' ');
                    }
                }
                string.push_str(s);
                if s == "include" {
                    string.push(' ');
                }
            } else if let Token::Literal(s) = t {
                string.push_str(s);
            } else {
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

    fn struct_tokens_to_string(tokens: &[Token]) -> String {
        let mut output = String::new();
        let mut indent_level = 0;
        let mut start_of_line = true;
    
        for (i, token) in tokens.iter().enumerate() {
            // At the beginning of a new line, insert indentation based on the current indent_level.
            if start_of_line {
                output.push_str(&"    ".repeat(indent_level));
                start_of_line = false;
            }
    
            match token {
                Token::Object(s) => {
                    if i > 0 && !output.ends_with('\n') {
                        // Add a space if the previous token is one of these.
                        match tokens[i - 1] {
                            Token::Object(_)
                            | Token::Literal(_)
                            | Token::CloseCurlyBrace
                            | Token::Asterisk => {
                                output.push(' ');
                            }
                            _ => {}
                        }
                    }
                    output.push_str(s);
                }
                Token::Literal(s) => {
                    output.push_str(s);
                }
                Token::OpenCurlyBrace => {
                    // Keep the '{' on the same line as the header.
                    output.push_str(" {");
                    output.push('\n');
                    indent_level += 1; // Increase indent level for the struct body.
                    start_of_line = true;
                }
                Token::CloseCurlyBrace => {
                    // Ensure the '}' starts on a new line.
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                    indent_level = indent_level.saturating_sub(1); // Decrease indent level.
                    output.push_str(&"    ".repeat(indent_level));
                    output.push('}');
                }
                Token::Semicolon => {
                    output.push(';');
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                    start_of_line = true;
                }
                Token::NewLine => {
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                    start_of_line = true;
                }
                _ => {
                    // For any other token, convert it via TOKEN_MAPPING.
                    for j in 0..TOKEN_MAPPING.len() {
                        if let Some(c) = TOKEN_MAPPING[j] {
                            if c == *token {
                                output.push((j as u8) as char);
                            }
                        }
                    }
                }
            }
        }
    
        // Post-process the output to remove any blank lines (lines that contain only whitespace).
        let cleaned = output
            .split("\n")
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
            
        cleaned
    }    
    
    
}

pub fn clean_source_code(code: String) -> String {
    // TDOD: skip over any `//` of `/*` that are in string literals

    let mut cleaned = String::with_capacity(code.len());

    let mut in_block_comment = false; // whether we're inside /* ... */
    for line in code.split("\n") {
        let line = line.trim();
        if line.len() == 0 {
            continue;
        }
        if in_block_comment {
            if let Some(bc_idx) = line.find("*/") {
                let line = line[(bc_idx+2)..].trim();
                in_block_comment = false;
                let comment_idx = line.find("//").unwrap_or(line.len());
                let mut line = line[..comment_idx].trim();
                if let Some(bc_idx) = line.find("/*") {
                    in_block_comment = true;
                    line = line[..bc_idx].trim();
                }
                if line.len() == 0 {
                    continue;
                }

                cleaned.push_str(line);
                cleaned.push('\n');
            }
        } else {
            if !line.contains("/") {
                cleaned.push_str(line);
                cleaned.push('\n');
                continue;
            }
            let comment_idx = line.find("//").unwrap_or(line.len());
            let mut line = line[..comment_idx].trim();
            if let Some(bc_idx) = line.find("/*") {
                in_block_comment = true;
                line = line[..bc_idx].trim();
            }
            if line.len() == 0 {
                continue;
            }

            cleaned.push_str(line);
            cleaned.push('\n');
        }
    }

    cleaned.trim().to_string()
}

pub fn tokenize(code: &str) -> Result<Vec<Token>> {
    let code_bytes = code.as_bytes();
    let mut tokens = Vec::with_capacity(4096);

    let mut idx: usize = 0;
    while idx < code.len() {
        if code_bytes[idx] == ' ' as u8 || code_bytes[idx] == '\t' as u8 {
            idx += 1;
            continue;
        }
        if let Some(sym) = is_symbol(&code[idx..]) {
            tokens.push(sym);
            idx += 1;
            continue;
        }
        if code_bytes[idx] == '"' as u8 {
            let len = find_len_stringliteral(&code_bytes[idx..])?;
            let val = &code[idx..(idx + len)];
            let tok = Token::Literal(val);
            tokens.push(tok);
            idx += len;
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

fn find_len_stringliteral(code_bytes: &[u8]) -> Result<usize> {
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
    Err(anyhow!("String listeral not closed"))
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
    None,
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
    None,
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

    // This is in fact used, idk why it's telling me it's not
    #[allow(unused)]
    let mut conditions = [
        false, // Starts with two objects
        false, // Has open paren
        false, // Has close paren
    ];

    let mut i: usize = 0;
    while i < tokens.len() {
        conditions = [false, false, false];
        if let Token::Object(obj) = tokens[i] {
            if RESTRICTED_KWARGS.contains(&obj) {
                skip_to(tokens, Token::CloseParen, &mut i);
                continue;
            } else if obj == "include" {
                skip_to_oneof(tokens, &[Token::GreaterThan, Token::Literal("\"")], &mut i);
                continue;
            } else if obj == "define" {
                skip_to(tokens, Token::NewLine, &mut i);
                continue;
            }

            let mut j = i + 1;
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
                        fn_defs.push(&tokens[i..j]);
                    }
                    break;
                } else if let Token::Semicolon = tokens[j] {
                    break;
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }
        i += 1;
    }

    fn_defs
}

pub fn get_includes<'a>(tokens: &'a Vec<Token>) -> Vec<&'a [Token<'a>]> {
    let mut includes = vec![];

    let mut idx: usize = 0;
    while idx < tokens.len() {
        if let Token::HashTag = tokens[idx] {
            if tokens[idx+1] != Token::Object("include") {
                idx += 1;
                continue;
            }
            let mut end = idx;
            skip_to_oneof(tokens, &[Token::GreaterThan, Token::Literal("\"")], &mut end);
            includes.push(&tokens[idx..(end + 1)]);
            idx = end;
            continue;
        }
        idx += 1;
    }

    includes
}

pub fn get_structs<'a>(tokens: &'a Vec<Token>) -> Vec<&'a [Token<'a>]> {
    let mut structs = vec![];
    if tokens.len() < 3 {
        return structs;
    }

    let mut idx: usize = 0;
    while idx < tokens.len() - 2 {
        if let Token::Object(obj) = tokens[idx] {
            if !["typedef", "struct"].contains(&obj) {
                idx += 1;
                continue;
            } else if "typedef" == obj {
                let obj_2 = if let Token::Object(obj_2) = tokens[idx + 1] {
                    obj_2
                } else {
                    "-"
                };
                if obj_2 != "struct" {
                    idx += 1;
                    continue;
                }
            }
            let length = match struct_len(&tokens[idx..]) {
                Some(l) => l,
                None => {
                    idx += 1;
                    continue;
                }
            };

            let end = idx + length + 1;
            if tokens[end - 1] != Token::Semicolon {
                idx += 1;
                continue;
            }
            if tokens[end - 2] != Token::CloseCurlyBrace
                && std::mem::discriminant(&tokens[end - 2])
                    != std::mem::discriminant(&Token::Object("_"))
            {
                idx += 1;
                continue;
            }

            structs.push(&tokens[idx..end]);
            idx = end - 1;
        } else {
            idx += 1;
        }
    }

    structs
}

pub fn get_defines<'a>(tokens: &'a Vec<Token>) -> Vec<&'a [Token<'a>]> {
    let mut defines = vec![];

    let mut idx: usize = 0;

    while idx < tokens.len() {
        if tokens[idx] != Token::HashTag {
            skip_to(tokens, Token::HashTag, &mut idx);
        }
        
        let start_idx = idx;

        if idx + 1 >= tokens.len() || tokens[idx+1] != Token::Object("define") {
            idx += 2;
            continue;
        }
        idx += 1;

        skip_to(tokens, Token::NewLine, &mut idx);
        while idx < tokens.len() && tokens[idx-1] == Token::BackSlash {
            skip_to(tokens, Token::NewLine, &mut idx);
        }

        defines.push(&tokens[start_idx..idx]);
    }

    defines
}

/// Gets the name of the struct
/// Ex) for `struct Point {...}`, this would return "Point"
pub(super) fn get_struct_name<'a>(tokens: &'a [Token]) -> &'a str {
    if tokens.len() < 3 {
        unreachable!("Token string is not a valid struct definition");
    }

    match &tokens[0] {
        // Handle typedef struct definitions
        Token::Object("typedef") => {
            // Ensure we are dealing with a typedef for a struct.
            // Typical patterns:
            //   typedef struct { ... } Alias;
            //   typedef struct Tag { ... } Alias;
            //
            // In both cases, the actual name (Alias) is the last Object token before the semicolon.
            let semicolon_index = tokens
                .iter()
                .rposition(|t| *t == Token::Semicolon)
                .expect("Missing semicolon in typedef struct definition");

            // Iterate backwards from the token just before the semicolon to find the typedef alias.
            for token in tokens[..semicolon_index].iter().rev() {
                if let Token::Object(name) = token {
                    return name;
                }
            }
            unreachable!("No valid struct name found in typedef struct definition");
        }
        // Handle regular struct definitions
        Token::Object("struct") => {
            // Expect the struct name to immediately follow the "struct" keyword.
            if let Token::Object(name) = tokens[1] {
                return name;
            } else {
                unreachable!("Expected struct name after 'struct' keyword");
            }
        }
        _ => unreachable!("Token string is not a valid struct definition"),
    }
}



/// Gets the name of the define statement
/// Ex) for `#define FOO 42`, this would return "FOO"
pub(super) fn get_define_name<'a>(tokens: &'a[Token]) -> &'a str  {
    if tokens.len() < 3 || tokens[0] != Token::HashTag || tokens[1] != Token::Object("define") {
        unreachable!("Token string is not a valid define macro");
    }

    if let Token::Object(s) = tokens[2] {
        return s;
    }
    unreachable!("Token string is not a valid define macro");
}

fn struct_len(tokens: &[Token]) -> Option<usize> {
    let mut num_brackets = 0;
    let mut contains_brackets = false;

    for (i, t) in tokens.iter().enumerate() {
        match t {
            Token::OpenCurlyBrace => {
                num_brackets += 1;
                contains_brackets = true;
            }
            Token::CloseCurlyBrace => {
                contains_brackets = true;
                num_brackets -= 1;
                if num_brackets < 0 {
                    return None;
                }
            }
            Token::Semicolon => {
                if num_brackets == 0 {
                    if contains_brackets {
                        return Some(i);
                    }
                    return None;
                }
            }
            _ => {}
        }
    }

    None
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

    // If the for loop ends, we haven't found it, so set idx appropriatly
    *idx = tokens.len();
}

/// Ignores values inside the targets, it just skips to the next token
/// that's one of the target varients
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


#[cfg(test)]
mod lexer_tests {
    use std::fs;

    use super::*;

    #[test]
    fn test_get_defines() {
        let s = fs::read_to_string("tests/lexer-define.c").unwrap();
        let s = clean_source_code(s);
        let tokens = tokenize(&s).unwrap();

        let defines = get_defines(&tokens);

        let mut log_dump = "".to_string();
        for &def in &defines {
            let x = format!("{:?}\n\n", def);
            log_dump.push_str(&x);
        }

        fs::write("tests/lexer.test_get_defines.log", format!("{}", log_dump)).unwrap();

        assert_eq!(defines.len(), s.split("#define").collect::<Vec<&str>>().len() - 1);
    }

    #[test]
    fn test_get_structs() {
        let s = fs::read_to_string("tests/lexer-struct.c").unwrap();
        let s = clean_source_code(s);
        let tokens = tokenize(&s).unwrap();

        let defines = get_structs(&tokens);

        let mut log_dump = "".to_string();
        for &def in &defines {
            let x = format!("{:?}\n\n", def);
            log_dump.push_str(&x);
        }

        fs::write("tests/lexer.test_get_structs.log", format!("{}", log_dump)).unwrap();
    }

    #[test]
    fn test_get_define_name() {
        let s = fs::read_to_string("tests/lexer-define.c").unwrap();
        let s = clean_source_code(s);
        let tokens = tokenize(&s).unwrap();

        let defines = get_defines(&tokens);

        let mut names = vec![];
        for &d in &defines {
            names.push(get_define_name(d));
        }
        
        assert_eq!(defines.len(), s.split("#define").collect::<Vec<&str>>().len() - 1);

        fs::write("tests/lexer.test_get_define_name.log", format!("{:#?}", names))
            .unwrap();
    }


    #[test]
    fn test_get_struct_name() {
        let s = fs::read_to_string("tests/lexer-struct.c").unwrap();
        let s = clean_source_code(s);
        let tokens = tokenize(&s).unwrap();

        let structs = get_structs(&tokens);

        let mut names = vec![];
        for &d in &structs {
            names.push(get_struct_name(d));
        }
        
        fs::write("tests/lexer.test_get_struct_name.log", format!("{:#?}", names))
            .unwrap();
    }

    #[test]
    fn test_struct_tokens_to_string() {
        let s = fs::read_to_string("tests/lexer-struct.c").unwrap();
        let s = clean_source_code(s);
        let tokens = tokenize(&s).unwrap();

        let structs = get_structs(&tokens);

        let mut log_dump = "".to_string();
        for &d in &structs {
            let s = Token::struct_tokens_to_string(d);
            let s_exact = format!("{:?}", &s);
            log_dump.push_str(&s);
            log_dump.push_str("\n");
            log_dump.push_str("___________\n");
            log_dump.push_str(&s_exact);
            log_dump.push_str("\n\n\n");
        }
    

        fs::write("tests/lexer.test_struct_tokens_to_string.log", &log_dump)
            .unwrap();
    }

}