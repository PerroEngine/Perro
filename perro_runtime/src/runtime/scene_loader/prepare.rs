use perro_core::{
    Quaternion, SceneNode, SceneNodeData, Vector2, Vector3, ambient_light_3d::AmbientLight3D,
    camera_2d::Camera2D, camera_3d::Camera3D, mesh_instance_3d::MeshInstance3D,
    node_2d::node_2d::Node2D, node_3d::node_3d::Node3D, point_light_3d::PointLight3D,
    ray_light_3d::RayLight3D, spot_light_3d::SpotLight3D, sprite_2d::Sprite2D,
};
use perro_io::load_asset;
use perro_render_bridge::Material3D;
use perro_scene::{
    Parser, RuntimeNodeData, RuntimeNodeEntry, RuntimeScene, RuntimeValue, StaticNodeData,
    StaticNodeEntry, StaticNodeType, StaticScene, StaticSceneValue,
};
use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

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
    pub(super) texture_source: Option<String>,
    pub(super) mesh_source: Option<String>,
    pub(super) material_source: Option<String>,
    pub(super) material_inline: Option<Material3D>,
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
        let (node, texture_source, mesh_source, material_source, material_inline) =
            scene_node_from_static_entry(static_node)?;
        if let Some(script) = static_node.script {
            scripts.push(PendingScript {
                node_key: static_node.key.0.to_string(),
                script_path: script.to_string(),
            });
        }
        nodes.push(PendingNode {
            key: static_node.key.0.to_string(),
            parent_key: static_node.parent.map(|k| k.0.to_string()),
            node,
            texture_source,
            mesh_source,
            material_source,
            material_inline,
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
        let (node, texture_source, mesh_source, material_source, material_inline) =
            scene_node_from_runtime_entry(&entry)?;
        if let Some(script) = entry.script {
            scripts.push(PendingScript {
                node_key: entry.key.clone(),
                script_path: script,
            });
        }
        prepared_nodes.push(PendingNode {
            key: entry.key,
            parent_key: entry.parent,
            node,
            texture_source,
            mesh_source,
            material_source,
            material_inline,
        });
    }

    Ok(PreparedScene {
        root_key: root,
        nodes: prepared_nodes,
        scripts,
    })
}
fn scene_node_from_static_entry(
    entry: &StaticNodeEntry,
) -> Result<
    (
        SceneNode,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<Material3D>,
    ),
    String,
> {
    let mut node = SceneNode::new(scene_node_data_from_static(&entry.data)?);
    if let Some(name) = entry.name {
        node.name = Cow::Borrowed(name);
    }
    let texture_source = extract_texture_source_static(&entry.data);
    let mesh_source_explicit = extract_mesh_source_static(&entry.data);
    let material_source_explicit = extract_material_source_static(&entry.data);
    let material_inline = extract_material_inline_static(&entry.data);
    let model_source = extract_model_source_static(&entry.data);
    let (mesh_source, material_source, material_inline) = if let Some(model) = model_source.as_ref()
    {
        (
            Some(format!("{model}:mesh[0]")),
            Some(format!("{model}:mat[0]")),
            None,
        )
    } else {
        (
            mesh_source_explicit,
            material_source_explicit,
            material_inline,
        )
    };
    Ok((
        node,
        texture_source,
        mesh_source,
        material_source,
        material_inline,
    ))
}

fn scene_node_from_runtime_entry(
    entry: &RuntimeNodeEntry,
) -> Result<
    (
        SceneNode,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<Material3D>,
    ),
    String,
> {
    let mut node = SceneNode::new(scene_node_data_from_runtime(&entry.data)?);
    if let Some(name) = &entry.name {
        node.name = Cow::Owned(name.clone());
    }
    let texture_source = extract_texture_source(&entry.data);
    let mesh_source_explicit = extract_mesh_source(&entry.data);
    let material_source_explicit = extract_material_source(&entry.data);
    let material_inline = extract_material_inline(&entry.data);
    let model_source = extract_model_source(&entry.data);
    let (mesh_source, material_source, material_inline) = if let Some(model) = model_source.as_ref()
    {
        (
            Some(format!("{model}:mesh[0]")),
            Some(format!("{model}:mat[0]")),
            None,
        )
    } else {
        (
            mesh_source_explicit,
            material_source_explicit,
            material_inline,
        )
    };
    Ok((
        node,
        texture_source,
        mesh_source,
        material_source,
        material_inline,
    ))
}

fn scene_node_data_from_runtime(data: &RuntimeNodeData) -> Result<SceneNodeData, String> {
    match data.ty.as_str() {
        "Node" => Ok(SceneNodeData::Node),
        "Node2D" => Ok(SceneNodeData::Node2D(build_runtime_node_2d(data))),
        "Sprite2D" => Ok(SceneNodeData::Sprite2D(build_runtime_sprite_2d(data))),
        "Camera2D" => Ok(SceneNodeData::Camera2D(build_runtime_camera_2d(data))),
        "Node3D" => Ok(SceneNodeData::Node3D(build_runtime_node_3d(data))),
        "MeshInstance3D" => Ok(SceneNodeData::MeshInstance3D(
            build_runtime_mesh_instance_3d(data),
        )),
        "Camera3D" => Ok(SceneNodeData::Camera3D(build_runtime_camera_3d(data))),
        "AmbientLight3D" => Ok(SceneNodeData::AmbientLight3D(
            build_runtime_ambient_light_3d(data),
        )),
        "RayLight3D" => Ok(SceneNodeData::RayLight3D(build_runtime_ray_light_3d(data))),
        "PointLight3D" => Ok(SceneNodeData::PointLight3D(build_runtime_point_light_3d(
            data,
        ))),
        "SpotLight3D" => Ok(SceneNodeData::SpotLight3D(build_runtime_spot_light_3d(
            data,
        ))),
        other => Err(format!("unsupported scene node type `{other}`")),
    }
}

fn scene_node_data_from_static(data: &StaticNodeData) -> Result<SceneNodeData, String> {
    match data.ty {
        StaticNodeType::Node => Ok(SceneNodeData::Node),
        StaticNodeType::Node2D => Ok(SceneNodeData::Node2D(build_static_node_2d(data))),
        StaticNodeType::Sprite2D => Ok(SceneNodeData::Sprite2D(build_static_sprite_2d(data))),
        StaticNodeType::Camera2D => Ok(SceneNodeData::Camera2D(build_static_camera_2d(data))),
        StaticNodeType::Node3D => Ok(SceneNodeData::Node3D(build_static_node_3d(data))),
        StaticNodeType::MeshInstance3D => Ok(SceneNodeData::MeshInstance3D(
            build_static_mesh_instance_3d(data),
        )),
        StaticNodeType::Camera3D => Ok(SceneNodeData::Camera3D(build_static_camera_3d(data))),
        StaticNodeType::AmbientLight3D => Ok(SceneNodeData::AmbientLight3D(
            build_static_ambient_light_3d(data),
        )),
        StaticNodeType::RayLight3D => {
            Ok(SceneNodeData::RayLight3D(build_static_ray_light_3d(data)))
        }
        StaticNodeType::PointLight3D => Ok(SceneNodeData::PointLight3D(
            build_static_point_light_3d(data),
        )),
        StaticNodeType::SpotLight3D => {
            Ok(SceneNodeData::SpotLight3D(build_static_spot_light_3d(data)))
        }
    }
}

// Runtime node builders (grouped by spatial domain)
fn build_runtime_node_2d(data: &RuntimeNodeData) -> Node2D {
    let mut node = Node2D::new();
    apply_node_2d_data(&mut node, data);
    node
}

fn build_runtime_sprite_2d(data: &RuntimeNodeData) -> Sprite2D {
    let mut node = Sprite2D::new();
    if let Some(base) = &data.base {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_sprite_2d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_camera_2d(data: &RuntimeNodeData) -> Camera2D {
    let mut node = Camera2D::new();
    if let Some(base) = &data.base {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_camera_2d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_node_3d(data: &RuntimeNodeData) -> Node3D {
    let mut node = Node3D::new();
    apply_node_3d_data(&mut node, data);
    node
}

fn build_runtime_mesh_instance_3d(data: &RuntimeNodeData) -> MeshInstance3D {
    let mut node = MeshInstance3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_mesh_instance_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_camera_3d(data: &RuntimeNodeData) -> Camera3D {
    let mut node = Camera3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_camera_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_ray_light_3d(data: &RuntimeNodeData) -> RayLight3D {
    let mut node = RayLight3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_ray_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_ambient_light_3d(data: &RuntimeNodeData) -> AmbientLight3D {
    let mut node = AmbientLight3D::new();
    apply_ambient_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_point_light_3d(data: &RuntimeNodeData) -> PointLight3D {
    let mut node = PointLight3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_point_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_spot_light_3d(data: &RuntimeNodeData) -> SpotLight3D {
    let mut node = SpotLight3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_spot_light_3d_fields(&mut node, &data.fields);
    node
}

// Static node builders (grouped by spatial domain)
fn build_static_node_2d(data: &StaticNodeData) -> Node2D {
    let mut node = Node2D::new();
    apply_node_2d_data_static(&mut node, data);
    node
}

fn build_static_sprite_2d(data: &StaticNodeData) -> Sprite2D {
    let mut node = Sprite2D::new();
    if let Some(base) = data.base {
        apply_node_2d_data_static(&mut node, base);
    }
    apply_node_2d_fields_static(&mut node, data.fields);
    apply_sprite_2d_fields_static(&mut node, data.fields);
    node
}

fn build_static_camera_2d(data: &StaticNodeData) -> Camera2D {
    let mut node = Camera2D::new();
    if let Some(base) = data.base {
        apply_node_2d_data_static(&mut node, base);
    }
    apply_node_2d_fields_static(&mut node, data.fields);
    apply_camera_2d_fields_static(&mut node, data.fields);
    node
}

fn build_static_node_3d(data: &StaticNodeData) -> Node3D {
    let mut node = Node3D::new();
    apply_node_3d_data_static(&mut node, data);
    node
}

fn build_static_mesh_instance_3d(data: &StaticNodeData) -> MeshInstance3D {
    let mut node = MeshInstance3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_mesh_instance_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_camera_3d(data: &StaticNodeData) -> Camera3D {
    let mut node = Camera3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_camera_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_ray_light_3d(data: &StaticNodeData) -> RayLight3D {
    let mut node = RayLight3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_ray_light_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_ambient_light_3d(data: &StaticNodeData) -> AmbientLight3D {
    let mut node = AmbientLight3D::new();
    apply_ambient_light_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_point_light_3d(data: &StaticNodeData) -> PointLight3D {
    let mut node = PointLight3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_point_light_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_spot_light_3d(data: &StaticNodeData) -> SpotLight3D {
    let mut node = SpotLight3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_spot_light_3d_fields_static(&mut node, data.fields);
    node
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

// Runtime field application: 2D
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

// Runtime field application: 3D
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

fn apply_mesh_instance_3d_fields(_node: &mut MeshInstance3D, _fields: &[(String, RuntimeValue)]) {}

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

fn apply_ray_light_3d_fields(node: &mut RayLight3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
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

fn apply_ambient_light_3d_fields(node: &mut AmbientLight3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
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

fn apply_point_light_3d_fields(node: &mut PointLight3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "range" => {
                if let Some(v) = as_f32(value) {
                    node.range = v;
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

fn apply_spot_light_3d_fields(node: &mut SpotLight3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "range" => {
                if let Some(v) = as_f32(value) {
                    node.range = v;
                }
            }
            "inner_angle_radians" => {
                if let Some(v) = as_f32(value) {
                    node.inner_angle_radians = v;
                }
            }
            "outer_angle_radians" => {
                if let Some(v) = as_f32(value) {
                    node.outer_angle_radians = v;
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

// Static field application: 2D
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

// Static field application: 3D
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
    _node: &mut MeshInstance3D,
    _fields: &[(&str, StaticSceneValue)],
) {
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

fn apply_ray_light_3d_fields_static(node: &mut RayLight3D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "color" => {
                if let Some(v) = as_vec3_static(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32_static(value) {
                    node.intensity = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool_static(value) {
                    node.active = v;
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

fn apply_ambient_light_3d_fields_static(
    node: &mut AmbientLight3D,
    fields: &[(&str, StaticSceneValue)],
) {
    for (name, value) in fields {
        match *name {
            "color" => {
                if let Some(v) = as_vec3_static(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32_static(value) {
                    node.intensity = v;
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

fn apply_point_light_3d_fields_static(
    node: &mut PointLight3D,
    fields: &[(&str, StaticSceneValue)],
) {
    for (name, value) in fields {
        match *name {
            "color" => {
                if let Some(v) = as_vec3_static(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32_static(value) {
                    node.intensity = v;
                }
            }
            "range" => {
                if let Some(v) = as_f32_static(value) {
                    node.range = v;
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

fn apply_spot_light_3d_fields_static(node: &mut SpotLight3D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "color" => {
                if let Some(v) = as_vec3_static(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32_static(value) {
                    node.intensity = v;
                }
            }
            "range" => {
                if let Some(v) = as_f32_static(value) {
                    node.range = v;
                }
            }
            "inner_angle_radians" => {
                if let Some(v) = as_f32_static(value) {
                    node.inner_angle_radians = v;
                }
            }
            "outer_angle_radians" => {
                if let Some(v) = as_f32_static(value) {
                    node.outer_angle_radians = v;
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

// Runtime value parsers
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

fn as_texture_index(value: &RuntimeValue) -> Option<u32> {
    match value {
        RuntimeValue::Object(entries) => entries.iter().find_map(|(name, inner)| {
            if name != "index" {
                return None;
            }
            match inner {
                RuntimeValue::I32(v) if *v >= 0 => Some(*v as u32),
                _ => None,
            }
        }),
        _ => None,
    }
}

fn as_alpha_mode(value: &RuntimeValue) -> Option<u32> {
    match value {
        RuntimeValue::Str(s) => match s.as_str() {
            "OPAQUE" => Some(0),
            "MASK" => Some(1),
            "BLEND" => Some(2),
            _ => None,
        },
        RuntimeValue::I32(v) if (0..=2).contains(v) => Some(*v as u32),
        _ => None,
    }
}

fn as_asset_source(value: &RuntimeValue) -> Option<String> {
    match value {
        RuntimeValue::Str(v) => Some(v.clone()),
        RuntimeValue::Key(v) => Some(v.clone()),
        _ => None,
    }
}

fn as_color4(value: &RuntimeValue) -> Option<[f32; 4]> {
    match value {
        RuntimeValue::Vec4 { x, y, z, w } => Some([*x, *y, *z, *w]),
        RuntimeValue::Vec3 { x, y, z } => Some([*x, *y, *z, 1.0]),
        _ => None,
    }
}

fn extract_texture_source(data: &RuntimeNodeData) -> Option<String> {
    if data.ty != "Sprite2D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (name == "texture")
            .then(|| as_asset_source(value))
            .flatten()
    })
}

fn extract_mesh_source(data: &RuntimeNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" {
        return None;
    }
    data.fields
        .iter()
        .find_map(|(name, value)| (name == "mesh").then(|| as_asset_source(value)).flatten())
}

fn extract_material_source(data: &RuntimeNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (name == "material")
            .then(|| as_asset_source(value))
            .flatten()
    })
}

fn extract_material_inline(data: &RuntimeNodeData) -> Option<Material3D> {
    if data.ty != "MeshInstance3D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        if name != "material" {
            return None;
        }
        match value {
            RuntimeValue::Object(entries) => material_from_runtime_object(entries),
            _ => None,
        }
    })
}

fn extract_model_source(data: &RuntimeNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" {
        return None;
    }
    data.fields
        .iter()
        .find_map(|(name, value)| (name == "model").then(|| as_asset_source(value)).flatten())
}

fn material_from_runtime_object(entries: &[(String, RuntimeValue)]) -> Option<Material3D> {
    let mut material = Material3D::default();
    let mut any = false;
    apply_runtime_material_entries(entries, &mut material, &mut any);
    any.then_some(material)
}

fn apply_runtime_material_entries(
    entries: &[(String, RuntimeValue)],
    material: &mut Material3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match name.as_str() {
            "roughnessFactor" => {
                if let Some(v) = as_f32(value) {
                    material.roughness_factor = v;
                    *any = true;
                }
            }
            "metallicFactor" => {
                if let Some(v) = as_f32(value) {
                    material.metallic_factor = v;
                    *any = true;
                }
            }
            "occlusionStrength" => {
                if let Some(v) = as_f32(value) {
                    material.occlusion_strength = v;
                    *any = true;
                }
            }
            "emissiveFactor" => {
                if let Some(color) = as_color4(value) {
                    material.emissive_factor = [color[0], color[1], color[2]];
                    *any = true;
                }
            }
            "baseColorFactor" => {
                if let Some(color) = as_color4(value) {
                    material.base_color_factor = color;
                    *any = true;
                }
            }
            "normalScale" => {
                if let Some(v) = as_f32(value) {
                    material.normal_scale = v;
                    *any = true;
                }
            }
            "alphaCutoff" => {
                if let Some(v) = as_f32(value) {
                    material.alpha_cutoff = v;
                    *any = true;
                }
            }
            "alphaMode" => {
                if let Some(mode) = as_alpha_mode(value) {
                    material.alpha_mode = mode;
                    *any = true;
                }
            }
            "doubleSided" => {
                if let Some(v) = as_bool(value) {
                    material.double_sided = v;
                    *any = true;
                }
            }
            "baseColorTexture" => {
                if let Some(index) = as_texture_index(value) {
                    material.base_color_texture = index;
                    *any = true;
                }
            }
            "metallicRoughnessTexture" => {
                if let Some(index) = as_texture_index(value) {
                    material.metallic_roughness_texture = index;
                    *any = true;
                }
            }
            "normalTexture" => {
                if let Some(index) = as_texture_index(value) {
                    material.normal_texture = index;
                    *any = true;
                }
            }
            "occlusionTexture" => {
                if let Some(index) = as_texture_index(value) {
                    material.occlusion_texture = index;
                    *any = true;
                }
            }
            "emissiveTexture" => {
                if let Some(index) = as_texture_index(value) {
                    material.emissive_texture = index;
                    *any = true;
                }
            }
            "pbrMetallicRoughness" => {
                if let RuntimeValue::Object(inner) = value {
                    apply_runtime_material_entries(inner, material, any);
                }
            }
            _ => {}
        }
    }
}

// Static value parsers
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

fn as_texture_index_static(value: &StaticSceneValue) -> Option<u32> {
    match value {
        StaticSceneValue::Object(entries) => entries.iter().find_map(|(name, inner)| {
            if *name != "index" {
                return None;
            }
            match inner {
                StaticSceneValue::I32(v) if *v >= 0 => Some(*v as u32),
                _ => None,
            }
        }),
        _ => None,
    }
}

fn as_alpha_mode_static(value: &StaticSceneValue) -> Option<u32> {
    match value {
        StaticSceneValue::Str(s) => match *s {
            "OPAQUE" => Some(0),
            "MASK" => Some(1),
            "BLEND" => Some(2),
            _ => None,
        },
        StaticSceneValue::I32(v) if (0..=2).contains(v) => Some(*v as u32),
        _ => None,
    }
}

fn as_asset_source_static(value: &StaticSceneValue) -> Option<String> {
    match value {
        StaticSceneValue::Str(v) => Some((*v).to_string()),
        StaticSceneValue::Key(v) => Some(v.0.to_string()),
        _ => None,
    }
}

fn as_color4_static(value: &StaticSceneValue) -> Option<[f32; 4]> {
    match value {
        StaticSceneValue::Vec4 { x, y, z, w } => Some([*x, *y, *z, *w]),
        StaticSceneValue::Vec3 { x, y, z } => Some([*x, *y, *z, 1.0]),
        _ => None,
    }
}

fn extract_texture_source_static(data: &StaticNodeData) -> Option<String> {
    if data.ty != StaticNodeType::Sprite2D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (*name == "texture")
            .then(|| as_asset_source_static(value))
            .flatten()
    })
}

fn extract_mesh_source_static(data: &StaticNodeData) -> Option<String> {
    if data.ty != StaticNodeType::MeshInstance3D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (*name == "mesh")
            .then(|| as_asset_source_static(value))
            .flatten()
    })
}

fn extract_material_source_static(data: &StaticNodeData) -> Option<String> {
    if data.ty != StaticNodeType::MeshInstance3D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (*name == "material")
            .then(|| as_asset_source_static(value))
            .flatten()
    })
}

fn extract_material_inline_static(data: &StaticNodeData) -> Option<Material3D> {
    if data.ty != StaticNodeType::MeshInstance3D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        if *name != "material" {
            return None;
        }
        match value {
            StaticSceneValue::Object(entries) => material_from_static_object(entries),
            _ => None,
        }
    })
}

fn extract_model_source_static(data: &StaticNodeData) -> Option<String> {
    if data.ty != StaticNodeType::MeshInstance3D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (*name == "model")
            .then(|| as_asset_source_static(value))
            .flatten()
    })
}

fn material_from_static_object(entries: &[(&str, StaticSceneValue)]) -> Option<Material3D> {
    let mut material = Material3D::default();
    let mut any = false;
    apply_static_material_entries(entries, &mut material, &mut any);
    any.then_some(material)
}

fn apply_static_material_entries(
    entries: &[(&str, StaticSceneValue)],
    material: &mut Material3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match *name {
            "roughnessFactor" => {
                if let Some(v) = as_f32_static(value) {
                    material.roughness_factor = v;
                    *any = true;
                }
            }
            "metallicFactor" => {
                if let Some(v) = as_f32_static(value) {
                    material.metallic_factor = v;
                    *any = true;
                }
            }
            "occlusionStrength" => {
                if let Some(v) = as_f32_static(value) {
                    material.occlusion_strength = v;
                    *any = true;
                }
            }
            "emissiveFactor" => {
                if let Some(color) = as_color4_static(value) {
                    material.emissive_factor = [color[0], color[1], color[2]];
                    *any = true;
                }
            }
            "baseColorFactor" => {
                if let Some(color) = as_color4_static(value) {
                    material.base_color_factor = color;
                    *any = true;
                }
            }
            "normalScale" => {
                if let Some(v) = as_f32_static(value) {
                    material.normal_scale = v;
                    *any = true;
                }
            }
            "alphaCutoff" => {
                if let Some(v) = as_f32_static(value) {
                    material.alpha_cutoff = v;
                    *any = true;
                }
            }
            "alphaMode" => {
                if let Some(mode) = as_alpha_mode_static(value) {
                    material.alpha_mode = mode;
                    *any = true;
                }
            }
            "doubleSided" => {
                if let Some(v) = as_bool_static(value) {
                    material.double_sided = v;
                    *any = true;
                }
            }
            "baseColorTexture" => {
                if let Some(index) = as_texture_index_static(value) {
                    material.base_color_texture = index;
                    *any = true;
                }
            }
            "metallicRoughnessTexture" => {
                if let Some(index) = as_texture_index_static(value) {
                    material.metallic_roughness_texture = index;
                    *any = true;
                }
            }
            "normalTexture" => {
                if let Some(index) = as_texture_index_static(value) {
                    material.normal_texture = index;
                    *any = true;
                }
            }
            "occlusionTexture" => {
                if let Some(index) = as_texture_index_static(value) {
                    material.occlusion_texture = index;
                    *any = true;
                }
            }
            "emissiveTexture" => {
                if let Some(index) = as_texture_index_static(value) {
                    material.emissive_texture = index;
                    *any = true;
                }
            }
            "pbrMetallicRoughness" => {
                if let StaticSceneValue::Object(inner) = value {
                    apply_static_material_entries(inner, material, any);
                }
            }
            _ => {}
        }
    }
}
