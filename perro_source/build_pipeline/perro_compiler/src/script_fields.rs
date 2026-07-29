fn parse_struct_fields(source: &str, struct_name: &str) -> Vec<ScriptField> {
    let lines = lex_code_lines(source);
    let mut struct_line = None;
    for (i, line) in lines.iter().enumerate() {
        if parse_struct_name(line.trim()) == Some(struct_name.to_string()) {
            struct_line = Some(i);
            break;
        }
    }
    let Some(start) = struct_line else {
        return Vec::new();
    };

    let mut fields = Vec::new();
    let mut depth = 0_i32;
    let mut opened = false;
    let mut i = start;

    while i < lines.len() {
        let line = lines[i].as_str();
        if !opened {
            if let Some(pos) = line.find('{') {
                opened = true;
                depth = 1;
                let rest = &line[pos + 1..];
                if depth == 1
                    && let Some(field) = parse_field_line(rest)
                {
                    fields.push(field);
                }
                depth += brace_delta(rest);
                if depth <= 0 {
                    break;
                }
            }
            i += 1;
            continue;
        }

        if depth == 1
            && let Some(field) = parse_field_line(line)
        {
            fields.push(field);
        }
        depth += brace_delta(line);
        if depth <= 0 {
            break;
        }
        i += 1;
    }

    fields
}

/// lex state carried btw lines: comments + strs span lines
#[derive(Clone, Copy, PartialEq, Eq)]
enum CodeLexState {
    Code,
    BlockComment(usize),
    Str { escaped: bool },
    RawStr(usize),
}

/// blank comments + delims inside str/char literals, per line
/// kp code layout + str text -> brace/paren counts see code only
/// `"res://x"` no longer read as line comment start
fn lex_code_lines(source: &str) -> Vec<String> {
    let mut state = CodeLexState::Code;
    source
        .lines()
        .map(|line| lex_code_line(line, &mut state))
        .collect()
}

fn lex_code_line(line: &str, state: &mut CodeLexState) -> String {
    let bytes = line.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        match *state {
            CodeLexState::Code => {
                if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
                    break;
                }
                if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    *state = CodeLexState::BlockComment(1);
                    push_blanks(&mut out, 2);
                    i += 2;
                    continue;
                }
                if let Some((prefix_len, hashes)) = raw_string_start_at(bytes, i) {
                    *state = CodeLexState::RawStr(hashes);
                    push_blanks(&mut out, prefix_len);
                    i += prefix_len;
                    continue;
                }
                if b == b'"' {
                    *state = CodeLexState::Str { escaped: false };
                    out.push(b'"');
                    i += 1;
                    continue;
                }
                if b == b'\''
                    && let Some(len) = char_literal_len(bytes, i)
                {
                    push_blanks(&mut out, len);
                    i += len;
                    continue;
                }
                out.push(b);
                i += 1;
            }
            CodeLexState::BlockComment(depth) => {
                if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    *state = CodeLexState::BlockComment(depth + 1);
                    push_blanks(&mut out, 2);
                    i += 2;
                    continue;
                }
                if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    *state = if depth <= 1 {
                        CodeLexState::Code
                    } else {
                        CodeLexState::BlockComment(depth - 1)
                    };
                    push_blanks(&mut out, 2);
                    i += 2;
                    continue;
                }
                push_blanks(&mut out, 1);
                i += 1;
            }
            CodeLexState::Str { escaped } => {
                if escaped {
                    *state = CodeLexState::Str { escaped: false };
                } else if b == b'\\' {
                    *state = CodeLexState::Str { escaped: true };
                } else if b == b'"' {
                    *state = CodeLexState::Code;
                    out.push(b'"');
                    i += 1;
                    continue;
                }
                push_literal_byte(&mut out, b);
                i += 1;
            }
            CodeLexState::RawStr(hashes) => {
                if b == b'"' && raw_string_end_at(bytes, i, hashes) {
                    *state = CodeLexState::Code;
                    push_blanks(&mut out, 1 + hashes);
                    i += 1 + hashes;
                    continue;
                }
                push_literal_byte(&mut out, b);
                i += 1;
            }
        }
    }

    String::from_utf8(out).unwrap_or_else(|_| line.to_string())
}

fn push_blanks(out: &mut Vec<u8>, count: usize) {
    out.extend(std::iter::repeat_n(b' ', count));
}

/// kp str text 4 attr readers, blank delims only
fn push_literal_byte(out: &mut Vec<u8>, b: u8) {
    if matches!(b, b'{' | b'}' | b'(' | b')' | b'[' | b']') {
        out.push(b' ');
    } else {
        out.push(b);
    }
}

/// ret byte len of char literal @ i, else None (= lifetime tick)
/// stop `'{'` / `'('` from skewing depth counts
fn char_literal_len(bytes: &[u8], i: usize) -> Option<usize> {
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
        j += utf8_char_width(*bytes.get(j)?);
    }

    (bytes.get(j) == Some(&b'\'')).then_some(j + 1 - i)
}

fn utf8_char_width(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

fn raw_string_end_at(bytes: &[u8], i: usize, hashes: usize) -> bool {
    if bytes.get(i) != Some(&b'"') {
        return false;
    }
    (0..hashes).all(|offset| bytes.get(i + 1 + offset) == Some(&b'#'))
}

fn brace_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|c| *c == '{').count() as i32;
    let closes = line.chars().filter(|c| *c == '}').count() as i32;
    opens - closes
}

fn parse_field_line(line: &str) -> Option<ScriptField> {
    let trimmed = line.trim().trim_end_matches(',').trim();
    if trimmed.is_empty()
        || trimmed.starts_with("#[")
        || trimmed.starts_with("///")
        || trimmed.starts_with("//")
    {
        return None;
    }

    let without_vis = if let Some(rest) = trimmed.strip_prefix("pub(") {
        let after = rest.split_once(')')?.1;
        after.trim()
    } else {
        trimmed.trim_start_matches("pub ").trim_start()
    };

    let (name, ty) = without_vis.split_once(':')?;
    let name = name.trim();
    let ty = ty.trim();
    if name.is_empty() || ty.is_empty() || !is_ident(name) {
        return None;
    }

    Some(ScriptField {
        name: name.to_string(),
        ty: ty.to_string(),
    })
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn normalize_type(ty: &str) -> String {
    ty.chars().filter(|c| !c.is_whitespace()).collect()
}

fn supported_fields(fields: &[ScriptField]) -> Vec<ScriptField> {
    fields.to_vec()
}

fn member_const_name(field_name: &str) -> String {
    let mut out = String::from("__PERRO_VAR_");
    for c in field_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

fn method_const_name(method_name: &str) -> String {
    let mut out = String::from("__PERRO_METHOD_");
    for c in method_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

#[derive(Clone, Debug)]
struct ScriptMethod {
    name: String,
    takes_raw_params: bool,
    params: Vec<ScriptMethodParam>,
    return_ty: Option<String>,
    returns_variant: bool,
}

#[derive(Clone, Debug)]
struct ScriptMethodParam {
    name: String,
    ty: String,
}

fn generate_member_consts(
    fields: &[ScriptField],
    nested_fields: &[NestedScriptField],
    methods: &[ScriptMethod],
) -> String {
    if fields.is_empty() && nested_fields.is_empty() && methods.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for field in fields {
        let const_name = member_const_name(&field.name);
        out.push_str(&format!(
            "const {const_name}: ScriptMemberID = var!(\"{}\");\n",
            field.name
        ));
    }
    for field in nested_fields {
        let const_name = nested_member_const_name(&field.member);
        out.push_str(&format!(
            "const {const_name}: ScriptMemberID = var!(\"{}\");\n",
            field.member
        ));
    }
    for method in methods {
        let const_name = method_const_name(&method.name);
        out.push_str(&format!(
            "const {const_name}: ScriptMemberID = func!(\"{}\");\n",
            method.name
        ));
    }
    out
}

fn nested_member_const_name(member: &str) -> String {
    format!(
        "{}_{}",
        member_const_name(member),
        perro_ids::string_to_u64(member)
    )
}

fn generate_call_method_body(methods: &[ScriptMethod]) -> String {
    if methods.is_empty() {
        return "        let _ = (method, ctx, params);\n        Variant::Null".to_string();
    }

    let mut out = String::new();
    out.push_str("        match method {\n");
    for method in methods {
        let const_name = method_const_name(&method.name);
        let call = if method.takes_raw_params {
            format!("self.{}(ctx, params)", method.name)
        } else if method.params.is_empty() {
            format!("self.{}(ctx)", method.name)
        } else {
            let args = method
                .params
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("self.{}(ctx, {args})", method.name)
        };

        let mut prelude = String::new();
        let mut supported = true;
        if !method.takes_raw_params && !method.params.is_empty() {
            for (i, param) in method.params.iter().enumerate() {
                if let Some(binding) = generate_call_param_binding(i, param) {
                    prelude.push_str("                ");
                    prelude.push_str(&binding);
                    prelude.push('\n');
                } else {
                    supported = false;
                    break;
                }
            }
        }

        if !supported {
            out.push_str(&format!(
                "            {const_name} => {{\n                let _ = (ctx, params);\n                Variant::Null\n            }}\n"
            ));
            continue;
        }

        if method.returns_variant {
            out.push_str(&format!(
                "            {const_name} => {{\n{prelude}                {call}\n            }}\n"
            ));
        } else if method_returns_variant_convertible(method.return_ty.as_deref()) {
            out.push_str(&format!(
                "            {const_name} => {{\n{prelude}                Variant::from({call})\n            }}\n"
            ));
        } else {
            out.push_str(&format!(
                "            {const_name} => {{\n{prelude}                {call};\n                Variant::Null\n            }}\n"
            ));
        }
    }
    out.push_str("            _ => Variant::Null,\n");
    out.push_str("        }");
    out
}

fn method_returns_variant_convertible(return_ty: Option<&str>) -> bool {
    let Some(return_ty) = return_ty else {
        return false;
    };
    let return_ty = normalize_type(return_ty);
    return_ty != "()"
}
