use super::Runtime;
use perro_core::SceneNodeData;
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_io::load_asset;
use perro_scene::{Parser, RuntimeValue};
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, Command3D, Material3D, PointLight3DState,
    RayLight3DState, RenderCommand, RenderRequestID, ResourceCommand, SpotLight3DState,
};

impl Runtime {
    fn mesh_request_id(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x3E)
    }

    fn material_request_id(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x3F)
    }

    pub fn extract_render_3d_commands(&mut self) {
        self.propagate_pending_transform_dirty();

        let mut traversal_ids = std::mem::take(&mut self.render_3d.traversal_ids);
        traversal_ids.clear();
        traversal_ids.extend(self.nodes.iter().map(|(id, _)| id));

        for node_id in traversal_ids.iter().copied() {
            let camera_data = self.nodes.get(node_id).and_then(|node| match &node.data {
                SceneNodeData::Camera3D(camera) if camera.active => Some(Camera3DState {
                    position: [
                        camera.transform.position.x,
                        camera.transform.position.y,
                        camera.transform.position.z,
                    ],
                    rotation: [
                        camera.transform.rotation.x,
                        camera.transform.rotation.y,
                        camera.transform.rotation.z,
                        camera.transform.rotation.w,
                    ],
                    zoom: camera.zoom,
                }),
                _ => None,
            });
            if let Some(camera) = camera_data {
                self.queue_render_command(RenderCommand::ThreeD(Command3D::SetCamera { camera }));
            }

            let ambient_light_data = self.nodes.get(node_id).and_then(|node| match &node.data {
                SceneNodeData::AmbientLight3D(light) if light.active && light.visible => {
                    Some(AmbientLight3DState {
                        color: light.color,
                        intensity: light.intensity.max(0.0),
                    })
                }
                _ => None,
            });
            if let Some(light) = ambient_light_data {
                self.queue_render_command(RenderCommand::ThreeD(Command3D::SetAmbientLight {
                    node: node_id,
                    light,
                }));
            }

            let ray_light_data = self.nodes.get(node_id).and_then(|node| match &node.data {
                SceneNodeData::RayLight3D(light) if light.active && light.visible => {
                    Some(RayLight3DState {
                        direction: quaternion_forward(light.transform.rotation),
                        color: light.color,
                        intensity: light.intensity.max(0.0),
                    })
                }
                _ => None,
            });
            if let Some(light) = ray_light_data {
                self.queue_render_command(RenderCommand::ThreeD(Command3D::SetRayLight {
                    node: node_id,
                    light,
                }));
            }

            let point_light_data = self.nodes.get(node_id).and_then(|node| match &node.data {
                SceneNodeData::PointLight3D(light) if light.active && light.visible => {
                    Some(PointLight3DState {
                        position: [
                            light.transform.position.x,
                            light.transform.position.y,
                            light.transform.position.z,
                        ],
                        color: light.color,
                        intensity: light.intensity.max(0.0),
                        range: light.range.max(0.001),
                    })
                }
                _ => None,
            });
            if let Some(light) = point_light_data {
                self.queue_render_command(RenderCommand::ThreeD(Command3D::SetPointLight {
                    node: node_id,
                    light,
                }));
            }

            let spot_light_data = self.nodes.get(node_id).and_then(|node| match &node.data {
                SceneNodeData::SpotLight3D(light) if light.active && light.visible => {
                    Some(SpotLight3DState {
                        position: [
                            light.transform.position.x,
                            light.transform.position.y,
                            light.transform.position.z,
                        ],
                        direction: quaternion_forward(light.transform.rotation),
                        color: light.color,
                        intensity: light.intensity.max(0.0),
                        range: light.range.max(0.001),
                        inner_angle_radians: light.inner_angle_radians.max(0.0),
                        outer_angle_radians: light
                            .outer_angle_radians
                            .max(light.inner_angle_radians),
                    })
                }
                _ => None,
            });
            if let Some(light) = spot_light_data {
                self.queue_render_command(RenderCommand::ThreeD(Command3D::SetSpotLight {
                    node: node_id,
                    light,
                }));
            }

            let mesh_data = self.nodes.get(node_id).and_then(|node| match &node.data {
                SceneNodeData::MeshInstance3D(mesh) => Some((
                    mesh.visible,
                    mesh.mesh_id,
                    mesh.material_id,
                    mesh.transform.to_mat4().to_cols_array_2d(),
                )),
                _ => None,
            });
            let Some((visible, mesh_id, material_id, model)) = mesh_data else {
                continue;
            };
            if !visible {
                continue;
            }

            let Some((mesh, material)) =
                self.resolve_mesh_instance_assets(node_id, mesh_id, material_id)
            else {
                continue;
            };
            self.queue_render_command(RenderCommand::ThreeD(Command3D::Draw {
                mesh,
                material,
                node: node_id,
                model,
            }));
        }

        traversal_ids.clear();
        self.render_3d.traversal_ids = traversal_ids;
    }

    fn resolve_mesh_instance_assets(
        &mut self,
        node_id: NodeID,
        mut mesh_id: MeshID,
        mut material_id: MaterialID,
    ) -> Option<(MeshID, MaterialID)> {
        if mesh_id.is_nil() {
            let request = Self::mesh_request_id(node_id);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Mesh(id) => {
                        mesh_id = id;
                        if let Some(node) = self.nodes.get_mut(node_id) {
                            if let SceneNodeData::MeshInstance3D(mesh_instance) = &mut node.data {
                                mesh_instance.mesh_id = id;
                            }
                        }
                    }
                    crate::RuntimeRenderResult::Failed(_)
                    | crate::RuntimeRenderResult::Texture(_)
                    | crate::RuntimeRenderResult::Material(_) => {}
                }
            }
            if mesh_id.is_nil() {
                let source = self
                    .render_3d
                    .mesh_sources
                    .get(&node_id)?
                    .trim()
                    .to_string();
                if source.is_empty() {
                    return None;
                }
                if !self.render.is_inflight(request) {
                    self.render.mark_inflight(request);
                    self.queue_render_command(RenderCommand::Resource(
                        ResourceCommand::CreateMesh {
                            request,
                            owner: node_id,
                            source,
                        },
                    ));
                }
                return None;
            }
        }

        if material_id.is_nil() {
            let request = Self::material_request_id(node_id);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Material(id) => {
                        material_id = id;
                        if let Some(node) = self.nodes.get_mut(node_id) {
                            if let SceneNodeData::MeshInstance3D(mesh_instance) = &mut node.data {
                                mesh_instance.material_id = id;
                            }
                        }
                    }
                    crate::RuntimeRenderResult::Failed(_)
                    | crate::RuntimeRenderResult::Texture(_)
                    | crate::RuntimeRenderResult::Mesh(_) => {}
                }
            }
            if material_id.is_nil() {
                let source = self.render_3d.material_sources.get(&node_id).cloned();
                let material = self
                    .render_3d
                    .material_overrides
                    .get(&node_id)
                    .copied()
                    .or_else(|| {
                        self.render_3d
                            .material_sources
                            .get(&node_id)
                            .and_then(|source| load_material_from_source(self, source))
                    })
                    .unwrap_or_else(Material3D::default);
                if !self.render.is_inflight(request) {
                    self.render.mark_inflight(request);
                    self.queue_render_command(RenderCommand::Resource(
                        ResourceCommand::CreateMaterial {
                            request,
                            owner: node_id,
                            material,
                            source,
                        },
                    ));
                }
                return None;
            }
        }

        Some((mesh_id, material_id))
    }
}

fn quaternion_forward(rotation: perro_core::Quaternion) -> [f32; 3] {
    let len_sq = rotation.x * rotation.x
        + rotation.y * rotation.y
        + rotation.z * rotation.z
        + rotation.w * rotation.w;
    let (x, y, z, w) = if len_sq.is_finite() && len_sq > 1.0e-6 {
        let inv_len = len_sq.sqrt().recip();
        (
            rotation.x * inv_len,
            rotation.y * inv_len,
            rotation.z * inv_len,
            rotation.w * inv_len,
        )
    } else {
        (0.0, 0.0, 0.0, 1.0)
    };

    let fx = -(2.0 * (x * z + w * y));
    let fy = -(2.0 * (y * z - w * x));
    let fz = -(1.0 - 2.0 * (x * x + y * y));
    let forward_len_sq = fx * fx + fy * fy + fz * fz;
    if forward_len_sq.is_finite() && forward_len_sq > 1.0e-6 {
        let inv_len = forward_len_sq.sqrt().recip();
        [fx * inv_len, fy * inv_len, fz * inv_len]
    } else {
        [0.0, 0.0, -1.0]
    }
}

fn load_material_from_source(runtime: &Runtime, source: &str) -> Option<Material3D> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }

    let (path, fragment) = split_source_fragment(source);
    if path.ends_with(".pmat") {
        if let Some(lookup) = runtime
            .project()
            .and_then(|project| project.static_material_lookup)
        {
            if let Some(material) = lookup(path).copied() {
                return Some(material);
            }
        }
        return load_pmat(path);
    }

    if path.ends_with(".glb") || path.ends_with(".gltf") {
        let index = parse_fragment_index(fragment, &["mat", "material"]).unwrap_or(0);
        return Some(material_from_glb_index(index));
    }

    None
}

fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return (source, None);
    };
    if path.is_empty() {
        return (source, None);
    }
    if !selector.contains('=') {
        return (source, None);
    }
    if selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    (path, Some(selector))
}

fn parse_fragment_index(fragment: Option<&str>, keys: &[&str]) -> Option<u32> {
    let fragment = fragment?;
    for segment in fragment.split('&') {
        let (key, value) = segment.split_once('=')?;
        let key = key.trim();
        let value = value.trim();
        if keys.iter().any(|candidate| *candidate == key) {
            if let Ok(parsed) = value.parse::<u32>() {
                return Some(parsed);
            }
        }
    }
    None
}

fn material_from_glb_index(index: u32) -> Material3D {
    let mut x = index ^ 0x9E37_79B9;
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;

    let r = ((x & 0xFF) as f32 / 255.0) * 0.65 + 0.2;
    let g = (((x >> 8) & 0xFF) as f32 / 255.0) * 0.65 + 0.2;
    let b = (((x >> 16) & 0xFF) as f32 / 255.0) * 0.65 + 0.2;
    Material3D {
        base_color: [r, g, b, 1.0],
        roughness: 0.5,
        metallic: 0.0,
        ao: 1.0,
        emissive: 0.0,
    }
}

fn load_pmat(source: &str) -> Option<Material3D> {
    let bytes = load_asset(source).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let value = std::panic::catch_unwind(|| Parser::new(text).parse_value_literal()).ok()?;
    material_from_runtime_value(&value)
}

fn material_from_runtime_value(value: &RuntimeValue) -> Option<Material3D> {
    let RuntimeValue::Object(entries) = value else {
        return None;
    };
    let mut out = Material3D::default();
    let mut any = false;
    for (name, value) in entries {
        match name.as_str() {
            "roughness" => {
                if let Some(v) = runtime_as_f32(value) {
                    out.roughness = v;
                    any = true;
                }
            }
            "metallic" => {
                if let Some(v) = runtime_as_f32(value) {
                    out.metallic = v;
                    any = true;
                }
            }
            "ao" => {
                if let Some(v) = runtime_as_f32(value) {
                    out.ao = v;
                    any = true;
                }
            }
            "emissive" => {
                if let Some(v) = runtime_as_f32(value) {
                    out.emissive = v;
                    any = true;
                }
            }
            "base_color" | "albedo" | "color" => {
                if let Some(color) = runtime_as_color4(value) {
                    out.base_color = color;
                    any = true;
                }
            }
            _ => {}
        }
    }
    any.then_some(out)
}

fn runtime_as_f32(value: &RuntimeValue) -> Option<f32> {
    match value {
        RuntimeValue::F32(v) => Some(*v),
        RuntimeValue::I32(v) => Some(*v as f32),
        _ => None,
    }
}

fn runtime_as_color4(value: &RuntimeValue) -> Option<[f32; 4]> {
    match value {
        RuntimeValue::Vec4 { x, y, z, w } => Some([*x, *y, *z, *w]),
        RuntimeValue::Vec3 { x, y, z } => Some([*x, *y, *z, 1.0]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::Runtime;
    use perro_core::{
        SceneNode, SceneNodeData, ambient_light_3d::AmbientLight3D, camera_3d::Camera3D,
        mesh_instance_3d::MeshInstance3D, ray_light_3d::RayLight3D,
    };
    use perro_ids::{MaterialID, MeshID};
    use perro_render_bridge::{Command3D, RenderCommand, RenderEvent, ResourceCommand};

    fn collect_commands(runtime: &mut Runtime) -> Vec<RenderCommand> {
        let mut out = Vec::new();
        runtime.drain_render_commands(&mut out);
        out
    }

    #[test]
    fn mesh_instance_without_mesh_source_requests_nothing() {
        let mut runtime = Runtime::new();
        let mut mesh = MeshInstance3D::new();
        mesh.mesh_id = MeshID::nil();
        mesh.material_id = MaterialID::nil();
        runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert!(first.is_empty());
    }

    #[test]
    fn mesh_instance_requests_missing_assets_once_until_events_arrive() {
        let mut runtime = Runtime::new();
        let mut mesh = MeshInstance3D::new();
        mesh.mesh_id = MeshID::nil();
        mesh.material_id = MaterialID::nil();
        let node_id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));
        runtime
            .render_3d
            .mesh_sources
            .insert(node_id, "__cube__".to_string());

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert_eq!(first.len(), 1);
        assert!(matches!(
            &first[0],
            RenderCommand::Resource(ResourceCommand::CreateMesh { owner, source, .. })
                if *owner == node_id && source == "__cube__"
        ));

        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        assert!(second.is_empty());
    }

    #[test]
    fn mesh_instance_emits_draw_after_mesh_and_material_created() {
        let mut runtime = Runtime::new();
        let node_id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        runtime
            .render_3d
            .mesh_sources
            .insert(node_id, "__cube__".to_string());

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        let mesh_request = match &first[0] {
            RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. }) => *request,
            _ => panic!("expected mesh create request"),
        };

        let mesh_id = MeshID::from_parts(9, 1);
        runtime.apply_render_event(RenderEvent::MeshCreated {
            request: mesh_request,
            id: mesh_id,
        });
        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        let material_request = match &second[0] {
            RenderCommand::Resource(ResourceCommand::CreateMaterial { request, .. }) => *request,
            _ => panic!("expected material create request"),
        };

        let material_id = MaterialID::from_parts(7, 4);
        runtime.apply_render_event(RenderEvent::MaterialCreated {
            request: material_request,
            id: material_id,
        });
        runtime.extract_render_3d_commands();
        let third = collect_commands(&mut runtime);
        assert_eq!(third.len(), 1);
        assert!(matches!(
            third[0],
            RenderCommand::ThreeD(Command3D::Draw {
                node,
                mesh,
                material,
                ..
            })
            if node == node_id && mesh == mesh_id && material == material_id
        ));
    }

    #[test]
    fn mesh_instance_can_request_mesh_and_material_in_separate_frames() {
        let mut runtime = Runtime::new();
        let inserted = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        runtime
            .render_3d
            .mesh_sources
            .insert(inserted, "__cube__".to_string());

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        let mesh_request = match first.first() {
            Some(RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. })) => *request,
            _ => panic!("expected mesh create request"),
        };

        runtime.apply_render_event(RenderEvent::MeshCreated {
            request: mesh_request,
            id: MeshID::from_parts(10, 0),
        });
        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        assert_eq!(second.len(), 1);
        assert!(matches!(
            second[0],
            RenderCommand::Resource(ResourceCommand::CreateMaterial { owner, .. }) if owner == inserted
        ));
    }

    #[test]
    fn active_camera_3d_emits_set_camera_command() {
        let mut runtime = Runtime::new();
        let mut camera = Camera3D::new();
        camera.active = true;
        camera.zoom = 1.75;
        camera.transform.position.x = 6.0;
        camera.transform.position.y = 7.0;
        camera.transform.position.z = 8.0;
        camera.transform.rotation.x = 0.1;
        camera.transform.rotation.y = 0.2;
        camera.transform.rotation.z = 0.3;
        camera.transform.rotation.w = 0.9;
        runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Camera3D(camera)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(Command3D::SetCamera { camera })
                if camera.position == [6.0, 7.0, 8.0]
                    && camera.rotation == [0.1, 0.2, 0.3, 0.9]
                    && camera.zoom == 1.75
        )));
    }

    #[test]
    fn active_ray_light_3d_emits_set_ray_light_command() {
        let mut runtime = Runtime::new();
        let mut light = RayLight3D::new();
        light.color = [0.8, 0.7, 0.6];
        light.intensity = 2.5;
        light.active = true;
        runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::RayLight3D(light)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(Command3D::SetRayLight { light, .. })
                if light.color == [0.8, 0.7, 0.6] && light.intensity == 2.5
        )));
    }

    #[test]
    fn active_ambient_light_3d_emits_set_ambient_light_command() {
        let mut runtime = Runtime::new();
        let mut light = AmbientLight3D::new();
        light.color = [0.25, 0.3, 0.4];
        light.intensity = 0.2;
        light.active = true;
        runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::AmbientLight3D(light)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(Command3D::SetAmbientLight { light, .. })
                if light.color == [0.25, 0.3, 0.4] && light.intensity == 0.2
        )));
    }
}
