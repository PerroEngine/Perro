use super::Runtime;
use crate::material_schema;
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_nodes::SceneNodeData;
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, Command3D, Material3D, PointLight3DState, RayLight3DState,
    RenderCommand, RenderRequestID, ResourceCommand, SpotLight3DState,
};

impl Runtime {
    fn mesh_request(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x3E)
    }

    fn material_request(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x3F)
    }

    pub fn extract_render_3d_commands(&mut self) {
        self.propagate_pending_transform_dirty();

        let mut traversal_ids = std::mem::take(&mut self.render_3d.traversal_ids);
        traversal_ids.clear();
        traversal_ids.extend(self.nodes.iter().map(|(id, _)| id));
        let mut visible_now = std::mem::take(&mut self.render_3d.visible_now);
        visible_now.clear();
        self.render_3d.removed_nodes.clear();

        for node in traversal_ids.iter().copied() {
            let effective_visible = self.is_effectively_visible(node);
            let camera_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Camera3D(camera) if camera.active && effective_visible => {
                    Some(Camera3DState {
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
                    })
                }
                _ => None,
            });
            if let Some(camera) = camera_data {
                self.queue_render_command(RenderCommand::ThreeD(Command3D::SetCamera { camera }));
            }

            let ambient_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::AmbientLight3D(light)
                    if light.active && light.visible && effective_visible =>
                {
                    Some(AmbientLight3DState {
                        color: light.color,
                        intensity: light.intensity.max(0.0),
                    })
                }
                _ => None,
            });
            if let Some(light) = ambient_light_data {
                self.queue_render_command(RenderCommand::ThreeD(Command3D::SetAmbientLight {
                    node,
                    light,
                }));
                visible_now.insert(node);
            }

            let ray_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::RayLight3D(light)
                    if light.active && light.visible && effective_visible =>
                {
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
                    node,
                    light,
                }));
                visible_now.insert(node);
            }

            let point_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::PointLight3D(light)
                    if light.active && light.visible && effective_visible =>
                {
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
                    node,
                    light,
                }));
                visible_now.insert(node);
            }

            let spot_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::SpotLight3D(light)
                    if light.active && light.visible && effective_visible =>
                {
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
                    node,
                    light,
                }));
                visible_now.insert(node);
            }

            let mesh_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::MeshInstance3D(mesh) => Some((
                    mesh.mesh,
                    mesh.material,
                    mesh.transform.to_mat4().to_cols_array_2d(),
                )),
                _ => None,
            });
            let Some((mesh, material, model)) = mesh_data else {
                continue;
            };
            if !effective_visible {
                continue;
            }

            let Some((mesh, material)) = self.resolve_mesh_instance_assets(node, mesh, material)
            else {
                continue;
            };
            self.queue_render_command(RenderCommand::ThreeD(Command3D::Draw {
                mesh,
                material,
                node,
                model,
            }));
            visible_now.insert(node);
        }
        self.remove_no_longer_visible_render_3d_nodes(&visible_now);
        std::mem::swap(&mut self.render_3d.prev_visible, &mut visible_now);
        visible_now.clear();
        self.render_3d.visible_now = visible_now;

        traversal_ids.clear();
        self.render_3d.traversal_ids = traversal_ids;
    }

    fn remove_no_longer_visible_render_3d_nodes(&mut self, visible_now: &ahash::AHashSet<NodeID>) {
        for node in self.render_3d.prev_visible.iter().copied() {
            if !visible_now.contains(&node) {
                self.render_3d.removed_nodes.push(node);
            }
        }
        while let Some(node) = self.render_3d.removed_nodes.pop() {
            self.queue_render_command(RenderCommand::ThreeD(Command3D::RemoveNode { node }));
        }
    }

    fn resolve_mesh_instance_assets(
        &mut self,
        node: NodeID,
        mut mesh: MeshID,
        mut material: MaterialID,
    ) -> Option<(MeshID, MaterialID)> {
        if mesh.is_nil() {
            let request = Self::mesh_request(node);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Mesh(id) => {
                        mesh = id;
                        if let Some(node) = self.nodes.get_mut(node)
                            && let SceneNodeData::MeshInstance3D(mesh_instance) = &mut node.data
                        {
                            mesh_instance.mesh = id;
                        }
                    }
                    crate::RuntimeRenderResult::Failed(_)
                    | crate::RuntimeRenderResult::Texture(_)
                    | crate::RuntimeRenderResult::Material(_) => {}
                }
            }
            if mesh.is_nil() {
                let source = self.render_3d.mesh_sources.get(&node)?.trim().to_string();
                if source.is_empty() {
                    return None;
                }
                if !self.render.is_inflight(request) {
                    self.render.mark_inflight(request);
                    self.queue_render_command(RenderCommand::Resource(
                        ResourceCommand::CreateMesh {
                            request,
                            id: MeshID::nil(),
                            source,
                            reserved: false,
                        },
                    ));
                }
                return None;
            }
        }

        if material.is_nil() {
            let request = Self::material_request(node);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Material(id) => {
                        material = id;
                        if let Some(node) = self.nodes.get_mut(node)
                            && let SceneNodeData::MeshInstance3D(mesh_instance) = &mut node.data
                        {
                            mesh_instance.material = id;
                        }
                    }
                    crate::RuntimeRenderResult::Failed(_)
                    | crate::RuntimeRenderResult::Texture(_)
                    | crate::RuntimeRenderResult::Mesh(_) => {}
                }
            }
            if material.is_nil() {
                let source = self.render_3d.material_sources.get(&node).cloned();
                let material = self
                    .render_3d
                    .material_overrides
                    .get(&node)
                    .copied()
                    .or_else(|| {
                        self.render_3d
                            .material_sources
                            .get(&node)
                            .and_then(|source| load_material_from_source(self, source))
                    })
                    .unwrap_or_else(Material3D::default);
                if !self.render.is_inflight(request) {
                    self.render.mark_inflight(request);
                    self.queue_render_command(RenderCommand::Resource(
                        ResourceCommand::CreateMaterial {
                            request,
                            id: MaterialID::nil(),
                            material,
                            source,
                            reserved: false,
                        },
                    ));
                }
                return None;
            }
        }

        Some((mesh, material))
    }
}

fn quaternion_forward(rotation: perro_structs::Quaternion) -> [f32; 3] {
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
    if let Some(lookup) = runtime
        .project()
        .and_then(|project| project.static_material_lookup)
    {
        if let Some(material) = lookup(source).copied() {
            return Some(material);
        }
        if let Some(material) = lookup(path).copied() {
            return Some(material);
        }
    }

    if path.ends_with(".pmat") {
        return material_schema::load_from_source(path);
    }

    if path.ends_with(".glb") || path.ends_with(".gltf") {
        let _index = parse_fragment_index(fragment, &["mat", "material"]).unwrap_or(0);
        return Some(Material3D::default());
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
    if selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return (path, Some(selector));
    }
    (source, None)
}

fn parse_fragment_index(fragment: Option<&str>, keys: &[&str]) -> Option<u32> {
    let fragment = fragment?;
    if let Some((name, rest)) = fragment.split_once('[') {
        let name = name.trim();
        if keys.contains(&name) {
            let value = rest.strip_suffix(']')?.trim();
            if let Ok(parsed) = value.parse::<u32>() {
                return Some(parsed);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::Runtime;
    use perro_ids::{MaterialID, MeshID};
    use perro_nodes::{
        SceneNode, SceneNodeData, ambient_light_3d::AmbientLight3D, camera_3d::Camera3D,
        mesh_instance_3d::MeshInstance3D, node_3d::Node3D, ray_light_3d::RayLight3D,
    };
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
        mesh.mesh = MeshID::nil();
        mesh.material = MaterialID::nil();
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
        mesh.mesh = MeshID::nil();
        mesh.material = MaterialID::nil();
        let expected_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));
        runtime
            .render_3d
            .mesh_sources
            .insert(expected_node, "__cube__".to_string());

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert_eq!(first.len(), 1);
        assert!(matches!(
            &first[0],
            RenderCommand::Resource(ResourceCommand::CreateMesh { source, .. })
                if source == "__cube__"
        ));

        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        assert!(second.is_empty());
    }

    #[test]
    fn mesh_instance_emits_draw_after_mesh_and_material_created() {
        let mut runtime = Runtime::new();
        let expected_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        runtime
            .render_3d
            .mesh_sources
            .insert(expected_node, "__cube__".to_string());

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        let mesh_request = match &first[0] {
            RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. }) => *request,
            _ => panic!("expected mesh create request"),
        };

        let mesh = MeshID::from_parts(9, 1);
        runtime.apply_render_event(RenderEvent::MeshCreated {
            request: mesh_request,
            id: mesh,
        });
        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        let material_request = match &second[0] {
            RenderCommand::Resource(ResourceCommand::CreateMaterial { request, .. }) => *request,
            _ => panic!("expected material create request"),
        };

        let material = MaterialID::from_parts(7, 4);
        runtime.apply_render_event(RenderEvent::MaterialCreated {
            request: material_request,
            id: material,
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
            if node == expected_node && mesh == mesh && material == material
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
            RenderCommand::Resource(ResourceCommand::CreateMaterial { .. })
        ));
    }

    #[test]
    fn mesh_under_invisible_parent_emits_remove_node() {
        let mut runtime = Runtime::new();
        let parent = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
        let child = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        if let Some(parent_node) = runtime.nodes.get_mut(parent) {
            parent_node.add_child(child);
        }
        if let Some(child_node) = runtime.nodes.get_mut(child) {
            child_node.parent = parent;
        }

        let mesh = MeshID::from_parts(20, 0);
        let material = MaterialID::from_parts(21, 0);
        if let Some(node) = runtime.nodes.get_mut(child)
            && let SceneNodeData::MeshInstance3D(mesh_instance) = &mut node.data
        {
            mesh_instance.mesh = mesh;
            mesh_instance.material = material;
        }

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert!(first.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(Command3D::Draw { node, .. }) if *node == child
        )));

        if let Some(node) = runtime.nodes.get_mut(parent)
            && let SceneNodeData::Node3D(parent_node) = &mut node.data
        {
            parent_node.visible = false;
        }
        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        assert!(second.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(Command3D::RemoveNode { node }) if *node == child
        )));
    }

    #[test]
    fn unchanged_mesh_instance_emits_draw() {
        let mut runtime = Runtime::new();
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        let mesh = MeshID::from_parts(30, 0);
        let material = MaterialID::from_parts(31, 0);
        if let Some(scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::MeshInstance3D(mesh_instance) = &mut scene_node.data
        {
            mesh_instance.mesh = mesh;
            mesh_instance.material = material;
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(Command3D::Draw { node: draw_node, .. })
                if *draw_node == node
        )));
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
