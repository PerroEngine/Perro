use super::*;

impl Runtime {
    pub(in super::super) fn remove_retained_render_3d_node(&mut self, node: NodeID) {
        self.render_3d.camera_activation_order.remove(&node);
        self.render_3d.dense_instance_pose_cache.remove(&node);
        self.render_3d.mesh_sources.remove(&node);
        self.render_3d.material_surface_sources.remove(&node);
        self.render_3d.material_surface_overrides.remove(&node);
        if let Some(prev) = self.render_3d.collision_debug_state.remove(&node) {
            Self::queue_remove_collision_debug_nodes(self, node, 0, prev.edge_count);
        }
        self.render_3d.retained_ambient_lights.remove(&node);
        self.render_3d.retained_skies.remove(&node);
        self.render_3d.retained_ray_lights.remove(&node);
        self.render_3d.retained_point_lights.remove(&node);
        self.render_3d.retained_spot_lights.remove(&node);
        self.render_3d.retained_decals.remove(&node);
        self.render_3d.retained_mesh_draws.remove(&node);
        self.queue_render_command(RenderCommand::ThreeD(Box::new(Command3D::RemoveNode {
            node,
        })));
        self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode { node }));
    }

    pub(in super::super) fn world_overlay_point_occluded_3d(
        &mut self,
        node: NodeID,
        point: Vector3,
        camera: &Camera3DState,
        candidates: &[NodeID],
    ) -> bool {
        let camera_position = Vec3::from_array(camera.position);
        let target = Vec3::new(point.x, point.y, point.z);
        let camera_rotation = Quat::from_xyzw(
            camera.rotation[0],
            camera.rotation[1],
            camera.rotation[2],
            camera.rotation[3],
        );
        let camera_rotation =
            if camera_rotation.is_finite() && camera_rotation.length_squared() > 1.0e-6 {
                camera_rotation.normalize()
            } else {
                Quat::IDENTITY
            };
        let (origin, dir, max_distance) = match camera.projection {
            CameraProjectionState::Orthographic { .. } => {
                let dir = camera_rotation * -Vec3::Z;
                let max_distance = (target - camera_position).dot(dir);
                (target - dir * max_distance, dir, max_distance)
            }
            _ => {
                let ray = target - camera_position;
                let max_distance = ray.length();
                (camera_position, ray / max_distance, max_distance)
            }
        };
        if !max_distance.is_finite() || max_distance <= 0.001 {
            return false;
        }
        let origin = Vector3::new(origin.x, origin.y, origin.z);
        let dir = Vector3::new(dir.x, dir.y, dir.z);
        let hit_limit = (max_distance - 0.03).max(0.0);
        if hit_limit <= 0.0 {
            return false;
        }

        for &candidate in candidates {
            if candidate == node {
                continue;
            }
            let Some((visible, layers)) = self.nodes.get(candidate).and_then(|scene_node| {
                let visible =
                    self.is_effectively_visible(candidate) && !self.is_under_sub_view(candidate);
                match &scene_node.data {
                    SceneNodeData::MeshInstance3D(mesh) => Some((visible, mesh.render_layers)),
                    SceneNodeData::MultiMeshInstance3D(mesh) => Some((visible, mesh.render_layers)),
                    _ => None,
                }
            }) else {
                continue;
            };
            if !visible || !render_mask_matches(camera.render_mask, layers) {
                continue;
            }
            if self
                .query_mesh_instance_surface_on_global_ray(candidate, origin, dir, hit_limit)
                .is_some()
            {
                return true;
            }
        }
        false
    }

    pub(in super::super) fn active_render_camera_3d(&mut self) -> Option<Camera3DState> {
        let mut found: Option<Camera3DPick> = None;
        for (node, scene_node) in self.nodes.iter() {
            let SceneNodeData::Camera3D(camera) = &scene_node.data else {
                continue;
            };
            if !camera.active || !self.is_effectively_visible(node) || self.is_under_sub_view(node)
            {
                continue;
            }
            let order = self
                .render_3d
                .camera_activation_order
                .get(&node)
                .copied()
                .unwrap_or(0);
            let priority = (order, node.generation(), node.index());
            let replace = found
                .as_ref()
                .map(|(current, ..)| priority > *current)
                .unwrap_or(true);
            if replace {
                found = Some((
                    priority,
                    node,
                    camera.transform,
                    camera.projection.clone(),
                    camera.render_mask,
                    camera.post_processing.clone(),
                    camera.audio_options.clone(),
                ));
            }
        }
        let (
            _priority,
            node,
            local_transform,
            projection,
            render_mask,
            post_processing,
            audio_options,
        ) = found?;
        let global = self
            .get_render_global_transform_3d(node)
            .unwrap_or(local_transform);
        Some(Camera3DState {
            position: [global.position.x, global.position.y, global.position.z],
            rotation: [
                global.rotation.x,
                global.rotation.y,
                global.rotation.z,
                global.rotation.w,
            ],
            projection: camera_projection_state(&projection),
            render_mask,
            post_processing: Arc::from(post_processing.to_effects_vec()),
            audio_options,
        })
    }

    pub(crate) fn note_camera_3d_activated(&mut self, node: NodeID) {
        let order = self.render_3d.next_camera_activation_order;
        self.render_3d.next_camera_activation_order = order.wrapping_add(1).max(1);
        self.render_3d.camera_activation_order.insert(node, order);
        self.request_full_3d_scan_once();
    }

    pub(in super::super) fn queue_collision_shape_debug_draws(
        &mut self,
        node: NodeID,
        shape: Shape3D,
        world_from_shape: Mat4,
    ) -> u32 {
        let segments = collision_shape_wire_segments(shape);
        let mut edge_count = 0u32;
        for (start, end) in segments {
            let world_start = world_from_shape.transform_point3(start).to_array();
            let world_end = world_from_shape.transform_point3(end).to_array();
            self.queue_render_command(RenderCommand::ThreeD(Box::new(
                Command3D::DrawDebugLine3D {
                    node: collision_debug_edge_node(node, edge_count),
                    start: world_start,
                    end: world_end,
                    thickness: 0.035,
                    color: [0.15, 0.95, 0.95, 1.0],
                },
            )));
            edge_count = edge_count.saturating_add(1);
        }
        edge_count
    }

    pub(in super::super) fn queue_remove_collision_debug_nodes(
        &mut self,
        node: NodeID,
        start_index: u32,
        end_exclusive: u32,
    ) {
        for i in start_index..end_exclusive {
            self.queue_render_command(RenderCommand::ThreeD(Box::new(Command3D::RemoveNode {
                node: collision_debug_edge_node(node, i),
            })));
        }
    }
}
