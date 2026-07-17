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

mod script_parser;
use script_parser::*;
mod lexer;
use lexer::*;

#[cfg(test)]
#[path = "doctor/tests/mod.rs"]
mod tests;
