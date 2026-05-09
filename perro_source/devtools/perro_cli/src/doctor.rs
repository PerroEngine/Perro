use crate::project::collect_rs_files_recursive;
use crate::{log_done, parse_flag_value, resolve_local_path};
use perro_project::{ProjectConfig, load_project_toml};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn doctor_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let report = validate_project(&project_dir)?;
    report.print();
    if report.errors > 0 {
        return Err(format!("validation failed: {} issue(s)", report.errors));
    }
    log_done("Project Valid");
    Ok(())
}

#[derive(Default)]
struct ValidationReport {
    checked_files: usize,
    checked_refs: usize,
    warnings: usize,
    errors: usize,
    messages: Vec<String>,
}

impl ValidationReport {
    fn warn(&mut self, msg: String) {
        self.warnings += 1;
        self.messages.push(format!("warn: {msg}"));
    }

    fn error(&mut self, msg: String) {
        self.errors += 1;
        self.messages.push(format!("err: {msg}"));
    }

    fn print(&self) {
        for msg in &self.messages {
            println!("{msg}");
        }
        println!(
            "checked {} file(s), {} reference(s), {} warning(s), {} error(s)",
            self.checked_files, self.checked_refs, self.warnings, self.errors
        );
    }
}

fn validate_project(project_dir: &Path) -> Result<ValidationReport, String> {
    if !project_dir.join("project.toml").exists() {
        return Err(format!(
            "invalid project path `{}`. Expected project.toml.",
            project_dir.display()
        ));
    }

    let mut report = ValidationReport::default();
    let config = match load_project_toml(project_dir) {
        Ok(config) => config,
        Err(err) => {
            report.error(format!("project.toml parse failed: {err}"));
            return Ok(report);
        }
    };

    validate_project_config_refs(project_dir, &config, &mut report);

    let mut files = Vec::new();
    collect_reference_text_files(project_dir, &mut files)?;
    for file in files {
        report.checked_files += 1;
        let text = fs::read_to_string(&file)
            .map_err(|err| format!("failed to read {}: {err}", file.display()))?;
        for raw_ref in extract_virtual_refs(&text) {
            report.checked_refs += 1;
            validate_virtual_ref(project_dir, Some(&file), &raw_ref, &mut report);
        }
    }

    validate_script_warnings(project_dir, &mut report)?;

    Ok(report)
}

fn validate_project_config_refs(
    project_dir: &Path,
    config: &ProjectConfig,
    report: &mut ValidationReport,
) {
    report.checked_refs += 1;
    validate_named_virtual_ref(
        project_dir,
        None,
        "project.main_scene",
        &config.main_scene,
        true,
        report,
    );
    report.checked_refs += 1;
    validate_named_virtual_ref(
        project_dir,
        None,
        "project.icon",
        &config.icon,
        false,
        report,
    );
    report.checked_refs += 1;
    validate_named_virtual_ref(
        project_dir,
        None,
        "project.startup_splash",
        &config.startup_splash,
        false,
        report,
    );
}

fn validate_virtual_ref(
    project_dir: &Path,
    source_file: Option<&Path>,
    raw_ref: &str,
    report: &mut ValidationReport,
) {
    validate_named_virtual_ref(project_dir, source_file, raw_ref, raw_ref, true, report);
}

fn validate_named_virtual_ref(
    project_dir: &Path,
    source_file: Option<&Path>,
    label: &str,
    raw_ref: &str,
    required: bool,
    report: &mut ValidationReport,
) {
    let Some(path) = resolve_virtual_ref_path(project_dir, source_file, raw_ref) else {
        report.error(format!("{label}: unsupported path `{raw_ref}`"));
        return;
    };
    if path.exists() {
        return;
    }
    let source = source_file
        .map(|p| format!(" in {}", p.display()))
        .unwrap_or_default();
    let msg = format!("{label}: missing `{raw_ref}` -> {}{source}", path.display());
    if required {
        report.error(msg);
    } else {
        report.warn(msg);
    }
}

fn resolve_virtual_ref_path(
    project_dir: &Path,
    source_file: Option<&Path>,
    raw_ref: &str,
) -> Option<PathBuf> {
    if let Some(rest) = raw_ref.strip_prefix("res://") {
        let rel = virtual_path_without_suffix(rest);
        return Some(project_dir.join("res").join(rel));
    }
    if let Some(rest) = raw_ref.strip_prefix("dlc://") {
        let (dlc, rel) = rest.split_once('/')?;
        let rel = virtual_path_without_suffix(rel);
        if dlc == "self" {
            let source = source_file?;
            let dlc_root = source_dlc_root(project_dir, source)?;
            return Some(dlc_root.join(rel));
        }
        return Some(project_dir.join("dlcs").join(dlc).join(rel));
    }
    None
}

fn virtual_path_without_suffix(input: &str) -> &str {
    input.split_once(':').map(|(path, _)| path).unwrap_or(input)
}

fn source_dlc_root(project_dir: &Path, source_file: &Path) -> Option<PathBuf> {
    let dlcs_root = project_dir.join("dlcs");
    let rel = source_file.strip_prefix(&dlcs_root).ok()?;
    let dlc_name = rel.components().next()?;
    Some(dlcs_root.join(dlc_name))
}

fn extract_virtual_refs(text: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for quote in ['"', '\''] {
        for part in text.split(quote).skip(1).step_by(2) {
            if part.starts_with("res://") || part.starts_with("dlc://") {
                refs.push(part.to_string());
            }
        }
    }
    refs
}

fn collect_reference_text_files(project_dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for root in [project_dir.join("res"), project_dir.join("dlcs")] {
        collect_reference_text_files_recursive(&root, out)?;
    }
    Ok(())
}

fn collect_reference_text_files_recursive(
    dir: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(dir)
        .map_err(|err| format!("failed to read directory {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry
            .map_err(|err| format!("failed to read directory entry in {}: {err}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_reference_text_files_recursive(&path, out)?;
        } else if path.extension().is_some_and(is_reference_text_extension) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_reference_text_extension(ext: &std::ffi::OsStr) -> bool {
    matches!(
        ext.to_string_lossy().to_ascii_lowercase().as_str(),
        "scn" | "panim" | "panimtree" | "ppart" | "pmat" | "toml" | "ron" | "json"
    )
}

#[derive(Default)]
struct ScriptDoctorIndex {
    state_fields: HashSet<String>,
    methods: HashSet<String>,
}

fn validate_script_warnings(
    project_dir: &Path,
    report: &mut ValidationReport,
) -> Result<(), String> {
    let mut script_files = Vec::new();
    collect_project_script_files(project_dir, &mut script_files)?;
    if script_files.is_empty() {
        return Ok(());
    }

    let mut sources = Vec::new();
    for file in script_files {
        report.checked_files += 1;
        let text = fs::read_to_string(&file)
            .map_err(|err| format!("failed to read script {}: {err}", file.display()))?;
        sources.push((file, text));
    }

    let mut index = ScriptDoctorIndex::default();
    for (_, text) in &sources {
        index_script_source(text, &mut index);
    }

    for (file, text) in &sources {
        let mut seen_refs = HashSet::new();
        for raw_ref in extract_aggressive_virtual_refs(text) {
            if !seen_refs.insert(raw_ref.clone()) {
                continue;
            }
            report.checked_refs += 1;
            validate_script_virtual_ref(project_dir, file, &raw_ref, report);
        }
        validate_script_member_calls(file, text, &index, report);
    }

    Ok(())
}

fn collect_project_script_files(project_dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for root in [project_dir.join("res"), project_dir.join("dlcs")] {
        collect_rs_files_recursive(&root, out)?;
    }
    Ok(())
}

fn validate_script_virtual_ref(
    project_dir: &Path,
    source_file: &Path,
    raw_ref: &str,
    report: &mut ValidationReport,
) {
    let Some(path) = resolve_script_virtual_ref_path(project_dir, source_file, raw_ref) else {
        report.warn(format!(
            "script ref unsupported: {}: `{raw_ref}`",
            source_file.display()
        ));
        return;
    };
    if !path.exists() {
        report.warn(format!(
            "script ref missing: {}: `{raw_ref}` -> {} (if used as load path)",
            source_file.display(),
            path.display()
        ));
    }
}

fn resolve_script_virtual_ref_path(
    project_dir: &Path,
    source_file: &Path,
    raw_ref: &str,
) -> Option<PathBuf> {
    if let Some(rest) = raw_ref.strip_prefix("res://") {
        let rel = virtual_path_without_suffix(rest);
        return Some(project_dir.join("res").join(rel));
    }
    if let Some(rest) = raw_ref.strip_prefix("dlc://") {
        let (dlc, rel) = rest.split_once('/').unwrap_or((rest, ""));
        let rel = virtual_path_without_suffix(rel);
        if dlc == "self" {
            let dlc_root = source_dlc_root(project_dir, source_file)?;
            return Some(dlc_root.join(rel));
        }
        return Some(project_dir.join("dlcs").join(dlc).join(rel));
    }
    None
}

fn extract_aggressive_virtual_refs(text: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut i = 0usize;
    while i < text.len() {
        let rest = &text[i..];
        let res_pos = rest.find("res://");
        let dlc_pos = rest.find("dlc://");
        let rel = match (res_pos, dlc_pos) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => break,
        };
        let start = i + rel;
        let mut end = start;
        for (offset, ch) in text[start..].char_indices() {
            if is_virtual_ref_delim(ch) {
                break;
            }
            end = start + offset + ch.len_utf8();
        }
        let raw = trim_virtual_ref_tail(&text[start..end]);
        if !raw.is_empty() {
            refs.push(raw.to_string());
        }
        i = end.max(start + 1);
    }
    refs
}

fn is_virtual_ref_delim(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '"' | '\'' | '`' | ',' | ';' | ')' | '(' | ']' | '[' | '}' | '{' | '<' | '>'
        )
}

fn trim_virtual_ref_tail(raw: &str) -> &str {
    raw.trim_end_matches(['.', ':', '!', '?'])
}

fn index_script_source(text: &str, index: &mut ScriptDoctorIndex) {
    for state_name in parse_state_struct_names(text) {
        for field in parse_struct_field_names(text, &state_name) {
            index.state_fields.insert(field);
        }
    }
    for method in parse_script_method_names(text) {
        index.methods.insert(method);
    }
}

fn parse_state_struct_names(text: &str) -> Vec<String> {
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

fn is_state_attribute(line: &str) -> bool {
    matches!(line, "#[State]" | "#[state]")
}

fn parse_struct_name_from_line(line: &str) -> Option<String> {
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

fn parse_struct_field_names(text: &str, struct_name: &str) -> Vec<String> {
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
    for line in lines.iter().skip(start) {
        let line = strip_line_comment_for_doctor(line);
        if !opened {
            if let Some(pos) = line.find('{') {
                opened = true;
                depth = 1;
                if let Some(field) = parse_field_name_for_doctor(&line[pos + 1..]) {
                    fields.push(field);
                }
                depth += brace_delta_for_doctor(&line[pos + 1..]);
            }
        } else {
            if depth == 1
                && let Some(field) = parse_field_name_for_doctor(line)
            {
                fields.push(field);
            }
            depth += brace_delta_for_doctor(line);
        }
        if opened && depth <= 0 {
            break;
        }
    }
    fields
}

fn parse_field_name_for_doctor(line: &str) -> Option<String> {
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
    let name = without_vis.split_once(':')?.0.trim();
    if is_ident_for_doctor(name) {
        Some(name.to_string())
    } else {
        None
    }
}

fn parse_script_method_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    names.extend(parse_methods_macro_names(text));
    names.extend(parse_inherent_method_names(text));
    names.sort();
    names.dedup();
    names
}

fn parse_methods_macro_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for inner in find_macro_calls(text, "methods") {
        if let Some(body) = parse_methods_macro_body(&inner) {
            names.extend(parse_method_names_from_block(body));
        }
    }
    names
}

fn parse_methods_macro_body(inner: &str) -> Option<&str> {
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

fn parse_inherent_method_names(text: &str) -> Vec<String> {
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

fn parse_method_names_from_block(body: &str) -> Vec<String> {
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

fn validate_script_member_calls(
    file: &Path,
    text: &str,
    index: &ScriptDoctorIndex,
    report: &mut ValidationReport,
) {
    validate_var_member_calls(file, text, "get_var", &index.state_fields, report);
    validate_var_member_calls(file, text, "set_var", &index.state_fields, report);
    validate_method_member_calls(file, text, &index.methods, report);
}

fn validate_var_member_calls(
    file: &Path,
    text: &str,
    macro_name: &str,
    known_fields: &HashSet<String>,
    report: &mut ValidationReport,
) {
    for inner in find_macro_calls(text, macro_name) {
        let args = split_top_level_args(&inner);
        if args.len() < 3 {
            continue;
        }
        let target = normalize_arg_text(args[1]);
        if target == "ctx.id" {
            let replacement = if macro_name == "get_var" {
                "with_state!"
            } else {
                "with_state_mut!"
            };
            report.warn(format!(
                "script self access: {}: `{macro_name}!(..., ctx.id, ...)` can use `{replacement}`",
                file.display()
            ));
        }
        if let Some(member) = extract_member_literal(args[2], &["var"])
            && !known_fields.contains(&member)
        {
            report.warn(format!(
                "script member missing: {}: `{macro_name}!` references state field `{member}`, but no script defines it",
                file.display()
            ));
        }
    }
}

fn validate_method_member_calls(
    file: &Path,
    text: &str,
    known_methods: &HashSet<String>,
    report: &mut ValidationReport,
) {
    for inner in find_macro_calls(text, "call_method") {
        let args = split_top_level_args(&inner);
        if args.len() < 3 {
            continue;
        }
        let target = normalize_arg_text(args[1]);
        let member = extract_member_literal(args[2], &["method", "func"]);
        if target == "ctx.id" {
            let replacement = member
                .as_ref()
                .map(|name| format!("self.{name}(ctx, ...)"))
                .unwrap_or_else(|| "self.method_name(ctx, ...)".to_string());
            report.warn(format!(
                "script self access: {}: `call_method!(..., ctx.id, ...)` can use `{replacement}`",
                file.display()
            ));
        }
        if let Some(member) = member
            && !known_methods.contains(&member)
        {
            report.warn(format!(
                "script member missing: {}: `call_method!` references method `{member}`, but no script defines it",
                file.display()
            ));
        }
    }
}

fn extract_member_literal(arg: &str, macro_names: &[&str]) -> Option<String> {
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

fn parse_string_literal_value(input: &str) -> Option<String> {
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

fn find_macro_calls(text: &str, macro_name: &str) -> Vec<String> {
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
        calls.push(text[open + 1..close].to_string());
        search_from = close + 1;
    }
    calls
}

fn split_top_level_args(input: &str) -> Vec<&str> {
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

fn normalize_arg_text(input: &str) -> String {
    input.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn extract_brace_block_for_doctor(input: &str) -> Option<&str> {
    if !input.starts_with('{') {
        return None;
    }
    let end = find_matching_delim_for_doctor(input, 0, '{', '}')?;
    Some(&input[1..end])
}

fn parse_fn_name_for_doctor(input: &str) -> Option<String> {
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

fn strip_line_comment_for_doctor(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

fn brace_delta_for_doctor(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| *ch == '{').count() as i32;
    let closes = line.chars().filter(|ch| *ch == '}').count() as i32;
    opens - closes
}

fn paren_delta_for_doctor(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| *ch == '(').count() as i32;
    let closes = line.chars().filter(|ch| *ch == ')').count() as i32;
    opens - closes
}

fn is_ident_for_doctor(input: &str) -> bool {
    let mut chars = input.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[derive(Clone, Copy)]
enum DoctorLexMode {
    Code,
    LineComment,
    BlockComment,
    String,
    RawString(usize),
}

fn find_matching_delim_for_doctor(
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

fn raw_string_start_at_for_doctor(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
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

fn raw_string_end_at_for_doctor(bytes: &[u8], i: usize, hashes: usize) -> bool {
    if bytes.get(i) != Some(&b'"') {
        return false;
    }
    (0..hashes).all(|offset| bytes.get(i + 1 + offset) == Some(&b'#'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_project() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("perro_cli_doctor_test_{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn script_ref_missing_warns_and_existing_ref_stays_clean() {
        let project = temp_project();
        fs::create_dir_all(project.join("res/scripts")).unwrap();
        fs::write(project.join("res/existing.png"), b"x").unwrap();
        let source = project.join("res/scripts/main.rs");
        fs::write(
            &source,
            r#"
            const OK: &str = "res://existing.png";
            const MISSING: &str = "res://missing.png";
            "#,
        )
        .unwrap();

        let mut report = ValidationReport::default();
        validate_script_warnings(&project, &mut report).unwrap();

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 1);
        assert!(report.messages[0].contains("script ref missing"));
        assert!(report.messages[0].contains("res://missing.png"));
    }

    #[test]
    fn script_ref_dlc_self_resolves_from_source_dlc_root() {
        let project = temp_project();
        fs::create_dir_all(project.join("dlcs/cosmetic/scripts")).unwrap();
        fs::create_dir_all(project.join("dlcs/cosmetic/textures")).unwrap();
        fs::write(project.join("dlcs/cosmetic/textures/hat.png"), b"x").unwrap();
        fs::write(
            project.join("dlcs/cosmetic/scripts/main.rs"),
            r#"const HAT: &str = "dlc://self/textures/hat.png";"#,
        )
        .unwrap();

        let mut report = ValidationReport::default();
        validate_script_warnings(&project, &mut report).unwrap();

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
        assert_eq!(report.checked_refs, 1);
    }

    #[test]
    fn script_index_reads_state_fields_and_methods_macro() {
        let mut index = ScriptDoctorIndex::default();
        index_script_source(
            r#"
            #[State]
            pub struct PlayerState {
                pub hp: i32,
                energy: f32,
            }

            methods!({
                fn heal(
                    &self,
                    ctx: &mut ScriptContext<'_, API>,
                    amount: i32,
                ) {}

                pub fn ping(&self, ctx: &mut ScriptContext<'_, API>) {}
            });
            "#,
            &mut index,
        );

        assert!(index.state_fields.contains("hp"));
        assert!(index.state_fields.contains("energy"));
        assert!(index.methods.contains("heal"));
        assert!(index.methods.contains("ping"));
    }

    #[test]
    fn script_member_checks_warn_missing_members_and_ctx_id_hints() {
        let file = PathBuf::from("res/scripts/main.rs");
        let text = r#"
            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = get_var!(ctx.run, ctx.id, var!("missing_hp"));
                set_var!(ctx.run, ctx.id, "missing_flag", variant!(true));
                let _ = call_method!(ctx.run, ctx.id, method!("missing_method"), params![]);
            }
        "#;
        let mut index = ScriptDoctorIndex::default();
        index.state_fields.insert("hp".to_string());
        index.methods.insert("heal".to_string());
        let mut report = ValidationReport::default();

        validate_script_member_calls(&file, text, &index, &mut report);

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 6);
        assert!(report.messages.iter().any(|m| m.contains("with_state!")));
        assert!(
            report
                .messages
                .iter()
                .any(|m| m.contains("with_state_mut!"))
        );
        assert!(
            report
                .messages
                .iter()
                .any(|m| m.contains("self.missing_method(ctx, ...)"))
        );
        assert!(report.messages.iter().any(|m| m.contains("missing_hp")));
        assert!(report.messages.iter().any(|m| m.contains("missing_flag")));
        assert!(report.messages.iter().any(|m| m.contains("missing_method")));
    }
}
