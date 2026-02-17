use perro_core::{
    Quaternion, SceneNode, SceneNodeData, Vector2, Vector3,
    camera_2d::Camera2D, camera_3d::Camera3D, mesh_instance_3d::MeshInstance3D,
    node_2d::node_2d::Node2D, node_3d::node_3d::Node3D, sprite_2d::Sprite2D,
};
use perro_io::load_asset;
use perro_scene::{
    Parser, RuntimeNodeData, RuntimeNodeEntry, RuntimeScene, RuntimeValue, StaticNodeData,
    StaticNodeEntry, StaticNodeType, StaticScene, StaticSceneValue,
};
use std::{borrow::Cow, time::{Duration, Instant}};

pub(super) struct RuntimeSceneLoadStats {
    pub(super) source_load: Duration,
    pub(super) parse: Duration,
}

pub(super) struct PreparedScene {
    pub(super) root_key: Option<String>,
    pub(super) nodes: Vec<PendingNode>,
    pub(super) scripts: Vec<PendingScript>,
}

pub(super) struct PendingScript {
    pub(super) node_key: String,
    pub(super) script_path: String,
}

pub(super) struct PendingNode {
    pub(super) key: String,
    pub(super) parent_key: Option<String>,
    pub(super) node: SceneNode,
}

pub(super) fn load_runtime_scene_from_disk(
    path: &str,
) -> Result<(RuntimeScene, RuntimeSceneLoadStats), String> {
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

pub(super) fn prepare_static_scene(scene: &'static StaticScene) -> Result<PreparedScene, String> {
    let mut nodes = Vec::with_capacity(scene.nodes.len());
    let mut scripts = Vec::new();

    for static_node in scene.nodes {
        let mut node = scene_node_from_static_entry(static_node)?;
        if let Some(script) = static_node.script {
            scripts.push(PendingScript {
                node_key: static_node.key.0.to_string(),
                script_path: script.to_string(),
            });
            node.script = None;
        }
        nodes.push(PendingNode {
            key: static_node.key.0.to_string(),
            parent_key: static_node.parent.map(|k| k.0.to_string()),
            node,
        });
    }

    Ok(PreparedScene {
        root_key: scene.root.map(|k| k.0.to_string()),
        nodes,
        scripts,
    })
}

pub(super) fn prepare_runtime_scene(scene: RuntimeScene) -> Result<PreparedScene, String> {
    let RuntimeScene { nodes, root } = scene;
    let mut prepared_nodes = Vec::with_capacity(nodes.len());
    let mut scripts = Vec::new();

    for entry in nodes {
        let mut node = scene_node_from_runtime_entry(&entry)?;
        if let Some(script) = entry.script {
            scripts.push(PendingScript {
                node_key: entry.key.clone(),
                script_path: script,
            });
            node.script = None;
        }
        prepared_nodes.push(PendingNode {
            key: entry.key,
            parent_key: entry.parent,
            node,
        });
    }

    Ok(PreparedScene {
        root_key: root,
        nodes: prepared_nodes,
        scripts,
    })
}
fn scene_node_from_static_entry(entry: &StaticNodeEntry) -> Result<SceneNode, String> {
    let mut node = SceneNode::new(scene_node_data_from_static(&entry.data)?);
    if let Some(name) = entry.name {
        node.name = Cow::Borrowed(name);
    }
    if let Some(script) = entry.script {
        node.script = Some(Cow::Borrowed(script));
    }
    Ok(node)
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

fn scene_node_data_from_static(data: &StaticNodeData) -> Result<SceneNodeData, String> {
    match data.ty {
        StaticNodeType::Node => Ok(SceneNodeData::Node),
        StaticNodeType::Node2D => {
            let mut node = Node2D::new();
            apply_node_2d_data_static(&mut node, data);
            Ok(SceneNodeData::Node2D(node))
        }
        StaticNodeType::Sprite2D => {
            let mut node = Sprite2D::new();
            if let Some(base) = data.base {
                apply_node_2d_data_static(&mut node.base, base);
            }
            apply_node_2d_fields_static(&mut node.base, data.fields);
            apply_sprite_2d_fields_static(&mut node, data.fields);
            Ok(SceneNodeData::Sprite2D(node))
        }
        StaticNodeType::Camera2D => {
            let mut node = Camera2D::new();
            if let Some(base) = data.base {
                apply_node_2d_data_static(&mut node.base, base);
            }
            apply_node_2d_fields_static(&mut node.base, data.fields);
            apply_camera_2d_fields_static(&mut node, data.fields);
            Ok(SceneNodeData::Camera2D(node))
        }
        StaticNodeType::Node3D => {
            let mut node = Node3D::new();
            apply_node_3d_data_static(&mut node, data);
            Ok(SceneNodeData::Node3D(node))
        }
        StaticNodeType::MeshInstance3D => {
            let mut node = MeshInstance3D::new();
            if let Some(base) = data.base {
                apply_node_3d_data_static(&mut node.base, base);
            }
            apply_node_3d_fields_static(&mut node.base, data.fields);
            apply_mesh_instance_3d_fields_static(&mut node, data.fields);
            Ok(SceneNodeData::MeshInstance3D(node))
        }
        StaticNodeType::Camera3D => {
            let mut node = Camera3D::new();
            if let Some(base) = data.base {
                apply_node_3d_data_static(&mut node.base, base);
            }
            apply_node_3d_fields_static(&mut node.base, data.fields);
            apply_camera_3d_fields_static(&mut node, data.fields);
            Ok(SceneNodeData::Camera3D(node))
        }
    }
}

fn apply_node_2d_data_static(target: &mut Node2D, data: &StaticNodeData) {
    if let Some(base) = data.base {
        apply_node_2d_data_static(target, base);
    }
    apply_node_2d_fields_static(target, data.fields);
}

fn apply_node_3d_data_static(target: &mut Node3D, data: &StaticNodeData) {
    if let Some(base) = data.base {
        apply_node_3d_data_static(target, base);
    }
    apply_node_3d_fields_static(target, data.fields);
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

fn apply_node_2d_fields_static(node: &mut Node2D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "position" => {
                if let Some(v) = as_vec2_static(value) {
                    node.transform.position = v;
                }
            }
            "scale" => {
                if let Some(v) = as_vec2_static(value) {
                    node.transform.scale = v;
                }
            }
            "rotation" => {
                if let Some(v) = as_f32_static(value) {
                    node.transform.rotation = v;
                }
            }
            "z_index" => {
                if let Some(v) = as_i32_static(value) {
                    node.z_index = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool_static(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_sprite_2d_fields_static(_node: &mut Sprite2D, _fields: &[(&str, StaticSceneValue)]) {}

fn apply_camera_2d_fields_static(node: &mut Camera2D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "zoom" => {
                if let Some(v) = as_f32_static(value) {
                    node.zoom = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool_static(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_node_3d_fields_static(node: &mut Node3D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "position" => {
                if let Some(v) = as_vec3_static(value) {
                    node.transform.position = v;
                }
            }
            "scale" => {
                if let Some(v) = as_vec3_static(value) {
                    node.transform.scale = v;
                }
            }
            "rotation" => {
                if let Some(v) = as_quat_static(value) {
                    node.transform.rotation = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool_static(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_mesh_instance_3d_fields_static(
    node: &mut MeshInstance3D,
    fields: &[(&str, StaticSceneValue)],
) {
    for (name, value) in fields {
        match *name {
            "mesh" => {
                if let Some(v) = as_mesh_source_static(value) {
                    node.mesh = Some(Cow::Owned(v));
                }
            }
            "mesh_id" => {
                if let Some(v) = as_u64_static(value) {
                    node.mesh_id = perro_ids::MeshID::from_u64(v);
                }
            }
            "material_id" => {
                if let Some(v) = as_u64_static(value) {
                    node.material_id = perro_ids::MaterialID::from_u64(v);
                }
            }
            _ => {}
        }
    }
}

fn apply_camera_3d_fields_static(node: &mut Camera3D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "zoom" => {
                if let Some(v) = as_f32_static(value) {
                    node.zoom = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool_static(value) {
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

fn as_bool_static(value: &StaticSceneValue) -> Option<bool> {
    match value {
        StaticSceneValue::Bool(v) => Some(*v),
        _ => None,
    }
}

fn as_i32_static(value: &StaticSceneValue) -> Option<i32> {
    match value {
        StaticSceneValue::I32(v) => Some(*v),
        StaticSceneValue::F32(v) => Some(*v as i32),
        _ => None,
    }
}

fn as_u64_static(value: &StaticSceneValue) -> Option<u64> {
    match value {
        StaticSceneValue::I32(v) => (*v >= 0).then_some(*v as u64),
        StaticSceneValue::F32(v) => (*v >= 0.0).then_some(*v as u64),
        StaticSceneValue::Str(v) => v.parse::<u64>().ok(),
        _ => None,
    }
}

fn as_f32_static(value: &StaticSceneValue) -> Option<f32> {
    match value {
        StaticSceneValue::F32(v) => Some(*v),
        StaticSceneValue::I32(v) => Some(*v as f32),
        _ => None,
    }
}

fn as_vec2_static(value: &StaticSceneValue) -> Option<Vector2> {
    match value {
        StaticSceneValue::Vec2 { x, y } => Some(Vector2::new(*x, *y)),
        _ => None,
    }
}

fn as_vec3_static(value: &StaticSceneValue) -> Option<Vector3> {
    match value {
        StaticSceneValue::Vec3 { x, y, z } => Some(Vector3::new(*x, *y, *z)),
        _ => None,
    }
}

fn as_quat_static(value: &StaticSceneValue) -> Option<Quaternion> {
    match value {
        StaticSceneValue::Vec4 { x, y, z, w } => Some(Quaternion::new(*x, *y, *z, *w)),
        _ => None,
    }
}

fn as_mesh_source_static(value: &StaticSceneValue) -> Option<String> {
    match value {
        StaticSceneValue::Str(v) => Some((*v).to_string()),
        StaticSceneValue::Key(v) => Some(v.0.to_string()),
        _ => None,
    }
}
