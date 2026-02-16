use crate::{Runtime, runtime_project::ProviderMode};
use perro_core::{
    Quaternion, SceneNode, SceneNodeData, Vector2, Vector3,
    camera_2d::Camera2D, camera_3d::Camera3D, mesh_instance_3d::MeshInstance3D,
    node_2d::node_2d::Node2D, node_3d::node_3d::Node3D, sprite_2d::Sprite2D,
};
use perro_ids::NodeID;
use perro_io::{ProjectRoot, load_asset, set_project_root};
use perro_scene::{
    Parser, RuntimeNodeData, RuntimeNodeEntry, RuntimeScene, RuntimeValue, StaticNodeData,
    StaticNodeEntry, StaticScene, StaticSceneValue,
};
use std::{borrow::Cow, collections::HashMap, time::{Duration, Instant}};

struct SceneLoadStats {
    mode_label: &'static str,
    source_load: Option<Duration>,
    parse: Option<Duration>,
    node_insert: Duration,
    total_excluding_debug_print: Duration,
}

impl Runtime {
    pub(crate) fn load_boot_scene(&mut self) -> Result<(), String> {
        let boot_start = Instant::now();
        let (project_root, project_name, main_scene_path, static_lookup, brk_bytes) = {
            let project = self
                .project()
                .ok_or_else(|| "Runtime project is not set".to_string())?;
            (
                project.root.clone(),
                project.config.name.clone(),
                project.config.main_scene.clone(),
                project.static_scene_lookup,
                project.brk_bytes,
            )
        };

        if self.provider_mode == ProviderMode::Static {
            if let Some(data) = brk_bytes {
                set_project_root(ProjectRoot::Brk {
                    data,
                    name: project_name,
                });
            } else {
                set_project_root(ProjectRoot::Disk {
                    root: project_root,
                    name: project_name,
                });
            }
        } else {
            set_project_root(ProjectRoot::Disk {
                root: project_root,
                name: project_name,
            });
        }

        self.nodes.clear();
        let mode_label;
        let mut source_load = None;
        let mut parse = None;
        let node_insert_start = Instant::now();
        match self.provider_mode {
            ProviderMode::Dynamic => {
                mode_label = "dynamic";
                let (runtime_scene, load_stats) = load_runtime_scene_from_disk(&main_scene_path)?;
                source_load = Some(load_stats.source_load);
                parse = Some(load_stats.parse);
                insert_runtime_scene_nodes(self, runtime_scene)?;
            }
            ProviderMode::Static => match static_lookup.and_then(|lookup| lookup(&main_scene_path)) {
                Some(scene) => {
                    mode_label = "static";
                    insert_static_scene_nodes(self, scene)?
                }
                None => {
                    mode_label = "static_fallback_dynamic";
                    let (runtime_scene, load_stats) = load_runtime_scene_from_disk(&main_scene_path)?;
                    source_load = Some(load_stats.source_load);
                    parse = Some(load_stats.parse);
                    insert_runtime_scene_nodes(self, runtime_scene)?;
                }
            },
        }
        let node_insert = node_insert_start.elapsed();
        let stats = SceneLoadStats {
            mode_label,
            source_load,
            parse,
            node_insert,
            total_excluding_debug_print: boot_start.elapsed(),
        };
        debug_print_scene_load(self, &main_scene_path, stats);
        Ok(())
    }
}

struct RuntimeSceneLoadStats {
    source_load: Duration,
    parse: Duration,
}

fn load_runtime_scene_from_disk(path: &str) -> Result<(RuntimeScene, RuntimeSceneLoadStats), String> {
    let source_load_start = Instant::now();
    let bytes = load_asset(path).map_err(|err| format!("failed to load scene `{path}`: {err}"))?;
    let source_load = source_load_start.elapsed();

    let source = std::str::from_utf8(&bytes)
        .map_err(|err| format!("scene `{path}` is not valid UTF-8: {err}"))?;
    let parse_start = Instant::now();
    let scene = Parser::new(source).parse_scene();
    let parse = parse_start.elapsed();
    Ok((scene, RuntimeSceneLoadStats { source_load, parse }))
}

fn debug_print_scene_load(runtime: &Runtime, path: &str, stats: SceneLoadStats) {
    println!(
        "[scene-load] mode={} path={} total_ms={:.3} source_ms={} parse_ms={} insert_ms={:.3}",
        stats.mode_label,
        path,
        as_ms(stats.total_excluding_debug_print),
        fmt_duration(stats.source_load),
        fmt_duration(stats.parse),
        as_ms(stats.node_insert),
    );
    print_scene_tree(runtime, NodeID::ROOT, "");
}

fn as_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn fmt_duration(duration: Option<Duration>) -> String {
    duration
        .map(|value| format!("{:.3}", as_ms(value)))
        .unwrap_or_else(|| "n/a".to_string())
}

fn print_scene_tree(runtime: &Runtime, node_id: NodeID, indent: &str) {
    let Some(node) = runtime.nodes.get(node_id) else {
        return;
    };
    println!(
        "{}- {} [{}] ({})",
        indent,
        node.name.as_ref(),
        node_id,
        node.node_type(),
    );
    let child_indent = format!("{indent}  ");
    for child_id in node.children_slice() {
        print_scene_tree(runtime, *child_id, &child_indent);
    }
}

fn runtime_node_entry_from_static(node: &StaticNodeEntry) -> RuntimeNodeEntry {
    RuntimeNodeEntry {
        data: runtime_node_data_from_static(&node.data),
        key: node.key.0.to_string(),
        name: node.name.map(str::to_string),
        parent: node.parent.map(|key| key.0.to_string()),
        script: node.script.map(str::to_string),
    }
}

fn runtime_node_data_from_static(data: &StaticNodeData) -> RuntimeNodeData {
    RuntimeNodeData {
        ty: data.ty.to_string(),
        fields: data
            .fields
            .iter()
            .map(|(name, value)| ((*name).to_string(), runtime_value_from_static(*value)))
            .collect(),
        base: data
            .base
            .map(|base| Box::new(runtime_node_data_from_static(base))),
    }
}

fn runtime_value_from_static(value: StaticSceneValue) -> RuntimeValue {
    match value {
        StaticSceneValue::Bool(v) => RuntimeValue::Bool(v),
        StaticSceneValue::I32(v) => RuntimeValue::I32(v),
        StaticSceneValue::F32(v) => RuntimeValue::F32(v),
        StaticSceneValue::Vec2 { x, y } => RuntimeValue::Vec2 { x, y },
        StaticSceneValue::Vec3 { x, y, z } => RuntimeValue::Vec3 { x, y, z },
        StaticSceneValue::Vec4 { x, y, z, w } => RuntimeValue::Vec4 { x, y, z, w },
        StaticSceneValue::Str(v) => RuntimeValue::Str(v.to_string()),
        StaticSceneValue::Key(key) => RuntimeValue::Key(key.0.to_string()),
    }
}

struct PendingNode {
    key: String,
    parent_key: Option<String>,
    node: SceneNode,
}

fn insert_static_scene_nodes(runtime: &mut Runtime, scene: &'static StaticScene) -> Result<(), String> {
    let mut engine_root = SceneNode::new(SceneNodeData::Node);
    engine_root.name = Cow::Borrowed("Root");
    let engine_root_id = runtime.nodes.insert(engine_root);

    let mut key_to_id: HashMap<&str, NodeID> = HashMap::with_capacity(scene.nodes.len());
    let mut key_order: Vec<&str> = Vec::with_capacity(scene.nodes.len());
    for static_node in scene.nodes {
        let key = static_node.key.0;
        if key_to_id.contains_key(key) {
            return Err(format!("duplicate scene key `{key}`"));
        }
        let node = scene_node_from_static_entry(static_node)?;
        let id = runtime.nodes.insert(node);
        key_to_id.insert(key, id);
        key_order.push(key);
    }

    let mut edges = Vec::new();
    for static_node in scene.nodes {
        let parent_id = *key_to_id
            .get(static_node.key.0)
            .ok_or_else(|| format!("node key `{}` not found", static_node.key.0))?;
        for child_key in static_node.children {
            let child_id = *key_to_id.get(child_key.0).ok_or_else(|| {
                format!(
                    "child node key `{}` not found while linking parent `{}`",
                    child_key.0, static_node.key.0
                )
            })?;
            if let Some(child) = runtime.nodes.get_mut(child_id) {
                child.parent = parent_id;
            }
            edges.push((parent_id, child_id));
        }
    }

    for (parent_id, child_id) in edges {
        if let Some(parent) = runtime.nodes.get_mut(parent_id) {
            parent.add_child(child_id);
        }
    }

    let root_key_opt = scene.root.map(|key| key.0);
    if let Some(root_key) = root_key_opt {
        if !key_to_id.contains_key(root_key) {
            return Err(format!("scene root `{root_key}` not found in node list"));
        }
    }

    let mut top_level_roots: Vec<NodeID> = Vec::new();
    for key in &key_order {
        let Some(&id) = key_to_id.get(*key) else {
            continue;
        };
        let Some(node) = runtime.nodes.get(id) else {
            continue;
        };
        if node.parent.is_nil() {
            top_level_roots.push(id);
        }
    }

    if top_level_roots.is_empty() {
        return Err("boot scene produced no top-level root nodes".to_string());
    }

    let primary_root = if let Some(root_key) = root_key_opt {
        *key_to_id
            .get(root_key)
            .ok_or_else(|| format!("scene root `{root_key}` not found in node list"))?
    } else {
        top_level_roots[0]
    };

    let mut attach_order = Vec::with_capacity(top_level_roots.len());
    attach_order.push(primary_root);
    for id in top_level_roots {
        if id != primary_root {
            attach_order.push(id);
        }
    }

    for root_id in &attach_order {
        if let Some(root_node) = runtime.nodes.get_mut(*root_id) {
            root_node.parent = engine_root_id;
        }
        if let Some(engine_root_node) = runtime.nodes.get_mut(engine_root_id) {
            engine_root_node.add_child(*root_id);
        }
    }

    Ok(())
}

fn insert_runtime_scene_nodes(runtime: &mut Runtime, scene: RuntimeScene) -> Result<(), String> {
    let RuntimeScene { nodes, root } = scene;

    let mut engine_root = SceneNode::new(SceneNodeData::Node);
    engine_root.name = Cow::Borrowed("Root");
    let engine_root_id = runtime.nodes.insert(engine_root);

    let mut pending_nodes = Vec::with_capacity(nodes.len());
    for entry in nodes {
        pending_nodes.push(PendingNode {
            key: entry.key.clone(),
            parent_key: entry.parent.clone(),
            node: scene_node_from_runtime_entry(&entry)?,
        });
    }

    let mut key_to_id: HashMap<String, NodeID> = HashMap::with_capacity(pending_nodes.len());
    let mut key_order: Vec<String> = Vec::with_capacity(pending_nodes.len());
    let mut parent_pairs = Vec::with_capacity(pending_nodes.len());

    for pending in pending_nodes {
        if key_to_id.contains_key(&pending.key) {
            return Err(format!("duplicate scene key `{}`", pending.key));
        }

        let node_id = runtime.nodes.insert(pending.node);
        if let Some(parent_key) = pending.parent_key {
            parent_pairs.push((pending.key.clone(), parent_key));
        }
        key_order.push(pending.key.clone());
        key_to_id.insert(pending.key, node_id);
    }

    let root_key_opt = root;
    if let Some(root_key) = &root_key_opt {
        if !key_to_id.contains_key(root_key.as_str()) {
            return Err(format!("scene root `{root_key}` not found in node list"));
        }
    }

    let mut edges = Vec::with_capacity(parent_pairs.len());
    for (child_key, parent_key) in parent_pairs {
        let child_id = *key_to_id
            .get(&child_key)
            .ok_or_else(|| format!("child node key `{child_key}` not found"))?;
        let parent_id = *key_to_id
            .get(&parent_key)
            .ok_or_else(|| {
                format!(
                    "parent node key `{parent_key}` not found while linking child `{child_key}`"
                )
            })?;

        if let Some(child) = runtime.nodes.get_mut(child_id) {
            child.parent = parent_id;
        }
        edges.push((parent_id, child_id));
    }

    for (parent_id, child_id) in edges {
        if let Some(parent) = runtime.nodes.get_mut(parent_id) {
            parent.add_child(child_id);
        }
    }

    let mut top_level_roots: Vec<NodeID> = Vec::new();
    for key in &key_order {
        let Some(&id) = key_to_id.get(key) else {
            continue;
        };
        let Some(node) = runtime.nodes.get(id) else {
            continue;
        };
        if node.parent.is_nil() {
            top_level_roots.push(id);
        }
    }

    if top_level_roots.is_empty() {
        return Err("boot scene produced no top-level root nodes".to_string());
    }

    let primary_root = if let Some(root_key) = root_key_opt.as_deref() {
        *key_to_id
            .get(root_key)
            .ok_or_else(|| format!("scene root `{root_key}` not found in node list"))?
    } else {
        top_level_roots[0]
    };

    let mut attach_order = Vec::with_capacity(top_level_roots.len());
    attach_order.push(primary_root);
    for id in top_level_roots {
        if id != primary_root {
            attach_order.push(id);
        }
    }

    for root_id in &attach_order {
        if let Some(root_node) = runtime.nodes.get_mut(*root_id) {
            root_node.parent = engine_root_id;
        }
        if let Some(engine_root_node) = runtime.nodes.get_mut(engine_root_id) {
            engine_root_node.add_child(*root_id);
        }
    }
    Ok(())
}

fn scene_node_from_static_entry(entry: &StaticNodeEntry) -> Result<SceneNode, String> {
    let runtime_entry = runtime_node_entry_from_static(entry);
    scene_node_from_runtime_entry(&runtime_entry)
}

fn scene_node_from_runtime_entry(entry: &RuntimeNodeEntry) -> Result<SceneNode, String> {
    let mut node = SceneNode::new(scene_node_data_from_runtime(&entry.data)?);
    if let Some(name) = &entry.name {
        node.name = Cow::Owned(name.clone());
    }
    if let Some(script) = &entry.script {
        node.script = Some(Cow::Owned(script.clone()));
    }
    Ok(node)
}

fn scene_node_data_from_runtime(data: &RuntimeNodeData) -> Result<SceneNodeData, String> {
    match data.ty.as_str() {
        "Node" => Ok(SceneNodeData::Node),
        "Node2D" => {
            let mut node = Node2D::new();
            apply_node_2d_data(&mut node, data);
            Ok(SceneNodeData::Node2D(node))
        }
        "Sprite2D" => {
            let mut node = Sprite2D::new();
            if let Some(base) = &data.base {
                apply_node_2d_data(&mut node.base, base);
            }
            apply_node_2d_fields(&mut node.base, &data.fields);
            apply_sprite_2d_fields(&mut node, &data.fields);
            Ok(SceneNodeData::Sprite2D(node))
        }
        "Camera2D" => {
            let mut node = Camera2D::new();
            if let Some(base) = &data.base {
                apply_node_2d_data(&mut node.base, base);
            }
            apply_node_2d_fields(&mut node.base, &data.fields);
            apply_camera_2d_fields(&mut node, &data.fields);
            Ok(SceneNodeData::Camera2D(node))
        }
        "Node3D" => {
            let mut node = Node3D::new();
            apply_node_3d_data(&mut node, data);
            Ok(SceneNodeData::Node3D(node))
        }
        "MeshInstance3D" => {
            let mut node = MeshInstance3D::new();
            if let Some(base) = &data.base {
                apply_node_3d_data(&mut node.base, base);
            }
            apply_node_3d_fields(&mut node.base, &data.fields);
            apply_mesh_instance_3d_fields(&mut node, &data.fields);
            Ok(SceneNodeData::MeshInstance3D(node))
        }
        "Camera3D" => {
            let mut node = Camera3D::new();
            if let Some(base) = &data.base {
                apply_node_3d_data(&mut node.base, base);
            }
            apply_node_3d_fields(&mut node.base, &data.fields);
            apply_camera_3d_fields(&mut node, &data.fields);
            Ok(SceneNodeData::Camera3D(node))
        }
        other => Err(format!("unsupported scene node type `{other}`")),
    }
}

fn apply_node_2d_data(target: &mut Node2D, data: &RuntimeNodeData) {
    if let Some(base) = &data.base {
        apply_node_2d_data(target, base);
    }
    apply_node_2d_fields(target, &data.fields);
}

fn apply_node_3d_data(target: &mut Node3D, data: &RuntimeNodeData) {
    if let Some(base) = &data.base {
        apply_node_3d_data(target, base);
    }
    apply_node_3d_fields(target, &data.fields);
}

fn apply_node_2d_fields(node: &mut Node2D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "position" => {
                if let Some(v) = as_vec2(value) {
                    node.transform.position = v;
                }
            }
            "scale" => {
                if let Some(v) = as_vec2(value) {
                    node.transform.scale = v;
                }
            }
            "rotation" => {
                if let Some(v) = as_f32(value) {
                    node.transform.rotation = v;
                }
            }
            "z_index" => {
                if let Some(v) = as_i32(value) {
                    node.z_index = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_sprite_2d_fields(_node: &mut Sprite2D, _fields: &[(String, RuntimeValue)]) {}

fn apply_camera_2d_fields(node: &mut Camera2D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "zoom" => {
                if let Some(v) = as_f32(value) {
                    node.zoom = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_node_3d_fields(node: &mut Node3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "position" => {
                if let Some(v) = as_vec3(value) {
                    node.transform.position = v;
                }
            }
            "scale" => {
                if let Some(v) = as_vec3(value) {
                    node.transform.scale = v;
                }
            }
            "rotation" => {
                if let Some(v) = as_quat(value) {
                    node.transform.rotation = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_mesh_instance_3d_fields(node: &mut MeshInstance3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "mesh" => {
                if let Some(v) = as_mesh_source(value) {
                    node.mesh = Some(Cow::Owned(v));
                }
            }
            "mesh_id" => {
                if let Some(v) = as_u64(value) {
                    node.mesh_id = perro_ids::MeshID::from_u64(v);
                }
            }
            "material_id" => {
                if let Some(v) = as_u64(value) {
                    node.material_id = perro_ids::MaterialID::from_u64(v);
                }
            }
            _ => {}
        }
    }
}

fn apply_camera_3d_fields(node: &mut Camera3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "zoom" => {
                if let Some(v) = as_f32(value) {
                    node.zoom = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

fn as_bool(value: &RuntimeValue) -> Option<bool> {
    match value {
        RuntimeValue::Bool(v) => Some(*v),
        _ => None,
    }
}

fn as_i32(value: &RuntimeValue) -> Option<i32> {
    match value {
        RuntimeValue::I32(v) => Some(*v),
        RuntimeValue::F32(v) => Some(*v as i32),
        _ => None,
    }
}

fn as_u64(value: &RuntimeValue) -> Option<u64> {
    match value {
        RuntimeValue::I32(v) => (*v >= 0).then_some(*v as u64),
        RuntimeValue::F32(v) => (*v >= 0.0).then_some(*v as u64),
        RuntimeValue::Str(v) => v.parse::<u64>().ok(),
        _ => None,
    }
}

fn as_f32(value: &RuntimeValue) -> Option<f32> {
    match value {
        RuntimeValue::F32(v) => Some(*v),
        RuntimeValue::I32(v) => Some(*v as f32),
        _ => None,
    }
}

fn as_vec2(value: &RuntimeValue) -> Option<Vector2> {
    match value {
        RuntimeValue::Vec2 { x, y } => Some(Vector2::new(*x, *y)),
        _ => None,
    }
}

fn as_vec3(value: &RuntimeValue) -> Option<Vector3> {
    match value {
        RuntimeValue::Vec3 { x, y, z } => Some(Vector3::new(*x, *y, *z)),
        _ => None,
    }
}

fn as_quat(value: &RuntimeValue) -> Option<Quaternion> {
    match value {
        RuntimeValue::Vec4 { x, y, z, w } => Some(Quaternion::new(*x, *y, *z, *w)),
        _ => None,
    }
}

fn as_mesh_source(value: &RuntimeValue) -> Option<String> {
    match value {
        RuntimeValue::Str(v) => Some(v.clone()),
        RuntimeValue::Key(v) => Some(v.clone()),
        _ => None,
    }
}
