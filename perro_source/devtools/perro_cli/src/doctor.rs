use crate::project::{collect_rs_files_recursive, scripts_command};
use crate::{COLOR_RESET, COLOR_YELLOW, log_done, parse_flag_value, resolve_local_path};
use perro_project::{ProjectConfig, load_project_toml};
use perro_scene::{
    NodeFieldType, NodeRefHint, NodeType, Parser, SceneDoc, SceneNodeData, SceneObjectField,
    SceneValue,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub(crate) fn doctor_command(args: &[String], cwd: &Path) -> Result<(), String> {
    scripts_command(args, cwd).map_err(|err| format!("check failed: {err}"))?;

    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    validate_project_and_print(&project_dir)?;
    log_done("Project Valid");
    Ok(())
}

pub(crate) fn validate_project_and_print(project_dir: &Path) -> Result<(), String> {
    let report = validate_project(project_dir)?;
    report.print();
    if report.errors > 0 {
        return Err(format!("validation failed: {} issue(s)", report.errors));
    }
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
        self.messages
            .push(format!("{COLOR_YELLOW}[WARN]{COLOR_RESET} {msg}"));
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
        for text_ref in extract_virtual_refs(&text) {
            report.checked_refs += 1;
            validate_virtual_ref(
                project_dir,
                Some(&file),
                Some(text_ref.line),
                &text_ref.raw,
                &mut report,
            );
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
    source_line: Option<usize>,
    raw_ref: &str,
    report: &mut ValidationReport,
) {
    validate_named_virtual_ref(
        project_dir,
        source_file,
        source_line,
        raw_ref,
        raw_ref,
        true,
        report,
    );
}

fn validate_named_virtual_ref(
    project_dir: &Path,
    source_file: Option<&Path>,
    source_line: Option<usize>,
    label: &str,
    raw_ref: &str,
    required: bool,
    report: &mut ValidationReport,
) {
    let Some(path) = resolve_virtual_ref_path(project_dir, source_file, raw_ref) else {
        let source = format_source_location(project_dir, source_file, source_line);
        report.error(format!("{source}{label}: unsupported path `{raw_ref}`"));
        return;
    };
    if path.exists() {
        return;
    }
    let source = format_source_location(project_dir, source_file, source_line);
    let resolved = format_project_path(project_dir, &path);
    let msg = format!("{source}{label}: missing `{raw_ref}` -> {resolved}");
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

fn format_source_location(
    project_dir: &Path,
    source_file: Option<&Path>,
    source_line: Option<usize>,
) -> String {
    let Some(source_file) = source_file else {
        return String::new();
    };
    match source_line {
        Some(line) => format!("{}:{line}: ", format_project_path(project_dir, source_file)),
        None => format!("{}: ", format_project_path(project_dir, source_file)),
    }
}

fn format_project_path(project_dir: &Path, path: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(project_dir.join("res")) {
        return format!("res://{}", path_slash_for_doctor(rel));
    }
    let dlcs_root = project_dir.join("dlcs");
    if let Ok(rel) = path.strip_prefix(&dlcs_root) {
        let mut parts = rel.components();
        if let Some(dlc_name) = parts.next() {
            let dlc = dlc_name.as_os_str().to_string_lossy();
            let rest = parts.as_path();
            if rest.as_os_str().is_empty() {
                return format!("dlc://{dlc}/");
            }
            return format!("dlc://{dlc}/{}", path_slash_for_doctor(rest));
        }
    }
    path_slash_for_doctor(path)
}

fn path_slash_for_doctor(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn line_number_at(text: &str, byte_index: usize) -> usize {
    text[..byte_index.min(text.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

fn strip_comments_for_doctor(text: &str, hash_line_comment: bool) -> String {
    let mut out = Vec::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut mode = DoctorLexMode::Code;
    let mut escaped = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match mode {
            DoctorLexMode::Code => {
                if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
                    out.extend_from_slice(b"  ");
                    i += 2;
                    mode = DoctorLexMode::LineComment;
                    continue;
                }
                if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    out.extend_from_slice(b"  ");
                    i += 2;
                    mode = DoctorLexMode::BlockComment;
                    continue;
                }
                if hash_line_comment && b == b'#' {
                    out.push(b' ');
                    i += 1;
                    mode = DoctorLexMode::LineComment;
                    continue;
                }
                if let Some((prefix_len, hashes)) = raw_string_start_at_for_doctor(bytes, i) {
                    out.extend_from_slice(&bytes[i..i + prefix_len]);
                    i += prefix_len;
                    mode = DoctorLexMode::RawString(hashes);
                    continue;
                }
                if b == b'"' {
                    mode = DoctorLexMode::String;
                }
                out.push(b);
            }
            DoctorLexMode::LineComment => {
                if b == b'\n' {
                    out.push(b'\n');
                    mode = DoctorLexMode::Code;
                } else {
                    out.push(b' ');
                }
            }
            DoctorLexMode::BlockComment => {
                if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    out.extend_from_slice(b"  ");
                    i += 2;
                    mode = DoctorLexMode::Code;
                    continue;
                }
                out.push(if b == b'\n' { b'\n' } else { b' ' });
            }
            DoctorLexMode::String => {
                out.push(b);
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    mode = DoctorLexMode::Code;
                }
            }
            DoctorLexMode::RawString(hashes) => {
                out.push(b);
                if raw_string_end_at_for_doctor(bytes, i, hashes) {
                    for offset in 0..hashes {
                        if let Some(hash) = bytes.get(i + 1 + offset) {
                            out.push(*hash);
                        }
                    }
                    i += hashes + 1;
                    mode = DoctorLexMode::Code;
                    continue;
                }
            }
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

struct TextRef {
    raw: String,
    line: usize,
}

fn extract_virtual_refs(text: &str) -> Vec<TextRef> {
    let text = strip_comments_for_doctor(text, true);
    let text = text.as_str();
    let mut refs = Vec::new();
    for quote in ['"', '\''] {
        let mut search_from = 0usize;
        for part in text.split(quote).skip(1).step_by(2) {
            if (part.starts_with("res://") || part.starts_with("dlc://"))
                && let Some(rel) = text[search_from..].find(part)
            {
                let start = search_from + rel;
                refs.push(TextRef {
                    raw: part.to_string(),
                    line: line_number_at(text, start),
                });
                search_from = start + part.len();
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
        "scn" | "panim" | "panimtree" | "ppart" | "pmat" | "uistyle" | "toml" | "ron" | "json"
    )
}

#[derive(Default)]
struct ScriptDoctorIndex {
    state_types: HashSet<String>,
    state_fields: HashSet<String>,
    state_field_types: HashMap<String, HashSet<String>>,
    state_field_owners: HashMap<String, String>,
    state_field_defs: HashMap<String, Vec<DoctorField>>,
    script_state_field_defs: HashMap<PathBuf, HashMap<String, DoctorField>>,
    custom_type_fields: HashMap<String, Vec<DoctorField>>,
    methods: HashSet<String>,
    signal_emits: HashMap<String, Vec<SignalUse>>,
    signal_connects: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DoctorField {
    name: String,
    ty: String,
    node_ref_types: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SignalUse {
    file: PathBuf,
    line: usize,
}

fn validate_script_warnings(
    project_dir: &Path,
    report: &mut ValidationReport,
) -> Result<(), String> {
    let mut script_files = Vec::new();
    collect_project_script_files(project_dir, &mut script_files)?;

    let mut sources = Vec::new();
    for file in script_files {
        report.checked_files += 1;
        let text = fs::read_to_string(&file)
            .map_err(|err| format!("failed to read script {}: {err}", file.display()))?;
        let text = strip_comments_for_doctor(&text, false);
        sources.push((file, text));
    }

    let mut index = ScriptDoctorIndex::default();
    for (file, text) in &sources {
        index_script_source(file, text, &mut index);
    }

    collect_resource_signal_emits(project_dir, &mut index)?;

    for (file, text) in &sources {
        let mut seen_refs = HashSet::new();
        for text_ref in extract_aggressive_virtual_refs(text) {
            if !seen_refs.insert(text_ref.raw.clone()) {
                continue;
            }
            report.checked_refs += 1;
            validate_script_virtual_ref(project_dir, file, text_ref.line, &text_ref.raw, report);
        }
        validate_script_member_calls(project_dir, file, text, &index, report);
        index_script_signal_uses(file, text, &mut index);
    }

    validate_signal_emits(project_dir, &index, report);
    validate_node_ref_type_warnings(project_dir, &index, report)?;

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
    source_line: usize,
    raw_ref: &str,
    report: &mut ValidationReport,
) {
    let Some(path) = resolve_script_virtual_ref_path(project_dir, source_file, raw_ref) else {
        let source = format_source_location(project_dir, Some(source_file), Some(source_line));
        report.warn(format!("script ref unsupported: {source}`{raw_ref}`"));
        return;
    };
    if !path.exists() {
        let source = format_source_location(project_dir, Some(source_file), Some(source_line));
        let resolved = format_project_path(project_dir, &path);
        report.warn(format!(
            "script ref missing: {source}`{raw_ref}` -> {resolved} (if used as load path)"
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

fn validate_node_ref_type_warnings(
    project_dir: &Path,
    index: &ScriptDoctorIndex,
    report: &mut ValidationReport,
) -> Result<(), String> {
    let mut files = Vec::new();
    collect_scene_files_recursive(&project_dir.join("res"), &mut files)?;
    collect_scene_files_recursive(&project_dir.join("dlcs"), &mut files)?;
    for file in files {
        let text = fs::read_to_string(&file)
            .map_err(|err| format!("failed to read scene {}: {err}", file.display()))?;
        let Ok(scene) = Parser::new(&text).try_parse_scene() else {
            continue;
        };
        let doc = SceneDoc::from_scene(scene);
        validate_scene_doc_node_refs(project_dir, &file, &doc, index, report);
    }
    Ok(())
}

fn collect_scene_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
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
            collect_scene_files_recursive(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "scn") {
            out.push(path);
        }
    }
    Ok(())
}

fn validate_scene_doc_node_refs(
    project_dir: &Path,
    file: &Path,
    doc: &SceneDoc,
    index: &ScriptDoctorIndex,
    report: &mut ValidationReport,
) {
    let node_types = doc
        .scene
        .nodes
        .iter()
        .map(|node| {
            (
                doc.scene.key_name_or_id(node.key).to_string(),
                node.data.node_type,
            )
        })
        .collect::<HashMap<_, _>>();
    for node in doc.scene.nodes.iter() {
        let node_name = doc.scene.key_name_or_id(node.key).to_string();
        let script_defs = node
            .script
            .as_deref()
            .and_then(|raw| resolve_script_virtual_ref_path(project_dir, file, raw))
            .and_then(|path| index.script_state_field_defs.get(&path));
        let mut ctx = NodeRefValidationCtx {
            project_dir,
            file,
            node_types: &node_types,
            report,
        };
        validate_builtin_node_ref_fields(&mut ctx, &node_name, &node.data);
        validate_script_var_node_ref_fields(
            &mut ctx,
            &node_name,
            node.script_vars.as_ref(),
            script_defs,
            index,
        );
    }
}

struct NodeRefValidationCtx<'a> {
    project_dir: &'a Path,
    file: &'a Path,
    node_types: &'a HashMap<String, NodeType>,
    report: &'a mut ValidationReport,
}

fn validate_builtin_node_ref_fields(
    ctx: &mut NodeRefValidationCtx<'_>,
    node_name: &str,
    data: &SceneNodeData,
) {
    for (field_name, value) in data.fields.iter() {
        let Some(field) = perro_scene::scene_node_field(data.node_type, field_name.as_ref()) else {
            continue;
        };
        validate_scene_value_node_ref_hint(
            ctx,
            &format!("{node_name}.{}", field.name),
            value,
            &field.ty,
        );
    }
    if let Some(base) = data.base_ref() {
        validate_builtin_node_ref_fields(ctx, node_name, base);
    }
}

fn validate_scene_value_node_ref_hint(
    ctx: &mut NodeRefValidationCtx<'_>,
    label: &str,
    value: &SceneValue,
    ty: &NodeFieldType,
) {
    match (ty, value) {
        (NodeFieldType::NodeRef(hint), SceneValue::Key(key)) => {
            validate_one_node_ref_hint(ctx, label, key.as_ref(), *hint);
        }
        (NodeFieldType::Array(item_ty), SceneValue::Array(items)) => {
            for (idx, item) in items.iter().enumerate() {
                validate_scene_value_node_ref_hint(ctx, &format!("{label}[{idx}]"), item, item_ty);
            }
        }
        (NodeFieldType::Object(fields), SceneValue::Object(values)) => {
            for field in fields {
                let Some((_, item)) = values.iter().find(|(name, _)| name.as_ref() == field.name)
                else {
                    continue;
                };
                validate_scene_value_node_ref_hint(
                    ctx,
                    &format!("{label}.{}", field.name),
                    item,
                    &field.ty,
                );
            }
        }
        _ => {}
    }
}

fn validate_script_var_node_ref_fields(
    ctx: &mut NodeRefValidationCtx<'_>,
    node_name: &str,
    fields: &[SceneObjectField],
    script_defs: Option<&HashMap<String, DoctorField>>,
    index: &ScriptDoctorIndex,
) {
    for (name, value) in fields {
        let label = format!("{node_name}.script_vars.{}", name.as_ref());
        // node w/ resolved script: validate against that script's own defs
        if let Some(defs) = script_defs {
            if let Some(field) = defs.get(name.as_ref()) {
                validate_script_value_node_ref_hint(ctx, &label, value, field, index);
            }
            continue;
        }
        // no attached script resolved: several scripts may declare state
        // fields with the same name but different types; stay quiet if any
        // candidate def validates cleanly
        let Some(candidates) = index.state_field_defs.get(name.as_ref()) else {
            continue;
        };
        let mut best_bad: Option<ValidationReport> = None;
        let mut any_clean = false;
        for field in candidates {
            let mut scratch = ValidationReport::default();
            let mut scratch_ctx = NodeRefValidationCtx {
                project_dir: ctx.project_dir,
                file: ctx.file,
                node_types: ctx.node_types,
                report: &mut scratch,
            };
            validate_script_value_node_ref_hint(&mut scratch_ctx, &label, value, field, index);
            if scratch.warnings == 0 && scratch.errors == 0 {
                any_clean = true;
                break;
            }
            if best_bad
                .as_ref()
                .is_none_or(|bad| scratch.warnings < bad.warnings)
            {
                best_bad = Some(scratch);
            }
        }
        if any_clean {
            continue;
        }
        if let Some(bad) = best_bad {
            ctx.report.warnings += bad.warnings;
            ctx.report.errors += bad.errors;
            ctx.report.messages.extend(bad.messages);
        }
    }
}

fn validate_script_value_node_ref_hint(
    ctx: &mut NodeRefValidationCtx<'_>,
    label: &str,
    value: &SceneValue,
    field: &DoctorField,
    index: &ScriptDoctorIndex,
) {
    if is_node_id_type_for_doctor(&field.ty)
        && let SceneValue::Key(key) = value
    {
        let hint = doctor_node_ref_hint(&field.node_ref_types);
        validate_one_node_ref_hint(ctx, label, key.as_ref(), hint);
        return;
    }
    if let SceneValue::Object(values) = value
        && let Some(nested) = index.custom_type_fields.get(&field.ty)
    {
        for (name, item) in values.iter() {
            let Some(nested_field) = nested.iter().find(|field| field.name == name.as_ref()) else {
                continue;
            };
            validate_script_value_node_ref_hint(
                ctx,
                &format!("{label}.{}", name.as_ref()),
                item,
                nested_field,
                index,
            );
        }
    }
}

fn validate_one_node_ref_hint(
    ctx: &mut NodeRefValidationCtx<'_>,
    label: &str,
    raw_ref: &str,
    hint: NodeRefHint,
) {
    if hint.allowed.is_empty() {
        return;
    }
    let target = raw_ref.trim().trim_start_matches('@');
    if matches!(target, "" | "null" | "none" | "-") {
        return;
    }
    let Some(target_type) = ctx.node_types.get(target).copied() else {
        return;
    };
    if hint.allows(target_type) {
        return;
    }
    let source = format_source_location(ctx.project_dir, Some(ctx.file), None);
    ctx.report.warn(format!(
        "node ref type mismatch: {source}{label} wants {}, got {} @{}",
        hint.label(),
        target_type.name(),
        target
    ));
}

fn doctor_node_ref_hint(types: &[String]) -> NodeRefHint {
    let allowed = types
        .iter()
        .filter_map(|ty| NodeType::from_str(ty).ok())
        .collect::<Vec<_>>();
    if allowed.is_empty() {
        return NodeRefHint::any();
    }
    NodeRefHint::many(Box::leak(allowed.into_boxed_slice()))
}

fn is_node_id_type_for_doctor(ty: &str) -> bool {
    ty == "NodeID"
}

fn extract_aggressive_virtual_refs(text: &str) -> Vec<TextRef> {
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
            refs.push(TextRef {
                raw: raw.to_string(),
                line: line_number_at(text, start),
            });
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

fn index_script_source(file: &Path, text: &str, index: &mut ScriptDoctorIndex) {
    for type_name in parse_struct_names(text) {
        let fields = parse_struct_fields(text, &type_name);
        if !fields.is_empty() {
            index.custom_type_fields.insert(type_name, fields);
        }
    }
    for type_name in parse_enum_names(text) {
        let fields = parse_enum_fields(text, &type_name);
        if !fields.is_empty() {
            index.custom_type_fields.insert(type_name, fields);
        }
    }
    for state_name in parse_state_struct_names(text) {
        index.state_types.insert(state_name.clone());
        for field in parse_struct_fields(text, &state_name) {
            index
                .state_field_types
                .entry(field.name.clone())
                .or_default()
                .insert(field.ty.clone());
            index
                .state_field_owners
                .insert(field.name.clone(), state_name.clone());
            let defs = index
                .state_field_defs
                .entry(field.name.clone())
                .or_default();
            if !defs.contains(&field) {
                defs.push(field.clone());
            }
            index
                .script_state_field_defs
                .entry(file.to_path_buf())
                .or_default()
                .insert(field.name.clone(), field.clone());
            index.state_fields.insert(field.name);
        }
    }
    for method in parse_script_method_names(text) {
        index.methods.insert(method);
    }
}

fn index_script_signal_uses(file: &Path, text: &str, index: &mut ScriptDoctorIndex) {
    for call in find_macro_calls_with_lines(text, "signal_emit") {
        let args = split_top_level_args(&call.inner);
        if args.len() < 2 {
            continue;
        }
        if let Some(signal) = extract_member_literal(args[1], &["signal"]) {
            index_signal_emit(index, signal, file, call.line);
        }
    }
    for call in find_macro_calls_with_lines(text, "signal_connect") {
        let args = split_top_level_args(&call.inner);
        if args.len() < 3 {
            continue;
        }
        if let Some(signal) = extract_member_literal(args[2], &["signal"]) {
            index.signal_connects.insert(signal);
        }
    }
    for call in find_macro_calls_with_lines(text, "signal_connect_many") {
        let args = split_top_level_args(&call.inner);
        if args.len() < 3 {
            continue;
        }
        for signal in extract_member_literals(args[2], &["signal"]) {
            index.signal_connects.insert(signal);
        }
    }
}

fn collect_resource_signal_emits(
    project_dir: &Path,
    index: &mut ScriptDoctorIndex,
) -> Result<(), String> {
    let mut files = Vec::new();
    collect_reference_text_files(project_dir, &mut files)?;
    for file in files {
        let text = fs::read_to_string(&file)
            .map_err(|err| format!("failed to read {}: {err}", file.display()))?;
        let text = strip_comments_for_doctor(&text, true);
        for signal_ref in extract_resource_signal_emits(&text) {
            index_signal_emit(index, signal_ref.raw, &file, signal_ref.line);
        }
    }
    Ok(())
}

fn extract_resource_signal_emits(text: &str) -> Vec<TextRef> {
    let mut refs = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let line_no = line_index + 1;
        if line.contains("_signals")
            && let Some((_, rhs)) = line.split_once('=')
        {
            refs.extend(
                extract_string_literals_for_doctor(rhs)
                    .into_iter()
                    .map(|raw| TextRef { raw, line: line_no }),
            );
        }
        if line.contains("emit_signal")
            && line.contains("name")
            && let Some(signal) = extract_named_emit_signal_for_doctor(line)
        {
            refs.push(TextRef {
                raw: signal,
                line: line_no,
            });
        }
    }
    refs
}

fn extract_named_emit_signal_for_doctor(line: &str) -> Option<String> {
    let name_pos = line.find("name")?;
    let eq_rel = line[name_pos..].find('=')?;
    let rhs = &line[name_pos + eq_rel + 1..];
    extract_string_literals_for_doctor(rhs).into_iter().next()
}

fn extract_string_literals_for_doctor(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            let mut escaped = false;
            while i < bytes.len() {
                let b = bytes[i];
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    if let Some(value) = parse_string_literal_value(&input[start..=i]) {
                        out.push(value);
                    }
                    break;
                }
                i += 1;
            }
        }
        i += 1;
    }
    out
}

fn index_signal_emit(index: &mut ScriptDoctorIndex, signal: String, file: &Path, line: usize) {
    index
        .signal_emits
        .entry(signal)
        .or_default()
        .push(SignalUse {
            file: file.to_path_buf(),
            line,
        });
}

fn validate_signal_emits(
    project_dir: &Path,
    index: &ScriptDoctorIndex,
    report: &mut ValidationReport,
) {
    let mut names: Vec<_> = index.signal_emits.keys().collect();
    names.sort();
    for signal in names {
        if index.signal_connects.contains(signal) {
            continue;
        }
        if let Some(uses) = index.signal_emits.get(signal) {
            for signal_use in uses {
                let source = format_source_location(
                    project_dir,
                    Some(&signal_use.file),
                    Some(signal_use.line),
                );
                let source = source.trim_end_matches(": ");
                report.warn(format!(
                    "signal: {signal} is emitted at {source} but never connected anywhere"
                ));
            }
        }
    }
}

fn parse_struct_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| parse_struct_name_from_line(line.trim()))
        .collect()
}

fn parse_enum_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| parse_enum_name_from_line(line.trim()))
        .collect()
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

fn parse_enum_name_from_line(line: &str) -> Option<String> {
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

fn parse_struct_fields(text: &str, struct_name: &str) -> Vec<DoctorField> {
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

fn parse_enum_fields(text: &str, enum_name: &str) -> Vec<DoctorField> {
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

fn parse_enum_line_fields(line: &str) -> Vec<DoctorField> {
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

fn parse_field_for_doctor(line: &str, node_ref_types: Vec<String>) -> Option<DoctorField> {
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

fn parse_node_ref_attr_for_doctor(line: &str) -> Option<Vec<String>> {
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

fn normalize_type_name_for_doctor(input: &str) -> String {
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

fn validate_state_access_calls(
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

fn extract_state_type_arg(arg: &str) -> Option<String> {
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

fn validate_var_member_calls(
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

fn var_self_access_replacement(
    index: &ScriptDoctorIndex,
    macro_name: &str,
    member: Option<&str>,
    args: &[&str],
) -> String {
    let Some(member) = member else {
        return if macro_name == "get_var" {
            "with_state!(ctx.run, StateType, ctx.id, |state| state.field)".to_string()
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
        format!("with_state!(ctx.run, {state_type}, ctx.id, |state| state.{member})")
    } else {
        let value = args.get(3).map(|arg| arg.trim()).unwrap_or("value");
        format!("with_state_mut!(ctx.run, {state_type}, ctx.id, |state| state.{member} = {value})")
    }
}

fn known_var_member(index: &ScriptDoctorIndex, member: &str) -> bool {
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

fn validate_method_member_calls(
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

fn extract_member_literals(arg: &str, macro_names: &[&str]) -> Vec<String> {
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

struct MacroCall {
    inner: String,
    line: usize,
}

fn find_macro_calls(text: &str, macro_name: &str) -> Vec<String> {
    find_macro_calls_with_lines(text, macro_name)
        .into_iter()
        .map(|call| call.inner)
        .collect()
}

fn find_macro_calls_with_lines(text: &str, macro_name: &str) -> Vec<MacroCall> {
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
        // Nanos alone collide on platforms w/ coarse SystemTime tick (macOS):
        // parallel tests land on same dir + cross-contaminate scans. Add pid +
        // atomic counter -> unique per call regardless of clock granularity.
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("perro_cli_doctor_test_{stamp}_{pid}_{seq}"));
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
        assert!(report.messages[0].contains("res://scripts/main.rs:3"));
        assert!(report.messages[0].contains("res://missing.png"));
        assert!(!report.messages[0].contains(&project.to_string_lossy().to_string()));
    }

    #[test]
    fn doctor_ignores_scene_refs_in_comments() {
        let project = temp_project();
        fs::create_dir_all(project.join("res")).unwrap();
        fs::write(
            project.join("res/main.scn"),
            r##"
            # texture = "res://missing_hash.png"
            // texture = "res://missing_slash.png"
            color = "#ffeeaa"
            "##,
        )
        .unwrap();

        let mut report = ValidationReport::default();
        let mut files = Vec::new();
        collect_reference_text_files(&project, &mut files).unwrap();
        for file in files {
            let text = fs::read_to_string(&file).unwrap();
            for text_ref in extract_virtual_refs(&text) {
                validate_virtual_ref(
                    &project,
                    Some(&file),
                    Some(text_ref.line),
                    &text_ref.raw,
                    &mut report,
                );
            }
        }

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
    }

    #[test]
    fn doctor_ignores_script_refs_and_macros_in_comments() {
        let project = temp_project();
        fs::create_dir_all(project.join("res/scripts")).unwrap();
        fs::write(
            project.join("res/scripts/main.rs"),
            r#"
            #[State]
            struct PlayerState {
                hp: i32,
            }

            fn run(ctx: &mut ScriptContext<'_, API>) {
                // let _ = get_var!(ctx.run, ctx.id, var!("missing_hp"));
                // let _ = "res://missing_script_ref.png";
                /*
                set_var!(ctx.run, ctx.id, var!("missing_flag"), variant!(true));
                let _ = "res://missing_block_ref.png";
                */
            }
            "#,
        )
        .unwrap();

        let mut report = ValidationReport::default();
        validate_script_warnings(&project, &mut report).unwrap();

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
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
            Path::new("res/scripts/main.rs"),
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
        index
            .state_field_owners
            .insert("hp".to_string(), "PlayerState".to_string());
        index.methods.insert("heal".to_string());
        let mut report = ValidationReport::default();

        validate_script_member_calls(Path::new(""), &file, text, &index, &mut report);

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
                .any(|m| m.contains("with_state_mut!(ctx.run, StateType, ctx.id, |state| state.missing_flag = variant!(true))"))
        );
        assert!(
            report
                .messages
                .iter()
                .any(|m| m.contains("self.missing_method(ctx, params...)"))
        );
        assert!(report.messages.iter().any(|m| m.contains("missing_hp")));
        assert!(report.messages.iter().any(|m| m.contains("missing_flag")));
        assert!(report.messages.iter().any(|m| m.contains("missing_method")));
    }

    #[test]
    fn script_state_access_checks_require_state_attribute() {
        let file = PathBuf::from("res/scripts/main.rs");
        let source = r#"
            #[State]
            struct PlayerState {
                hp: i32,
            }

            struct HelperState {
                hp: i32,
            }

            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = with_state!(ctx.run, PlayerState, ctx.id, |state| state.hp);
                let _ = with_state!(ctx.run, crate::PlayerState, ctx.id, |state| state.hp);
                let _ = with_state_mut!(ctx.run, HelperState, ctx.id, |state| state.hp += 1);
            }
        "#;
        let mut index = ScriptDoctorIndex::default();
        index_script_source(Path::new("res/scripts/main.rs"), source, &mut index);
        let mut report = ValidationReport::default();

        validate_script_member_calls(Path::new(""), &file, source, &index, &mut report);

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 1);
        assert!(report.messages[0].contains("script state missing"));
        assert!(report.messages[0].contains("with_state_mut!"));
        assert!(report.messages[0].contains("HelperState"));
        assert!(!report.messages[0].contains("PlayerState"));
    }

    #[test]
    fn script_member_checks_walk_nested_custom_state_fields() {
        let file = PathBuf::from("res/scripts/main.rs");
        let source = r#"
            pub struct Aim {
                pub axis: Axis,
            }

            pub enum Axis {
                Local { x: f32, y: f32 },
                World {
                    dir: Direction,
                },
            }

            pub struct Direction {
                pub yaw: f32,
            }

            #[State]
            pub struct SpinnerState {
                pub aim: Aim,
                pub hp: i32,
            }

            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = get_var!(ctx.run, other, var!("aim.axis.dir.yaw"));
                let _ = get_var!(ctx.run, other, var!("aim.axis.dir.pitch"));
                let _ = get_var!(ctx.run, other, var!("hp.value"));
            }
        "#;
        let mut index = ScriptDoctorIndex::default();
        index_script_source(Path::new("res/scripts/main.rs"), source, &mut index);
        let mut report = ValidationReport::default();

        validate_script_member_calls(Path::new(""), &file, source, &index, &mut report);

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 2);
        assert!(
            report
                .messages
                .iter()
                .any(|m| m.contains("aim.axis.dir.pitch"))
        );
        assert!(report.messages.iter().any(|m| m.contains("hp.value")));
        assert!(
            report
                .messages
                .iter()
                .all(|m| !m.contains("aim.axis.dir.yaw"))
        );
    }

    #[test]
    fn script_member_checks_accept_shared_state_field_name_across_scripts() {
        let file = PathBuf::from("res/scripts/golf_manager.rs");
        let golf_source = r#"
            pub struct GolfAgentConfigState {
                pub club_index: i32,
                pub right_handed: bool,
                pub orbit_yaw_degrees: f32,
            }

            #[State]
            pub struct GolfAgentState {
                pub config: GolfAgentConfigState,
            }
        "#;
        let volleyball_source = r#"
            pub struct VolleyballConfigState {
                pub serve_power: f32,
            }

            #[State]
            pub struct VolleyballAgentState {
                pub config: VolleyballConfigState,
            }
        "#;
        let caller_source = r#"
            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = get_var!(ctx.run, agent, var!("config.club_index"));
                let _ = get_var!(ctx.run, agent, var!("config.right_handed"));
                let _ = get_var!(ctx.run, agent, var!("config.serve_power"));
                let _ = get_var!(ctx.run, agent, var!("config.missing_member"));
            }
        "#;
        let mut index = ScriptDoctorIndex::default();
        index_script_source(
            Path::new("res/scripts/golf_agent.rs"),
            golf_source,
            &mut index,
        );
        index_script_source(
            Path::new("res/scripts/volleyball_agent.rs"),
            volleyball_source,
            &mut index,
        );
        let mut report = ValidationReport::default();

        validate_script_member_calls(Path::new(""), &file, caller_source, &index, &mut report);

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 1);
        assert!(report.messages[0].contains("config.missing_member"));

        // same lookups stay valid when scripts index in the opposite order
        let mut index = ScriptDoctorIndex::default();
        index_script_source(
            Path::new("res/scripts/volleyball_agent.rs"),
            volleyball_source,
            &mut index,
        );
        index_script_source(
            Path::new("res/scripts/golf_agent.rs"),
            golf_source,
            &mut index,
        );
        let mut report = ValidationReport::default();

        validate_script_member_calls(Path::new(""), &file, caller_source, &index, &mut report);

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 1);
        assert!(report.messages[0].contains("config.missing_member"));
    }

    #[test]
    fn signal_emit_without_connect_warns_once_per_emit_location() {
        let file = PathBuf::from("res/scripts/main.rs");
        let source = r#"
            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = signal_emit!(ctx, signal!("loose_signal"));
                let _ = signal_emit!(ctx, signal!("wired_signal"), params![1_i32]);
                let _ = signal_connect!(ctx, ctx.id, signal!("wired_signal"), func!("on_wired"));
            }
        "#;
        let mut index = ScriptDoctorIndex::default();
        index_script_signal_uses(&file, source, &mut index);
        let mut report = ValidationReport::default();

        validate_signal_emits(Path::new(""), &index, &mut report);

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 1);
        assert!(report.messages[0].contains("signal: loose_signal"));
        assert!(report.messages[0].contains("never connected anywhere"));
        assert!(!report.messages[0].contains("wired_signal"));
    }

    #[test]
    fn signal_connect_without_emit_stays_clean() {
        let file = PathBuf::from("res/scripts/main.rs");
        let source = r#"
            fn ready(ctx: &mut ScriptContext<'_, API>) {
                let _ = signal_connect!(ctx, ctx.id, signal!("future_button_click"), func!("on_click"));
            }
        "#;
        let mut index = ScriptDoctorIndex::default();
        index_script_signal_uses(&file, source, &mut index);
        let mut report = ValidationReport::default();

        validate_signal_emits(Path::new(""), &index, &mut report);

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
    }

    #[test]
    fn signal_connect_many_counts_as_connected() {
        let file = PathBuf::from("res/scripts/main.rs");
        let source = r#"
            fn ready(ctx: &mut ScriptContext<'_, API>) {
                let _ = signal_emit!(ctx, signal!("wired_a"));
                let _ = signal_emit!(ctx, signal!("wired_b"));
                let _ = signal_connect_many!(
                    ctx,
                    ctx.id,
                    [signal!("wired_a"), signal!("wired_b")],
                    [func!("on_signal")]
                );
            }
        "#;
        let mut index = ScriptDoctorIndex::default();
        index_script_signal_uses(&file, source, &mut index);
        let mut report = ValidationReport::default();

        validate_signal_emits(Path::new(""), &index, &mut report);

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
    }

    #[test]
    fn resource_signal_fields_count_as_emits() {
        let text = r#"
            clicked_signals = ["play_clicked", "any_button_clicked"]
            emit_signal = { name="step", params=[0] }
        "#;

        let refs = extract_resource_signal_emits(text);

        assert_eq!(
            refs.iter().map(|r| r.raw.as_str()).collect::<Vec<_>>(),
            vec!["play_clicked", "any_button_clicked", "step"]
        );
    }

    #[test]
    fn resource_signal_emit_warns_without_scripts() {
        let project = temp_project();
        fs::create_dir_all(project.join("res")).unwrap();
        fs::write(
            project.join("res/ui.scn"),
            r#"clicked_signals = ["play_clicked"]"#,
        )
        .unwrap();

        let mut report = ValidationReport::default();
        validate_script_warnings(&project, &mut report).unwrap();

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 1);
        assert!(report.messages[0].contains("signal: play_clicked"));
        assert!(report.messages[0].contains("res://ui.scn:1"));
    }

    #[test]
    fn node_ref_type_hints_warn_for_script_vars_and_builtin_fields() {
        let project = temp_project();
        fs::create_dir_all(project.join("res/scripts")).unwrap();
        fs::write(
            project.join("res/scripts/player.rs"),
            r#"
            use perro_api::prelude::*;

            #[State]
            pub struct PlayerState {
                #[expose]
                #[node_ref(Camera3D)]
                camera: NodeID,
            }
            "#,
        )
        .unwrap();
        fs::write(
            project.join("res/main.scn"),
            r#"
            $root = @Player

            [Player]
            script = "res://scripts/player.rs"
            script_vars = { camera = @Mesh }
            [Node3D/]
            [/Player]

            [Stream]
            [UiCameraStream]
                camera = @Mesh
            [/UiCameraStream]
            [/Stream]

            [Mesh]
            [MeshInstance3D/]
            [/Mesh]
            "#,
        )
        .unwrap();

        let mut report = ValidationReport::default();
        validate_script_warnings(&project, &mut report).unwrap();

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 2);
        assert!(
            report
                .messages
                .iter()
                .any(|msg| msg.contains("Player.script_vars.camera wants Node(Camera3D)"))
        );
        assert!(
            report
                .messages
                .iter()
                .any(|msg| msg.contains("Stream.camera wants Node(Camera2D|Camera3D|Webcam)"))
        );
    }

    #[test]
    fn node_ref_hints_resolve_by_attached_script_for_shared_field_names() {
        let project = temp_project();
        fs::create_dir_all(project.join("res/scripts")).unwrap();
        fs::write(
            project.join("res/scripts/golf_agent.rs"),
            r#"
            use perro_api::prelude::*;

            pub struct GolfConfig {
                #[expose]
                #[node_ref(Camera3D)]
                pub orbit_camera: NodeID,
            }

            #[State]
            pub struct GolfAgentState {
                #[expose]
                pub config: GolfConfig,
            }
            "#,
        )
        .unwrap();
        fs::write(
            project.join("res/scripts/volleyball_agent.rs"),
            r#"
            use perro_api::prelude::*;

            pub struct VolleyballConfig {
                #[expose]
                pub serve_power: f32,
            }

            #[State]
            pub struct VolleyballAgentState {
                #[expose]
                pub config: VolleyballConfig,
            }
            "#,
        )
        .unwrap();
        fs::write(
            project.join("res/main.scn"),
            r#"
            $root = @Golfer

            [Golfer]
            script = "res://scripts/golf_agent.rs"
            script_vars = { config = { orbit_camera = @Cam } }
            [Node3D/]
            [/Golfer]

            [BadGolfer]
            script = "res://scripts/golf_agent.rs"
            script_vars = { config = { orbit_camera = @Mesh } }
            [Node3D/]
            [/BadGolfer]

            [Cam]
            [Camera3D/]
            [/Cam]

            [Mesh]
            [MeshInstance3D/]
            [/Mesh]
            "#,
        )
        .unwrap();

        let mut report = ValidationReport::default();
        validate_script_warnings(&project, &mut report).unwrap();

        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 1, "messages: {:?}", report.messages);
        assert!(
            report.messages[0]
                .contains("BadGolfer.script_vars.config.orbit_camera wants Node(Camera3D)")
        );
    }
}
