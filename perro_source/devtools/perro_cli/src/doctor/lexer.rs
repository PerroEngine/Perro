use super::*;

pub(super) struct MacroCall {
    pub(super) inner: String,
    pub(super) line: usize,
}

pub(super) fn find_macro_calls(text: &str, macro_name: &str) -> Vec<String> {
    find_macro_calls_with_lines(text, macro_name)
        .into_iter()
        .map(|call| call.inner)
        .collect()
}

pub(super) fn find_macro_calls_with_lines(text: &str, macro_name: &str) -> Vec<MacroCall> {
    let mut calls = Vec::new();
    let needle = format!("{macro_name}!");
    let mut search_from = 0usize;
    while search_from < text.len() {
        let Some(rel) = text[search_from..].find(&needle) else {
            break;
        };
        let start = search_from + rel;
        let after = start + needle.len();
        let rest = text[after..].trim_start();
        let open = after + (text[after..].len() - rest.len());
        if !text[open..].starts_with('(') {
            search_from = after;
            continue;
        }
        let Some(close) = find_matching_delim_for_doctor(text, open, '(', ')') else {
            break;
        };
        calls.push(MacroCall {
            inner: text[open + 1..close].to_string(),
            line: line_number_at(text, start),
        });
        search_from = close + 1;
    }
    calls
}

pub(super) fn split_top_level_args(input: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0usize;
    let mut paren = 0_i32;
    let mut bracket = 0_i32;
    let mut brace = 0_i32;
    let mut mode = DoctorLexMode::Code;
    let mut escaped = false;
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match mode {
            DoctorLexMode::Code => {
                if b == b'"' {
                    mode = DoctorLexMode::String;
                } else if let Some((prefix_len, hashes)) = raw_string_start_at_for_doctor(bytes, i)
                {
                    mode = DoctorLexMode::RawString(hashes);
                    i += prefix_len;
                    continue;
                } else {
                    match b {
                        b'(' => paren += 1,
                        b')' => paren -= 1,
                        b'[' => bracket += 1,
                        b']' => bracket -= 1,
                        b'{' => brace += 1,
                        b'}' => brace -= 1,
                        b',' if paren == 0 && bracket == 0 && brace == 0 => {
                            args.push(input[start..i].trim());
                            start = i + 1;
                        }
                        _ => {}
                    }
                }
            }
            DoctorLexMode::String => {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    mode = DoctorLexMode::Code;
                }
            }
            DoctorLexMode::RawString(hashes) => {
                if raw_string_end_at_for_doctor(bytes, i, hashes) {
                    i += hashes + 1;
                    mode = DoctorLexMode::Code;
                    continue;
                }
            }
            DoctorLexMode::LineComment | DoctorLexMode::BlockComment => {}
        }
        i += 1;
    }
    args.push(input[start..].trim());
    args
}

pub(super) fn normalize_arg_text(input: &str) -> String {
    input.chars().filter(|ch| !ch.is_whitespace()).collect()
}

pub(super) fn extract_brace_block_for_doctor(input: &str) -> Option<&str> {
    if !input.starts_with('{') {
        return None;
    }
    let end = find_matching_delim_for_doctor(input, 0, '{', '}')?;
    Some(&input[1..end])
}

pub(super) fn parse_fn_name_for_doctor(input: &str) -> Option<String> {
    let input = input.trim().trim_start_matches("pub ").trim_start();
    let rest = input.strip_prefix("fn ")?;
    let name = rest
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .next()?;
    if is_ident_for_doctor(name) {
        Some(name.to_string())
    } else {
        None
    }
}

/// lex state carried btw lines: comments + strs span lines
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CodeLexStateForDoctor {
    Code,
    BlockComment(usize),
    Str { escaped: bool },
    RawStr(usize),
}

/// blank comments + delims inside str/char literals, per line
/// kp code layout + str text -> brace/paren counts see code only
/// `"res://x"` no longer read as line comment start
pub(super) fn lex_code_lines_for_doctor(text: &str) -> Vec<String> {
    let mut state = CodeLexStateForDoctor::Code;
    text.lines()
        .map(|line| lex_code_line_for_doctor(line, &mut state))
        .collect()
}

pub(super) fn lex_code_line_for_doctor(line: &str, state: &mut CodeLexStateForDoctor) -> String {
    let bytes = line.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        match *state {
            CodeLexStateForDoctor::Code => {
                if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
                    break;
                }
                if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    *state = CodeLexStateForDoctor::BlockComment(1);
                    push_blanks_for_doctor(&mut out, 2);
                    i += 2;
                    continue;
                }
                if let Some((prefix_len, hashes)) = raw_string_start_at_for_doctor(bytes, i) {
                    *state = CodeLexStateForDoctor::RawStr(hashes);
                    push_blanks_for_doctor(&mut out, prefix_len);
                    i += prefix_len;
                    continue;
                }
                if b == b'"' {
                    *state = CodeLexStateForDoctor::Str { escaped: false };
                    out.push(b'"');
                    i += 1;
                    continue;
                }
                if b == b'\''
                    && let Some(len) = char_literal_len_for_doctor(bytes, i)
                {
                    push_blanks_for_doctor(&mut out, len);
                    i += len;
                    continue;
                }
                out.push(b);
                i += 1;
            }
            CodeLexStateForDoctor::BlockComment(depth) => {
                if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    *state = CodeLexStateForDoctor::BlockComment(depth + 1);
                    push_blanks_for_doctor(&mut out, 2);
                    i += 2;
                    continue;
                }
                if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    *state = if depth <= 1 {
                        CodeLexStateForDoctor::Code
                    } else {
                        CodeLexStateForDoctor::BlockComment(depth - 1)
                    };
                    push_blanks_for_doctor(&mut out, 2);
                    i += 2;
                    continue;
                }
                push_blanks_for_doctor(&mut out, 1);
                i += 1;
            }
            CodeLexStateForDoctor::Str { escaped } => {
                if escaped {
                    *state = CodeLexStateForDoctor::Str { escaped: false };
                } else if b == b'\\' {
                    *state = CodeLexStateForDoctor::Str { escaped: true };
                } else if b == b'"' {
                    *state = CodeLexStateForDoctor::Code;
                    out.push(b'"');
                    i += 1;
                    continue;
                }
                push_literal_byte_for_doctor(&mut out, b);
                i += 1;
            }
            CodeLexStateForDoctor::RawStr(hashes) => {
                if b == b'"' && raw_string_end_at_for_doctor(bytes, i, hashes) {
                    *state = CodeLexStateForDoctor::Code;
                    push_blanks_for_doctor(&mut out, 1 + hashes);
                    i += 1 + hashes;
                    continue;
                }
                push_literal_byte_for_doctor(&mut out, b);
                i += 1;
            }
        }
    }

    String::from_utf8(out).unwrap_or_else(|_| line.to_string())
}

fn push_blanks_for_doctor(out: &mut Vec<u8>, count: usize) {
    out.extend(std::iter::repeat_n(b' ', count));
}

/// kp str text 4 attr readers (`#[node_ref(...)]`), blank delims only
fn push_literal_byte_for_doctor(out: &mut Vec<u8>, b: u8) {
    if matches!(b, b'{' | b'}' | b'(' | b')' | b'[' | b']') {
        out.push(b' ');
    } else {
        out.push(b);
    }
}

/// ret byte len of char literal @ i, else None (= lifetime tick)
/// stop `'{'` / `'('` from skewing depth counts
pub(super) fn char_literal_len_for_doctor(bytes: &[u8], i: usize) -> Option<usize> {
    if bytes.get(i) != Some(&b'\'') {
        return None;
    }

    let mut j = i + 1;
    if bytes.get(j) == Some(&b'\\') {
        j += 1;
        match bytes.get(j)? {
            b'u' => {
                j += 1;
                if bytes.get(j) != Some(&b'{') {
                    return None;
                }
                while bytes.get(j) != Some(&b'}') {
                    j += 1;
                    if j >= bytes.len() {
                        return None;
                    }
                }
                j += 1;
            }
            b'x' => j += 3,
            _ => j += 1,
        }
    } else {
        j += utf8_char_width_for_doctor(*bytes.get(j)?);
    }

    (bytes.get(j) == Some(&b'\'')).then_some(j + 1 - i)
}

fn utf8_char_width_for_doctor(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

pub(super) fn brace_delta_for_doctor(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| *ch == '{').count() as i32;
    let closes = line.chars().filter(|ch| *ch == '}').count() as i32;
    opens - closes
}

pub(super) fn paren_delta_for_doctor(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| *ch == '(').count() as i32;
    let closes = line.chars().filter(|ch| *ch == ')').count() as i32;
    opens - closes
}

pub(super) fn is_ident_for_doctor(input: &str) -> bool {
    let mut chars = input.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[derive(Clone, Copy)]
pub(super) enum DoctorLexMode {
    Code,
    LineComment,
    BlockComment,
    String,
    RawString(usize),
}

pub(super) fn find_matching_delim_for_doctor(
    source: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let bytes = source.as_bytes();
    if open_index >= bytes.len() || bytes[open_index] != open as u8 {
        return None;
    }
    let mut mode = DoctorLexMode::Code;
    let mut depth = 0_i32;
    let mut i = open_index;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        match mode {
            DoctorLexMode::Code => {
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    mode = DoctorLexMode::LineComment;
                    i += 2;
                    continue;
                }
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    mode = DoctorLexMode::BlockComment;
                    i += 2;
                    continue;
                }
                if let Some((prefix_len, hashes)) = raw_string_start_at_for_doctor(bytes, i) {
                    mode = DoctorLexMode::RawString(hashes);
                    i += prefix_len;
                    continue;
                }
                if b == b'\''
                    && let Some(len) = char_literal_len_for_doctor(bytes, i)
                {
                    i += len;
                    continue;
                }
                if b == b'"' {
                    mode = DoctorLexMode::String;
                } else if b == open as u8 {
                    depth += 1;
                } else if b == close as u8 {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
            }
            DoctorLexMode::LineComment => {
                if b == b'\n' {
                    mode = DoctorLexMode::Code;
                }
            }
            DoctorLexMode::BlockComment => {
                if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    mode = DoctorLexMode::Code;
                    i += 2;
                    continue;
                }
            }
            DoctorLexMode::String => {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    mode = DoctorLexMode::Code;
                }
            }
            DoctorLexMode::RawString(hashes) => {
                if raw_string_end_at_for_doctor(bytes, i, hashes) {
                    i += hashes + 1;
                    mode = DoctorLexMode::Code;
                    continue;
                }
            }
        }
        i += 1;
    }
    None
}

pub(super) fn raw_string_start_at_for_doctor(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    if bytes.get(i) != Some(&b'r') {
        return None;
    }
    let mut j = i + 1;
    let mut hashes = 0usize;
    while bytes.get(j) == Some(&b'#') {
        hashes += 1;
        j += 1;
    }
    if bytes.get(j) == Some(&b'"') {
        Some((j - i + 1, hashes))
    } else {
        None
    }
}

pub(super) fn raw_string_end_at_for_doctor(bytes: &[u8], i: usize, hashes: usize) -> bool {
    if bytes.get(i) != Some(&b'"') {
        return false;
    }
    (0..hashes).all(|offset| bytes.get(i + 1 + offset) == Some(&b'#'))
}
