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
    let lines = lex_code_lines_for_doctor(text);
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
        let line = line.as_str();
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
    let lines = lex_code_lines_for_doctor(text);
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
        let line = line.as_str();
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
    let (without_vis, is_pub) = strip_visibility_for_doctor(trimmed);
    let (name, ty) = without_vis.split_once(':')?;
    let name = name.trim();
    if is_ident_for_doctor(name) {
        Some(DoctorField {
            name: name.to_string(),
            ty: normalize_type_name_for_doctor(ty),
            node_ref_types,
            is_pub,
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

/// one parsed fn def: vis + dispatch shape
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DoctorParsedMethod {
    pub(super) name: String,
    pub(super) is_pub: bool,
    /// sig takes ScriptContext/ScriptCtx -> compiler emits call glue 4 it;
    /// plain helper fns get no glue, pub on them is free
    pub(super) has_ctx: bool,
}

/// all script method defs in file; dup names kp per-def flags,
/// index merge happens at insert time
pub(super) fn parse_script_methods(text: &str) -> Vec<DoctorParsedMethod> {
    let mut methods = Vec::new();
    methods.extend(parse_methods_macro_fns(text));
    methods.extend(parse_inherent_method_fns(text));
    methods
}

pub(super) fn parse_methods_macro_fns(text: &str) -> Vec<DoctorParsedMethod> {
    let mut names = Vec::new();
    for inner in find_macro_calls(text, "methods") {
        if let Some(body) = parse_methods_macro_body(&inner) {
            names.extend(parse_method_fns_from_block(body));
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

pub(super) fn parse_inherent_method_fns(text: &str) -> Vec<DoctorParsedMethod> {
    let lines = lex_code_lines_for_doctor(text);
    let mut methods = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i].trim();
        if !line.starts_with("impl") || line.contains(" for ") {
            i += 1;
            continue;
        }
        // gather impl body (already lexed) -> shared buffered sig parse,
        // multi-line sigs kp their ctx params visible
        let mut depth = brace_delta_for_doctor(line);
        let mut opened = line.contains('{');
        let mut body: Vec<String> = Vec::new();
        if opened && let Some(pos) = lines[i].find('{') {
            body.push(lines[i][pos + 1..].to_string());
        }
        i += 1;
        while i < lines.len() {
            let l = lines[i].as_str();
            if !opened {
                if let Some(pos) = l.find('{') {
                    opened = true;
                    body.push(l[pos + 1..].to_string());
                    depth += brace_delta_for_doctor(l);
                    if depth <= 0 {
                        break;
                    }
                    i += 1;
                    continue;
                }
            } else {
                body.push(l.to_string());
            }
            depth += brace_delta_for_doctor(l);
            if opened && depth <= 0 {
                break;
            }
            i += 1;
        }
        methods.extend(parse_method_fns_from_lexed(body.into_iter()));
        i += 1;
    }
    methods
}

pub(super) fn parse_method_fns_from_block(body: &str) -> Vec<DoctorParsedMethod> {
    parse_method_fns_from_lexed(lex_code_lines_for_doctor(body).into_iter())
}

fn parse_method_fns_from_lexed(lines: impl Iterator<Item = String>) -> Vec<DoctorParsedMethod> {
    let mut methods = Vec::new();
    let mut depth = 0_i32;
    let mut sig_buf: Option<String> = None;
    let mut sig_paren_depth = 0_i32;
    let push_sig = |sig: &str, methods: &mut Vec<DoctorParsedMethod>| {
        if let Some((name, is_pub)) = parse_fn_vis_name_for_doctor(sig) {
            methods.push(DoctorParsedMethod {
                name,
                is_pub,
                has_ctx: sig.contains("ScriptContext") || sig.contains("ScriptCtx"),
            });
        }
    };
    for line in lines {
        let trimmed = line.trim();
        if depth == 0 {
            if let Some(buf) = sig_buf.as_mut() {
                if !trimmed.is_empty() {
                    buf.push(' ');
                    buf.push_str(trimmed);
                }
                sig_paren_depth += paren_delta_for_doctor(trimmed);
                if sig_paren_depth <= 0 {
                    let buf = sig_buf.take().unwrap_or_default();
                    push_sig(&buf, &mut methods);
                }
            } else if is_fn_sig_start_for_doctor(trimmed) {
                sig_buf = Some(trimmed.to_string());
                sig_paren_depth = paren_delta_for_doctor(trimmed);
                if sig_paren_depth <= 0 {
                    let buf = sig_buf.take().unwrap_or_default();
                    push_sig(&buf, &mut methods);
                }
            }
        }
        depth += brace_delta_for_doctor(&line);
    }
    methods
}

fn is_fn_sig_start_for_doctor(line: &str) -> bool {
    let (rest, _) = strip_visibility_for_doctor(line);
    rest.starts_with("fn ")
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
    validate_var_member_calls(project_dir, file, text, "broadcast_var", index, report);
    validate_state_access_calls(project_dir, file, text, "with_state", index, report);
    validate_state_access_calls(project_dir, file, text, "with_state_mut", index, report);
    validate_method_member_calls(project_dir, file, text, index, report);
    validate_signal_handler_targets(project_dir, file, text, index, report);
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
        // broadcast root = subtree write, self root is fine there
        if target == "ctx.id" && macro_name != "broadcast_var" {
            let member = extract_member_literal(args[2], &["var"]);
            let replacement =
                var_self_access_replacement(index, macro_name, member.as_deref(), &args);
            let source = format_source_location(project_dir, Some(file), Some(call.line));
            report.warn(format!(
                "script self access: {source}`{macro_name}!` can use `{replacement}`"
            ));
        }
        if let Some(member) = extract_member_literal(args[2], &["var"]) {
            if !known_var_member(index, &member) {
                let source = format_source_location(project_dir, Some(file), Some(call.line));
                report.warn(format!(
                    "script member missing: {source}`{macro_name}!` references state field `{member}`, but no script defines it"
                ));
            } else if !var_member_pub(index, &member) {
                let root = member.split('.').next().unwrap_or(member.as_str());
                let found = format_member_found_in(
                    project_dir,
                    index.state_field_files.get(root).map(Vec::as_slice),
                );
                let source = format_source_location(project_dir, Some(file), Some(call.line));
                report.warn(format!(
                    "script member private: {source}`{macro_name}!` references state field `{member}`, but no `pub {root}` exists{found} — make the field `pub` to expose it to other scripts (non-pub fields get no get/set glue)"
                ));
            }
        }
    }
}

/// runtime get/set glue exists only 4 pub root fields; nested paths ride the
/// pub root's variant tree, so root vis alone gates access
pub(super) fn var_member_pub(index: &ScriptDoctorIndex, member: &str) -> bool {
    let root = member.split('.').next().unwrap_or(member);
    index
        .state_field_defs
        .get(root)
        .is_none_or(|defs| defs.iter().any(|def| def.is_pub))
}

/// ` (found `x` in res://a.rs, res://b.rs)` or empty
pub(super) fn format_member_found_in(project_dir: &Path, files: Option<&[PathBuf]>) -> String {
    let Some(files) = files else {
        return String::new();
    };
    if files.is_empty() {
        return String::new();
    }
    let mut names: Vec<String> = files
        .iter()
        .take(3)
        .map(|file| format_project_path(project_dir, file))
        .collect();
    if files.len() > 3 {
        names.push(format!("+{} more", files.len() - 3));
    }
    format!(" (found in {})", names.join(", "))
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
    index: &ScriptDoctorIndex,
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
        if let Some(member) = member {
            warn_private_or_missing_method(
                project_dir,
                file,
                call.line,
                "call_method!",
                &member,
                index,
                report,
            );
        }
    }
}

/// method reached thru generated call_method glue (call_method! or signal
/// dispatch): warn when undefined, or defined but never pub
pub(super) fn warn_private_or_missing_method(
    project_dir: &Path,
    file: &Path,
    line: usize,
    macro_label: &str,
    member: &str,
    index: &ScriptDoctorIndex,
    report: &mut ValidationReport,
) {
    match index.methods.get(member) {
        None => {
            let source = format_source_location(project_dir, Some(file), Some(line));
            report.warn(format!(
                "script member missing: {source}`{macro_label}` references method `{member}`, but no script defines it"
            ));
        }
        Some(def) if !def.dispatch => {
            let found = format_member_found_in(project_dir, Some(def.files.as_slice()));
            let source = format_source_location(project_dir, Some(file), Some(line));
            report.warn(format!(
                "script member not callable: {source}`{macro_label}` references `{member}`{found}, but no definition takes `ctx: &mut ScriptContext<'_, API>` — only ctx methods get call glue"
            ));
        }
        Some(def) if !def.is_pub => {
            let found = format_member_found_in(project_dir, Some(def.files.as_slice()));
            let source = format_source_location(project_dir, Some(file), Some(line));
            report.warn(format!(
                "script member private: {source}`{macro_label}` references method `{member}`, but no `pub fn {member}` exists{found} — make it `pub fn` so the compiler generates its call glue"
            ));
        }
        Some(_) => {}
    }
}

/// signal handlers dispatch thru call_method glue -> handler fns need pub
pub(super) fn validate_signal_handler_targets(
    project_dir: &Path,
    file: &Path,
    text: &str,
    index: &ScriptDoctorIndex,
    report: &mut ValidationReport,
) {
    for macro_name in ["signal_connect", "signal_connect_many"] {
        for call in find_macro_calls_with_lines(text, macro_name) {
            let args = split_top_level_args(&call.inner);
            if args.len() < 4 {
                continue;
            }
            for member in extract_member_literals(args[3], &["func", "method"]) {
                warn_private_or_missing_method(
                    project_dir,
                    file,
                    call.line,
                    &format!("{macro_name}!"),
                    &member,
                    index,
                    report,
                );
            }
        }
    }
    for call in find_macro_calls_with_lines(text, "signal_connect_pairs") {
        for (_, handler) in extract_signal_pairs(&call.inner) {
            warn_private_or_missing_method(
                project_dir,
                file,
                call.line,
                "signal_connect_pairs!",
                &handler,
                index,
                report,
            );
        }
    }
}

/// pairs list arg: `[ (signal, handler), ... ]` w/ raw str or macro-wrapped
/// literals -> resolved (signal, handler) name pairs
pub(super) fn extract_signal_pairs(inner: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let args = split_top_level_args(inner);
    let Some(list) = args.get(2) else {
        return out;
    };
    let list = list.trim();
    if !list.starts_with('[') {
        return out;
    }
    let Some(close) = find_matching_delim_for_doctor(list, 0, '[', ']') else {
        return out;
    };
    for pair in split_top_level_args(&list[1..close]) {
        let pair = pair.trim();
        if !pair.starts_with('(') {
            continue;
        }
        let Some(pair_close) = find_matching_delim_for_doctor(pair, 0, '(', ')') else {
            continue;
        };
        let parts = split_top_level_args(&pair[1..pair_close]);
        if parts.len() < 2 {
            continue;
        }
        let signal = extract_member_literal(parts[0], &["signal"]);
        let handler = extract_member_literal(parts[1], &["func", "method"]);
        if let (Some(signal), Some(handler)) = (signal, handler) {
            out.push((signal, handler));
        }
    }
    out
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
