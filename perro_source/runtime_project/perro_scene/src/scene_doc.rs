use crate::{Parser, Scene, SceneKey, SceneNodeData, SceneNodeEntry, SceneObjectField, SceneValue};
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
        let root_name = scene.root.as_ref().map(|root| root.as_ref().to_string());
        let vars = vars
            .into_iter()
            .filter(|(name, _)| Some(name.as_str()) != root_name.as_deref())
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
        let mut doc = self.clone();
        doc.normalize_links();
        SceneDocWriter::new(&doc).write()
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
    fn new(doc: &'a SceneDoc) -> Self {
        let value_vars = collect_dedupe_vars(doc);
        Self { doc, value_vars }
    }

    fn write(&self) -> String {
        let mut out = String::new();
        if let Some(root) = &self.doc.scene.root {
            out.push_str("@root = ");
            out.push_str(root.as_ref());
            out.push('\n');
        }

        for (name, value) in self.doc.vars.iter() {
            out.push('@');
            out.push_str(name.as_ref());
            out.push_str(" = ");
            self.write_value(value, &mut out, 0, false);
            out.push('\n');
        }

        let mut dedupe_items = self
            .value_vars
            .iter()
            .map(|(value, name)| (name.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        dedupe_items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in dedupe_items {
            if self.doc.vars.iter().any(|(var, _)| var.as_ref() == name) {
                continue;
            }
            out.push('@');
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
        out.push_str(node.key.as_ref());
        out.push_str("]\n");
        if node
            .name
            .as_ref()
            .is_some_and(|name| name.as_ref() != node.key.as_ref())
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
            out.push_str(parent.as_ref());
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
            self.write_object(node.script_vars.as_ref(), out, 0, true);
            out.push('\n');
        }

        if node.has_data_override {
            self.write_data(&node.data, out, 0);
            out.push_str("[/");
            out.push_str(node.key.as_ref());
            out.push_str("]\n");
        } else {
            out.push_str("[/");
            out.push_str(node.key.as_ref());
            out.push_str("]\n");
        }
    }

    fn write_data(&self, data: &SceneNodeData, out: &mut String, depth: usize) {
        indent(out, depth);
        out.push('[');
        out.push_str(data.ty.as_ref());
        out.push_str("]\n");
        if let Some(base) = data.base_ref() {
            self.write_data(base, out, depth + 1);
        }
        for (name, value) in data.fields.iter() {
            indent(out, depth + 1);
            out.push_str(name.as_ref());
            out.push_str(" = ");
            self.write_value(value, out, depth + 1, true);
            out.push('\n');
        }
        indent(out, depth);
        out.push_str("[/");
        out.push_str(data.ty.as_ref());
        out.push_str("]\n");
    }

    fn write_value(&self, value: &SceneValue, out: &mut String, depth: usize, dedupe: bool) {
        if dedupe {
            let key = value_key(value);
            if let Some(var) = self.value_vars.get(&key) {
                out.push('@');
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
            SceneValue::Str(v) => write_str(v, out),
            SceneValue::Hashed(v) => out.push_str(&v.to_string()),
            SceneValue::Key(v) => out.push_str(v.as_ref()),
            SceneValue::Object(fields) => self.write_object(fields.as_ref(), out, depth, false),
            SceneValue::Array(items) => {
                out.push('[');
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(", ");
                    }
                    self.write_value(item, out, depth, true);
                }
                out.push(']');
            }
        }
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
                out.push_str(": ");
                self.write_value(value, out, depth, true);
            }
            out.push_str(" }");
            return;
        }
        out.push_str("{\n");
        for (name, value) in fields {
            indent(out, depth + 1);
            out.push_str(name.as_ref());
            out.push_str(" = ");
            self.write_value(value, out, depth + 1, true);
            out.push('\n');
        }
        indent(out, depth);
        out.push('}');
    }
}

fn sync_children_from_parents(scene: &mut Scene) {
    let mut children: BTreeMap<String, Vec<SceneKey>> = BTreeMap::new();
    for node in scene.nodes.iter() {
        if let Some(parent) = &node.parent {
            children
                .entry(parent.as_ref().to_string())
                .or_default()
                .push(node.key.clone());
        }
    }
    for node in scene.nodes.to_mut() {
        let next = children.remove(node.key.as_ref()).unwrap_or_default();
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
        if count < 2 || value.len() < 24 {
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
        SceneValue::Str(v) => write_str(v, out),
        SceneValue::Hashed(v) => out.push_str(&v.to_string()),
        SceneValue::Key(v) => out.push_str(v.as_ref()),
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
