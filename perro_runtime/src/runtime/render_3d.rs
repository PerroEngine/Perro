use super::Runtime;
use perro_core::SceneNodeData;
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_render_bridge::{
    Camera3DState, Command3D, RenderCommand, RenderRequestID, ResourceCommand,
};

impl Runtime {
    fn mesh_request_id(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x3E)
    }

    fn material_request_id(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x3F)
    }

    pub fn extract_render_3d_commands(&mut self) {
        let mut traversal_ids = std::mem::take(&mut self.render_3d.traversal_ids);
        traversal_ids.clear();
        traversal_ids.extend(self.nodes.iter().map(|(id, _)| id));

        for node_id in traversal_ids.iter().copied() {
            let camera_data = self.nodes.get(node_id).and_then(|node| match &node.data {
                SceneNodeData::Camera3D(camera) if camera.active => Some(Camera3DState {
                    position: [
                        camera.base.transform.position.x,
                        camera.base.transform.position.y,
                        camera.base.transform.position.z,
                    ],
                    rotation: [
                        camera.base.transform.rotation.x,
                        camera.base.transform.rotation.y,
                        camera.base.transform.rotation.z,
                        camera.base.transform.rotation.w,
                    ],
                    zoom: camera.zoom,
                }),
                _ => None,
            });
            if let Some(camera) = camera_data {
                self.queue_render_command(RenderCommand::ThreeD(Command3D::SetCamera { camera }));
            }

            let mesh_data = self.nodes.get(node_id).and_then(|node| match &node.data {
                SceneNodeData::MeshInstance3D(mesh) => Some((
                    mesh.base.visible,
                    mesh.mesh.as_deref().map(str::to_owned),
                    mesh.mesh_id,
                    mesh.material_id,
                    mesh.base.transform.to_mat4().to_cols_array_2d(),
                )),
                _ => None,
            });
            let Some((visible, mesh_source, mesh_id, material_id, model)) = mesh_data else {
                continue;
            };
            if !visible {
                continue;
            }

            let Some((mesh, material)) =
                self.resolve_mesh_instance_assets(node_id, mesh_source, mesh_id, material_id)
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
        mesh_source: Option<String>,
        mut mesh_id: MeshID,
        mut material_id: MaterialID,
    ) -> Option<(MeshID, MaterialID)> {
        if mesh_id.is_nil() {
            let source = mesh_source?.trim().to_string();
            if source.is_empty() {
                return None;
            }
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
                if !self.render.is_inflight(request) {
                    self.render.mark_inflight(request);
                    self.queue_render_command(RenderCommand::Resource(
                        ResourceCommand::CreateMaterial {
                            request,
                            owner: node_id,
                        },
                    ));
                }
                return None;
            }
        }

        Some((mesh_id, material_id))
    }
}

#[cfg(test)]
mod tests {
    use super::Runtime;
    use perro_core::{
        SceneNode, SceneNodeData, camera_3d::Camera3D, mesh_instance_3d::MeshInstance3D,
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
        mesh.mesh = None;
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
        mesh.mesh = Some("__cube__".into());
        mesh.mesh_id = MeshID::nil();
        mesh.material_id = MaterialID::nil();
        let node_id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

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
                MeshInstance3D {
                    mesh: Some("__cube__".into()),
                    ..MeshInstance3D::new()
                },
            )));

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
                MeshInstance3D {
                    mesh: Some("__cube__".into()),
                    ..MeshInstance3D::new()
                },
            )));

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
        camera.base.transform.position.x = 6.0;
        camera.base.transform.position.y = 7.0;
        camera.base.transform.position.z = 8.0;
        camera.base.transform.rotation.x = 0.1;
        camera.base.transform.rotation.y = 0.2;
        camera.base.transform.rotation.z = 0.3;
        camera.base.transform.rotation.w = 0.9;
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
}
