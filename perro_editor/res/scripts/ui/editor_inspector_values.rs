use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_editor_main_rs::{EditorState, cached_scene_doc, set_state_scene_doc};
use crate::scripts_scene_editor_animation_rs::*;
use crate::scripts_scene_editor_viewport_rs::*;
use crate::scripts_ui_editor_ui_rs::*;
use perro_api::prelude::*;
use perro_api::scene::{Parser, SceneDoc, SceneFieldName, SceneValue, SceneValueKey};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::UNIX_EPOCH;

const MAX_INSPECTOR_DEPTH: usize = 4;

static SCRIPT_SCHEMA_CACHE: OnceLock<Mutex<BTreeMap<String, CachedScriptSchema>>> = OnceLock::new();
static SCRIPT_FILE_SCHEMA_CACHE: OnceLock<Mutex<BTreeMap<String, CachedScriptFileSchema>>> =
    OnceLock::new();
static INSPECTOR_ROW_CACHE: OnceLock<Mutex<Option<CachedInspectorRows>>> = OnceLock::new();

#[derive(Clone)]
struct CachedInspectorRows {
    key: String,
    rows: Vec<InspectorValueRow>,
}

#[derive(Clone)]
pub struct InspectorValueRow {
    pub source: String,
    pub depth: usize,
    pub path: Vec<ValuePathStep>,
    pub path_key: String,
    pub name: String,
    pub kind: String,
    pub value: String,
    pub components: Vec<String>,
    pub color_preview: Option<String>,
    pub enum_options: Vec<String>,
    pub default_child: Option<SceneValue>,
    pub editable: bool,
    pub expandable: bool,
    pub addable: bool,
    pub removable: bool,
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
    kind_overrides: &'a BTreeMap<String, String>,
    enum_options: &'a BTreeMap<String, Vec<String>>,
    default_children: &'a BTreeMap<String, SceneValue>,
    warnings: &'a BTreeMap<String, String>,
    quat_mode: &'a str,
}

#[derive(Default)]
struct ScriptInspectorMeta {
    kind_overrides: BTreeMap<String, String>,
    color_paths: Vec<String>,
    node_paths: Vec<String>,
    enum_options: BTreeMap<String, Vec<String>>,
    default_children: BTreeMap<String, SceneValue>,
    warnings: BTreeMap<String, String>,
}

#[derive(Clone)]
struct CachedScriptSchema {
    struct_name: String,
    schema: ScriptSchema,
}

#[derive(Clone)]
struct CachedScriptFileSchema {
    sig: String,
    schema: ScriptSchema,
}

pub fn inspector_script_var_rows(
    fields: &[(SceneFieldName, SceneValue)],
    expanded_paths: &[String],
) -> Vec<InspectorValueRow> {
    inspector_script_var_rows_with_color_paths(
        fields,
        expanded_paths,
        &[],
        &[],
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        "quat",
    )
}

#[allow(clippy::too_many_arguments)]
pub fn inspector_script_var_rows_with_color_paths(
    fields: &[(SceneFieldName, SceneValue)],
    expanded_paths: &[String],
    color_paths: &[String],
    node_paths: &[String],
    kind_overrides: &BTreeMap<String, String>,
    enum_options: &BTreeMap<String, Vec<String>>,
    default_children: &BTreeMap<String, SceneValue>,
    warnings: &BTreeMap<String, String>,
    quat_mode: &str,
) -> Vec<InspectorValueRow> {
    let mut rows = Vec::new();
    let color_paths = prefixed_path_list("script", color_paths);
    let node_paths = prefixed_path_list("script", node_paths);
    let kind_overrides = prefixed_path_map("script", kind_overrides);
    let enum_options = prefixed_path_map("script", enum_options);
    let default_children = prefixed_path_map("script", default_children);
    let warnings = prefixed_path_map("script", warnings);
    let ctx = ValueRowContext {
        expanded_paths,
        color_paths: &color_paths,
        node_paths: &node_paths,
        kind_overrides: &kind_overrides,
        enum_options: &enum_options,
        default_children: &default_children,
        warnings: &warnings,
        quat_mode,
    };
    for (idx, (name, value)) in fields.iter().enumerate() {
        let mut path = vec![ValuePathStep::Root(idx)];
        push_value_rows(
            &mut rows,
            "script",
            name.as_ref(),
            value,
            &mut path,
            0,
            &ctx,
        );
    }
    rows
}

fn prefixed_path_list(prefix: &str, values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| format!("{prefix}:{value}"))
        .collect()
}

fn prefixed_path_map<T: Clone>(prefix: &str, values: &BTreeMap<String, T>) -> BTreeMap<String, T> {
    values
        .iter()
        .map(|(key, value)| (format!("{prefix}:{key}"), value.clone()))
        .collect()
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
    let schema = parse_project_script_schema(&state.project_root, script_path.as_ref(), &source);
    let mut out = Vec::new();
    collect_script_node_path_keys(
        &schema,
        &struct_name,
        true,
        fields,
        String::new(),
        0,
        None,
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
    let schema = parse_project_script_schema(&state.project_root, script_path.as_ref(), &source);
    let mut out = BTreeMap::new();
    collect_script_enum_path_options(
        &schema,
        &struct_name,
        true,
        fields,
        String::new(),
        0,
        None,
        &mut out,
    );
    out
}

pub fn inspector_script_var_rows_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
) -> Vec<InspectorValueRow> {
    let Some((schema, struct_name)) = script_schema_for_node(state, node) else {
        return Vec::new();
    };
    let mut fields = script_struct_default_fields_with_expose(&schema, &struct_name, 0, true);
    merge_script_var_overrides(&mut fields, node.script_vars.as_ref());
    let meta = script_inspector_meta(&schema, &struct_name, &fields);
    inspector_script_var_rows_with_color_paths(
        &fields,
        &state.inspector_expanded_paths,
        &meta.color_paths,
        &meta.node_paths,
        &meta.kind_overrides,
        &meta.enum_options,
        &meta.default_children,
        &meta.warnings,
        &state.inspector_rotation_mode,
    )
}

pub fn inspector_value_rows_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
) -> Vec<InspectorValueRow> {
    let scene_fields = inspector_scene_value_fields_for_node(node);
    let mut rows = inspector_scene_value_rows_for_node(state, node, &scene_fields);
    rows.extend(inspector_script_var_rows_for_node(state, node));
    rows
}

pub fn inspector_display_rows_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
) -> Vec<InspectorValueRow> {
    let cache_key = inspector_row_cache_key(state, node);
    let cache = INSPECTOR_ROW_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(cache) = cache.lock()
        && let Some(cached) = cache.as_ref()
        && cached.key == cache_key
    {
        return cached.rows.clone();
    }
    let scene_fields = inspector_scene_value_fields_for_node(node);
    let mut rows = grouped_scene_value_rows_for_node(state, node, &scene_fields);
    let script_rows = inspector_script_var_rows_for_node(state, node);
    if !script_rows.is_empty() {
        rows.push(inspector_section_row("section:script", "Script"));
        rows.extend(script_rows.into_iter().map(indent_inspector_row));
    }
    if let Ok(mut cache) = cache.lock() {
        *cache = Some(CachedInspectorRows {
            key: cache_key,
            rows: rows.clone(),
        });
    }
    rows
}

fn inspector_row_cache_key(state: &EditorState, node: &perro_api::scene::SceneNodeEntry) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}",
        node.key.as_u32(),
        state.inspector_expanded_paths.join("\n"),
        state.inspector_rotation_mode,
        state.inspector_collapsed_sections.join("\n"),
        state.doc_text
    )
}

fn grouped_scene_value_rows_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
    fields: &[(SceneFieldName, SceneValue)],
) -> Vec<InspectorValueRow> {
    let flat = inspector_scene_value_rows_for_node(state, node, fields);
    let owners = inspector_field_owner_map(node.data.node_type);
    let chain = inspector_node_type_chain(node.data.node_type);
    let mut grouped: BTreeMap<String, Vec<InspectorValueRow>> = BTreeMap::new();
    for row in flat {
        let owner = owners
            .get(row.name.trim())
            .copied()
            .unwrap_or_else(|| node.data.type_name());
        grouped
            .entry(owner.to_string())
            .or_default()
            .push(indent_inspector_row(row));
    }
    let mut rows = Vec::new();
    for ty in chain {
        let name = ty.name();
        let Some(group_rows) = grouped.remove(name) else {
            continue;
        };
        rows.push(inspector_section_row(&format!("section:{name}"), name));
        rows.extend(group_rows);
    }
    for (name, group_rows) in grouped {
        rows.push(inspector_section_row(&format!("section:{name}"), &name));
        rows.extend(group_rows);
    }
    rows
}

fn inspector_node_type_chain(node_type: perro_scene::NodeType) -> Vec<perro_scene::NodeType> {
    let mut chain = Vec::new();
    let mut cursor = Some(node_type);
    while let Some(ty) = cursor {
        chain.push(ty);
        cursor = ty.parent_type();
    }
    chain.reverse();
    chain
}

fn inspector_field_owner_map(
    node_type: perro_scene::NodeType,
) -> BTreeMap<&'static str, &'static str> {
    let mut out = BTreeMap::new();
    for ty in inspector_node_type_chain(node_type) {
        let parent_names = ty
            .parent_type()
            .map(|parent| {
                perro_scene::scene_node_fields(parent)
                    .into_iter()
                    .map(|field| field.name)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for field in perro_scene::scene_node_fields(ty) {
            if parent_names.contains(&field.name) {
                continue;
            }
            out.entry(field.name).or_insert(ty.name());
        }
    }
    out
}

fn inspector_section_row(path_key: &str, name: &str) -> InspectorValueRow {
    InspectorValueRow {
        source: "section".to_string(),
        depth: 0,
        path: Vec::new(),
        path_key: path_key.to_string(),
        name: name.to_string(),
        kind: String::new(),
        value: String::new(),
        components: Vec::new(),
        color_preview: None,
        enum_options: Vec::new(),
        default_child: None,
        editable: false,
        expandable: false,
        addable: false,
        removable: false,
    }
}

fn indent_inspector_row(mut row: InspectorValueRow) -> InspectorValueRow {
    row.depth += 1;
    row.name = format!("  {}", row.name);
    row
}

fn inspector_scene_value_rows_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
    fields: &[(SceneFieldName, SceneValue)],
) -> Vec<InspectorValueRow> {
    let mut kind_overrides = BTreeMap::new();
    let mut color_paths = Vec::new();
    let mut node_paths = Vec::new();
    let mut default_children = BTreeMap::new();
    let schema_fields = perro_scene::scene_node_fields(node.data.node_type);
    for (idx, (name, value)) in fields.iter().enumerate() {
        let Some(schema_field) = schema_fields
            .iter()
            .find(|field| field.name == name.as_ref() || field.aliases.contains(&name.as_ref()))
        else {
            continue;
        };
        let mut path = vec![ValuePathStep::Root(idx)];
        collect_node_field_type_paths(
            "scene",
            &schema_field.ty,
            value,
            &mut path,
            &mut kind_overrides,
            &mut color_paths,
            &mut node_paths,
            &mut default_children,
        );
    }
    let ctx = ValueRowContext {
        expanded_paths: &state.inspector_expanded_paths,
        color_paths: &color_paths,
        node_paths: &node_paths,
        kind_overrides: &kind_overrides,
        enum_options: &BTreeMap::new(),
        default_children: &default_children,
        warnings: &BTreeMap::new(),
        quat_mode: &state.inspector_rotation_mode,
    };
    let mut rows = Vec::new();
    for (idx, (name, value)) in fields.iter().enumerate() {
        let mut path = vec![ValuePathStep::Root(idx)];
        push_value_rows(&mut rows, "scene", name.as_ref(), value, &mut path, 0, &ctx);
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn collect_node_field_type_paths(
    source: &str,
    ty: &perro_scene::NodeFieldType,
    value: &SceneValue,
    path: &mut Vec<ValuePathStep>,
    kind_overrides: &mut BTreeMap<String, String>,
    color_paths: &mut Vec<String>,
    node_paths: &mut Vec<String>,
    default_children: &mut BTreeMap<String, SceneValue>,
) {
    let key = format!("{source}:{}", value_path_key(path));
    let label = node_field_type_label(ty);
    kind_overrides.insert(key.clone(), label);
    match ty {
        perro_scene::NodeFieldType::Color => color_paths.push(key),
        perro_scene::NodeFieldType::NodeRef => node_paths.push(key),
        perro_scene::NodeFieldType::Array(item_ty) => {
            default_children.insert(key, item_ty.default_value());
            if let SceneValue::Array(values) = value {
                for (idx, item) in values.iter().enumerate() {
                    path.push(ValuePathStep::Index(idx));
                    collect_node_field_type_paths(
                        source,
                        item_ty,
                        item,
                        path,
                        kind_overrides,
                        color_paths,
                        node_paths,
                        default_children,
                    );
                    path.pop();
                }
            }
        }
        perro_scene::NodeFieldType::Object(fields) => {
            if let SceneValue::Object(values) = value {
                for field in fields {
                    let Some((_, item)) =
                        values.iter().find(|(name, _)| name.as_ref() == field.name)
                    else {
                        continue;
                    };
                    path.push(ValuePathStep::Field(field.name.to_string()));
                    collect_node_field_type_paths(
                        source,
                        &field.ty,
                        item,
                        path,
                        kind_overrides,
                        color_paths,
                        node_paths,
                        default_children,
                    );
                    path.pop();
                }
            }
        }
        _ => {}
    }
}

fn node_field_type_label(ty: &perro_scene::NodeFieldType) -> String {
    match ty {
        perro_scene::NodeFieldType::Bool => "Bool".to_string(),
        perro_scene::NodeFieldType::I32 => "I32".to_string(),
        perro_scene::NodeFieldType::U32 => "U32".to_string(),
        perro_scene::NodeFieldType::F32 => "F32".to_string(),
        perro_scene::NodeFieldType::Vec2 => "Vec2".to_string(),
        perro_scene::NodeFieldType::Vec3 => "Vec3".to_string(),
        perro_scene::NodeFieldType::Vec4 => "Vec4".to_string(),
        perro_scene::NodeFieldType::Quat => "Quat".to_string(),
        perro_scene::NodeFieldType::Color => "Color".to_string(),
        perro_scene::NodeFieldType::String => "String".to_string(),
        perro_scene::NodeFieldType::NodeRef => "Node".to_string(),
        perro_scene::NodeFieldType::BitMask => "BitMask".to_string(),
        perro_scene::NodeFieldType::Asset(kind) => format!("Asset({kind:?})"),
        perro_scene::NodeFieldType::Array(item) => {
            format!("Array({})", node_field_type_label(item))
        }
        perro_scene::NodeFieldType::Object(_) => "Object".to_string(),
        perro_scene::NodeFieldType::Unknown => "Unknown".to_string(),
    }
}

pub fn inspector_script_var_fields_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
) -> Vec<(SceneFieldName, SceneValue)> {
    let mut fields = inspector_script_var_default_fields_for_node(state, node);
    merge_script_var_overrides(&mut fields, node.script_vars.as_ref());
    fields
}

pub fn inspector_script_var_default_fields_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
) -> Vec<(SceneFieldName, SceneValue)> {
    let Some((schema, struct_name)) = script_schema_for_node(state, node) else {
        return Vec::new();
    };
    script_struct_default_fields_with_expose(&schema, &struct_name, 0, true)
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

fn script_schema_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
) -> Option<(ScriptSchema, String)> {
    let script_path = node.script.as_ref()?;
    let cache_key = format!("{}|{}", state.project_root, script_path.as_ref());
    if let Some(cached) = cached_script_schema(&cache_key) {
        return Some((cached.schema, cached.struct_name));
    }
    let abs = res_to_abs(&state.project_root, script_path.as_ref());
    let source = fs::read_to_string(abs).ok()?;
    let struct_name = parse_state_struct_name(&source)?;
    let schema = parse_project_script_schema(&state.project_root, script_path.as_ref(), &source);
    store_script_schema_cache(
        cache_key,
        CachedScriptSchema {
            struct_name: struct_name.clone(),
            schema: schema.clone(),
        },
    );
    Some((schema, struct_name))
}

pub fn clear_script_schema_cache() {
    clear_inspector_row_cache();
    let Some(cache) = SCRIPT_SCHEMA_CACHE.get() else {
        clear_script_file_schema_cache();
        return;
    };
    if let Ok(mut cache) = cache.lock() {
        cache.clear();
    }
    clear_script_file_schema_cache();
}

pub fn invalidate_script_schema_cache_paths(project_root: &str, changed_paths: &[String]) {
    clear_inspector_row_cache();
    clear_merged_script_schema_cache();
    let Some(cache) = SCRIPT_FILE_SCHEMA_CACHE.get() else {
        return;
    };
    if let Ok(mut cache) = cache.lock() {
        for path in changed_paths {
            if !path.replace('\\', "/").ends_with(".rs") {
                continue;
            }
            let abs = changed_script_abs_path(project_root, path);
            cache.remove(&abs);
        }
    }
}

fn clear_inspector_row_cache() {
    let Some(cache) = INSPECTOR_ROW_CACHE.get() else {
        return;
    };
    if let Ok(mut cache) = cache.lock() {
        *cache = None;
    }
}

fn clear_merged_script_schema_cache() {
    let Some(cache) = SCRIPT_SCHEMA_CACHE.get() else {
        return;
    };
    if let Ok(mut cache) = cache.lock() {
        cache.clear();
    }
}

fn clear_script_file_schema_cache() {
    let Some(cache) = SCRIPT_FILE_SCHEMA_CACHE.get() else {
        return;
    };
    if let Ok(mut cache) = cache.lock() {
        cache.clear();
    }
}

fn cached_script_schema(cache_key: &str) -> Option<CachedScriptSchema> {
    let cache = SCRIPT_SCHEMA_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let cache = cache.lock().ok()?;
    cache.get(cache_key).cloned()
}

fn store_script_schema_cache(cache_key: String, cached: CachedScriptSchema) {
    let cache = SCRIPT_SCHEMA_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(mut cache) = cache.lock() {
        cache.insert(cache_key, cached);
    }
}

fn script_inspector_meta(
    schema: &ScriptSchema,
    struct_name: &str,
    fields: &[(SceneFieldName, SceneValue)],
) -> ScriptInspectorMeta {
    let mut meta = ScriptInspectorMeta::default();
    collect_script_inspector_meta(
        schema,
        struct_name,
        true,
        fields,
        String::new(),
        0,
        None,
        &mut meta,
    );
    meta
}

fn script_state_color_field_names(project_root: &str, script_path: &str) -> Vec<String> {
    let abs = res_to_abs(project_root, script_path);
    let Ok(source) = fs::read_to_string(abs) else {
        return Vec::new();
    };
    let Some(struct_name) = parse_state_struct_name(&source) else {
        return Vec::new();
    };
    let schema = parse_project_script_schema(project_root, script_path, &source);
    schema
        .struct_fields(&struct_name)
        .into_iter()
        .flat_map(|fields| fields.iter())
        .filter(|field| field.exposed && is_color_type(&field.ty))
        .map(|field| field.name.clone())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn collect_script_inspector_meta(
    schema: &ScriptSchema,
    struct_name: &str,
    require_expose: bool,
    values: &[(SceneFieldName, SceneValue)],
    prefix: String,
    depth: usize,
    override_fields: Option<&[RawScriptField]>,
    meta: &mut ScriptInspectorMeta,
) {
    if depth > MAX_INSPECTOR_DEPTH {
        return;
    }
    let Some(schema_fields) = schema.struct_fields(struct_name) else {
        return;
    };
    let fields = override_fields.unwrap_or(schema_fields);
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
        if let Some((_, value)) = values.get(idx) {
            collect_script_value_meta(schema, &field.ty, value, key, depth, meta);
        }
    }
}

fn collect_script_value_meta(
    schema: &ScriptSchema,
    ty: &str,
    value: &SceneValue,
    key: String,
    depth: usize,
    meta: &mut ScriptInspectorMeta,
) {
    if depth > MAX_INSPECTOR_DEPTH {
        return;
    }
    let path_key = key.clone();
    meta.kind_overrides
        .insert(path_key.clone(), script_type_label(schema, ty));
    if is_color_type(ty) {
        meta.color_paths.push(key.clone());
    }
    if is_node_ref_type(ty) {
        meta.node_paths.push(key.clone());
    }
    match resolve_script_enum(schema, ty) {
        TypeResolution::Found(info) => {
            meta.enum_options
                .insert(key.clone(), info.def.variants.clone());
            if !info.def.has_variant {
                meta.warnings.insert(
                    key.clone(),
                    format!("warn missing Variant derive\n{}", info.def.display_name()),
                );
            }
        }
        TypeResolution::Ambiguous(name, origins) => {
            meta.warnings.insert(
                key.clone(),
                format!("warn ambiguous enum\n{name}: {}", origins.join(" + ")),
            );
        }
        TypeResolution::Missing => {}
    }
    match resolve_script_struct(schema, ty) {
        TypeResolution::Found(nested) => {
            if !nested.def.has_variant {
                meta.warnings.insert(
                    key.clone(),
                    format!("warn missing Variant derive\n{}", nested.def.display_name()),
                );
            }
            if let SceneValue::Object(nested_values) = value {
                let nested_fields = resolved_struct_fields(&nested);
                collect_script_inspector_meta(
                    schema,
                    &nested.def.name,
                    false,
                    nested_values.as_ref(),
                    key.clone(),
                    depth + 1,
                    Some(&nested_fields),
                    meta,
                );
            }
        }
        TypeResolution::Ambiguous(name, origins) => {
            meta.warnings.insert(
                key.clone(),
                format!("warn ambiguous struct\n{name}: {}", origins.join(" + ")),
            );
        }
        TypeResolution::Missing => {}
    }
    let normalized = normalized_type(ty);
    if let Some(inner_ty) = generic_inner(normalized.as_str(), "Vec") {
        meta.default_children.insert(
            key.clone(),
            default_scene_value_for_type(schema, &inner_ty, depth + 1),
        );
        if let SceneValue::Array(values) = value {
            for (idx, item) in values.iter().enumerate() {
                collect_script_value_meta(
                    schema,
                    &inner_ty,
                    item,
                    format!("{key}[{idx}]"),
                    depth + 1,
                    meta,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_script_node_path_keys(
    schema: &ScriptSchema,
    struct_name: &str,
    require_expose: bool,
    values: &[(SceneFieldName, SceneValue)],
    prefix: String,
    depth: usize,
    override_fields: Option<&[RawScriptField]>,
    out: &mut Vec<String>,
) {
    if depth > MAX_INSPECTOR_DEPTH {
        return;
    }
    let Some(schema_fields) = schema.struct_fields(struct_name) else {
        return;
    };
    let fields = override_fields.unwrap_or(schema_fields);
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
        } else if let TypeResolution::Found(nested) = resolve_script_struct(schema, &field.ty)
            && let Some((_, SceneValue::Object(nested_values))) = values.get(idx)
        {
            let nested_fields = resolved_struct_fields(&nested);
            collect_script_node_path_keys(
                schema,
                &nested.def.name,
                false,
                nested_values.as_ref(),
                key,
                depth + 1,
                Some(&nested_fields),
                out,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_script_enum_path_options(
    schema: &ScriptSchema,
    struct_name: &str,
    require_expose: bool,
    values: &[(SceneFieldName, SceneValue)],
    prefix: String,
    depth: usize,
    override_fields: Option<&[RawScriptField]>,
    out: &mut BTreeMap<String, Vec<String>>,
) {
    if depth > MAX_INSPECTOR_DEPTH {
        return;
    }
    let Some(schema_fields) = schema.struct_fields(struct_name) else {
        return;
    };
    let fields = override_fields.unwrap_or(schema_fields);
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
        if let TypeResolution::Found(info) = resolve_script_enum(schema, &field.ty) {
            out.insert(key, info.def.variants.clone());
        } else if let TypeResolution::Found(nested) = resolve_script_struct(schema, &field.ty)
            && let Some((_, SceneValue::Object(nested_values))) = values.get(idx)
        {
            let nested_fields = resolved_struct_fields(&nested);
            collect_script_enum_path_options(
                schema,
                &nested.def.name,
                false,
                nested_values.as_ref(),
                key,
                depth + 1,
                Some(&nested_fields),
                out,
            );
        }
    }
}

pub fn script_state_schema_warnings(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
    fields: &[(SceneFieldName, SceneValue)],
) -> BTreeMap<String, String> {
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
    let schema = parse_project_script_schema(&state.project_root, script_path.as_ref(), &source);
    let mut out = BTreeMap::new();
    collect_script_schema_warnings(
        &schema,
        &struct_name,
        true,
        fields,
        String::new(),
        0,
        None,
        &mut out,
    );
    out
}

#[allow(clippy::too_many_arguments)]
fn collect_script_schema_warnings(
    schema: &ScriptSchema,
    struct_name: &str,
    require_expose: bool,
    values: &[(SceneFieldName, SceneValue)],
    prefix: String,
    depth: usize,
    override_fields: Option<&[RawScriptField]>,
    out: &mut BTreeMap<String, String>,
) {
    if depth > MAX_INSPECTOR_DEPTH {
        return;
    }
    let Some(schema_fields) = schema.struct_fields(struct_name) else {
        return;
    };
    let fields = override_fields.unwrap_or(schema_fields);
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
        match resolve_script_struct(schema, &field.ty) {
            TypeResolution::Found(resolved) => {
                if !resolved.def.has_variant {
                    out.insert(
                        key.clone(),
                        format!(
                            "warn missing Variant derive\n{}",
                            resolved.def.display_name()
                        ),
                    );
                }
                if let Some((_, SceneValue::Object(nested_values))) = values.get(idx) {
                    let nested_fields = resolved_struct_fields(&resolved);
                    collect_script_schema_warnings(
                        schema,
                        &resolved.def.name,
                        false,
                        nested_values.as_ref(),
                        key,
                        depth + 1,
                        Some(&nested_fields),
                        out,
                    );
                }
                continue;
            }
            TypeResolution::Ambiguous(name, origins) => {
                out.insert(
                    key.clone(),
                    format!("warn ambiguous type\n{name}: {}", origins.join(" + ")),
                );
                continue;
            }
            TypeResolution::Missing => {}
        }
        match resolve_script_enum(schema, &field.ty) {
            TypeResolution::Found(resolved) => {
                if !resolved.def.has_variant {
                    out.insert(
                        key,
                        format!(
                            "warn missing Variant derive\n{}",
                            resolved.def.display_name()
                        ),
                    );
                }
            }
            TypeResolution::Ambiguous(name, origins) => {
                out.insert(
                    key,
                    format!("warn ambiguous enum\n{name}: {}", origins.join(" + ")),
                );
            }
            TypeResolution::Missing => {}
        }
    }
}

#[derive(Clone)]
struct ScriptSchema {
    root_module: String,
    imports: ScriptImports,
    structs: BTreeMap<String, Vec<ScriptStruct>>,
    enums: BTreeMap<String, Vec<ScriptEnum>>,
}

#[derive(Clone, Default)]
struct ScriptImports {
    named: BTreeMap<String, String>,
    globs: Vec<String>,
    script_modules_glob: bool,
}

#[derive(Clone)]
struct ScriptStruct {
    name: String,
    generic_params: Vec<String>,
    module: String,
    short_module: String,
    origin: String,
    has_variant: bool,
    fields: Vec<RawScriptField>,
}

#[derive(Clone)]
struct ScriptEnum {
    name: String,
    generic_params: Vec<String>,
    module: String,
    short_module: String,
    origin: String,
    has_variant: bool,
    default: String,
    variants: Vec<String>,
}

#[derive(Clone)]
struct RawScriptField {
    name: String,
    ty: String,
    default_attr: Option<String>,
    exposed: bool,
}

trait ScriptTypeDef {
    fn name(&self) -> &str;
    fn module(&self) -> &str;
    fn short_module(&self) -> &str;
    fn origin(&self) -> &str;
}

impl ScriptTypeDef for ScriptStruct {
    fn name(&self) -> &str {
        &self.name
    }

    fn module(&self) -> &str {
        &self.module
    }

    fn short_module(&self) -> &str {
        &self.short_module
    }

    fn origin(&self) -> &str {
        &self.origin
    }
}

impl ScriptTypeDef for ScriptEnum {
    fn name(&self) -> &str {
        &self.name
    }

    fn module(&self) -> &str {
        &self.module
    }

    fn short_module(&self) -> &str {
        &self.short_module
    }

    fn origin(&self) -> &str {
        &self.origin
    }
}

impl ScriptStruct {
    fn display_name(&self) -> String {
        format!("{} @ {}", self.name, self.origin)
    }
}

impl ScriptEnum {
    fn display_name(&self) -> String {
        format!("{} @ {}", self.name, self.origin)
    }
}

impl ScriptSchema {
    fn struct_fields(&self, struct_name: &str) -> Option<&[RawScriptField]> {
        self.structs
            .get(struct_name)
            .and_then(|defs| defs.first())
            .map(|def| def.fields.as_slice())
    }
}

enum TypeResolution<'a, T> {
    Found(ResolvedScriptType<'a, T>),
    Ambiguous(String, Vec<String>),
    Missing,
}

struct ResolvedScriptType<'a, T> {
    def: &'a T,
    type_args: Vec<String>,
}

struct StructHeader {
    name: String,
    generic_params: Vec<String>,
}

fn parse_state_struct_name(source: &str) -> Option<String> {
    let mut saw_state_attr = false;
    for line in source.lines() {
        let line = strip_line_comment(line).trim();
        let (attrs, rest) = split_leading_attrs(line);
        if attrs.iter().any(|attr| is_state_attr(attr)) {
            saw_state_attr = true;
            if let Some(header) = parse_struct_header(rest) {
                return Some(header.name);
            }
        }
        if saw_state_attr {
            if rest.is_empty() {
                continue;
            }
            return parse_struct_header(rest).map(|header| header.name);
        }
    }
    None
}

fn parse_struct_name(line: &str) -> Option<String> {
    parse_struct_header(line).map(|header| header.name)
}

fn parse_struct_header(line: &str) -> Option<StructHeader> {
    let mut parts = line.split_whitespace().peekable();
    while let Some(part) = parts.next() {
        if part == "struct" {
            let raw = parts.next()?.trim_matches(['{', '(', ';']).trim();
            let name = raw.split('<').next().unwrap_or(raw).trim().to_string();
            if name.is_empty() {
                return None;
            }
            let generic_params = parse_generic_params(raw);
            return Some(StructHeader {
                name,
                generic_params,
            });
        }
    }
    None
}

fn parse_script_schema(
    source: &str,
    module: &str,
    short_module: &str,
    origin: &str,
    root_module: &str,
) -> ScriptSchema {
    let mut structs = BTreeMap::new();
    let mut enums = BTreeMap::new();
    let mut pending_derives = Vec::<String>::new();
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        let clean = strip_line_comment(line).trim();
        let (attrs, rest) = split_leading_attrs(clean);
        for attr in attrs {
            if attr.starts_with("#[derive") {
                pending_derives.push(attr.to_string());
            }
        }
        let has_variant = pending_derives
            .iter()
            .any(|attr| attr_has_variant_derive(attr));
        if let Some(header) = parse_struct_header(rest) {
            let name = header.name;
            let fields = parse_script_struct_fields(source, &name);
            structs
                .entry(name.clone())
                .or_insert_with(Vec::new)
                .push(ScriptStruct {
                    name,
                    generic_params: header.generic_params,
                    module: module.to_string(),
                    short_module: short_module.to_string(),
                    origin: origin.to_string(),
                    has_variant,
                    fields,
                });
            pending_derives.clear();
        } else if let Some(header) = parse_enum_header(rest) {
            let name = header.name;
            let mut info = parse_enum_info(line, &mut lines);
            info.name = name.clone();
            info.generic_params = header.generic_params;
            info.module = module.to_string();
            info.short_module = short_module.to_string();
            info.origin = origin.to_string();
            info.has_variant = has_variant;
            enums.entry(name).or_insert_with(Vec::new).push(info);
            pending_derives.clear();
        } else if !rest.is_empty() && !rest.starts_with("#[") {
            pending_derives.clear();
        }
    }
    ScriptSchema {
        root_module: root_module.to_string(),
        imports: parse_script_imports(source),
        structs,
        enums,
    }
}

fn parse_project_script_schema(
    project_root: &str,
    script_path: &str,
    source: &str,
) -> ScriptSchema {
    let root_module = module_name_from_script_path(script_path);
    let root_short_module = module_short_name_from_script_path(script_path);
    let mut schema = parse_script_schema(
        source,
        &root_module,
        &root_short_module,
        script_path,
        &root_module,
    );
    let res_dir = Path::new(project_root).join("res");
    let skip_rel = script_path.trim_start_matches("res://");
    merge_script_schema_dir(&mut schema, &res_dir, &res_dir, skip_rel);
    schema
}

fn merge_script_schema_dir(schema: &mut ScriptSchema, dir: &Path, res_dir: &Path, skip_rel: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            merge_script_schema_dir(schema, &path, res_dir, skip_rel);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let rel = path
            .strip_prefix(res_dir)
            .ok()
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| path.to_string_lossy().replace('\\', "/"));
        if rel == skip_rel {
            continue;
        }
        let module = module_name_from_rel(&rel);
        let short_module = module_short_name_from_rel(&rel);
        let origin = format!("res://{rel}");
        let Some(other) = parse_cached_script_schema_file(
            &path,
            &module,
            &short_module,
            &origin,
            &schema.root_module,
        ) else {
            continue;
        };
        for (name, mut defs) in other.structs {
            schema.structs.entry(name).or_default().append(&mut defs);
        }
        for (name, mut defs) in other.enums {
            schema.enums.entry(name).or_default().append(&mut defs);
        }
    }
}

fn parse_cached_script_schema_file(
    path: &Path,
    module: &str,
    short_module: &str,
    origin: &str,
    root_module: &str,
) -> Option<ScriptSchema> {
    let key = path.to_string_lossy().replace('\\', "/");
    let sig = script_file_sig(path)?;
    let cache = SCRIPT_FILE_SCHEMA_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(cached) = cache.get(&key)
        && cached.sig == sig
    {
        return Some(cached.schema.clone());
    }
    let source = fs::read_to_string(path).ok()?;
    let schema = parse_script_schema(&source, module, short_module, origin, root_module);
    if let Ok(mut cache) = cache.lock() {
        cache.insert(
            key,
            CachedScriptFileSchema {
                sig,
                schema: schema.clone(),
            },
        );
    }
    Some(schema)
}

fn script_file_sig(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_secs())
        .unwrap_or(0);
    Some(format!("{}|{modified}", meta.len()))
}

fn changed_script_abs_path(project_root: &str, path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let path = Path::new(&normalized);
    if path.is_absolute() {
        path.to_string_lossy().replace('\\', "/")
    } else {
        Path::new(project_root)
            .join(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

fn attr_has_variant_derive(attr: &str) -> bool {
    let Some(inner) = attr
        .trim()
        .strip_prefix("#[derive")
        .and_then(|value| value.strip_suffix(']'))
    else {
        return false;
    };
    inner
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .map(str::trim)
        .any(|item| matches!(item, "Variant" | "DeriveVariant"))
}

fn parse_script_imports(source: &str) -> ScriptImports {
    let mut imports = ScriptImports::default();
    for line in source.lines() {
        let line = strip_line_comment(line).trim();
        let Some(mut rest) = line.strip_prefix("use ") else {
            continue;
        };
        rest = rest.trim_end_matches(';').trim();
        if rest == "crate::script_modules::*" {
            imports.script_modules_glob = true;
            continue;
        }
        if let Some(module) = rest
            .strip_prefix("crate::")
            .and_then(|value| value.strip_suffix("::*"))
        {
            imports.globs.push(module.to_string());
            continue;
        }
        if let Some(inner) = rest
            .strip_prefix("crate::")
            .and_then(|value| value.strip_suffix('}'))
            .and_then(|value| value.split_once("::{"))
        {
            let (module, names) = inner;
            for name in names
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
            {
                let name = name
                    .split_once(" as ")
                    .map(|(_, alias)| alias)
                    .unwrap_or(name);
                imports.named.insert(name.to_string(), module.to_string());
            }
            continue;
        }
        if let Some((module, name)) = rest
            .strip_prefix("crate::")
            .and_then(|value| value.rsplit_once("::"))
        {
            let name = name
                .split_once(" as ")
                .map(|(_, alias)| alias)
                .unwrap_or(name);
            imports.named.insert(name.to_string(), module.to_string());
        }
    }
    imports
}

fn parse_enum_name(line: &str) -> Option<String> {
    parse_enum_header(line).map(|header| header.name)
}

fn parse_enum_header(line: &str) -> Option<StructHeader> {
    let mut parts = line.split_whitespace().peekable();
    while let Some(part) = parts.next() {
        if part == "enum" {
            let raw = parts.next()?.trim_matches(['{', ';']).trim();
            let name = raw.split('<').next().unwrap_or(raw).trim().to_string();
            if name.is_empty() {
                return None;
            }
            let generic_params = parse_generic_params(raw);
            return Some(StructHeader {
                name,
                generic_params,
            });
        }
    }
    None
}

fn parse_generic_params(raw: &str) -> Vec<String> {
    let Some(inner) = raw
        .split_once('<')
        .and_then(|(_, rest)| rest.rsplit_once('>').map(|(inner, _)| inner))
    else {
        return Vec::new();
    };
    split_top_level_csv(inner)
        .into_iter()
        .filter_map(|param| {
            let name = param.split([':', '=', ' ']).next().unwrap_or("").trim();
            (!name.is_empty()
                && name
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_'))
            .then(|| name.to_string())
        })
        .collect()
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
    ScriptEnum {
        name: String::new(),
        generic_params: Vec::new(),
        module: String::new(),
        short_module: String::new(),
        origin: String::new(),
        has_variant: false,
        default,
        variants,
    }
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
    if let Some(fields) = parse_tuple_struct_fields(&lines, idx, struct_name) {
        return fields;
    }
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

fn parse_tuple_struct_fields(
    lines: &[&str],
    start_idx: usize,
    struct_name: &str,
) -> Option<Vec<RawScriptField>> {
    let mut content = String::new();
    let mut saw_open = false;
    let mut depth = 0_i32;
    let mut idx = start_idx;
    while idx < lines.len() {
        let line = strip_line_comment(lines[idx]);
        if !saw_open {
            let start = line.find("struct")?;
            let after_struct = &line[start + "struct".len()..];
            let name_pos = after_struct.find(struct_name)?;
            let after_name = &after_struct[name_pos + struct_name.len()..];
            if after_name.trim_start().starts_with('{') {
                return None;
            }
            let open_pos = after_name.find('(')?;
            saw_open = true;
            let rest = &after_name[open_pos + 1..];
            depth = 1;
            content.push_str(rest);
        } else {
            content.push('\n');
            content.push_str(line);
        }
        let mut closed = false;
        let mut end_at = content.len();
        for (pos, ch) in content.char_indices() {
            match ch {
                '(' | '<' | '[' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth <= 0 {
                        end_at = pos;
                        closed = true;
                        break;
                    }
                }
                '>' | ']' => depth -= 1,
                _ => {}
            }
        }
        if closed {
            content.truncate(end_at);
            break;
        }
        idx += 1;
    }
    if !saw_open {
        return None;
    }
    let fields = split_top_level_csv(&content)
        .into_iter()
        .enumerate()
        .filter_map(|(idx, ty)| {
            let ty = ty.trim();
            if ty.is_empty() {
                return None;
            }
            let ty = ty
                .strip_prefix("pub ")
                .or_else(|| {
                    ty.strip_prefix("pub(")
                        .and_then(|rest| rest.split_once(')').map(|(_, after)| after.trim()))
                })
                .unwrap_or(ty)
                .trim();
            Some(RawScriptField {
                name: idx.to_string(),
                ty: ty.to_string(),
                default_attr: None,
                exposed: true,
            })
        })
        .collect::<Vec<_>>();
    Some(fields)
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

fn split_top_level_csv(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut angle = 0_i32;
    let mut paren = 0_i32;
    let mut bracket = 0_i32;
    for (idx, ch) in value.char_indices() {
        match ch {
            '<' => angle += 1,
            '>' => angle -= 1,
            '(' => paren += 1,
            ')' => paren -= 1,
            '[' => bracket += 1,
            ']' => bracket -= 1,
            ',' if angle == 0 && paren == 0 && bracket == 0 => {
                out.push(value[start..idx].trim().to_string());
                start = idx + 1;
            }
            _ => {}
        }
    }
    out.push(value[start..].trim().to_string());
    out
}

fn parse_default_attr(line: &str) -> Option<String> {
    let inner = line.strip_prefix("#[default")?.strip_suffix(']')?.trim();
    if let Some(value) = inner.strip_prefix('=') {
        return Some(value.trim().to_string());
    }
    if let Some(value) = inner
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    {
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
    let Some(fields) = schema.struct_fields(struct_name) else {
        return Vec::new();
    };
    script_struct_default_fields_from_fields(schema, fields, depth, require_expose)
}

fn script_struct_default_fields_from_fields(
    schema: &ScriptSchema,
    fields: &[RawScriptField],
    depth: usize,
    require_expose: bool,
) -> Vec<(SceneFieldName, SceneValue)> {
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
    if let Some((enum_ty, variant)) = value.rsplit_once("::")
        && script_enum_type_name(schema, enum_ty).is_some()
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
    last_type_segment(&normalized_type(ty)) == "Color"
}

fn is_node_ref_type(ty: &str) -> bool {
    let ty = normalized_type(ty);
    let ty = generic_inner(ty.as_str(), "Option").unwrap_or(ty);
    last_type_segment(&ty) == "NodeID"
}

fn script_struct_type_name(schema: &ScriptSchema, ty: &str) -> Option<String> {
    if let TypeResolution::Found(def) = resolve_script_struct(schema, ty) {
        return Some(def.def.name.clone());
    }
    None
}

fn script_enum_type_name(schema: &ScriptSchema, ty: &str) -> Option<String> {
    if let TypeResolution::Found(def) = resolve_script_enum(schema, ty) {
        return Some(def.def.name.clone());
    }
    None
}

fn resolve_script_struct<'a>(
    schema: &'a ScriptSchema,
    ty: &str,
) -> TypeResolution<'a, ScriptStruct> {
    resolve_script_type(schema, ty, &schema.structs)
}

fn resolve_script_enum<'a>(schema: &'a ScriptSchema, ty: &str) -> TypeResolution<'a, ScriptEnum> {
    resolve_script_type(schema, ty, &schema.enums)
}

fn resolve_script_type<'a, T: ScriptTypeDef>(
    schema: &'a ScriptSchema,
    ty: &str,
    defs_by_name: &'a BTreeMap<String, Vec<T>>,
) -> TypeResolution<'a, T> {
    let ty = normalized_type(ty);
    let ty = generic_inner(ty.as_str(), "Option").unwrap_or(ty);
    if ty.starts_with("Vec<") {
        return TypeResolution::Missing;
    }
    let (name, type_args) = type_name_and_args(&ty);
    if ty.contains("::")
        && let Some(module) = module_from_qualified_type(&ty)
    {
        return resolve_defs_in_module(defs_by_name, &name, &module, &type_args);
    }
    if let TypeResolution::Found(found) =
        resolve_defs_in_module(defs_by_name, &name, &schema.root_module, &type_args)
    {
        return TypeResolution::Found(found);
    }
    if let Some(module) = schema.imports.named.get(&name)
        && let TypeResolution::Found(found) =
            resolve_defs_in_module(defs_by_name, &name, module, &type_args)
    {
        return TypeResolution::Found(found);
    }
    for module in &schema.imports.globs {
        if let TypeResolution::Found(found) =
            resolve_defs_in_module(defs_by_name, &name, module, &type_args)
        {
            return TypeResolution::Found(found);
        }
    }
    resolve_defs_global(defs_by_name, &name, &type_args)
}

fn resolve_defs_in_module<'a, T: ScriptTypeDef>(
    defs_by_name: &'a BTreeMap<String, Vec<T>>,
    name: &str,
    module: &str,
    type_args: &[String],
) -> TypeResolution<'a, T> {
    let matches = defs_by_name
        .get(name)
        .into_iter()
        .flat_map(|defs| defs.iter())
        .filter(|def| def.module() == module || def.short_module() == module)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => TypeResolution::Missing,
        [only] => TypeResolution::Found(ResolvedScriptType {
            def: *only,
            type_args: type_args.to_vec(),
        }),
        many => TypeResolution::Ambiguous(
            name.to_string(),
            many.iter().map(|def| def.origin().to_string()).collect(),
        ),
    }
}

fn resolve_defs_global<'a, T: ScriptTypeDef>(
    defs_by_name: &'a BTreeMap<String, Vec<T>>,
    name: &str,
    type_args: &[String],
) -> TypeResolution<'a, T> {
    let Some(defs) = defs_by_name.get(name) else {
        return TypeResolution::Missing;
    };
    match defs.as_slice() {
        [] => TypeResolution::Missing,
        [only] => TypeResolution::Found(ResolvedScriptType {
            def: only,
            type_args: type_args.to_vec(),
        }),
        many => TypeResolution::Ambiguous(
            name.to_string(),
            many.iter().map(|def| def.origin().to_string()).collect(),
        ),
    }
}

fn type_name_and_args(ty: &str) -> (String, Vec<String>) {
    let name_part = ty.split('<').next().unwrap_or(ty);
    let name = last_type_segment(name_part).to_string();
    let args = ty
        .split_once('<')
        .and_then(|(_, rest)| rest.rsplit_once('>').map(|(inner, _)| inner))
        .map(split_top_level_csv)
        .unwrap_or_default();
    (name, args)
}

fn resolved_struct_fields(resolved: &ResolvedScriptType<'_, ScriptStruct>) -> Vec<RawScriptField> {
    if resolved.def.generic_params.is_empty() || resolved.type_args.is_empty() {
        return resolved.def.fields.clone();
    }
    resolved
        .def
        .fields
        .iter()
        .map(|field| RawScriptField {
            name: field.name.clone(),
            ty: substitute_generic_type(
                &field.ty,
                &resolved.def.generic_params,
                &resolved.type_args,
            ),
            default_attr: field.default_attr.clone(),
            exposed: field.exposed,
        })
        .collect()
}

fn substitute_generic_type(ty: &str, params: &[String], args: &[String]) -> String {
    if params.is_empty() || args.is_empty() {
        return ty.to_string();
    }
    let mut out = String::new();
    let mut token = String::new();
    for ch in ty.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            token.push(ch);
            continue;
        }
        push_substituted_token(&mut out, &mut token, params, args);
        out.push(ch);
    }
    push_substituted_token(&mut out, &mut token, params, args);
    out
}

fn push_substituted_token(
    out: &mut String,
    token: &mut String,
    params: &[String],
    args: &[String],
) {
    if token.is_empty() {
        return;
    }
    if let Some(idx) = params.iter().position(|param| param == token)
        && let Some(arg) = args.get(idx)
    {
        out.push_str(arg);
    } else {
        out.push_str(token);
    }
    token.clear();
}

fn module_from_qualified_type(ty: &str) -> Option<String> {
    let rest = ty.strip_prefix("crate::")?;
    let (module, _name) = rest.rsplit_once("::")?;
    Some(module.to_string())
}

fn module_name_from_script_path(path: &str) -> String {
    module_name_from_rel(path.trim_start_matches("res://"))
}

fn module_short_name_from_script_path(path: &str) -> String {
    module_short_name_from_rel(path.trim_start_matches("res://"))
}

fn module_name_from_rel(rel: &str) -> String {
    let mut out = String::with_capacity(rel.len());
    for ch in rel.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
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
    if name.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name
}

fn module_short_name_from_rel(rel: &str) -> String {
    module_name_from_rel(rel.strip_suffix(".rs").unwrap_or(rel))
}

fn last_type_segment(ty: &str) -> &str {
    ty.rsplit("::").next().unwrap_or(ty)
}

fn script_type_label(schema: &ScriptSchema, ty: &str) -> String {
    let ty = normalized_type(ty);
    if let Some(inner) = generic_inner(ty.as_str(), "Option") {
        if is_node_ref_type(&inner) {
            return "Node".to_string();
        }
        return script_type_label(schema, &inner);
    }
    if let Some(inner) = generic_inner(ty.as_str(), "Vec") {
        return format!("Array({})", script_type_label(schema, &inner));
    }
    match ty.as_str() {
        "bool" => "Bool".to_string(),
        "f32" | "f64" => "F32".to_string(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "I32".to_string(),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "U32".to_string(),
        "String" | "Arc<str>" | "std::sync::Arc<str>" | "Cow<'static,str>" => "String".to_string(),
        "Vector2" | "perro_api::prelude::Vector2" => "Vec2".to_string(),
        "Vector3" | "perro_api::prelude::Vector3" => "Vec3".to_string(),
        "Vector4" | "perro_api::prelude::Vector4" => "Vec4".to_string(),
        "Quaternion" | "perro_api::prelude::Quaternion" => "Quat".to_string(),
        "Color" | "perro_api::prelude::Color" => "Color".to_string(),
        _ if is_node_ref_type(&ty) => "Node".to_string(),
        _ if let TypeResolution::Found(def) = resolve_script_enum(schema, &ty) => {
            format!("Enum({})", def.def.default)
        }
        _ if matches!(resolve_script_struct(schema, &ty), TypeResolution::Found(_)) => {
            "Object".to_string()
        }
        _ => "Unknown".to_string(),
    }
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
        "Quaternion" | "perro_api::prelude::Quaternion" => SceneValue::Vec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
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
        _ if let TypeResolution::Found(def) = resolve_script_struct(schema, &ty) => {
            let fields = resolved_struct_fields(&def);
            SceneValue::Object(Cow::Owned(script_struct_default_fields_from_fields(
                schema,
                &fields,
                depth + 1,
                false,
            )))
        }
        _ if let TypeResolution::Found(def) = resolve_script_enum(schema, &ty) => {
            let default_variant = def.def.default.clone();
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
    source: &str,
    name: &str,
    value: &SceneValue,
    path: &mut Vec<ValuePathStep>,
    depth: usize,
    ctx: &ValueRowContext<'_>,
) {
    let composite = matches!(value, SceneValue::Array(_) | SceneValue::Object(_));
    let local_path_key = value_path_key(path);
    let path_key = format!("{source}:{local_path_key}");
    let expanded = composite && ctx.expanded_paths.iter().any(|item| item == &path_key);
    let color_preview = color_preview_for_value(name, value, &path_key, ctx.color_paths);
    let enum_options = ctx.enum_options.get(&path_key).cloned().unwrap_or_default();
    let warning = ctx.warnings.get(&path_key).cloned();
    let kind = if color_preview.is_some() {
        "Color"
    } else if !enum_options.is_empty() {
        "Enum"
    } else if let Some(kind) = ctx.kind_overrides.get(&path_key) {
        kind.as_str()
    } else if matches!(value, SceneValue::Key(_))
        && !ctx.node_paths.iter().any(|item| item == &path_key)
    {
        "Key"
    } else {
        scene_value_kind(value)
    };
    let components = scene_value_component_texts_for_kind(value, kind, ctx.quat_mode);
    rows.push(InspectorValueRow {
        source: source.to_string(),
        depth,
        path: path.clone(),
        path_key: path_key.clone(),
        name: name.to_string(),
        kind: kind.to_string(),
        value: if !enum_options.is_empty() {
            scene_value_enum_text(value)
        } else if composite {
            scene_value_summary(value, expanded)
        } else {
            scene_value_edit_text(value)
        },
        components,
        color_preview,
        enum_options,
        default_child: ctx.default_children.get(&path_key).cloned(),
        editable: !composite,
        expandable: composite,
        addable: matches!(value, SceneValue::Array(_)),
        removable: matches!(path.last(), Some(ValuePathStep::Index(_))),
    });
    if let Some(warning) = warning {
        let mut lines = warning.lines();
        rows.push(InspectorValueRow {
            source: source.to_string(),
            depth: depth + 1,
            path: path.clone(),
            path_key: format!("{path_key}.warn"),
            name: format!(
                "{}! {}",
                "  ".repeat(depth + 1),
                lines.next().unwrap_or("warn")
            ),
            kind: "Warn".to_string(),
            value: lines.collect::<Vec<_>>().join(" "),
            components: Vec::new(),
            color_preview: None,
            enum_options: Vec::new(),
            default_child: None,
            editable: false,
            expandable: false,
            addable: false,
            removable: false,
        });
    }
    if !expanded || depth >= MAX_INSPECTOR_DEPTH {
        return;
    }
    match value {
        SceneValue::Array(values) => {
            for (idx, item) in values.iter().enumerate() {
                path.push(ValuePathStep::Index(idx));
                push_value_rows(
                    rows,
                    source,
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
                push_value_rows(rows, source, field.as_ref(), item, path, depth + 1, ctx);
                path.pop();
            }
        }
        _ => {}
    }
}

fn scene_value_enum_text(value: &SceneValue) -> String {
    match value {
        SceneValue::Key(value) => value.as_ref().to_string(),
        SceneValue::Str(value) => value.as_ref().to_string(),
        _ => scene_value_edit_text(value),
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

pub fn scene_value_component_texts_for_kind(
    value: &SceneValue,
    kind: &str,
    quat_mode: &str,
) -> Vec<String> {
    if kind == "Color" {
        return Vec::new();
    }
    if kind == "Quat"
        && quat_mode == "euler"
        && let SceneValue::Vec4 { x, y, z, w } = value
    {
        return quat_to_euler_deg_components(*x, *y, *z, *w);
    }
    scene_value_component_texts(value)
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
        let doc = cached_scene_doc(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        Some(inspector_display_rows_for_node(state, node))
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
    } else if row.kind == "Quat" {
        let mut values = Vec::new();
        for component in 0..row.components.len() {
            let Some(text) = read_text_box(ctx, &format!("inspector_var_{idx}_{component}_box"))
            else {
                return;
            };
            let Ok(value) = text.trim().parse::<f32>() else {
                set_log(ctx, "script var parse fail\nbad quat component");
                return;
            };
            values.push(value);
        }
        let euler = with_state!(ctx.run, EditorState, ctx.id, |state| {
            state.inspector_rotation_mode == "euler"
        });
        if euler {
            let [x, y, z] = values.as_slice() else {
                set_log(ctx, "script var parse fail\nbad euler component count");
                return;
            };
            let quat = Quaternion::from_euler_xyz(x.to_radians(), y.to_radians(), z.to_radians());
            format!(
                "({}, {}, {}, {})",
                format_compact_f32(quat.x),
                format_compact_f32(quat.y),
                format_compact_f32(quat.z),
                format_compact_f32(quat.w)
            )
        } else {
            format!(
                "({})",
                values
                    .iter()
                    .map(|value| format_compact_f32(*value))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
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
    let value_for_preview = value.clone();
    let script_preview = if row.source == "script" {
        with_state!(ctx.run, EditorState, ctx.id, |state| {
            let key = state.selected_key?;
            let doc = cached_scene_doc(&state.doc_text);
            let node = doc
                .scene
                .nodes
                .iter()
                .find(|node| node.key.as_u32() == key)?;
            let fields = inspector_script_var_fields_for_node(state, node);
            let member = script_member_path_for_row(&fields, &row.path)?;
            let variant = scene_value_to_preview_variant(&value_for_preview, &doc, state);
            Some((key, member, variant))
        })
    } else {
        None
    };
    let preview_field = if row.source == "scene" && row.path.len() == 1 {
        Some(row.name.trim().to_string())
    } else {
        None
    };
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        let mut doc = cached_scene_doc(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if row.source == "script" {
            let defaults = inspector_script_var_default_fields_for_node(state, node);
            let mut fields = inspector_script_var_fields_for_node(state, node);
            if !set_value_at_path(&mut fields, &row.path, value) {
                return false;
            }
            if !write_script_var_override(node.script_vars.to_mut(), &defaults, &fields, &row.path)
            {
                return false;
            }
        } else {
            let mut fields = inspector_scene_value_fields_for_node(node);
            if !set_value_at_path(&mut fields, &row.path, value) {
                return false;
            }
            if !write_scene_field_override(node.data.fields.to_mut(), &fields, &row.path) {
                return false;
            }
        }
        set_state_scene_doc(state, &doc);
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
        let preview_synced = if let Some((key, member, variant)) = script_preview {
            preview_node_for_key(ctx, key)
                .map(|id| {
                    ctx.run.Scripts().set_var(id, member, variant);
                    true
                })
                .unwrap_or(false)
        } else {
            preview_field
                .as_deref()
                .is_some_and(|field| sync_selected_preview_field(ctx, field, &value_for_preview))
        };
        if !preview_synced {
            rebuild_preview(ctx);
        }
        refresh_all(ctx);
    }
}

fn script_member_path_for_row(
    fields: &[(SceneFieldName, SceneValue)],
    path: &[ValuePathStep],
) -> Option<String> {
    let Some(ValuePathStep::Root(idx)) = path.first() else {
        return None;
    };
    let mut out = fields.get(*idx)?.0.as_ref().to_string();
    for step in &path[1..] {
        match step {
            ValuePathStep::Field(name) => {
                out.push('.');
                out.push_str(name);
            }
            ValuePathStep::Index(_) => return None,
            ValuePathStep::Root(_) => return None,
        }
    }
    Some(out)
}

fn scene_value_to_preview_variant(
    value: &SceneValue,
    doc: &SceneDoc,
    state: &EditorState,
) -> perro_api::variant::Variant {
    match value {
        SceneValue::Bool(value) => perro_api::variant::Variant::from(*value),
        SceneValue::I32(value) => perro_api::variant::Variant::from(*value),
        SceneValue::F32(value) => perro_api::variant::Variant::from(*value),
        SceneValue::Vec2 { x, y } => perro_api::variant::Variant::from(Vector2::new(*x, *y)),
        SceneValue::Vec3 { x, y, z } => perro_api::variant::Variant::from(Vector3::new(*x, *y, *z)),
        SceneValue::Vec4 { x, y, z, w } => perro_api::variant::Variant::Array(vec![
            perro_api::variant::Variant::from(*x),
            perro_api::variant::Variant::from(*y),
            perro_api::variant::Variant::from(*z),
            perro_api::variant::Variant::from(*w),
        ]),
        SceneValue::Str(value) => perro_api::variant::Variant::from(value.to_string()),
        SceneValue::Hashed(value) => perro_api::variant::Variant::from(*value),
        SceneValue::Key(value) => {
            let raw = value.as_ref();
            if let Some(id) = preview_node_ref(raw, doc, state) {
                perro_api::variant::Variant::from(id)
            } else {
                perro_api::variant::Variant::from(raw.to_string())
            }
        }
        SceneValue::Array(values) => perro_api::variant::Variant::Array(
            values
                .iter()
                .map(|value| scene_value_to_preview_variant(value, doc, state))
                .collect(),
        ),
        SceneValue::Object(values) => {
            let mut out = BTreeMap::new();
            for (name, value) in values.iter() {
                out.insert(
                    Arc::<str>::from(name.as_ref()),
                    scene_value_to_preview_variant(value, doc, state),
                );
            }
            perro_api::variant::Variant::Object(out)
        }
    }
}

fn preview_node_ref(raw: &str, doc: &SceneDoc, state: &EditorState) -> Option<NodeID> {
    let raw = raw.trim();
    if matches!(raw, "" | "null" | "none" | "-") {
        return None;
    }
    let key = raw
        .strip_prefix('#')
        .and_then(|value| value.parse::<u32>().ok())
        .or_else(|| {
            let name = raw.trim_start_matches('@');
            doc.scene
                .key_names
                .iter()
                .position(|item| item.as_ref() == name)
                .map(|idx| idx as u32)
        })?;
    state
        .preview_node_keys
        .iter()
        .position(|item| *item == key)
        .and_then(|idx| state.preview_node_ids.get(idx).copied())
        .map(NodeID::from_u64)
}

pub fn mutate_selected_inspector_array<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    add: bool,
) {
    let row = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        let doc = cached_scene_doc(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        inspector_display_rows_for_node(state, node)
            .get(idx)
            .cloned()
    });
    let Some(row) = row else {
        return;
    };
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        let mut doc = cached_scene_doc(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if row.source == "script" {
            let defaults = inspector_script_var_default_fields_for_node(state, node);
            let mut fields = inspector_script_var_fields_for_node(state, node);
            let ok = if add {
                add_value_at_path(&mut fields, &row)
            } else {
                remove_value_at_path(&mut fields, &row.path)
            };
            if !ok
                || !write_script_var_override(
                    node.script_vars.to_mut(),
                    &defaults,
                    &fields,
                    &row.path,
                )
            {
                return false;
            }
        } else {
            let mut fields = inspector_scene_value_fields_for_node(node);
            let ok = if add {
                add_value_at_path(&mut fields, &row)
            } else {
                remove_value_at_path(&mut fields, &row.path)
            };
            if !ok || !write_scene_field_override(node.data.fields.to_mut(), &fields, &row.path) {
                return false;
            }
        }
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = if add {
            format!("array add\n{}", row.name.trim())
        } else {
            format!("array rm\n{}", row.name.trim())
        };
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn toggle_selected_inspector_bitmask_bit<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    bit: usize,
) {
    let Some(mask) = current_inspector_bitmask(ctx, idx) else {
        return;
    };
    if !(1..=32).contains(&bit) {
        return;
    }
    let layer = BitMask::layer(bit as u8);
    let next = if BitMask::from_bits(mask).intersects(layer) {
        BitMask::from_bits(mask).popped(bit as u8).bits()
    } else {
        BitMask::from_bits(mask).pushed(bit as u8).bits()
    };
    write_selected_inspector_bitmask(ctx, idx, next);
}

pub fn set_selected_inspector_bitmask_all<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    all: bool,
) {
    write_selected_inspector_bitmask(
        ctx,
        idx,
        if all {
            BitMask::ALL.bits()
        } else {
            BitMask::NONE.bits()
        },
    );
}

fn current_inspector_bitmask<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) -> Option<u32> {
    with_state!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        let doc = cached_scene_doc(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        let row = inspector_display_rows_for_node(state, node)
            .get(idx)?
            .clone();
        if row.kind != "BitMask" {
            return None;
        }
        Some(scene_value_bitmask_from_text(&row.value))
    })
}

fn write_selected_inspector_bitmask<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    value: u32,
) {
    let row = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        let doc = cached_scene_doc(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        inspector_display_rows_for_node(state, node)
            .get(idx)
            .cloned()
    });
    let Some(row) = row else {
        return;
    };
    let scene_value = SceneValue::Key(SceneValueKey::from(bitmask_scene_text(value)));
    let value_for_preview = scene_value.clone();
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        let mut doc = cached_scene_doc(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if row.source == "script" {
            let defaults = inspector_script_var_default_fields_for_node(state, node);
            let mut fields = inspector_script_var_fields_for_node(state, node);
            if !set_value_at_path(&mut fields, &row.path, scene_value) {
                return false;
            }
            if !write_script_var_override(node.script_vars.to_mut(), &defaults, &fields, &row.path)
            {
                return false;
            }
        } else {
            let mut fields = inspector_scene_value_fields_for_node(node);
            if !set_value_at_path(&mut fields, &row.path, scene_value) {
                return false;
            }
            if !write_scene_field_override(node.data.fields.to_mut(), &fields, &row.path) {
                return false;
            }
        }
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("bitmask edit\n{}", row.name.trim());
        true
    })
    .unwrap_or(false);
    if changed {
        crate::scripts_ui_bitmask_rs::update_inspector_bitmask_grid(ctx, idx, value);
        if !sync_bitmask_preview(ctx, &row, &value_for_preview) {
            rebuild_preview(ctx);
        }
        refresh_all(ctx);
    }
}

fn sync_bitmask_preview<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    row: &InspectorValueRow,
    value: &SceneValue,
) -> bool {
    if row.source == "scene" && row.path.len() == 1 {
        return sync_selected_preview_field(ctx, row.name.trim(), value);
    }
    false
}

pub fn scene_value_bitmask_from_text(value: &str) -> u32 {
    parse_bitmask_scene_text(value)
        .or_else(|| value.trim().parse::<i32>().ok().map(|value| value as u32))
        .unwrap_or(0)
}

fn bitmask_scene_text(bits: u32) -> String {
    let mask = BitMask::from_bits(bits);
    if mask == BitMask::ALL {
        return "all".to_string();
    }
    if mask == BitMask::NONE {
        return "none".to_string();
    }
    let layers = bitmask_layers(mask);
    let off_count = 32_usize.saturating_sub(layers.len());
    if off_count > 0 && off_count <= 4 {
        let off = (1..=32)
            .filter(|layer| !mask.intersects(BitMask::layer(*layer)))
            .map(|layer| layer.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return format!("without({off})");
    }
    format!(
        "only({})",
        layers
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn parse_bitmask_scene_text(value: &str) -> Option<u32> {
    let raw = value.trim().trim_matches('"');
    match raw {
        "all" | "ALL" => return Some(BitMask::ALL.bits()),
        "none" | "NONE" => return Some(BitMask::NONE.bits()),
        _ => {}
    }
    let (op, rest) = raw.split_once('(')?;
    let args = rest.strip_suffix(')')?.trim();
    let args = args
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(args);
    let mut layers = Vec::new();
    if !args.is_empty() {
        for arg in args.split(',') {
            let layer = arg.trim().parse::<u8>().ok()?;
            if !(1..=32).contains(&layer) {
                return None;
            }
            layers.push(layer);
        }
    }
    match op {
        "only" | "ONLY" => BitMask::try_from_layers(layers).map(BitMask::bits),
        "without" | "WITHOUT" => Some(BitMask::without(&layers).bits()),
        _ => None,
    }
}

pub fn parse_bitmask_scene_text_public(value: &str) -> Option<u32> {
    parse_bitmask_scene_text(value)
}

fn bitmask_layers(mask: BitMask) -> Vec<u8> {
    (1..=32)
        .filter(|layer| mask.intersects(BitMask::layer(*layer)))
        .collect()
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

pub fn add_value_at_path(
    fields: &mut [(SceneFieldName, SceneValue)],
    row: &InspectorValueRow,
) -> bool {
    let Some(ValuePathStep::Root(idx)) = row.path.first() else {
        return false;
    };
    let Some((_name, root_value)) = fields.get_mut(*idx) else {
        return false;
    };
    add_nested_value(root_value, &row.path[1..], default_value_for_row(row))
}

pub fn remove_value_at_path(
    fields: &mut [(SceneFieldName, SceneValue)],
    path: &[ValuePathStep],
) -> bool {
    let Some(ValuePathStep::Root(idx)) = path.first() else {
        return false;
    };
    let Some((_name, root_value)) = fields.get_mut(*idx) else {
        return false;
    };
    remove_nested_value(root_value, &path[1..])
}

fn default_value_for_row(row: &InspectorValueRow) -> SceneValue {
    if let Some(value) = row.default_child.clone() {
        return value;
    }
    if let Some(inner) = row
        .kind
        .strip_prefix("Array(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return default_value_for_kind_label(inner);
    }
    SceneValue::Str(Cow::Borrowed(""))
}

fn default_value_for_kind_label(kind: &str) -> SceneValue {
    match kind {
        "Bool" => SceneValue::Bool(false),
        "I32" | "U32" | "BitMask" => SceneValue::I32(0),
        "F32" => SceneValue::F32(0.0),
        "Vec2" => SceneValue::Vec2 { x: 0.0, y: 0.0 },
        "Vec3" => SceneValue::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        "Vec4" | "Quat" | "Color" => SceneValue::Vec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        "Node" => SceneValue::Key(SceneValueKey::from("null")),
        value if value.starts_with("Enum(") && value.ends_with(')') => {
            let default = value
                .strip_prefix("Enum(")
                .and_then(|value| value.strip_suffix(')'))
                .unwrap_or("Default");
            SceneValue::Key(SceneValueKey::from(default.to_string()))
        }
        value if value.starts_with("Asset(") => SceneValue::Str(Cow::Borrowed("")),
        _ => SceneValue::Str(Cow::Borrowed("")),
    }
}

pub fn write_script_var_override(
    overrides: &mut Vec<(SceneFieldName, SceneValue)>,
    defaults: &[(SceneFieldName, SceneValue)],
    fields: &[(SceneFieldName, SceneValue)],
    path: &[ValuePathStep],
) -> bool {
    let Some(ValuePathStep::Root(idx)) = path.first() else {
        return false;
    };
    let Some((name, value)) = fields.get(*idx).cloned() else {
        return false;
    };
    let default = defaults
        .iter()
        .find(|(field, _)| field == &name)
        .map(|(_, value)| value);
    let pruned = prune_default_script_value(&value, default);
    if let Some(value) = pruned {
        if let Some((_, existing)) = overrides.iter_mut().find(|(field, _)| field == &name) {
            *existing = value;
        } else {
            overrides.push((name, value));
        }
    } else {
        overrides.retain(|(field, _)| field != &name);
    }
    true
}

fn prune_default_script_value(
    value: &SceneValue,
    default: Option<&SceneValue>,
) -> Option<SceneValue> {
    if default.is_some_and(|default| scene_values_equal(default, value)) {
        return None;
    }
    match (value, default) {
        (SceneValue::Object(fields), Some(SceneValue::Object(default_fields))) => {
            let mut out = Vec::new();
            for (name, value) in fields.iter() {
                let default = default_fields
                    .iter()
                    .find(|(field, _)| field == name)
                    .map(|(_, value)| value);
                if let Some(value) = prune_default_script_value(value, default) {
                    out.push((name.clone(), value));
                }
            }
            (!out.is_empty()).then_some(SceneValue::Object(Cow::Owned(out)))
        }
        (SceneValue::Array(values), _) if values.is_empty() => None,
        (SceneValue::Bool(false), None) => None,
        (SceneValue::F32(value), None) if *value == 0.0 => None,
        (SceneValue::I32(0), None) => None,
        (SceneValue::Key(key), None)
            if matches!(
                key.as_ref().trim().trim_start_matches('@'),
                "" | "null" | "none" | "-"
            ) =>
        {
            None
        }
        _ => Some(value.clone()),
    }
}

fn scene_values_equal(a: &SceneValue, b: &SceneValue) -> bool {
    match (a, b) {
        (SceneValue::Bool(a), SceneValue::Bool(b)) => a == b,
        (SceneValue::I32(a), SceneValue::I32(b)) => a == b,
        (SceneValue::F32(a), SceneValue::F32(b)) => a == b,
        (SceneValue::Vec2 { x: ax, y: ay }, SceneValue::Vec2 { x: bx, y: by }) => {
            ax == bx && ay == by
        }
        (
            SceneValue::Vec3 {
                x: ax,
                y: ay,
                z: az,
            },
            SceneValue::Vec3 {
                x: bx,
                y: by,
                z: bz,
            },
        ) => ax == bx && ay == by && az == bz,
        (
            SceneValue::Vec4 {
                x: ax,
                y: ay,
                z: az,
                w: aw,
            },
            SceneValue::Vec4 {
                x: bx,
                y: by,
                z: bz,
                w: bw,
            },
        ) => ax == bx && ay == by && az == bz && aw == bw,
        (SceneValue::Str(a), SceneValue::Str(b)) => a.as_ref() == b.as_ref(),
        (SceneValue::Hashed(a), SceneValue::Hashed(b)) => a == b,
        (SceneValue::Key(a), SceneValue::Key(b)) => a.as_ref() == b.as_ref(),
        (SceneValue::Array(a), SceneValue::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(a, b)| scene_values_equal(a, b))
        }
        (SceneValue::Object(a), SceneValue::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((a_name, a_value), (b_name, b_value))| {
                        a_name == b_name && scene_values_equal(a_value, b_value)
                    })
        }
        _ => false,
    }
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

fn add_nested_value(target: &mut SceneValue, path: &[ValuePathStep], value: SceneValue) -> bool {
    if path.is_empty() {
        let SceneValue::Array(values) = target else {
            return false;
        };
        values.to_mut().push(value);
        return true;
    }
    match (&mut *target, &path[0]) {
        (SceneValue::Array(values), ValuePathStep::Index(idx)) => {
            let Some(item) = values.to_mut().get_mut(*idx) else {
                return false;
            };
            add_nested_value(item, &path[1..], value)
        }
        (SceneValue::Object(fields), ValuePathStep::Field(name)) => {
            let Some((_field, item)) = fields
                .to_mut()
                .iter_mut()
                .find(|(field, _)| field.as_ref() == name)
            else {
                return false;
            };
            add_nested_value(item, &path[1..], value)
        }
        _ => false,
    }
}

fn remove_nested_value(target: &mut SceneValue, path: &[ValuePathStep]) -> bool {
    if let [ValuePathStep::Index(idx)] = path {
        let SceneValue::Array(values) = target else {
            return false;
        };
        if *idx >= values.len() {
            return false;
        }
        values.to_mut().remove(*idx);
        return true;
    }
    match (&mut *target, &path[0]) {
        (SceneValue::Array(values), ValuePathStep::Index(idx)) => {
            let Some(item) = values.to_mut().get_mut(*idx) else {
                return false;
            };
            remove_nested_value(item, &path[1..])
        }
        (SceneValue::Object(fields), ValuePathStep::Field(name)) => {
            let Some((_field, item)) = fields
                .to_mut()
                .iter_mut()
                .find(|(field, _)| field.as_ref() == name)
            else {
                return false;
            };
            remove_nested_value(item, &path[1..])
        }
        _ => false,
    }
}
