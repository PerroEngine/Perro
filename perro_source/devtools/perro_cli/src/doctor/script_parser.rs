use super::*;

pub(super) fn parse_struct_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| parse_struct_name_from_line(line.trim()))
        .collect()
}

pub(super) fn parse_enum_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| parse_enum_name_from_line(line.trim()))
        .collect()
}

pub(super) fn parse_state_struct_names(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut names = Vec::new();
    for i in 0..lines.len() {
        let line = lines[i].trim();
        if !is_state_attribute(line) {
            continue;
        }
        for next in lines.iter().skip(i + 1) {
            let n = next.trim();
            if n.is_empty() || n.starts_with("#[") {
                continue;
            }
            if let Some(name) = parse_struct_name_from_line(n) {
                names.push(name);
            }
            break;
        }
    }
    names
}

pub(super) fn is_state_attribute(line: &str) -> bool {
    matches!(line, "#[State]" | "#[state]")
}

pub(super) fn parse_struct_name_from_line(line: &str) -> Option<String> {
    let line = line.trim_start_matches("pub ").trim_start();
    let rest = line.strip_prefix("struct ")?;
    let name = rest
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub(super) fn parse_enum_name_from_line(line: &str) -> Option<String> {
    let line = line.trim_start_matches("pub ").trim_start();
    let rest = line.strip_prefix("enum ")?;
    let name = rest
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub(super) fn parse_struct_fields(text: &str, struct_name: &str) -> Vec<DoctorField> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines
        .iter()
        .position(|line| parse_struct_name_from_line(line.trim()).as_deref() == Some(struct_name))
    else {
        return Vec::new();
    };

    let mut fields = Vec::new();
    let mut depth = 0_i32;
    let mut opened = false;
    let mut pending_node_ref_types = Vec::new();
    for line in lines.iter().skip(start) {
        let line = strip_line_comment_for_doctor(line);
        if !opened {
            if let Some(pos) = line.find('{') {
                opened = true;
                depth = 1;
                if let Some(field) = parse_field_for_doctor(
                    &line[pos + 1..],
                    std::mem::take(&mut pending_node_ref_types),
                ) {
                    fields.push(field);
                }
                depth += brace_delta_for_doctor(&line[pos + 1..]);
            }
        } else {
            if depth == 1
                && let Some(types) = parse_node_ref_attr_for_doctor(line.trim())
            {
                pending_node_ref_types = types;
                depth += brace_delta_for_doctor(line);
                continue;
            }
            if depth == 1
                && let Some(field) =
                    parse_field_for_doctor(line, std::mem::take(&mut pending_node_ref_types))
            {
                fields.push(field);
            } else if depth == 1 && !line.trim().is_empty() && !line.trim().starts_with("#[") {
                pending_node_ref_types.clear();
            }
            depth += brace_delta_for_doctor(line);
        }
        if opened && depth <= 0 {
            break;
        }
    }
    fields
}

pub(super) fn parse_enum_fields(text: &str, enum_name: &str) -> Vec<DoctorField> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines
        .iter()
        .position(|line| parse_enum_name_from_line(line.trim()).as_deref() == Some(enum_name))
    else {
        return Vec::new();
    };

    let mut fields = Vec::new();
    let mut depth = 0_i32;
    let mut variant_field_depth = 0_i32;
    let mut opened = false;
    for line in lines.iter().skip(start) {
        let line = strip_line_comment_for_doctor(line);
        if !opened {
            if let Some(pos) = line.find('{') {
                opened = true;
                depth = 1;
                fields.extend(parse_enum_line_fields(&line[pos + 1..]));
                depth += brace_delta_for_doctor(&line[pos + 1..]);
            }
        } else {
            if variant_field_depth > 0 {
                if variant_field_depth == 1
                    && let Some(field) = parse_field_for_doctor(line, Vec::new())
                {
                    fields.push(field);
                }
                variant_field_depth += brace_delta_for_doctor(line);
                depth += brace_delta_for_doctor(line);
                continue;
            }
            if depth == 1
                && let Some(pos) = line.find('{')
            {
                if find_matching_delim_for_doctor(line, pos, '{', '}').is_some() {
                    fields.extend(parse_enum_line_fields(line));
                } else {
                    variant_field_depth = 1;
                    if let Some(field) = parse_field_for_doctor(&line[pos + 1..], Vec::new()) {
                        fields.push(field);
                    }
                }
            }
            depth += brace_delta_for_doctor(line);
        }
        if opened && depth <= 0 {
            break;
        }
    }
    fields.sort_by(|a, b| a.name.cmp(&b.name));
    fields.dedup_by(|a, b| a.name == b.name);
    fields
}

pub(super) fn parse_enum_line_fields(line: &str) -> Vec<DoctorField> {
    let Some(open) = line.find('{') else {
        return Vec::new();
    };
    let Some(close) = find_matching_delim_for_doctor(line, open, '{', '}') else {
        return Vec::new();
    };
    split_top_level_args(&line[open + 1..close])
        .into_iter()
        .filter_map(|field| parse_field_for_doctor(field, Vec::new()))
        .collect()
}

pub(super) fn parse_field_for_doctor(
    line: &str,
    node_ref_types: Vec<String>,
) -> Option<DoctorField> {
    let trimmed = line.trim().trim_end_matches(',').trim();
    if trimmed.is_empty()
        || trimmed.starts_with("#[")
        || trimmed.starts_with("//")
        || trimmed.starts_with("///")
    {
        return None;
    }
    let without_vis = if let Some(rest) = trimmed.strip_prefix("pub(") {
        rest.split_once(')')?.1.trim()
    } else {
        trimmed.trim_start_matches("pub ").trim_start()
    };
    let (name, ty) = without_vis.split_once(':')?;
    let name = name.trim();
    if is_ident_for_doctor(name) {
        Some(DoctorField {
            name: name.to_string(),
            ty: normalize_type_name_for_doctor(ty),
            node_ref_types,
        })
    } else {
        None
    }
}

pub(super) fn parse_node_ref_attr_for_doctor(line: &str) -> Option<Vec<String>> {
    let inner = line
        .trim()
        .strip_prefix("#[node_ref")?
        .strip_suffix(']')?
        .trim();
    let inner = inner.strip_prefix('(')?.strip_suffix(')')?;
    Some(
        split_top_level_args(inner)
            .into_iter()
            .map(|item| item.trim().trim_matches('"').to_string())
            .filter(|item| !item.is_empty())
            .collect(),
    )
}

pub(super) fn normalize_type_name_for_doctor(input: &str) -> String {
    let mut ty = input.trim().trim_end_matches(',').trim();
    while let Some(rest) = ty.strip_prefix('&') {
        ty = rest.trim_start();
    }
    if let Some(rest) = ty.strip_prefix("mut ") {
        ty = rest.trim_start();
    }
    let without_path = ty.rsplit("::").next().unwrap_or(ty);
    without_path
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .next()
        .unwrap_or("")
        .to_string()
}

pub(super) fn parse_script_method_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    names.extend(parse_methods_macro_names(text));
    names.extend(parse_inherent_method_names(text));
    names.sort();
    names.dedup();
    names
}

pub(super) fn parse_methods_macro_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for inner in find_macro_calls(text, "methods") {
        if let Some(body) = parse_methods_macro_body(&inner) {
            names.extend(parse_method_names_from_block(body));
        }
    }
    names
}

pub(super) fn parse_methods_macro_body(inner: &str) -> Option<&str> {
    let trimmed = inner.trim();
    if trimmed.starts_with('{') {
        return extract_brace_block_for_doctor(trimmed);
    }
    let target_len = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .map(char::len_utf8)
        .sum::<usize>();
    if target_len == 0 {
        return None;
    }
    extract_brace_block_for_doctor(trimmed[target_len..].trim_start())
}

pub(super) fn parse_inherent_method_names(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut names = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let line = strip_line_comment_for_doctor(lines[i]).trim();
        if !line.starts_with("impl") || line.contains(" for ") {
            i += 1;
            continue;
        }
        let mut depth = brace_delta_for_doctor(line);
        let mut opened = line.contains('{');
        i += 1;
        while i < lines.len() {
            let l = strip_line_comment_for_doctor(lines[i]);
            if opened
                && depth == 1
                && let Some(name) = parse_fn_name_for_doctor(l.trim())
            {
                names.push(name);
            }
            if !opened && l.contains('{') {
                opened = true;
            }
            depth += brace_delta_for_doctor(l);
            if opened && depth <= 0 {
                break;
            }
            i += 1;
        }
        i += 1;
    }
    names
}

pub(super) fn parse_method_names_from_block(body: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut depth = 0_i32;
    let mut sig_buf: Option<String> = None;
    let mut sig_paren_depth = 0_i32;
    for line in body.lines() {
        let line = strip_line_comment_for_doctor(line);
        let trimmed = line.trim();
        if depth == 0 {
            if let Some(buf) = sig_buf.as_mut() {
                if !trimmed.is_empty() {
                    buf.push(' ');
                    buf.push_str(trimmed);
                }
                sig_paren_depth += paren_delta_for_doctor(trimmed);
                if sig_paren_depth <= 0 {
                    if let Some(name) = parse_fn_name_for_doctor(buf) {
                        names.push(name);
                    }
                    sig_buf = None;
                }
            } else if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                sig_buf = Some(trimmed.to_string());
                sig_paren_depth = paren_delta_for_doctor(trimmed);
                if sig_paren_depth <= 0 {
                    if let Some(name) = parse_fn_name_for_doctor(trimmed) {
                        names.push(name);
                    }
                    sig_buf = None;
                }
            }
        }
        depth += brace_delta_for_doctor(line);
    }
    names
}

pub(super) fn validate_script_member_calls(
    project_dir: &Path,
    file: &Path,
    text: &str,
    index: &ScriptDoctorIndex,
    report: &mut ValidationReport,
) {
    validate_var_member_calls(project_dir, file, text, "get_var", index, report);
    validate_var_member_calls(project_dir, file, text, "set_var", index, report);
    validate_state_access_calls(project_dir, file, text, "with_state", index, report);
    validate_state_access_calls(project_dir, file, text, "with_state_mut", index, report);
    validate_method_member_calls(project_dir, file, text, &index.methods, report);
}

pub(super) fn validate_state_access_calls(
    project_dir: &Path,
    file: &Path,
    text: &str,
    macro_name: &str,
    index: &ScriptDoctorIndex,
    report: &mut ValidationReport,
) {
    for call in find_macro_calls_with_lines(text, macro_name) {
        let args = split_top_level_args(&call.inner);
        if args.len() < 2 {
            continue;
        }
        let Some(state_type) = extract_state_type_arg(args[1]) else {
            continue;
        };
        if !index.state_types.contains(&state_type) {
            let source = format_source_location(project_dir, Some(file), Some(call.line));
            report.warn(format!(
                "script state missing: {source}`{macro_name}!` uses `{state_type}`, but no `#[State]` struct defines it"
            ));
        }
    }
}

pub(super) fn extract_state_type_arg(arg: &str) -> Option<String> {
    let arg = arg.trim();
    if arg.is_empty()
        || arg.contains('<')
        || arg.contains('>')
        || arg.contains('(')
        || arg.contains(')')
        || arg.contains('{')
        || arg.contains('}')
        || arg.contains('[')
        || arg.contains(']')
    {
        return None;
    }
    let name = arg.rsplit("::").next()?.trim();
    if is_ident_for_doctor(name) {
        Some(name.to_string())
    } else {
        None
    }
}

pub(super) fn validate_var_member_calls(
    project_dir: &Path,
    file: &Path,
    text: &str,
    macro_name: &str,
    index: &ScriptDoctorIndex,
    report: &mut ValidationReport,
) {
    for call in find_macro_calls_with_lines(text, macro_name) {
        let args = split_top_level_args(&call.inner);
        if args.len() < 3 {
            continue;
        }
        let target = normalize_arg_text(args[1]);
        if target == "ctx.id" {
            let member = extract_member_literal(args[2], &["var"]);
            let replacement =
                var_self_access_replacement(index, macro_name, member.as_deref(), &args);
            let source = format_source_location(project_dir, Some(file), Some(call.line));
            report.warn(format!(
                "script self access: {source}`{macro_name}!` can use `{replacement}`"
            ));
        }
        if let Some(member) = extract_member_literal(args[2], &["var"])
            && !known_var_member(index, &member)
        {
            let source = format_source_location(project_dir, Some(file), Some(call.line));
            report.warn(format!(
                "script member missing: {source}`{macro_name}!` references state field `{member}`, but no script defines it"
            ));
        }
    }
}

pub(super) fn var_self_access_replacement(
    index: &ScriptDoctorIndex,
    macro_name: &str,
    member: Option<&str>,
    args: &[&str],
) -> String {
    let Some(member) = member else {
        return if macro_name == "get_var" {
            "with_state!(ctx.run, StateType, ctx.id, |state| state.field).unwrap_or_default()"
                .to_string()
        } else {
            "with_state_mut!(ctx.run, StateType, ctx.id, |state| state.field = value)".to_string()
        };
    };
    let root = member.split('.').next().unwrap_or(member);
    let state_type = index
        .state_field_owners
        .get(root)
        .map(String::as_str)
        .unwrap_or("StateType");
    if macro_name == "get_var" {
        format!(
            "with_state!(ctx.run, {state_type}, ctx.id, |state| state.{member}).unwrap_or_default()"
        )
    } else {
        let value = args.get(3).map(|arg| arg.trim()).unwrap_or("value");
        format!("with_state_mut!(ctx.run, {state_type}, ctx.id, |state| state.{member} = {value})")
    }
}

pub(super) fn known_var_member(index: &ScriptDoctorIndex, member: &str) -> bool {
    let mut parts = member.split('.');
    let Some(root) = parts.next() else {
        return false;
    };
    if !index.state_fields.contains(root) {
        return false;
    }
    let Some(mut candidate_types) = index.state_field_types.get(root).cloned() else {
        return parts.next().is_none();
    };
    // several scripts may declare state fields with the same name but
    // different struct types; accept the member if any candidate resolves
    for part in parts {
        let mut next_types = HashSet::new();
        for current_type in &candidate_types {
            let Some(fields) = index.custom_type_fields.get(current_type) else {
                continue;
            };
            if let Some(field) = fields.iter().find(|field| field.name == part) {
                next_types.insert(field.ty.clone());
            }
        }
        if next_types.is_empty() {
            return false;
        }
        candidate_types = next_types;
    }
    true
}

pub(super) fn validate_method_member_calls(
    project_dir: &Path,
    file: &Path,
    text: &str,
    known_methods: &HashSet<String>,
    report: &mut ValidationReport,
) {
    for call in find_macro_calls_with_lines(text, "call_method") {
        let args = split_top_level_args(&call.inner);
        if args.len() < 3 {
            continue;
        }
        let target = normalize_arg_text(args[1]);
        let member = extract_member_literal(args[2], &["method", "func"]);
        if target == "ctx.id" {
            let replacement = member
                .as_ref()
                .map(|name| format!("self.{name}(ctx, params...)"))
                .unwrap_or_else(|| "self.method_name(ctx, params...)".to_string());
            let source = format_source_location(project_dir, Some(file), Some(call.line));
            report.warn(format!(
                "script self access: {source}`call_method!` can use `{replacement}`"
            ));
        }
        if let Some(member) = member
            && !known_methods.contains(&member)
        {
            let source = format_source_location(project_dir, Some(file), Some(call.line));
            report.warn(format!(
                "script member missing: {source}`call_method!` references method `{member}`, but no script defines it"
            ));
        }
    }
}

pub(super) fn extract_member_literal(arg: &str, macro_names: &[&str]) -> Option<String> {
    let arg = arg.trim();
    for macro_name in macro_names {
        let prefix = format!("{macro_name}!");
        if let Some(rest) = arg.strip_prefix(&prefix) {
            let rest = rest.trim_start();
            if rest.starts_with('(')
                && let Some(end) = find_matching_delim_for_doctor(rest, 0, '(', ')')
            {
                return parse_string_literal_value(rest[1..end].trim());
            }
        }
    }
    parse_string_literal_value(arg)
}

pub(super) fn extract_member_literals(arg: &str, macro_names: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(member) = extract_member_literal(arg, macro_names) {
        out.push(member);
    }
    for macro_name in macro_names {
        for call in find_macro_calls(arg, macro_name) {
            if let Some(member) = parse_string_literal_value(call.trim()) {
                out.push(member);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

pub(super) fn parse_string_literal_value(input: &str) -> Option<String> {
    let input = input.trim();
    if input.starts_with('"') {
        let mut escaped = false;
        for (i, ch) in input.char_indices().skip(1) {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                return Some(input[1..i].to_string());
            }
        }
    }
    if input.starts_with('r') {
        let quote = input.find('"')?;
        let hashes = input[1..quote].chars().filter(|ch| *ch == '#').count();
        let end = format!("\"{}", "#".repeat(hashes));
        let body_start = quote + 1;
        let body_end = input[body_start..].find(&end)? + body_start;
        return Some(input[body_start..body_end].to_string());
    }
    None
}
