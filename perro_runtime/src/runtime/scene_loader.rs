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
use std::{borrow::Cow, collections::HashMap};

impl Runtime {
    pub(crate) fn load_boot_scene(&mut self) -> Result<(), String> {
        let (project_root, project_name, main_scene_path, static_lookup) = {
            let project = self
                .project()
                .ok_or_else(|| "Runtime project is not set".to_string())?;
            (
                project.root.clone(),
                project.config.name.clone(),
                project.config.main_scene.clone(),
                project.static_scene_lookup,
            )
        };

        set_project_root(ProjectRoot::Disk {
            root: project_root,
            name: project_name,
        });

        let runtime_scene = match self.provider_mode {
            ProviderMode::Dynamic => load_runtime_scene_from_disk(&main_scene_path)?,
            ProviderMode::Static => match static_lookup.and_then(|lookup| lookup(&main_scene_path)) {
                Some(scene) => runtime_scene_from_static_scene(scene),
                None => load_runtime_scene_from_disk(&main_scene_path)?,
            },
        };

        self.nodes.clear();
        insert_runtime_scene_nodes(self, runtime_scene)
    }
}

fn load_runtime_scene_from_disk(path: &str) -> Result<RuntimeScene, String> {
    let bytes = load_asset(path).map_err(|err| format!("failed to load scene `{path}`: {err}"))?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|err| format!("scene `{path}` is not valid UTF-8: {err}"))?;
    Ok(Parser::new(source).parse_scene())
}

fn runtime_scene_from_static_scene(scene: &'static StaticScene) -> RuntimeScene {
    RuntimeScene {
        nodes: scene
            .nodes
            .iter()
            .map(runtime_node_entry_from_static)
            .collect(),
        root: scene.root.map(|key| key.0.to_string()),
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

fn insert_runtime_scene_nodes(runtime: &mut Runtime, scene: RuntimeScene) -> Result<(), String> {
    let RuntimeScene { nodes, root } = scene;
    let mut pending_nodes = Vec::with_capacity(nodes.len());
    for entry in nodes {
        pending_nodes.push(PendingNode {
            key: entry.key.clone(),
            parent_key: entry.parent.clone(),
            node: scene_node_from_runtime_entry(&entry)?,
        });
    }

    let mut key_to_id: HashMap<String, NodeID> = HashMap::with_capacity(pending_nodes.len());
    let mut parent_pairs = Vec::with_capacity(pending_nodes.len());

    for pending in pending_nodes {
        if key_to_id.contains_key(&pending.key) {
            return Err(format!("duplicate scene key `{}`", pending.key));
        }

        let node_id = runtime.nodes.insert(pending.node);
        if let Some(parent_key) = pending.parent_key {
            parent_pairs.push((pending.key.clone(), parent_key));
        }
        key_to_id.insert(pending.key, node_id);
    }

    if let Some(root_key) = root {
        if !key_to_id.contains_key(&root_key) {
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
            .ok_or_else(|| format!("parent node key `{parent_key}` not found"))?;

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

    Ok(())
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
