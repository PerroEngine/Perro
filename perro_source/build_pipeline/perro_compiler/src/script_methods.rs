fn parse_inherent_methods(source: &str, struct_name: &str) -> Vec<ScriptMethod> {
    let lines: Vec<&str> = source.lines().collect();
    let mut methods = Vec::new();
    let mut i = 0usize;

    while i < lines.len() {
        let line = strip_line_comment(lines[i]).trim();
        if !line.starts_with("impl") {
            i += 1;
            continue;
        }

        if line.contains(" for ") || !line.contains(struct_name) {
            i += 1;
            continue;
        }

        let mut depth = brace_delta(line);
        let mut opened = line.contains('{');
        i += 1;

        while i < lines.len() {
            let raw_line = lines[i];
            let l = strip_line_comment(raw_line);
            if opened
                && depth == 1
                && let Some(method) = parse_script_method_signature(l.trim())
            {
                methods.push(method);
            }

            if !opened && l.contains('{') {
                opened = true;
            }
            depth += brace_delta(l);
            if opened && depth <= 0 {
                break;
            }
            i += 1;
        }
        i += 1;
    }

    methods.extend(parse_methods_macro_methods(source, struct_name));
    methods.sort_by(|a, b| a.name.cmp(&b.name));
    methods.dedup_by(|a, b| a.name == b.name);
    methods
}

fn parse_attributed_struct_name(source: &str, attribute_name: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    for i in 0..lines.len() {
        let l = lines[i].trim();
        if !is_attribute_line_named(l, attribute_name) {
            continue;
        }
        for next in lines.iter().skip(i + 1) {
            let n = next.trim();
            if n.is_empty() {
                continue;
            }
            if n.starts_with("#[") {
                continue;
            }
            if let Some(name) = parse_struct_name(n) {
                return Some(name);
            }
            break;
        }
    }
    None
}

fn is_attribute_line_named(line: &str, attribute_name: &str) -> bool {
    let Some(inner) = line.strip_prefix("#[").and_then(|v| v.strip_suffix(']')) else {
        return false;
    };
    let inner = inner.trim();
    if inner.eq_ignore_ascii_case(attribute_name) {
        return true;
    }
    if let Some(open) = inner.find('(') {
        let name = inner[..open].trim();
        return name.eq_ignore_ascii_case(attribute_name);
    }
    false
}

fn parse_methods_macro_methods(source: &str, struct_name: &str) -> Vec<ScriptMethod> {
    let mut methods = Vec::new();
    let needle = "methods!(";
    let mut search_from = 0usize;

    while search_from < source.len() {
        let Some(rel) = source[search_from..].find(needle) else {
            break;
        };
        let start = search_from + rel;
        let open_paren = start + "methods!".len();
        let Some(close_paren) = find_matching_delim(source, open_paren, '(', ')') else {
            break;
        };

        let inner = &source[open_paren + 1..close_paren];
        if let Some((target_name, body)) = parse_methods_macro_inner(inner)
            && target_name == struct_name
        {
            methods.extend(parse_methods_block_signatures(body));
        }

        search_from = close_paren + 1;
    }

    methods
}

fn has_script_macro_invocation(source: &str, macro_name: &str) -> bool {
    parse_script_macro_target(source, macro_name).is_some()
}

fn parse_script_macro_target(source: &str, macro_name: &str) -> Option<String> {
    let needle = format!("{macro_name}!(");
    let mut search_from = 0usize;

    while search_from < source.len() {
        let Some(rel) = source[search_from..].find(&needle) else {
            break;
        };
        let start = search_from + rel;
        let open_paren = start + needle.len() - 1;
        let Some(close_paren) = find_matching_delim(source, open_paren, '(', ')') else {
            break;
        };

        let inner = &source[open_paren + 1..close_paren];
        if let Some((target_name, _)) = parse_script_macro_inner(inner) {
            return Some(target_name);
        }

        search_from = close_paren + 1;
    }

    None
}

fn find_matching_delim(source: &str, open_index: usize, open: char, close: char) -> Option<usize> {
    find_matching_delim_lexed(source, open_index, open, close)
}

fn parse_methods_macro_inner(inner: &str) -> Option<(String, &str)> {
    parse_script_macro_inner(inner)
}

fn parse_script_macro_inner(inner: &str) -> Option<(String, &str)> {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('{') {
        let body = extract_brace_block(trimmed)?;
        return Some(("Script".to_string(), body));
    }

    let mut target = String::new();
    for c in trimmed.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            target.push(c);
        } else {
            break;
        }
    }
    if target.is_empty() {
        return None;
    }

    let rest = trimmed[target.len()..].trim_start();
    if !rest.starts_with('{') {
        return None;
    }
    let body = extract_brace_block(rest)?;
    Some((target, body))
}

fn extract_brace_block(s: &str) -> Option<&str> {
    if !s.starts_with('{') {
        return None;
    }
    let end = find_matching_delim_lexed(s, 0, '{', '}')?;
    Some(&s[1..end])
}

fn find_matching_delim_lexed(
    source: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Mode {
        Code,
        LineComment,
        BlockComment,
        String,
        RawString(usize),
    }

    let bytes = source.as_bytes();
    if open_index >= bytes.len() || bytes[open_index] != open as u8 {
        return None;
    }

    let mut mode = Mode::Code;
    let mut block_comment_depth: usize = 0;
    let mut depth = 0_i32;
    let mut i = open_index;
    let mut escaped = false;

    while i < bytes.len() {
        let b = bytes[i];
        match mode {
            Mode::Code => {
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    mode = Mode::LineComment;
                    i += 2;
                    continue;
                }
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    mode = Mode::BlockComment;
                    block_comment_depth = 1;
                    i += 2;
                    continue;
                }
                if let Some((prefix_len, hashes)) = raw_string_start_at(bytes, i) {
                    mode = Mode::RawString(hashes);
                    i += prefix_len;
                    continue;
                }
                if b == b'"' {
                    mode = Mode::String;
                    escaped = false;
                    i += 1;
                    continue;
                }
                if b == open as u8 {
                    depth += 1;
                } else if b == close as u8 {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                i += 1;
            }
            Mode::LineComment => {
                if b == b'\n' {
                    mode = Mode::Code;
                }
                i += 1;
            }
            Mode::BlockComment => {
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    block_comment_depth += 1;
                    i += 2;
                    continue;
                }
                if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    block_comment_depth = block_comment_depth.saturating_sub(1);
                    i += 2;
                    if block_comment_depth == 0 {
                        mode = Mode::Code;
                    }
                    continue;
                }
                i += 1;
            }
            Mode::String => {
                if escaped {
                    escaped = false;
                    i += 1;
                    continue;
                }
                if b == b'\\' {
                    escaped = true;
                    i += 1;
                    continue;
                }
                if b == b'"' {
                    mode = Mode::Code;
                }
                i += 1;
            }
            Mode::RawString(hashes) => {
                if b == b'"' {
                    let mut ok = true;
                    for j in 0..hashes {
                        if i + 1 + j >= bytes.len() || bytes[i + 1 + j] != b'#' {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        mode = Mode::Code;
                        i += 1 + hashes;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }
    None
}

fn raw_string_start_at(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    if i >= bytes.len() {
        return None;
    }

    let (start, prefix_len) = if bytes[i] == b'r' {
        (i, 1usize)
    } else if i + 1 < bytes.len()
        && ((bytes[i] == b'b' && bytes[i + 1] == b'r')
            || (bytes[i] == b'r' && bytes[i + 1] == b'b'))
    {
        (i + 1, 2usize)
    } else {
        return None;
    };

    let mut j = start + 1;
    let mut hashes = 0usize;
    while j < bytes.len() && bytes[j] == b'#' {
        hashes += 1;
        j += 1;
    }
    if j < bytes.len() && bytes[j] == b'"' {
        return Some((prefix_len + hashes + 1, hashes));
    }
    None
}

fn parse_methods_block_signatures(body: &str) -> Vec<ScriptMethod> {
    let mut methods = Vec::new();
    let mut depth = 0_i32;
    let mut sig_buf: Option<String> = None;
    let mut sig_paren_depth: i32 = 0;
    let debug_methods = methods_debug_enabled();

    for line in body.lines() {
        let l = strip_line_comment(line);
        let trimmed = l.trim();

        if depth == 0 {
            if let Some(buf) = sig_buf.as_mut() {
                if !trimmed.is_empty() {
                    buf.push(' ');
                    buf.push_str(trimmed);
                }
                sig_paren_depth += paren_delta(trimmed);
                if sig_paren_depth <= 0 {
                    match parse_script_method_signature_detailed(buf.trim()) {
                        Ok(method) => methods.push(method),
                        Err(reason) => {
                            if debug_methods {
                                eprintln!(
                                    "[perro][methods][skip] {} | signature=`{}`",
                                    reason,
                                    buf.trim()
                                );
                            }
                        }
                    }
                    sig_buf = None;
                    sig_paren_depth = 0;
                }
            } else if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                sig_buf = Some(trimmed.to_string());
                sig_paren_depth = paren_delta(trimmed);
                if sig_paren_depth <= 0 {
                    match parse_script_method_signature_detailed(trimmed) {
                        Ok(method) => methods.push(method),
                        Err(reason) => {
                            if debug_methods {
                                eprintln!(
                                    "[perro][methods][skip] {} | signature=`{}`",
                                    reason, trimmed
                                );
                            }
                        }
                    }
                    sig_buf = None;
                    sig_paren_depth = 0;
                }
            } else if let Ok(method) = parse_script_method_signature_detailed(trimmed) {
                methods.push(method);
            }
        }

        depth += brace_delta(l);
    }

    methods
}

fn paren_delta(s: &str) -> i32 {
    let mut depth = 0_i32;
    for c in s.chars() {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
        }
    }
    depth
}

fn parse_script_method_signature(line: &str) -> Option<ScriptMethod> {
    parse_script_method_signature_detailed(line).ok()
}

fn parse_script_method_signature_detailed(line: &str) -> Result<ScriptMethod, String> {
    let line = line.trim_start_matches("pub ").trim_start();
    if !line.starts_with("fn ") {
        return Err("not a function signature".to_string());
    }

    let rest = line.trim_start_matches("fn ").trim_start();
    let mut name = String::new();
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name.push(c);
        } else {
            break;
        }
    }
    if name.is_empty() {
        Err("missing function name".to_string())
    } else {
        let params_sig = extract_fn_params_segment(line)
            .ok_or_else(|| "could not extract function parameters".to_string())?;
        let mut takes_raw_params = false;
        let mut params = Vec::new();
        let mut has_self = false;
        let mut has_ctx = false;
        let mut consumed_ctx = false;

        for raw in split_top_level_commas(params_sig) {
            let token = raw.trim();
            if token.is_empty()
                || token == "&self"
                || token == "self"
                || token == "&mut self"
                || token == "mut self"
            {
                has_self = true;
                continue;
            }

            let Some((name_part, ty_part)) = token.split_once(':') else {
                continue;
            };
            let param_name = name_part.trim();
            let param_ty = ty_part.trim();

            let normalized = normalize_type(param_ty);
            if is_script_context_type(&normalized) && !consumed_ctx {
                consumed_ctx = true;
                has_ctx = true;
                continue;
            }

            let is_raw_params = param_name == "params"
                && (normalized == "&[Variant]" || normalized == "&[perro_api::variant::Variant]");
            if is_raw_params {
                takes_raw_params = true;
                continue;
            }

            params.push(ScriptMethodParam {
                name: param_name.to_string(),
                ty: param_ty.to_string(),
            });
        }

        if takes_raw_params && !params.is_empty() {
            return Err("`params: &[Variant]` cannot be mixed with typed params".to_string());
        }
        if !(has_self && has_ctx) {
            let mut missing = Vec::new();
            if !has_self {
                missing.push("&self");
            }
            if !has_ctx {
                missing.push("ctx: &mut ScriptContext<...> (or ScriptCtx<...>)");
            }
            return Err(format!(
                "missing required leading parameters: {}",
                missing.join(", ")
            ));
        }

        let return_ty = extract_fn_return_type(line).map(str::to_string);
        let returns_variant = matches!(
            return_ty.as_deref().map(normalize_type).as_deref(),
            Some("Variant" | "perro_api::variant::Variant")
        );
        Ok(ScriptMethod {
            name,
            takes_raw_params,
            params,
            return_ty,
            returns_variant,
        })
    }
}

fn methods_debug_enabled() -> bool {
    let Ok(v) = std::env::var("PERRO_DEBUG_METHODS") else {
        return false;
    };
    let normalized = v.trim().to_ascii_lowercase();
    !normalized.is_empty()
        && !matches!(
            normalized.as_str(),
            "0" | "false" | "off" | "no" | "n" | "disabled"
        )
}

fn is_script_context_type(ty: &str) -> bool {
    ty.starts_with("&mutScriptContext<")
        || ty == "&mutScriptContext"
        || ty.starts_with("&mutScriptCtx<")
        || ty == "&mutScriptCtx"
        || ty.starts_with("&mutperro_api::scripting::ScriptContext<")
        || ty == "&mutperro_api::scripting::ScriptContext"
        || ty.starts_with("&mutperro_api::scripting::ScriptCtx<")
        || ty == "&mutperro_api::scripting::ScriptCtx"
        || ty.starts_with("ScriptContext<")
        || ty == "ScriptContext"
        || ty.starts_with("ScriptCtx<")
        || ty == "ScriptCtx"
        || ty.starts_with("perro_api::scripting::ScriptContext<")
        || ty == "perro_api::scripting::ScriptContext"
        || ty.starts_with("perro_api::scripting::ScriptCtx<")
        || ty == "perro_api::scripting::ScriptCtx"
}

fn parse_transpiler_attr_name(line: &str) -> Option<String> {
    let line = line.trim();
    if let Some(comment) = line.strip_prefix("///").or_else(|| line.strip_prefix("//")) {
        let comment = comment.trim();
        let rest = comment
            .strip_prefix('@')
            .or_else(|| comment.strip_prefix('#'))?
            .trim();
        if rest.is_empty() {
            return None;
        }
        let mut name = String::new();
        for c in rest.chars() {
            if c.is_ascii_alphanumeric() || c == '_' {
                name.push(c);
            } else {
                break;
            }
        }
        return is_ident(&name).then(|| name.to_ascii_lowercase());
    }

    if line.starts_with("#[") {
        let inner = line.strip_prefix("#[")?.strip_suffix(']')?.trim();
        if inner.is_empty() {
            return None;
        }
        let name = inner.split('(').next()?.trim();
        if !is_ident(name) || is_rust_attribute_name(name) {
            return None;
        }
        return Some(name.to_ascii_lowercase());
    }

    let rest = line.strip_prefix('#')?;
    if rest.starts_with('[') || rest.starts_with('!') {
        return None;
    }
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    let mut name = String::new();
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name.push(c);
        } else {
            break;
        }
    }
    is_ident(&name).then(|| name.to_ascii_lowercase())
}

fn is_rust_attribute_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        "state"
            | "default"
            | "derive"
            | "allow"
            | "warn"
            | "deny"
            | "forbid"
            | "cfg"
            | "cfg_attr"
            | "doc"
            | "path"
            | "test"
            | "inline"
            | "cold"
            | "deprecated"
            | "must_use"
            | "repr"
            | "non_exhaustive"
            | "no_mangle"
            | "unsafe"
    )
}

fn is_transpiler_attr_line(line: &str) -> bool {
    parse_transpiler_attr_name(line).is_some()
}

fn extract_fn_params_segment(line: &str) -> Option<&str> {
    let start = line.find('(')?;
    let mut depth = 0_i32;
    let mut end = None;
    for (i, c) in line.char_indices().skip(start) {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                end = Some(i);
                break;
            }
        }
    }
    let end = end?;
    Some(&line[start + 1..end])
}

fn extract_fn_return_type(line: &str) -> Option<&str> {
    let start = line.find('(')?;
    let mut depth = 0_i32;
    let mut end = None;
    for (i, c) in line.char_indices().skip(start) {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                end = Some(i);
                break;
            }
        }
    }
    let mut rest = line[end? + 1..].trim_start();
    rest = rest.strip_prefix("->")?.trim_start();

    let mut end = rest.len();
    for (i, c) in rest.char_indices() {
        if matches!(c, '{' | ';' | '=') {
            end = i;
            break;
        }
        if rest[i..].starts_with("where ") {
            end = i;
            break;
        }
    }

    let ty = rest[..end].trim().trim_end_matches(',');
    (!ty.is_empty()).then_some(ty)
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth_angle = 0_i32;
    let mut depth_paren = 0_i32;
    let mut depth_bracket = 0_i32;
    let mut start = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            ',' if depth_angle == 0 && depth_paren == 0 && depth_bracket == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start <= s.len() {
        out.push(&s[start..]);
    }
    out
}

fn params_get_expr(index: usize) -> String {
    if index == 0 {
        "params.first()".to_string()
    } else {
        format!("params.get({index})")
    }
}

fn generate_call_param_binding(index: usize, param: &ScriptMethodParam) -> Option<String> {
    let ty = normalize_type(&param.ty);
    let name = &param.name;
    let param_ref = params_get_expr(index);
    let line = match ty.as_str() {
        "bool" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Bool(v)) => *v, Some(_) => return Variant::Null, None => false }};"
        ),
        "i8" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I8(v))) => *v, Some(_) => return Variant::Null, None => 0_i8 }};"
        ),
        "i16" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I16(v))) => *v, Some(_) => return Variant::Null, None => 0_i16 }};"
        ),
        "i32" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I32(v))) => *v, Some(_) => return Variant::Null, None => 0_i32 }};"
        ),
        "i64" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I64(v))) => *v, Some(_) => return Variant::Null, None => 0_i64 }};"
        ),
        "i128" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I128(v))) => *v, Some(_) => return Variant::Null, None => 0_i128 }};"
        ),
        "isize" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I64(v))) => match isize::try_from(*v) {{ Ok(v) => v, Err(_) => return Variant::Null }}, Some(_) => return Variant::Null, None => 0_isize }};"
        ),
        "u8" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U8(v))) => *v, Some(_) => return Variant::Null, None => 0_u8 }};"
        ),
        "u16" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U16(v))) => *v, Some(_) => return Variant::Null, None => 0_u16 }};"
        ),
        "u32" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U32(v))) => *v, Some(_) => return Variant::Null, None => 0_u32 }};"
        ),
        "u64" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U64(v))) => *v, Some(_) => return Variant::Null, None => 0_u64 }};"
        ),
        "u128" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U128(v))) => *v, Some(_) => return Variant::Null, None => 0_u128 }};"
        ),
        "usize" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U64(v))) => match usize::try_from(*v) {{ Ok(v) => v, Err(_) => return Variant::Null }}, Some(_) => return Variant::Null, None => 0_usize }};"
        ),
        "f32" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::F32(v))) => *v, Some(_) => return Variant::Null, None => 0.0_f32 }};"
        ),
        "f64" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::F64(v))) => *v, Some(_) => return Variant::Null, None => 0.0_f64 }};"
        ),
        "String" | "std::string::String" | "alloc::string::String" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::String(v)) => v.to_string(), Some(_) => return Variant::Null, None => String::new() }};"
        ),
        "&str" => format!(
            "let {name}: &str = match {param_ref} {{ Some(Variant::String(v)) => v.as_ref(), Some(_) => return Variant::Null, None => \"\" }};"
        ),
        "Arc<str>" | "std::sync::Arc<str>" | "alloc::sync::Arc<str>" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::String(v)) => std::sync::Arc::<str>::clone(v), Some(_) => return Variant::Null, None => std::sync::Arc::<str>::from(\"\") }};"
        ),
        "NodeID" | "perro_api::ids::NodeID" => format!(
            "let {name} = match {param_ref} {{ Some(v) => match v.as_node() {{ Some(v) => v, None => return Variant::Null }}, None => perro_api::ids::NodeID::nil() }};"
        ),
        "TextureID" | "perro_api::ids::TextureID" => format!(
            "let {name} = match {param_ref} {{ Some(v) => match v.as_texture() {{ Some(v) => v, None => return Variant::Null }}, None => perro_api::ids::TextureID::nil() }};"
        ),
        "Variant" | "perro_api::variant::Variant" => format!(
            "let {name} = match {param_ref} {{ Some(v) => v.clone(), None => Variant::Null }};"
        ),
        _ => {
            if ty.starts_with('&') {
                return None;
            }
            format!(
                "let {name}: {raw_ty} = match {param_ref} {{ \
                    Some(v) => match v.parse::<{raw_ty}>() {{ Ok(v) => v, Err(_) => return Variant::Null }}, \
                    None => Default::default() \
                }};",
                raw_ty = param.ty.trim()
            )
        }
    };
    Some(line)
}

fn generate_get_var_body(fields: &[ScriptField]) -> String {
    if fields.is_empty() {
        return String::from("           Variant::Null");
    }

    let mut out = String::new();
    out.push_str("        let state = __perro_state_ref(state);\n");
    out.push_str("        match var {\n");
    for field in fields {
        let const_name = member_const_name(&field.name);
        out.push_str(&format!(
            "            {const_name} => perro_api::variant::DeriveVariant::to_variant(&state.{}),\n",
            field.name
        ));
    }
    out.push_str("            _ => __perro_get_nested_var(state, var).unwrap_or(Variant::Null),\n");
    out.push_str("        }");
    out
}

fn generate_set_var_body(fields: &[ScriptField]) -> String {
    if fields.is_empty() {
        return String::from("");
    }

    let mut out = String::new();
    out.push_str("        let state = __perro_state_mut(state);\n");
    out.push_str("        __perro_set_var_match(state, var, value);\n");
    out
}

fn generate_apply_scene_injected_vars_body(fields: &[ScriptField]) -> String {
    if fields.is_empty() {
        return String::from("");
    }

    let mut out = String::new();
    out.push_str("        let state = __perro_state_mut(state);\n");
    out.push_str("        for (var, value) in vars {\n");
    out.push_str("            __perro_set_var_match(state, var, value);\n");
    out.push_str("        }\n");
    out
}

fn generate_state_cast_helpers(state_ty: &str, fields: &[ScriptField]) -> String {
    if fields.is_empty() {
        return String::new();
    }

    format!(
        r#"#[inline(always)]
fn __perro_state_ref(state: &dyn std::any::Any) -> &{state_ty} {{
    // SAFETY: Perro runtime calls generated script methods only with this script's state type.
    unsafe {{ perro_api::scripting::state_ref_unchecked::<{state_ty}>(state) }}
}}

#[inline(always)]
fn __perro_state_mut(state: &mut dyn std::any::Any) -> &mut {state_ty} {{
    // SAFETY: Perro runtime calls generated script methods only with this script's state type.
    unsafe {{ perro_api::scripting::state_mut_unchecked::<{state_ty}>(state) }}
}}
"#
    )
}

fn variant_schema_field_names_expr(ty: &str) -> String {
    if variant_type_has_no_schema_fields(ty) {
        "&[]".to_string()
    } else {
        format!("<{ty} as perro_api::variant::VariantSchema>::field_names()")
    }
}

fn variant_type_has_no_schema_fields(ty: &str) -> bool {
    if ty.contains('<') || ty.starts_with('&') {
        return true;
    }
    matches!(
        ty,
        "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
            | "String"
            | "std::string::String"
            | "alloc::string::String"
            | "Variant"
            | "perro_api::variant::Variant"
            | "NodeID"
            | "TextureID"
            | "MaterialID"
            | "MeshID"
            | "AnimationID"
            | "LightID"
            | "SignalID"
            | "AudioBusID"
            | "TagID"
            | "PreloadedSceneID"
            | "perro_api::ids::NodeID"
            | "perro_api::ids::TextureID"
            | "perro_api::ids::MaterialID"
            | "perro_api::ids::MeshID"
            | "perro_api::ids::AnimationID"
            | "perro_api::ids::LightID"
            | "perro_api::ids::SignalID"
            | "perro_api::ids::AudioBusID"
            | "perro_api::ids::TagID"
            | "perro_api::ids::PreloadedSceneID"
            | "Vector2"
            | "Vector3"
            | "IVector2"
            | "IVector3"
            | "UVector2"
            | "UVector3"
            | "Quaternion"
            | "Transform2D"
            | "Transform3D"
            | "PostProcessSet"
            | "VisualAccessibilitySettings"
            | "perro_api::structs::Vector2"
            | "perro_api::structs::Vector3"
            | "perro_api::structs::IVector2"
            | "perro_api::structs::IVector3"
            | "perro_api::structs::UVector2"
            | "perro_api::structs::UVector3"
            | "perro_api::structs::Quaternion"
            | "perro_api::structs::Transform2D"
            | "perro_api::structs::Transform3D"
            | "perro_api::structs::PostProcessSet"
            | "perro_api::structs::VisualAccessibilitySettings"
    )
}

fn generate_set_var_match_fn(state_ty: &str, fields: &[ScriptField]) -> String {
    if fields.is_empty() {
        return String::from(
            "fn __perro_set_var_match(_state: &mut (), _var: ScriptMemberID, _value: Variant) {}",
        );
    }

    let mut out = String::new();
    out.push_str(&format!(
        "fn __perro_set_var_match(state: &mut {state_ty}, var: ScriptMemberID, value: Variant) {{\n"
    ));
    out.push_str("        match var {\n");
    for field in fields {
        let const_name = member_const_name(&field.name);
        let ty = normalize_type(&field.ty);
        let schema_fields = variant_schema_field_names_expr(&ty);
        let assign_block = format!(
            "if let Ok(v) = value.clone().into_parse::<{ty}>() {{\n                    state.{field_name} = v;\n                }} else {{\n                    let mut nested_root = perro_api::variant::DeriveVariant::to_variant(&state.{field_name});\n                    if __perro_apply_nested_object(\"{field_name}\", &mut nested_root, value, {schema_fields}) {{\n                        if let Ok(decoded) = nested_root.into_parse::<{ty}>() {{\n                            state.{field_name} = decoded;\n                        }}\n                    }}\n                }}",
            field_name = field.name
        );
        out.push_str(&format!(
            "            {const_name} => {{\n                {assign_block}\n            }}\n"
        ));
    }
    out.push_str("            _ => {\n");
    out.push_str("                __perro_set_nested_var(state, var, value);\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("}\n\n");

    out.push_str(
        "fn __perro_get_nested_by_hash(prefix: &str, value: Variant, var: ScriptMemberID, field_names: &[&str]) -> Option<Variant> {\n",
    );
    out.push_str("    match value {\n");
    out.push_str("        Variant::Object(obj) => {\n");
    out.push_str("            for (key, child) in obj {\n");
    out.push_str("                let full = if prefix.is_empty() {\n");
    out.push_str("                    key.to_string()\n");
    out.push_str("                } else {\n");
    out.push_str("                    format!(\"{prefix}.{}\", key.as_ref())\n");
    out.push_str("                };\n");
    out.push_str("                if ScriptMemberID::from_string(full.as_str()) == var {\n");
    out.push_str("                    return Some(child);\n");
    out.push_str("                }\n");
    out.push_str("                if let Some(found) = __perro_get_nested_by_hash(full.as_str(), child, var, &[]) {\n");
    out.push_str("                    return Some(found);\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("            None\n");
    out.push_str("        }\n");
    out.push_str("        Variant::Array(items) => {\n");
    out.push_str("            for (idx, child) in items.into_iter().enumerate() {\n");
    out.push_str("                let Some(key) = field_names.get(idx) else { continue; };\n");
    out.push_str("                let full = if prefix.is_empty() {\n");
    out.push_str("                    (*key).to_string()\n");
    out.push_str("                } else {\n");
    out.push_str("                    format!(\"{prefix}.{}\", key)\n");
    out.push_str("                };\n");
    out.push_str("                if ScriptMemberID::from_string(full.as_str()) == var {\n");
    out.push_str("                    return Some(child);\n");
    out.push_str("                }\n");
    out.push_str("                if let Some(found) = __perro_get_nested_by_hash(full.as_str(), child, var, &[]) {\n");
    out.push_str("                    return Some(found);\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("            None\n");
    out.push_str("        }\n");
    out.push_str("        _ => None,\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str(
        "fn __perro_set_nested_by_hash(prefix: &str, value: &mut Variant, var: ScriptMemberID, new_value: &mut Option<Variant>, field_names: &[&str]) -> bool {\n",
    );
    out.push_str("    match value {\n");
    out.push_str("        Variant::Object(obj) => {\n");
    out.push_str("            for (key, child) in obj {\n");
    out.push_str("                let full = if prefix.is_empty() {\n");
    out.push_str("                    key.to_string()\n");
    out.push_str("                } else {\n");
    out.push_str("                    format!(\"{prefix}.{}\", key.as_ref())\n");
    out.push_str("                };\n");
    out.push_str("                if ScriptMemberID::from_string(full.as_str()) == var {\n");
    out.push_str("                    if let Some(new_value) = new_value.take() {\n");
    out.push_str("                        *child = new_value;\n");
    out.push_str("                        return true;\n");
    out.push_str("                    }\n");
    out.push_str("                    return false;\n");
    out.push_str("                }\n");
    out.push_str("                if __perro_set_nested_by_hash(full.as_str(), child, var, new_value, &[]) {\n");
    out.push_str("                    return true;\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("            false\n");
    out.push_str("        }\n");
    out.push_str("        Variant::Array(items) => {\n");
    out.push_str("            for (idx, child) in items.iter_mut().enumerate() {\n");
    out.push_str("                let Some(key) = field_names.get(idx) else { continue; };\n");
    out.push_str("                let full = if prefix.is_empty() {\n");
    out.push_str("                    (*key).to_string()\n");
    out.push_str("                } else {\n");
    out.push_str("                    format!(\"{prefix}.{}\", key)\n");
    out.push_str("                };\n");
    out.push_str("                if ScriptMemberID::from_string(full.as_str()) == var {\n");
    out.push_str("                    if let Some(new_value) = new_value.take() {\n");
    out.push_str("                        *child = new_value;\n");
    out.push_str("                        return true;\n");
    out.push_str("                    }\n");
    out.push_str("                    return false;\n");
    out.push_str("                }\n");
    out.push_str("                if __perro_set_nested_by_hash(full.as_str(), child, var, new_value, &[]) {\n");
    out.push_str("                    return true;\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("            false\n");
    out.push_str("        }\n");
    out.push_str("        _ => false,\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str(
        "fn __perro_apply_nested_object(prefix: &str, target: &mut Variant, incoming: Variant, field_names: &[&str]) -> bool {\n",
    );
    out.push_str("    let Variant::Object(obj) = incoming else {\n");
    out.push_str("        return false;\n");
    out.push_str("    };\n");
    out.push_str("    let mut changed = false;\n");
    out.push_str("    for (key, value) in obj {\n");
    out.push_str("        let full = if prefix.is_empty() {\n");
    out.push_str("            key.to_string()\n");
    out.push_str("        } else {\n");
    out.push_str("            format!(\"{prefix}.{}\", key.as_ref())\n");
    out.push_str("        };\n");
    out.push_str("        let mut value = Some(value);\n");
    out.push_str("        changed |= __perro_set_nested_by_hash(prefix, target, ScriptMemberID::from_string(full.as_str()), &mut value, field_names);\n");
    out.push_str("    }\n");
    out.push_str("    changed\n");
    out.push_str("}\n\n");

    out.push_str(&format!(
        "fn __perro_get_nested_var(state: &{state_ty}, var: ScriptMemberID) -> Option<Variant> {{\n"
    ));
    for field in fields {
        let ty = normalize_type(&field.ty);
        let schema_fields = variant_schema_field_names_expr(&ty);
        out.push_str(&format!(
            "    {{\n        let nested_root = perro_api::variant::DeriveVariant::to_variant(&state.{field_name});\n        if let Some(value) = __perro_get_nested_by_hash(\"{field_name}\", nested_root, var, {schema_fields}) {{\n            return Some(value);\n        }}\n    }}\n",
            field_name = field.name,
            schema_fields = schema_fields
        ));
    }
    out.push_str("    None\n");
    out.push_str("}\n\n");

    out.push_str(&format!(
        "fn __perro_set_nested_var(state: &mut {state_ty}, var: ScriptMemberID, value: Variant) -> bool {{\n"
    ));
    out.push_str("    let mut value = Some(value);\n");
    for field in fields {
        let ty = normalize_type(&field.ty);
        let schema_fields = variant_schema_field_names_expr(&ty);
        out.push_str(&format!(
            "    {{\n        let mut nested_root = perro_api::variant::DeriveVariant::to_variant(&state.{field_name});\n        if __perro_set_nested_by_hash(\"{field_name}\", &mut nested_root, var, &mut value, {schema_fields}) {{\n            if let Ok(decoded) = nested_root.into_parse::<{ty}>() {{\n                state.{field_name} = decoded;\n            }}\n            return true;\n        }}\n    }}\n",
            field_name = field.name,
            ty = ty,
            schema_fields = schema_fields
        ));
    }
    out.push_str("    false\n}\n");
    out
}

fn module_name_from_rel(rel: &str) -> String {
    let mut out = String::with_capacity(rel.len());
    for c in rel.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    let mut name = if trimmed.is_empty() {
        "script".to_string()
    } else {
        trimmed.to_string()
    };
    if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name
}

fn module_short_name_from_rel(rel: &str) -> String {
    module_name_from_rel(rel.strip_suffix(".rs").unwrap_or(rel))
}

fn generated_script_rel(rel: &str) -> String {
    if let Some(base) = rel.strip_suffix(".rs") {
        format!("{base}.gen.rs")
    } else {
        format!("{rel}.gen.rs")
    }
}

#[allow(dead_code)]
fn rel_to_path(base: &Path, rel: &str) -> PathBuf {
    base.join(rel.replace('/', "\\"))
}
