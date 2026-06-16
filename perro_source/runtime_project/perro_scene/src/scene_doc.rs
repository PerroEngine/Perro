use crate::{
    Parser, Scene, SceneKey, SceneNodeData, SceneNodeEntry, SceneObjectField, SceneValue,
    default_scene_field_value,
};
use perro_structs::BitMask;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

pub type SceneVar = (Cow<'static, str>, SceneValue);

#[derive(Clone, Debug)]
pub struct SceneDoc {
    pub vars: Cow<'static, [SceneVar]>,
    pub scene: Scene,
}

impl SceneDoc {
    pub fn parse(src: &str) -> Self {
        let vars = Parser::new(src).collect_var_entries();
        let scene = Parser::new(src).parse_scene();
        Self::from_parts(vars, scene)
    }

    pub(crate) fn parse_lenient(src: &str) -> Self {
        let vars = Parser::new_lenient(src).collect_var_entries();
        let scene = Parser::new(src).parse_scene_lenient();
        Self::from_parts(vars, scene)
    }

    fn from_parts(vars: Vec<(String, SceneValue)>, scene: Scene) -> Self {
        let root_name = scene
            .root
            .map(|root| scene.key_name_or_id(root).to_string());
        let vars = vars
            .into_iter()
            .filter(|(name, _)| name != "root" && Some(name.as_str()) != root_name.as_deref())
            .map(|(name, value)| (Cow::Owned(name), value))
            .collect();
        Self {
            vars: Cow::Owned(vars),
            scene,
        }
    }

    pub fn from_scene(scene: Scene) -> Self {
        Self {
            vars: Cow::Borrowed(&[]),
            scene,
        }
    }

    pub fn into_scene(self) -> Scene {
        self.scene
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn normalize_links(&mut self) {
        sync_children_from_parents(&mut self.scene);
    }

    pub fn to_text(&self) -> String {
        self.to_text_with_dedup(false)
    }

    pub fn to_text_dedup(&self) -> String {
        self.to_text_with_dedup(true)
    }

    pub fn to_text_with_dedup(&self, dedup: bool) -> String {
        let mut doc = self.clone();
        doc.normalize_links();
        SceneDocWriter::new(&doc, dedup).write()
    }
}

impl From<Scene> for SceneDoc {
    fn from(scene: Scene) -> Self {
        Self::from_scene(scene)
    }
}

impl From<SceneDoc> for Scene {
    fn from(doc: SceneDoc) -> Self {
        doc.scene
    }
}

pub struct SceneWrite<'a> {
    doc: &'a SceneDoc,
}

impl<'a> SceneWrite<'a> {
    pub fn new(doc: &'a SceneDoc) -> Self {
        Self { doc }
    }

    pub fn to_text(&self) -> String {
        self.doc.to_text()
    }
}

struct SceneDocWriter<'a> {
    doc: &'a SceneDoc,
    value_vars: HashMap<String, String>,
}

impl<'a> SceneDocWriter<'a> {
    fn new(doc: &'a SceneDoc, dedup: bool) -> Self {
        let value_vars = if dedup {
            collect_dedupe_vars(doc)
        } else {
            HashMap::new()
        };
        Self { doc, value_vars }
    }

    fn write(&self) -> String {
        let mut out = String::new();
        if let Some(root) = &self.doc.scene.root {
            out.push_str("$root = ");
            out.push('@');
            out.push_str(self.doc.scene.key_name_or_id(*root).as_ref());
            out.push('\n');
        }

        let mut dedupe_items = self
            .value_vars
            .iter()
            .map(|(value, name)| (name.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        dedupe_items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in dedupe_items {
            out.push('$');
            out.push_str(name);
            out.push_str(" = ");
            out.push_str(value);
            out.push('\n');
        }

        if !out.is_empty() {
            out.push('\n');
        }

        for node in self.doc.scene.nodes.iter() {
            self.write_node(node, &mut out);
            out.push('\n');
        }

        out
    }

    fn write_node(&self, node: &SceneNodeEntry, out: &mut String) {
        out.push('[');
        let node_key = self.doc.scene.key_name_or_id(node.key);
        out.push_str(node_key.as_ref());
        out.push_str("]\n");
        if node
            .name
            .as_ref()
            .is_some_and(|name| name.as_ref() != node_key.as_ref())
        {
            out.push_str("name = ");
            write_str(node.name.as_ref().expect("checked"), out);
            out.push('\n');
        }
        if !node.tags.is_empty() {
            out.push_str("tags = [");
            for (idx, tag) in node.tags.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                write_str(tag, out);
            }
            out.push_str("]\n");
        }
        if let Some(parent) = &node.parent {
            out.push_str("parent = ");
            out.push('@');
            out.push_str(self.doc.scene.key_name_or_id(*parent).as_ref());
            out.push('\n');
        }
        if let Some(script) = &node.script {
            out.push_str("script = ");
            write_str(script, out);
            out.push('\n');
        } else if node.clear_script {
            out.push_str("script = null\n");
        }
        if let Some(root_of) = &node.root_of {
            out.push_str("root_of = ");
            write_str(root_of, out);
            out.push('\n');
        }
        if !node.script_vars.is_empty() {
            out.push_str("script_vars = ");
            self.write_object(
                node.script_vars.as_ref(),
                out,
                0,
                node.script_vars.len() > 1,
            );
            out.push('\n');
        }

        if node.has_data_override {
            self.write_data(&node.data, out, 1);
            out.push_str("[/");
            out.push_str(node_key.as_ref());
            out.push_str("]\n");
        } else {
            out.push_str("[/");
            out.push_str(node_key.as_ref());
            out.push_str("]\n");
        }
    }

    fn write_data(&self, data: &SceneNodeData, out: &mut String, depth: usize) {
        indent(out, depth);
        out.push('[');
        out.push_str(data.type_name());
        let fields = data
            .fields
            .iter()
            .filter(|(name, value)| !scene_field_matches_default(data, name, value))
            .collect::<Vec<_>>();
        if data.base_ref().is_none() && fields.is_empty() {
            out.push_str("/]\n");
            return;
        }
        out.push_str("]\n");
        for (name, value) in fields {
            indent(out, depth + 1);
            out.push_str(name.as_ref());
            out.push_str(" = ");
            self.write_field_value(name.as_ref(), value, out, depth + 1);
            out.push('\n');
        }
        if let Some(base) = data.base_ref() {
            self.write_data(base, out, depth + 1);
        }
        indent(out, depth);
        out.push_str("[/");
        out.push_str(data.type_name());
        out.push_str("]\n");
    }

    fn write_value(&self, value: &SceneValue, out: &mut String, depth: usize, dedupe: bool) {
        if dedupe {
            let key = value_key(value);
            if let Some(var) = self.value_vars.get(&key) {
                out.push('$');
                out.push_str(var);
                return;
            }
        }

        match value {
            SceneValue::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
            SceneValue::I32(v) => out.push_str(&v.to_string()),
            SceneValue::F32(v) => out.push_str(&fmt_f32(*v)),
            SceneValue::Vec2 { x, y } => {
                out.push('(');
                out.push_str(&fmt_f32(*x));
                out.push_str(", ");
                out.push_str(&fmt_f32(*y));
                out.push(')');
            }
            SceneValue::Vec3 { x, y, z } => {
                out.push('(');
                out.push_str(&fmt_f32(*x));
                out.push_str(", ");
                out.push_str(&fmt_f32(*y));
                out.push_str(", ");
                out.push_str(&fmt_f32(*z));
                out.push(')');
            }
            SceneValue::Vec4 { x, y, z, w } => {
                out.push('(');
                out.push_str(&fmt_f32(*x));
                out.push_str(", ");
                out.push_str(&fmt_f32(*y));
                out.push_str(", ");
                out.push_str(&fmt_f32(*z));
                out.push_str(", ");
                out.push_str(&fmt_f32(*w));
                out.push(')');
            }
            SceneValue::IVec2 { x, y } => write_int_vec(out, &[*x, *y]),
            SceneValue::IVec3 { x, y, z } => write_int_vec(out, &[*x, *y, *z]),
            SceneValue::IVec4 { x, y, z, w } => write_int_vec(out, &[*x, *y, *z, *w]),
            SceneValue::UVec2 { x, y } => write_uint_vec(out, &[*x, *y]),
            SceneValue::UVec3 { x, y, z } => write_uint_vec(out, &[*x, *y, *z]),
            SceneValue::UVec4 { x, y, z, w } => write_uint_vec(out, &[*x, *y, *z, *w]),
            SceneValue::Str(v) => write_str(v, out),
            SceneValue::Hashed(v) => out.push_str(&v.to_string()),
            SceneValue::Key(v) => self.write_key_value(v.as_ref(), out),
            SceneValue::Object(fields) => {
                self.write_object(fields.as_ref(), out, depth, fields.len() > 1)
            }
            SceneValue::Array(items) => self.write_array(items.as_ref(), out, depth),
        }
    }

    fn write_field_value(&self, name: &str, value: &SceneValue, out: &mut String, depth: usize) {
        if is_node_ref_field(name)
            && let SceneValue::Key(key) = value
        {
            self.write_node_ref_value(key.as_ref(), out);
            return;
        }
        self.write_value(value, out, depth, true);
    }

    fn write_key_value(&self, value: &str, out: &mut String) {
        if self
            .doc
            .scene
            .key_names
            .iter()
            .any(|name| name.as_ref() == value)
        {
            self.write_node_ref_value(value, out);
            return;
        }
        write_key_value(value, out);
    }

    fn write_node_ref_value(&self, value: &str, out: &mut String) {
        out.push('@');
        out.push_str(value);
    }

    fn write_object(
        &self,
        fields: &[SceneObjectField],
        out: &mut String,
        depth: usize,
        multiline: bool,
    ) {
        if fields.is_empty() {
            out.push_str("{}");
            return;
        }
        if !multiline {
            out.push_str("{ ");
            for (idx, (name, value)) in fields.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                out.push_str(name.as_ref());
                out.push_str(" = ");
                self.write_value_inline(value, out);
            }
            out.push_str(" }");
            return;
        }
        out.push_str("{\n");
        for (idx, (name, value)) in fields.iter().enumerate() {
            indent(out, depth + 1);
            out.push_str(name.as_ref());
            out.push_str(" = ");
            self.write_value(value, out, depth + 1, true);
            if idx + 1 < fields.len() {
                out.push(',');
            }
            out.push('\n');
        }
        indent(out, depth);
        out.push('}');
    }

    fn write_array(&self, items: &[SceneValue], out: &mut String, depth: usize) {
        if items.is_empty() {
            out.push_str("[]");
            return;
        }
        if items.len() == 1 {
            out.push('[');
            self.write_value_inline(&items[0], out);
            out.push(']');
            return;
        }
        out.push_str("[\n");
        for (idx, item) in items.iter().enumerate() {
            indent(out, depth + 1);
            self.write_value(item, out, depth + 1, true);
            if idx + 1 < items.len() {
                out.push(',');
            }
            out.push('\n');
        }
        indent(out, depth);
        out.push(']');
    }

    fn write_value_inline(&self, value: &SceneValue, out: &mut String) {
        let key = value_key(value);
        if let Some(var) = self.value_vars.get(&key) {
            out.push('$');
            out.push_str(var);
            return;
        }

        match value {
            SceneValue::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
            SceneValue::I32(v) => out.push_str(&v.to_string()),
            SceneValue::F32(v) => out.push_str(&fmt_f32(*v)),
            SceneValue::Vec2 { x, y } => {
                out.push('(');
                out.push_str(&fmt_f32(*x));
                out.push_str(", ");
                out.push_str(&fmt_f32(*y));
                out.push(')');
            }
            SceneValue::Vec3 { x, y, z } => {
                out.push('(');
                out.push_str(&fmt_f32(*x));
                out.push_str(", ");
                out.push_str(&fmt_f32(*y));
                out.push_str(", ");
                out.push_str(&fmt_f32(*z));
                out.push(')');
            }
            SceneValue::Vec4 { x, y, z, w } => {
                out.push('(');
                out.push_str(&fmt_f32(*x));
                out.push_str(", ");
                out.push_str(&fmt_f32(*y));
                out.push_str(", ");
                out.push_str(&fmt_f32(*z));
                out.push_str(", ");
                out.push_str(&fmt_f32(*w));
                out.push(')');
            }
            SceneValue::IVec2 { x, y } => write_int_vec(out, &[*x, *y]),
            SceneValue::IVec3 { x, y, z } => write_int_vec(out, &[*x, *y, *z]),
            SceneValue::IVec4 { x, y, z, w } => write_int_vec(out, &[*x, *y, *z, *w]),
            SceneValue::UVec2 { x, y } => write_uint_vec(out, &[*x, *y]),
            SceneValue::UVec3 { x, y, z } => write_uint_vec(out, &[*x, *y, *z]),
            SceneValue::UVec4 { x, y, z, w } => write_uint_vec(out, &[*x, *y, *z, *w]),
            SceneValue::Str(v) => write_str(v, out),
            SceneValue::Hashed(v) => out.push_str(&v.to_string()),
            SceneValue::Key(v) => self.write_key_value(v.as_ref(), out),
            SceneValue::Object(fields) => {
                out.push_str("{ ");
                for (idx, (name, value)) in fields.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(name.as_ref());
                    out.push_str(" = ");
                    self.write_value_inline(value, out);
                }
                out.push_str(" }");
            }
            SceneValue::Array(items) => {
                out.push('[');
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(", ");
                    }
                    self.write_value_inline(item, out);
                }
                out.push(']');
            }
        }
    }
}

fn scene_field_matches_default(
    data: &SceneNodeData,
    name: &crate::SceneFieldName,
    value: &SceneValue,
) -> bool {
    default_scene_field_value(data.node_type, name)
        .as_ref()
        .is_some_and(|default| scene_values_match(default, value))
}

fn scene_values_match(default: &SceneValue, value: &SceneValue) -> bool {
    if default == value {
        return true;
    }
    match (default, value) {
        (SceneValue::F32(a), SceneValue::I32(b)) => (*a - *b as f32).abs() <= f32::EPSILON,
        (SceneValue::I32(a), SceneValue::F32(b)) => (*a as f32 - *b).abs() <= f32::EPSILON,
        (SceneValue::Str(a), SceneValue::Key(b)) | (SceneValue::Key(b), SceneValue::Str(a)) => {
            a.as_ref() == b.as_ref()
        }
        _ => scene_value_bitmask(default)
            .zip(scene_value_bitmask(value))
            .is_some_and(|(a, b)| a == b),
    }
}

fn scene_value_bitmask(value: &SceneValue) -> Option<BitMask> {
    match value {
        SceneValue::I32(v) => Some(BitMask::from_bits(*v as u32)),
        SceneValue::F32(v) if v.fract() == 0.0 && *v >= 0.0 => Some(BitMask::from_bits(*v as u32)),
        SceneValue::Key(v) => parse_bitmask_text(v.as_ref()),
        SceneValue::Str(v) => parse_bitmask_text(v.as_ref()),
        SceneValue::Array(items) => {
            let mut mask = BitMask::NONE;
            for item in items.iter() {
                let layer = match item {
                    SceneValue::I32(v) => *v,
                    SceneValue::F32(v) if v.fract() == 0.0 => *v as i32,
                    _ => return None,
                };
                let layer = u8::try_from(layer).ok()?;
                mask = mask.union(BitMask::try_layer(layer)?);
            }
            Some(mask)
        }
        _ => None,
    }
}

fn parse_bitmask_text(raw: &str) -> Option<BitMask> {
    match raw {
        "all" | "ALL" => Some(BitMask::ALL),
        "none" | "NONE" => Some(BitMask::NONE),
        _ => parse_bitmask_call(raw),
    }
}

fn parse_bitmask_call(raw: &str) -> Option<BitMask> {
    let (op, rest) = raw.split_once('(')?;
    let args = rest.strip_suffix(')')?.trim();
    let args = args
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(args);
    let mut layers = Vec::new();
    if !args.trim().is_empty() {
        for arg in args.split(',') {
            let layer = arg.trim().parse::<u8>().ok()?;
            if !(1..=32).contains(&layer) {
                return None;
            }
            layers.push(layer);
        }
    }
    match op {
        "only" | "ONLY" => BitMask::try_from_layers(layers),
        "without" | "WITHOUT" => Some(BitMask::without(&layers)),
        _ => None,
    }
}

fn sync_children_from_parents(scene: &mut Scene) {
    let mut children: BTreeMap<String, Vec<SceneKey>> = BTreeMap::new();
    for node in scene.nodes.iter() {
        if let Some(parent) = &node.parent {
            children
                .entry(scene.key_name_or_id(*parent).to_string())
                .or_default()
                .push(node.key);
        }
    }
    let key_names = scene
        .nodes
        .iter()
        .map(|node| (node.key, scene.key_name_or_id(node.key).to_string()))
        .collect::<Vec<_>>();
    for node in scene.nodes.to_mut() {
        let node_key = key_names
            .iter()
            .find(|(key, _)| *key == node.key)
            .map(|(_, name)| name.as_str())
            .unwrap_or_default();
        let next = children.remove(node_key).unwrap_or_default();
        node.children = Cow::Owned(next);
    }
}

fn collect_dedupe_vars(doc: &SceneDoc) -> HashMap<String, String> {
    let existing = doc
        .vars
        .iter()
        .map(|(name, value)| (value_key(value), name.to_string()))
        .collect::<HashMap<_, _>>();
    let mut counts = BTreeMap::<String, usize>::new();
    for node in doc.scene.nodes.iter() {
        collect_fields(&node.script_vars, &mut counts);
        collect_data(&node.data, &mut counts);
    }
    let mut out = HashMap::new();
    let mut idx = 0usize;
    for (value, count) in counts {
        if count < 3 || value.len() < 24 {
            continue;
        }
        if let Some(name) = existing.get(&value) {
            out.insert(value, name.clone());
        } else {
            idx += 1;
            out.insert(value, format!("var{idx}"));
        }
    }
    out
}

fn collect_data(data: &SceneNodeData, counts: &mut BTreeMap<String, usize>) {
    if let Some(base) = data.base_ref() {
        collect_data(base, counts);
    }
    collect_fields(&data.fields, counts);
}

fn collect_fields(fields: &[SceneObjectField], counts: &mut BTreeMap<String, usize>) {
    for (_, value) in fields {
        collect_value(value, counts);
    }
}

fn collect_value(value: &SceneValue, counts: &mut BTreeMap<String, usize>) {
    if matches!(
        value,
        SceneValue::Object(_) | SceneValue::Array(_) | SceneValue::Str(_)
    ) {
        *counts.entry(value_key(value)).or_default() += 1;
    }
    match value {
        SceneValue::Object(fields) => collect_fields(fields, counts),
        SceneValue::Array(items) => {
            for item in items.iter() {
                collect_value(item, counts);
            }
        }
        _ => {}
    }
}

fn value_key(value: &SceneValue) -> String {
    let mut out = String::new();
    write_value_plain(value, &mut out);
    out
}

fn write_value_plain(value: &SceneValue, out: &mut String) {
    match value {
        SceneValue::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
        SceneValue::I32(v) => out.push_str(&v.to_string()),
        SceneValue::F32(v) => out.push_str(&fmt_f32(*v)),
        SceneValue::Vec2 { x, y } => {
            out.push('(');
            out.push_str(&fmt_f32(*x));
            out.push_str(", ");
            out.push_str(&fmt_f32(*y));
            out.push(')');
        }
        SceneValue::Vec3 { x, y, z } => {
            out.push('(');
            out.push_str(&fmt_f32(*x));
            out.push_str(", ");
            out.push_str(&fmt_f32(*y));
            out.push_str(", ");
            out.push_str(&fmt_f32(*z));
            out.push(')');
        }
        SceneValue::Vec4 { x, y, z, w } => {
            out.push('(');
            out.push_str(&fmt_f32(*x));
            out.push_str(", ");
            out.push_str(&fmt_f32(*y));
            out.push_str(", ");
            out.push_str(&fmt_f32(*z));
            out.push_str(", ");
            out.push_str(&fmt_f32(*w));
            out.push(')');
        }
        SceneValue::IVec2 { x, y } => write_int_vec(out, &[*x, *y]),
        SceneValue::IVec3 { x, y, z } => write_int_vec(out, &[*x, *y, *z]),
        SceneValue::IVec4 { x, y, z, w } => write_int_vec(out, &[*x, *y, *z, *w]),
        SceneValue::UVec2 { x, y } => write_uint_vec(out, &[*x, *y]),
        SceneValue::UVec3 { x, y, z } => write_uint_vec(out, &[*x, *y, *z]),
        SceneValue::UVec4 { x, y, z, w } => write_uint_vec(out, &[*x, *y, *z, *w]),
        SceneValue::Str(v) => write_str(v, out),
        SceneValue::Hashed(v) => out.push_str(&v.to_string()),
        SceneValue::Key(v) => write_key_value(v.as_ref(), out),
        SceneValue::Object(fields) => {
            out.push_str("{ ");
            for (idx, (name, value)) in fields.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                out.push_str(name.as_ref());
                out.push_str(": ");
                write_value_plain(value, out);
            }
            out.push_str(" }");
        }
        SceneValue::Array(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                write_value_plain(item, out);
            }
            out.push(']');
        }
    }
}

fn write_int_vec(out: &mut String, values: &[i32]) {
    out.push('(');
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&value.to_string());
    }
    out.push(')');
}

fn write_uint_vec(out: &mut String, values: &[u32]) {
    out.push('(');
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&value.to_string());
    }
    out.push(')');
}

fn write_str(value: &str, out: &mut String) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
}

fn is_node_ref_field(name: &str) -> bool {
    matches!(name, "camera" | "body_a" | "body_b")
}

fn write_key_value(value: &str, out: &mut String) {
    if value.starts_with('@') {
        out.push('@');
    }
    out.push_str(value);
}

fn fmt_f32(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("    ");
    }
}
