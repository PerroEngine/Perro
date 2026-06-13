use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_editor_main_rs::EditorState;
use crate::scripts_scene_editor_animation_rs::*;
use crate::scripts_scene_editor_viewport_rs::*;
use crate::scripts_ui_editor_ui_rs::*;
use perro_api::prelude::*;
use perro_api::scene::{Parser, SceneDoc, SceneFieldName, SceneValue};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;

const MAX_INSPECTOR_DEPTH: usize = 4;

#[derive(Clone)]
pub struct InspectorValueRow {
    pub path: Vec<ValuePathStep>,
    pub path_key: String,
    pub name: String,
    pub kind: String,
    pub value: String,
    pub components: Vec<String>,
    pub color_preview: Option<String>,
    pub enum_options: Vec<String>,
    pub editable: bool,
    pub expandable: bool,
}

#[derive(Clone)]
pub enum ValuePathStep {
    Root(usize),
    Field(String),
    Index(usize),
}

struct ValueRowContext<'a> {
    expanded_paths: &'a [String],
    color_paths: &'a [String],
    node_paths: &'a [String],
    enum_options: &'a BTreeMap<String, Vec<String>>,
}

pub fn inspector_script_var_rows(
    fields: &[(SceneFieldName, SceneValue)],
    expanded_paths: &[String],
) -> Vec<InspectorValueRow> {
    inspector_script_var_rows_with_color_paths(fields, expanded_paths, &[], &[], &BTreeMap::new())
}

pub fn inspector_script_var_rows_with_color_paths(
    fields: &[(SceneFieldName, SceneValue)],
    expanded_paths: &[String],
    color_paths: &[String],
    node_paths: &[String],
    enum_options: &BTreeMap<String, Vec<String>>,
) -> Vec<InspectorValueRow> {
    let mut rows = Vec::new();
    let ctx = ValueRowContext {
        expanded_paths,
        color_paths,
        node_paths,
        enum_options,
    };
    for (idx, (name, value)) in fields.iter().enumerate() {
        let mut path = vec![ValuePathStep::Root(idx)];
        push_value_rows(&mut rows, name.as_ref(), value, &mut path, 0, &ctx);
    }
    rows
}

pub fn script_state_color_path_keys(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
    fields: &[(SceneFieldName, SceneValue)],
) -> Vec<String> {
    let Some(script_path) = node.script.as_ref() else {
        return Vec::new();
    };
    script_state_color_field_names(&state.project_root, script_path.as_ref())
        .into_iter()
        .filter_map(|name| {
            fields
                .iter()
                .position(|(field, _)| field.as_ref() == name)
                .map(|idx| format!("r{idx}"))
        })
        .collect()
}

pub fn script_state_node_path_keys(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
    fields: &[(SceneFieldName, SceneValue)],
) -> Vec<String> {
    let Some(script_path) = node.script.as_ref() else {
        return Vec::new();
    };
    let abs = res_to_abs(&state.project_root, script_path.as_ref());
    let Ok(source) = fs::read_to_string(abs) else {
        return Vec::new();
    };
    let Some(struct_name) = parse_state_struct_name(&source) else {
        return Vec::new();
    };
    let schema = parse_script_schema(&source);
    let mut out = Vec::new();
    collect_script_node_path_keys(
        &schema,
        &struct_name,
        true,
        fields,
        String::new(),
        0,
        &mut out,
    );
    out
}

pub fn script_state_enum_path_options(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
    fields: &[(SceneFieldName, SceneValue)],
) -> BTreeMap<String, Vec<String>> {
    let Some(script_path) = node.script.as_ref() else {
        return BTreeMap::new();
    };
    let abs = res_to_abs(&state.project_root, script_path.as_ref());
    let Ok(source) = fs::read_to_string(abs) else {
        return BTreeMap::new();
    };
    let Some(struct_name) = parse_state_struct_name(&source) else {
        return BTreeMap::new();
    };
    let schema = parse_script_schema(&source);
    let mut out = BTreeMap::new();
    collect_script_enum_path_options(
        &schema,
        &struct_name,
        true,
        fields,
        String::new(),
        0,
        &mut out,
    );
    out
}

pub fn inspector_script_var_rows_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
) -> Vec<InspectorValueRow> {
    let fields = inspector_script_var_fields_for_node(state, node);
    let node_paths = script_state_node_path_keys(state, node, &fields);
    let enum_options = script_state_enum_path_options(state, node, &fields);
    inspector_script_var_rows_with_color_paths(
        &fields,
        &state.inspector_expanded_paths,
        &[],
        &node_paths,
        &enum_options,
    )
}

pub fn inspector_script_var_fields_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
) -> Vec<(SceneFieldName, SceneValue)> {
    let mut fields = node
        .script
        .as_ref()
        .map(|path| script_state_default_fields(&state.project_root, path.as_ref()))
        .unwrap_or_default();
    merge_script_var_overrides(&mut fields, node.script_vars.as_ref());
    fields
}

fn merge_script_var_overrides(
    fields: &mut [(SceneFieldName, SceneValue)],
    overrides: &[(SceneFieldName, SceneValue)],
) {
    for (name, value) in overrides {
        if let Some((_, existing)) = fields.iter_mut().find(|(field, _)| field == name) {
            merge_script_var_override_value(existing, value);
        }
    }
}

fn merge_script_var_override_value(existing: &mut SceneValue, override_value: &SceneValue) {
    match (existing, override_value) {
        (SceneValue::Object(existing_fields), SceneValue::Object(override_fields)) => {
            for (name, value) in override_fields.iter() {
                if let Some((_, existing_value)) = existing_fields
                    .to_mut()
                    .iter_mut()
                    .find(|(field, _)| field == name)
                {
                    merge_script_var_override_value(existing_value, value);
                }
            }
        }
        (existing, override_value) => {
            *existing = override_value.clone();
        }
    }
}

fn script_state_default_fields(
    project_root: &str,
    script_path: &str,
) -> Vec<(SceneFieldName, SceneValue)> {
    let abs = res_to_abs(project_root, script_path);
    let Ok(source) = fs::read_to_string(abs) else {
        return Vec::new();
    };
    let Some(struct_name) = parse_state_struct_name(&source) else {
        return Vec::new();
    };
    let schema = parse_script_schema(&source);
    script_struct_default_fields_with_expose(&schema, &struct_name, 0, true)
}

fn script_state_color_field_names(project_root: &str, script_path: &str) -> Vec<String> {
    let abs = res_to_abs(project_root, script_path);
    let Ok(source) = fs::read_to_string(abs) else {
        return Vec::new();
    };
    let Some(struct_name) = parse_state_struct_name(&source) else {
        return Vec::new();
    };
    let schema = parse_script_schema(&source);
    schema
        .structs
        .get(&struct_name)
        .into_iter()
        .flat_map(|fields| fields.iter())
        .filter(|field| field.exposed && is_color_type(&field.ty))
        .map(|field| field.name.clone())
        .collect()
}

fn collect_script_node_path_keys(
    schema: &ScriptSchema,
    struct_name: &str,
    require_expose: bool,
    values: &[(SceneFieldName, SceneValue)],
    prefix: String,
    depth: usize,
    out: &mut Vec<String>,
) {
    if depth > MAX_INSPECTOR_DEPTH {
        return;
    }
    let Some(fields) = schema.structs.get(struct_name) else {
        return;
    };
    for field in fields
        .iter()
        .filter(|field| !require_expose || field.exposed)
    {
        let Some(idx) = values
            .iter()
            .position(|(name, _)| name.as_ref() == field.name)
        else {
            continue;
        };
        let key = if prefix.is_empty() {
            format!("r{idx}")
        } else {
            format!("{prefix}.{}", field.name)
        };
        if is_node_ref_type(&field.ty) {
            out.push(key);
        } else if let Some(nested) = script_struct_type_name(schema, &field.ty)
            && let Some((_, SceneValue::Object(nested_values))) = values.get(idx)
        {
            collect_script_node_path_keys(
                schema,
                &nested,
                false,
                nested_values.as_ref(),
                key,
                depth + 1,
                out,
            );
        }
    }
}

fn collect_script_enum_path_options(
    schema: &ScriptSchema,
    struct_name: &str,
    require_expose: bool,
    values: &[(SceneFieldName, SceneValue)],
    prefix: String,
    depth: usize,
    out: &mut BTreeMap<String, Vec<String>>,
) {
    if depth > MAX_INSPECTOR_DEPTH {
        return;
    }
    let Some(fields) = schema.structs.get(struct_name) else {
        return;
    };
    for field in fields
        .iter()
        .filter(|field| !require_expose || field.exposed)
    {
        let Some(idx) = values
            .iter()
            .position(|(name, _)| name.as_ref() == field.name)
        else {
            continue;
        };
        let key = if prefix.is_empty() {
            format!("r{idx}")
        } else {
            format!("{prefix}.{}", field.name)
        };
        if let Some(enum_name) = script_enum_type_name(schema, &field.ty) {
            if let Some(info) = schema.enums.get(&enum_name) {
                out.insert(key, info.variants.clone());
            }
        } else if let Some(nested) = script_struct_type_name(schema, &field.ty)
            && let Some((_, SceneValue::Object(nested_values))) = values.get(idx)
        {
            collect_script_enum_path_options(
                schema,
                &nested,
                false,
                nested_values.as_ref(),
                key,
                depth + 1,
                out,
            );
        }
    }
}

struct ScriptSchema {
    structs: BTreeMap<String, Vec<RawScriptField>>,
    enums: BTreeMap<String, ScriptEnum>,
}

struct ScriptEnum {
    default: String,
    variants: Vec<String>,
}

struct RawScriptField {
    name: String,
    ty: String,
    default_attr: Option<String>,
    exposed: bool,
}

fn parse_state_struct_name(source: &str) -> Option<String> {
    let mut saw_state_attr = false;
    for line in source.lines() {
        let line = strip_line_comment(line).trim();
        let (attrs, rest) = split_leading_attrs(line);
        if attrs.iter().any(|attr| is_state_attr(attr)) {
            saw_state_attr = true;
            if let Some(name) = parse_struct_name(rest) {
                return Some(name);
            }
        }
        if saw_state_attr {
            if rest.is_empty() {
                continue;
            }
            return parse_struct_name(rest);
        }
    }
    None
}

fn parse_struct_name(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace().peekable();
    while let Some(part) = parts.next() {
        if part == "struct" {
            return parts
                .next()
                .map(|value| value.trim_matches('{').trim().to_string())
                .filter(|value| !value.is_empty());
        }
    }
    None
}

fn parse_script_schema(source: &str) -> ScriptSchema {
    let mut structs = BTreeMap::new();
    let mut enums = BTreeMap::new();
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(name) = parse_struct_name(strip_line_comment(line).trim()) {
            let fields = parse_script_struct_fields(source, &name);
            structs.insert(name, fields);
        } else if let Some(name) = parse_enum_name(strip_line_comment(line).trim()) {
            enums.insert(name, parse_enum_info(line, &mut lines));
        }
    }
    ScriptSchema { structs, enums }
}

fn parse_enum_name(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace().peekable();
    while let Some(part) = parts.next() {
        if part == "enum" {
            return parts
                .next()
                .map(|value| value.trim_matches('{').trim().to_string())
                .filter(|value| !value.is_empty());
        }
    }
    None
}

fn parse_enum_info<'a>(
    first_line: &str,
    lines: &mut std::iter::Peekable<std::str::Lines<'a>>,
) -> ScriptEnum {
    let mut first_variant = None;
    let mut default_variant = None;
    let mut variants = Vec::new();
    let mut default_next = false;
    let mut opened = first_line.contains('{');
    let mut depth = if opened { brace_delta(first_line) } else { 0 };
    while let Some(line) = lines.peek().copied() {
        let line = strip_line_comment(line).trim();
        if !opened {
            opened = line.contains('{');
            depth += brace_delta(line);
            lines.next();
            continue;
        }
        if line == "#[default]" {
            default_next = true;
            lines.next();
            continue;
        }
        if depth == 1
            && let Some(variant) = parse_enum_variant(line)
        {
            if first_variant.is_none() {
                first_variant = Some(variant.clone());
            }
            variants.push(variant.clone());
            if default_next {
                default_variant = Some(variant);
                default_next = false;
            }
        }
        depth += brace_delta(line);
        lines.next();
        if depth <= 0 {
            break;
        }
    }
    let default = default_variant
        .or(first_variant)
        .unwrap_or_else(|| "Default".to_string());
    ScriptEnum { default, variants }
}

fn parse_enum_variant(line: &str) -> Option<String> {
    let name = line
        .trim()
        .trim_end_matches(',')
        .split(['(', '{', '='])
        .next()?
        .trim();
    if name.is_empty() || name.starts_with("#[") {
        return None;
    }
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
        .then(|| name.to_string())
}

fn parse_script_struct_fields(source: &str, struct_name: &str) -> Vec<RawScriptField> {
    let lines = source.lines().collect::<Vec<_>>();
    let Some(mut idx) = lines.iter().position(|line| {
        parse_struct_name(strip_line_comment(line).trim()) == Some(struct_name.to_string())
    }) else {
        return Vec::new();
    };
    let mut fields = Vec::new();
    let mut depth = 0_i32;
    let mut opened = false;
    let mut pending_default = None;
    let mut pending_expose = false;
    while idx < lines.len() {
        let line = strip_line_comment(lines[idx]).trim();
        if !opened {
            if let Some(pos) = line.find('{') {
                opened = true;
                depth = 1 + brace_delta(&line[pos + 1..]);
            }
            idx += 1;
            continue;
        }
        if depth == 1 {
            let (attrs, rest) = split_leading_attrs(line);
            for attr in attrs {
                if is_expose_attr(attr) {
                    pending_expose = true;
                }
                if let Some(default_value) = parse_default_attr(attr) {
                    pending_default = Some(default_value);
                }
            }
            if rest.is_empty() {
            } else if let Some(field) =
                parse_script_field_line(rest, pending_default.take(), pending_expose)
            {
                fields.push(field);
                pending_expose = false;
            } else if !rest.is_empty() {
                pending_expose = false;
            }
        }
        depth += brace_delta(line);
        if depth <= 0 {
            break;
        }
        idx += 1;
    }
    fields
}

fn parse_script_field_line(
    line: &str,
    default_attr: Option<String>,
    exposed: bool,
) -> Option<RawScriptField> {
    let trimmed = line.trim().trim_end_matches(',').trim();
    if trimmed.is_empty() || trimmed.starts_with("#[") || trimmed.starts_with("///") {
        return None;
    }
    let without_vis = trimmed.strip_prefix("pub ").unwrap_or(trimmed).trim_start();
    let (name, ty) = without_vis.split_once(':')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some(RawScriptField {
        name: name.to_string(),
        ty: ty.trim().to_string(),
        default_attr,
        exposed,
    })
}

fn parse_default_attr(line: &str) -> Option<String> {
    let inner = line.strip_prefix("#[default")?.strip_suffix(']')?.trim();
    if let Some(value) = inner.strip_prefix('=') {
        return Some(value.trim().to_string());
    }
    if let Some(value) = inner.strip_prefix('(').and_then(|value| value.strip_suffix(')')) {
        return Some(value.trim().to_string());
    }
    None
}

fn is_expose_attr(line: &str) -> bool {
    matches!(line.trim(), "#[expose]" | "#[Expose]")
}

fn is_state_attr(line: &str) -> bool {
    matches!(line.trim(), "#[State]" | "#[state]")
}

fn split_leading_attrs(mut line: &str) -> (Vec<&str>, &str) {
    let mut attrs = Vec::new();
    loop {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("#[") else {
            return (attrs, trimmed);
        };
        let Some(end) = rest.find(']') else {
            return (attrs, trimmed);
        };
        let attr = &trimmed[..end + 3];
        attrs.push(attr);
        line = &rest[end + 1..];
    }
}

fn script_struct_default_fields(
    schema: &ScriptSchema,
    struct_name: &str,
    depth: usize,
) -> Vec<(SceneFieldName, SceneValue)> {
    script_struct_default_fields_with_expose(schema, struct_name, depth, false)
}

fn script_struct_default_fields_with_expose(
    schema: &ScriptSchema,
    struct_name: &str,
    depth: usize,
    require_expose: bool,
) -> Vec<(SceneFieldName, SceneValue)> {
    if depth > MAX_INSPECTOR_DEPTH {
        return Vec::new();
    }
    let Some(fields) = schema.structs.get(struct_name) else {
        return Vec::new();
    };
    fields
        .iter()
        .filter(|field| !require_expose || field.exposed)
        .map(|field| {
            (
                SceneFieldName::Custom(Cow::Owned(field.name.clone())),
                default_value_for_field(schema, field, depth + 1),
            )
        })
        .collect()
}

fn default_value_for_field(
    schema: &ScriptSchema,
    field: &RawScriptField,
    depth: usize,
) -> SceneValue {
    field
        .default_attr
        .as_deref()
        .and_then(|value| parse_default_value(schema, value, &field.ty, depth))
        .unwrap_or_else(|| default_scene_value_for_type(schema, &field.ty, depth))
}

fn parse_default_value(
    schema: &ScriptSchema,
    value: &str,
    ty: &str,
    depth: usize,
) -> Option<SceneValue> {
    if is_int_type(ty)
        && let Ok(value) = value.parse::<i32>()
    {
        return Some(SceneValue::I32(value));
    }
    if let Some(inner) = value
        .strip_suffix("::default()")
        .map(|value| value.rsplit("::").next().unwrap_or(value))
    {
        return Some(default_scene_value_for_type(schema, inner, depth + 1));
    }
    if value.ends_with("ID::nil()")
        || matches!(value, "NodeID::nil()" | "perro_api::prelude::NodeID::nil()")
    {
        return Some(SceneValue::Key(perro_api::scene::SceneValueKey::from(
            "null",
        )));
    }
    if let Some((enum_ty, variant)) = value.split_once("::")
        && schema.enums.contains_key(enum_ty)
    {
        return Some(SceneValue::Key(perro_api::scene::SceneValueKey::from(
            variant.to_string(),
        )));
    }
    if let Some(value) = parse_vec_default_value(schema, value, ty, depth) {
        return Some(value);
    }
    if value.contains("::") || value.contains('{') || value.contains('[') {
        return None;
    }
    Some(Parser::new(value).parse_value_literal())
}

fn parse_vec_default_value(
    schema: &ScriptSchema,
    value: &str,
    ty: &str,
    depth: usize,
) -> Option<SceneValue> {
    let inner_ty = generic_inner(normalized_type(ty).as_str(), "Vec")?;
    let content = value.strip_prefix("vec![")?.strip_suffix(']')?;
    if let Some((item, count)) = content.split_once(';') {
        let count = count.trim().parse::<usize>().ok()?.min(32);
        let item = parse_default_value(schema, item.trim(), &inner_ty, depth + 1)
            .unwrap_or_else(|| default_scene_value_for_type(schema, &inner_ty, depth + 1));
        return Some(SceneValue::Array(Cow::Owned(vec![item; count])));
    }
    if content.trim().is_empty() {
        return Some(SceneValue::Array(Cow::Owned(Vec::new())));
    }
    let mut values = Vec::new();
    for item in content
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        values.push(
            parse_default_value(schema, item, &inner_ty, depth + 1)
                .unwrap_or_else(|| default_scene_value_for_type(schema, &inner_ty, depth + 1)),
        );
    }
    Some(SceneValue::Array(Cow::Owned(values)))
}

fn is_int_type(ty: &str) -> bool {
    matches!(
        normalized_type(ty).as_str(),
        "i8" | "i16" | "i32" | "u8" | "u16" | "u32"
    )
}

fn is_color_type(ty: &str) -> bool {
    matches!(
        normalized_type(ty).as_str(),
        "Color" | "perro_api::prelude::Color"
    )
}

fn is_node_ref_type(ty: &str) -> bool {
    let ty = normalized_type(ty);
    let ty = generic_inner(ty.as_str(), "Option").unwrap_or(ty);
    last_type_segment(&ty) == "NodeID"
}

fn script_struct_type_name(schema: &ScriptSchema, ty: &str) -> Option<String> {
    let ty = normalized_type(ty);
    let direct = last_type_segment(&ty).to_string();
    if schema.structs.contains_key(&direct) {
        return Some(direct);
    }
    None
}

fn script_enum_type_name(schema: &ScriptSchema, ty: &str) -> Option<String> {
    let ty = normalized_type(ty);
    let direct = last_type_segment(&ty).to_string();
    if schema.enums.contains_key(&direct) {
        return Some(direct);
    }
    None
}

fn last_type_segment(ty: &str) -> &str {
    ty.rsplit("::").next().unwrap_or(ty)
}

fn default_scene_value_for_type(schema: &ScriptSchema, ty: &str, depth: usize) -> SceneValue {
    let ty = normalized_type(ty);
    match ty.as_str() {
        "bool" => SceneValue::Bool(false),
        "f32" | "f64" => SceneValue::F32(0.0),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => SceneValue::I32(0),
        "String" | "Arc<str>" | "std::sync::Arc<str>" | "Cow<'static,str>" => {
            SceneValue::Str(Cow::Borrowed(""))
        }
        "Vector2" | "perro_api::prelude::Vector2" => SceneValue::Vec2 { x: 0.0, y: 0.0 },
        "Vector3" | "perro_api::prelude::Vector3" => SceneValue::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        "Vector4" | "perro_api::prelude::Vector4" | "Color" | "perro_api::prelude::Color" => {
            SceneValue::Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            }
        }
        _ if ty.ends_with("ID") => SceneValue::Key(perro_api::scene::SceneValueKey::from("null")),
        _ if ty.starts_with("Option<") => {
            SceneValue::Key(perro_api::scene::SceneValueKey::from("null"))
        }
        _ if ty.starts_with("Vec<") => SceneValue::Array(Cow::Owned(Vec::new())),
        _ if let Some(struct_name) = script_struct_type_name(schema, &ty) => SceneValue::Object(
            Cow::Owned(script_struct_default_fields(schema, &struct_name, depth + 1)),
        ),
        _ if let Some(enum_name) = script_enum_type_name(schema, &ty) => {
            let default_variant = schema
                .enums
                .get(&enum_name)
                .map(|item| item.default.clone())
                .unwrap_or_default();
            SceneValue::Key(perro_api::scene::SceneValueKey::from(default_variant))
        }
        _ => SceneValue::Object(Cow::Owned(Vec::new())),
    }
}

fn normalized_type(ty: &str) -> String {
    ty.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn generic_inner(ty: &str, outer: &str) -> Option<String> {
    ty.strip_prefix(outer)?
        .strip_prefix('<')?
        .strip_suffix('>')
        .map(str::to_string)
}

fn strip_line_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

fn brace_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| *ch == '{').count() as i32;
    let closes = line.chars().filter(|ch| *ch == '}').count() as i32;
    opens - closes
}

fn push_value_rows(
    rows: &mut Vec<InspectorValueRow>,
    name: &str,
    value: &SceneValue,
    path: &mut Vec<ValuePathStep>,
    depth: usize,
    ctx: &ValueRowContext<'_>,
) {
    let composite = matches!(value, SceneValue::Array(_) | SceneValue::Object(_));
    let path_key = value_path_key(path);
    let expanded = composite && ctx.expanded_paths.iter().any(|item| item == &path_key);
    let color_preview = color_preview_for_value(name, value, &path_key, ctx.color_paths);
    let enum_options = ctx
        .enum_options
        .get(&path_key)
        .cloned()
        .unwrap_or_default();
    let kind = if color_preview.is_some() {
        "Color"
    } else if !enum_options.is_empty() {
        "Enum"
    } else if matches!(value, SceneValue::Key(_))
        && !ctx.node_paths.iter().any(|item| item == &path_key)
    {
        "Key"
    } else {
        scene_value_kind(value)
    };
    rows.push(InspectorValueRow {
        path: path.clone(),
        path_key,
        name: format!("{}{}", "  ".repeat(depth), name),
        kind: kind.to_string(),
        value: if composite {
            scene_value_summary(value, expanded)
        } else {
            scene_value_edit_text(value)
        },
        components: scene_value_component_texts(value),
        color_preview,
        enum_options,
        editable: !composite,
        expandable: composite,
    });
    if !expanded || depth >= MAX_INSPECTOR_DEPTH {
        return;
    }
    match value {
        SceneValue::Array(values) => {
            for (idx, item) in values.iter().enumerate() {
                path.push(ValuePathStep::Index(idx));
                push_value_rows(
                    rows,
                    &format!("[{idx}]"),
                    item,
                    path,
                    depth + 1,
                    ctx,
                );
                path.pop();
            }
        }
        SceneValue::Object(fields) => {
            for (field, item) in fields.iter() {
                path.push(ValuePathStep::Field(field.as_ref().to_string()));
                push_value_rows(
                    rows,
                    field.as_ref(),
                    item,
                    path,
                    depth + 1,
                    ctx,
                );
                path.pop();
            }
        }
        _ => {}
    }
}

pub fn scene_value_summary(value: &SceneValue, expanded: bool) -> String {
    let marker = if expanded { "v" } else { ">" };
    match value {
        SceneValue::Array(values) if values.is_empty() => format!("{marker} Array []"),
        SceneValue::Array(values) => format!("{marker} Array [{}]", values.len()),
        SceneValue::Object(fields) if fields.is_empty() => format!("{marker} Object {{}}"),
        SceneValue::Object(fields) => format!("{marker} Object [{}]", fields.len()),
        _ => scene_value_edit_text(value),
    }
}

pub fn scene_value_component_texts(value: &SceneValue) -> Vec<String> {
    match value {
        SceneValue::Vec2 { .. } | SceneValue::Vec3 { .. } | SceneValue::Vec4 { .. } => {
            scene_value_components_from_value(value)
        }
        _ => Vec::new(),
    }
}

fn color_preview_for_value(
    name: &str,
    value: &SceneValue,
    path_key: &str,
    color_paths: &[String],
) -> Option<String> {
    if !field_name_looks_like_color(name) && !color_paths.iter().any(|item| item == path_key) {
        return None;
    }
    let SceneValue::Vec4 { x, y, z, w } = value else {
        return None;
    };
    let color = Color::new(*x, *y, *z, *w);
    Some(color.to_hex_rgba())
}

fn field_name_looks_like_color(name: &str) -> bool {
    let name = name.trim().trim_start_matches('>').trim();
    let Some(last) = name.split('.').next_back() else {
        return false;
    };
    let name = last.to_ascii_lowercase();
    matches!(
        name.as_str(),
        "color"
            | "tint"
            | "modulate"
            | "self_modulate"
            | "children_modulate"
            | "child_modulate"
            | "hover_tint"
            | "hover_color"
            | "hover_modulate"
            | "pressed_tint"
            | "pressed_color"
            | "pressed_modulate"
    ) || name.ends_with("_color")
        || name.ends_with("_colors")
        || name.ends_with("_tint")
        || name.ends_with("_modulate")
}

pub fn value_path_key(path: &[ValuePathStep]) -> String {
    let mut out = String::new();
    for step in path {
        match step {
            ValuePathStep::Root(idx) => out.push_str(&format!("r{idx}")),
            ValuePathStep::Field(name) => {
                out.push('.');
                out.push_str(name);
            }
            ValuePathStep::Index(idx) => out.push_str(&format!("[{idx}]")),
        }
    }
    out
}

pub fn edit_selected_script_var_path<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let rows = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        let doc = SceneDoc::parse(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        let fields = inspector_script_var_fields_for_node(state, node);
        let color_paths = script_state_color_path_keys(state, node, &fields);
        let node_paths = script_state_node_path_keys(state, node, &fields);
        let enum_options = script_state_enum_path_options(state, node, &fields);
        Some(inspector_script_var_rows_with_color_paths(
            &fields,
            &state.inspector_expanded_paths,
            &color_paths,
            &node_paths,
            &enum_options,
        ))
    });
    let Some(rows) = rows else {
        return;
    };
    let Some(row) = rows.get(idx).cloned() else {
        return;
    };
    if !row.editable {
        set_log(ctx, "script var edit fail\ncontainer row");
        return;
    }
    let text = if row.kind == "Color" {
        let Some(text) = read_color_picker_value(ctx, &format!("inspector_var_{idx}_color_swatch"))
        else {
            return;
        };
        text
    } else if !row.enum_options.is_empty() {
        let Some(text) = read_dropdown_value(ctx, &format!("inspector_var_{idx}_dropdown")) else {
            return;
        };
        text
    } else if row.kind == "Bool" {
        let Some(checked) = read_checkbox_checked(ctx, &format!("inspector_var_{idx}_check"))
        else {
            return;
        };
        checked.to_string()
    } else if row.components.is_empty() {
        let Some(text) = read_text_box(ctx, &format!("inspector_var_{idx}_value")) else {
            return;
        };
        text
    } else {
        let mut values = Vec::new();
        for component in 0..row.components.len() {
            let Some(text) = read_text_box(ctx, &format!("inspector_var_{idx}_{component}_box"))
            else {
                return;
            };
            values.push(text);
        }
        format!("({})", values.join(", "))
    };
    let value = match parse_script_var_value(text.trim()) {
        Ok(value) => value,
        Err(err) => {
            set_log(ctx, &format!("script var parse fail\n{err}"));
            return;
        }
    };
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        let mut fields = inspector_script_var_fields_for_node(state, node);
        if !set_value_at_path(&mut fields, &row.path, value) {
            return false;
        }
        if !write_script_var_override(node.script_vars.to_mut(), &fields, &row.path) {
            return false;
        }
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("edit script var\n{}", row.name.trim());
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn set_value_at_path(
    fields: &mut [(SceneFieldName, SceneValue)],
    path: &[ValuePathStep],
    value: SceneValue,
) -> bool {
    let Some(ValuePathStep::Root(idx)) = path.first() else {
        return false;
    };
    let Some((_name, root_value)) = fields.get_mut(*idx) else {
        return false;
    };
    set_nested_value(root_value, &path[1..], value)
}

pub fn write_script_var_override(
    overrides: &mut Vec<(SceneFieldName, SceneValue)>,
    fields: &[(SceneFieldName, SceneValue)],
    path: &[ValuePathStep],
) -> bool {
    let Some(ValuePathStep::Root(idx)) = path.first() else {
        return false;
    };
    let Some((name, value)) = fields.get(*idx).cloned() else {
        return false;
    };
    if let Some((_, existing)) = overrides.iter_mut().find(|(field, _)| field == &name) {
        *existing = value;
    } else {
        overrides.push((name, value));
    }
    true
}

pub fn root_field_name<'a>(
    fields: &'a [(SceneFieldName, SceneValue)],
    path: &[ValuePathStep],
) -> Option<&'a str> {
    let Some(ValuePathStep::Root(idx)) = path.first() else {
        return None;
    };
    fields.get(*idx).map(|(name, _)| name.as_ref())
}

pub fn root_field_value<'a>(
    fields: &'a [(SceneFieldName, SceneValue)],
    path: &[ValuePathStep],
) -> Option<&'a SceneValue> {
    let Some(ValuePathStep::Root(idx)) = path.first() else {
        return None;
    };
    fields.get(*idx).map(|(_, value)| value)
}

pub fn write_scene_field_override(
    overrides: &mut Vec<(SceneFieldName, SceneValue)>,
    fields: &[(SceneFieldName, SceneValue)],
    path: &[ValuePathStep],
) -> bool {
    let Some(ValuePathStep::Root(idx)) = path.first() else {
        return false;
    };
    let Some((name, value)) = fields.get(*idx).cloned() else {
        return false;
    };
    if let Some((_, existing)) = overrides.iter_mut().find(|(field, _)| field == &name) {
        *existing = value;
    } else {
        overrides.push((name, value));
    }
    true
}

fn set_nested_value(target: &mut SceneValue, path: &[ValuePathStep], value: SceneValue) -> bool {
    if path.is_empty() {
        *target = value;
        return true;
    }
    match (&mut *target, &path[0]) {
        (SceneValue::Array(values), ValuePathStep::Index(idx)) => {
            let Some(item) = values.to_mut().get_mut(*idx) else {
                return false;
            };
            set_nested_value(item, &path[1..], value)
        }
        (SceneValue::Object(fields), ValuePathStep::Field(name)) => {
            let Some((_field, item)) = fields
                .to_mut()
                .iter_mut()
                .find(|(field, _)| field.as_ref() == name)
            else {
                return false;
            };
            set_nested_value(item, &path[1..], value)
        }
        _ => false,
    }
}
