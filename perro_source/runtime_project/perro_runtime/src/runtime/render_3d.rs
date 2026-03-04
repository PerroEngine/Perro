use super::Runtime;
use crate::material_schema;
use glam::{Mat4, Vec3};
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_nodes::{
    CameraProjection, SceneNodeData,
    particle_emitter_3d::{ParticleEmitterSimMode3D, ParticleType},
};
use perro_particle_math::compile_expression;
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, CameraProjectionState, Command3D, Material3D,
    ParticlePath3D, ParticleProfile3D, ParticleRenderMode3D, ParticleSimulationMode3D,
    PointLight3DState, PointParticles3DState, RayLight3DState, RenderCommand, RenderRequestID,
    ResourceCommand, RuntimeMeshData, RuntimeMeshVertex, SpotLight3DState,
};
use perro_terrain::{ChunkCoord, TerrainChunk};
use std::borrow::Cow;

impl Runtime {
    fn mesh_request(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x3E)
    }

    fn material_request(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x3F)
    }

    fn terrain_material_request() -> RenderRequestID {
        RenderRequestID::new(0x5445_5252_4D41_544Cu64)
    }

    fn terrain_chunk_request(node: NodeID, coord: ChunkCoord) -> RenderRequestID {
        let mut h = node.as_u64() ^ 0xA5A5_5A5A_D3C1_BEEF;
        h ^= (coord.x as u32 as u64).wrapping_mul(0x9E37_79B1);
        h = h.rotate_left(17);
        h ^= (coord.z as u32 as u64).wrapping_mul(0x85EB_CA77);
        RenderRequestID::new(h)
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
                        projection: match &camera.projection {
                            CameraProjection::Perspective {
                                fov_y_degrees,
                                near,
                                far,
                            } => CameraProjectionState::Perspective {
                                fov_y_degrees: *fov_y_degrees,
                                near: *near,
                                far: *far,
                            },
                            CameraProjection::Orthographic { size, near, far } => {
                                CameraProjectionState::Orthographic {
                                    size: *size,
                                    near: *near,
                                    far: *far,
                                }
                            }
                            CameraProjection::Frustum {
                                left,
                                right,
                                bottom,
                                top,
                                near,
                                far,
                            } => CameraProjectionState::Frustum {
                                left: *left,
                                right: *right,
                                bottom: *bottom,
                                top: *top,
                                near: *near,
                                far: *far,
                            },
                        },
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
            if let Some((mesh, material, model)) = mesh_data
                && effective_visible
                    && let Some((mesh, material)) =
                        self.resolve_render_mesh_assets(node, mesh, material)
                    {
                        self.queue_render_command(RenderCommand::ThreeD(Command3D::Draw {
                            mesh,
                            material,
                            node,
                            model,
                        }));
                        visible_now.insert(node);
                    }
            let terrain_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::TerrainInstance3D(terrain) => Some((
                    terrain.transform.to_mat4(),
                    terrain.show_debug_vertices,
                    terrain.show_debug_edges,
                    terrain.terrain,
                )),
                _ => None,
            });
            if let Some((world_from_terrain, show_debug_vertices, show_debug_edges, terrain_id)) =
                terrain_data
                && effective_visible {
                    if !self.ensure_terrain_instance_data(node) {
                        continue;
                    }
                    let active_terrain_id = if terrain_id.is_nil() {
                        self.nodes
                            .get(node)
                            .and_then(|scene_node| match &scene_node.data {
                                SceneNodeData::TerrainInstance3D(terrain) => Some(terrain.terrain),
                                _ => None,
                            })
                    } else {
                        Some(terrain_id)
                    };
                    let (chunk_size, chunk_snapshots) = {
                        let terrain_store = self
                            .terrain_store
                            .lock()
                            .expect("terrain store mutex poisoned");
                        if let Some(id) = active_terrain_id
                            && let Some(data) = terrain_store.get(id)
                        {
                            let chunk_size = data.chunk_size_meters();
                            let chunk_snapshots: Vec<(ChunkCoord, TerrainChunk)> = data
                                .chunks()
                                .map(|(coord, chunk)| (coord, chunk.clone()))
                                .collect();
                            let mut chunk_snapshots = chunk_snapshots;
                            chunk_snapshots.sort_unstable_by_key(|(coord, _)| (coord.x, coord.z));
                            (Some(chunk_size), chunk_snapshots)
                        } else {
                            (None, Vec::new())
                        }
                    };
                    if let Some(chunk_size) = chunk_size {
                        let terrain_signature = self.queue_terrain_chunk_draws(
                            node,
                            chunk_size,
                            &chunk_snapshots,
                            world_from_terrain,
                        );
                        if show_debug_vertices || show_debug_edges {
                            let debug_signature = terrain_debug_signature(
                                node,
                                active_terrain_id,
                                show_debug_vertices,
                                show_debug_edges,
                                world_from_terrain,
                                terrain_signature,
                            );
                            let prev = self.render_3d.terrain_debug_state.get(&node).copied();
                            let needs_rebuild = prev
                                .map(|state| state.signature != debug_signature)
                                .unwrap_or(true);
                            if needs_rebuild {
                                if let Some(prev) = prev {
                                    Self::queue_remove_terrain_debug_nodes(self, node, prev);
                                }
                                let (point_count, edge_count) = Self::queue_terrain_debug_draws(
                                    self,
                                    node,
                                    chunk_size,
                                    &chunk_snapshots,
                                    world_from_terrain,
                                    show_debug_vertices,
                                    show_debug_edges,
                                );
                                self.render_3d.terrain_debug_state.insert(
                                    node,
                                    crate::runtime::TerrainDebugState {
                                        signature: debug_signature,
                                        point_count,
                                        edge_count,
                                    },
                                );
                            }
                        } else if let Some(prev) = self.render_3d.terrain_debug_state.remove(&node) {
                            Self::queue_remove_terrain_debug_nodes(self, node, prev);
                        }
                    }
                    visible_now.insert(node);
                }

            let point_emitter_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::ParticleEmitter3D(emitter) => Some(emitter.clone()),
                _ => None,
            });
            if effective_visible && let Some(emitter) = point_emitter_data {
                let profile = resolve_particle_profile(self, &emitter.profile).unwrap_or_default();
                let lifetime_min = profile.lifetime_min.max(0.001);
                let lifetime_max = profile.lifetime_max.max(lifetime_min);
                if let Some(node_mut) = self.nodes.get_mut(node)
                    && let SceneNodeData::ParticleEmitter3D(emitter_mut) = &mut node_mut.data
                {
                    emitter_mut.internal_lifetime_max = lifetime_max;
                }
                let default_sim_mode = self
                    .project()
                    .map(|project| project.config.particle_sim_default)
                    .unwrap_or(perro_project::ParticleSimDefault::Cpu);
                let sim_mode = resolve_particle_sim_mode(emitter.sim_mode, default_sim_mode);
                let render_mode = resolve_particle_render_mode(emitter.render_mode);
                self.queue_render_command(RenderCommand::ThreeD(Command3D::UpsertPointParticles {
                    node,
                    particles: PointParticles3DState {
                        model: emitter.transform.to_mat4().to_cols_array_2d(),
                        active: emitter.active,
                        looping: emitter.looping,
                        prewarm: emitter.prewarm,
                        lifetime_min,
                        lifetime_max,
                        alive_budget: derived_particle_budget(
                            emitter.spawn_rate.max(0.0),
                            lifetime_max,
                        ),
                        emission_rate: emitter.spawn_rate.max(0.0),
                        speed_min: profile.speed_min.max(0.0),
                        speed_max: profile.speed_max.max(profile.speed_min.max(0.0)),
                        spread_radians: profile.spread_radians.clamp(0.0, std::f32::consts::PI),
                        size: profile.size.max(1.0),
                        size_min: profile.size_min.max(0.01),
                        size_max: profile.size_max.max(profile.size_min.max(0.01)),
                        gravity: profile.force,
                        color_start: profile.color_start,
                        color_end: profile.color_end,
                        emissive: profile.emissive,
                        seed: emitter.seed,
                        params: emitter.params.clone(),
                        simulation_time: emitter.internal_simulation_time.max(0.0),
                        simulation_delta: self.time.delta.max(0.0),
                        profile,
                        sim_mode,
                        render_mode,
                    },
                }));
                visible_now.insert(node);
            }
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
            if let Some(prev) = self.render_3d.terrain_debug_state.remove(&node) {
                Self::queue_remove_terrain_debug_nodes(self, node, prev);
            }
            self.queue_render_command(RenderCommand::ThreeD(Command3D::RemoveNode { node }));
        }
    }

    fn resolve_terrain_material(&mut self) -> Option<MaterialID> {
        if !self.render_3d.terrain_material.is_nil() {
            return Some(self.render_3d.terrain_material);
        }
        let request = Self::terrain_material_request();
        if let Some(result) = self.take_render_result(request) {
            match result {
                crate::RuntimeRenderResult::Material(id) => {
                    self.render_3d.terrain_material = id;
                    return Some(id);
                }
                crate::RuntimeRenderResult::Failed(_)
                | crate::RuntimeRenderResult::Texture(_)
                | crate::RuntimeRenderResult::Mesh(_) => {}
            }
        }
        if !self.render.is_inflight(request) {
            self.render.mark_inflight(request);
            self.queue_render_command(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                id: MaterialID::nil(),
                material: Material3D {
                    base_color_factor: [0.32, 0.56, 0.29, 1.0],
                    roughness_factor: 0.92,
                    metallic_factor: 0.0,
                    ..Material3D::default()
                },
                source: Some("__terrain_runtime_material__".to_string()),
                reserved: false,
            }));
        }
        None
    }

    fn queue_terrain_chunk_draws(
        &mut self,
        node: NodeID,
        chunk_size_meters: f32,
        chunks: &[(ChunkCoord, TerrainChunk)],
        world_from_terrain: Mat4,
    ) -> u64 {
        let material = self.resolve_terrain_material();
        let mut terrain_signature = 0xD6E8_FD91_4A2C_7C3Bu64;
        terrain_signature ^= chunk_size_meters.to_bits() as u64;
        terrain_signature = terrain_signature.rotate_left(13);

        for (coord, chunk) in chunks {
            let key = crate::runtime::TerrainChunkMeshKey {
                node,
                coord: *coord,
            };
            let hash = terrain_chunk_hash(chunk);
            terrain_signature ^= (coord.x as u32 as u64).wrapping_mul(0x9E37_79B1);
            terrain_signature = terrain_signature.rotate_left(11);
            terrain_signature ^= (coord.z as u32 as u64).wrapping_mul(0x85EB_CA77);
            terrain_signature = terrain_signature.rotate_left(11);
            terrain_signature ^= hash;
            terrain_signature = terrain_signature.rotate_left(11);
            let source = format!(
                "__terrain_runtime__/n{}_x{}_z{}_h{:016x}",
                node.as_u64(),
                coord.x,
                coord.z,
                hash
            );
            let request = Self::terrain_chunk_request(node, *coord);

            let mut prev_mesh_to_drop = MeshID::nil();
            let mut current_mesh = {
                let entry = self
                    .render_3d
                    .terrain_chunk_meshes
                    .entry(key)
                    .or_insert_with(|| crate::runtime::TerrainChunkMeshState {
                        source: source.clone(),
                        hash,
                        mesh: MeshID::nil(),
                    });

                if entry.hash != hash || entry.source != source {
                    prev_mesh_to_drop = entry.mesh;
                    entry.hash = hash;
                    entry.source = source.clone();
                    entry.mesh = MeshID::nil();
                }
                entry.mesh
            };

            if !prev_mesh_to_drop.is_nil() {
                self.queue_render_command(RenderCommand::Resource(ResourceCommand::DropMesh {
                    id: prev_mesh_to_drop,
                }));
            }

            if current_mesh.is_nil() {
                if let Some(result) = self.take_render_result(request)
                    && let crate::RuntimeRenderResult::Mesh(id) = result
                {
                    current_mesh = id;
                }
                if current_mesh.is_nil() && !self.render.is_inflight(request) {
                    self.render.mark_inflight(request);
                    if let Some(mesh) = terrain_chunk_to_runtime_mesh(chunk) {
                        self.queue_render_command(RenderCommand::Resource(
                            ResourceCommand::CreateRuntimeMesh {
                                request,
                                id: MeshID::nil(),
                                source: source.clone(),
                                reserved: false,
                                mesh,
                            },
                        ));
                    }
                }
                if current_mesh.is_nil() {
                    continue;
                }
                if let Some(entry) = self.render_3d.terrain_chunk_meshes.get_mut(&key) {
                    entry.mesh = current_mesh;
                }
            }

            if let Some(material) = material {
                let chunk_center_x = coord.x as f32 * chunk_size_meters;
                let chunk_center_z = coord.z as f32 * chunk_size_meters;
                let model = world_from_terrain
                    * Mat4::from_translation(Vec3::new(chunk_center_x, 0.0, chunk_center_z));
                self.queue_render_command(RenderCommand::ThreeD(Command3D::Draw {
                    mesh: current_mesh,
                    material,
                    node,
                    model: model.to_cols_array_2d(),
                }));
            }
        }
        terrain_signature
    }

    fn queue_terrain_debug_draws(
        &mut self,
        node: NodeID,
        chunk_size_meters: f32,
        chunks: &[(ChunkCoord, TerrainChunk)],
        world_from_terrain: Mat4,
        show_vertices: bool,
        show_edges: bool,
    ) -> (u32, u32) {
        let mut point_count = 0u32;
        let mut edge_count = 0u32;
        for (coord, chunk) in chunks.iter() {
            let vertices = chunk.vertices();
            let world_vertices: Vec<Vec3> = vertices
                .iter()
                .map(|vertex| {
                    world_from_terrain.transform_point3(terrain_chunk_local_to_world(
                        vertex.position,
                        *coord,
                        chunk_size_meters,
                    ))
                })
                .collect();

            let mut edge_pairs = Vec::<(usize, usize, f32)>::new();
            let mut unique_edges = ahash::AHashSet::<(usize, usize)>::new();
            let mut vertex_length_sum = vec![0.0f32; vertices.len()];
            let mut vertex_length_count = vec![0u32; vertices.len()];
            for tri in chunk.triangles() {
                let pairs = [(tri.a, tri.b), (tri.b, tri.c), (tri.c, tri.a)];
                for (a, b) in pairs {
                    let key = if a <= b { (a, b) } else { (b, a) };
                    if !unique_edges.insert(key) {
                        continue;
                    }
                    let len = (world_vertices[a] - world_vertices[b]).length();
                    edge_pairs.push((a, b, len));
                    vertex_length_sum[a] += len;
                    vertex_length_sum[b] += len;
                    vertex_length_count[a] = vertex_length_count[a].saturating_add(1);
                    vertex_length_count[b] = vertex_length_count[b].saturating_add(1);
                }
            }

            if show_vertices {
                for (i, world) in world_vertices.iter().enumerate() {
                    let avg_edge_len = if vertex_length_count[i] > 0 {
                        vertex_length_sum[i] / vertex_length_count[i] as f32
                    } else {
                        chunk_size_meters * 0.1
                    };
                    self.queue_render_command(RenderCommand::ThreeD(Command3D::DrawDebugPoint3D {
                        node: terrain_debug_point_node(node, point_count),
                        position: world.to_array(),
                        size: debug_vertex_size(avg_edge_len),
                    }));
                    point_count = point_count.saturating_add(1);
                }
            }

            if show_edges {
                for (a, b, len) in edge_pairs {
                    self.queue_render_command(RenderCommand::ThreeD(Command3D::DrawDebugLine3D {
                        node: terrain_debug_edge_node(node, edge_count),
                        start: world_vertices[a].to_array(),
                        end: world_vertices[b].to_array(),
                        thickness: debug_edge_thickness(len),
                    }));
                    edge_count = edge_count.saturating_add(1);
                }
            }
        }
        (point_count, edge_count)
    }

    fn queue_remove_terrain_debug_nodes(
        &mut self,
        node: NodeID,
        state: crate::runtime::TerrainDebugState,
    ) {
        for i in 0..state.point_count {
            self.queue_render_command(RenderCommand::ThreeD(Command3D::RemoveNode {
                node: terrain_debug_point_node(node, i),
            }));
        }
        for i in 0..state.edge_count {
            self.queue_render_command(RenderCommand::ThreeD(Command3D::RemoveNode {
                node: terrain_debug_edge_node(node, i),
            }));
        }
    }

    fn resolve_render_mesh_assets(
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
                let source = self
                    .render_3d
                    .mesh_sources
                    .get(&node)
                    .map(|source| source.trim().to_string())
                    .filter(|source| !source.is_empty())?;
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

fn terrain_chunk_local_to_world(
    local: perro_structs::Vector3,
    coord: ChunkCoord,
    chunk_size_meters: f32,
) -> Vec3 {
    let size = chunk_size_meters;
    // Debug overlays should align with terrain draw origin where chunk (0,0) is centered at (0,0).
    let center_x = coord.x as f32 * size;
    let center_z = coord.z as f32 * size;
    Vec3::new(local.x + center_x, local.y, local.z + center_z)
}

fn terrain_debug_signature(
    node: NodeID,
    terrain_id: Option<perro_ids::TerrainID>,
    show_vertices: bool,
    show_edges: bool,
    world_from_terrain: Mat4,
    terrain_signature: u64,
) -> u64 {
    let mut h = 0xA35F_1C2D_4B77_9E01u64;
    h ^= node.as_u64();
    h = h.rotate_left(7);
    h ^= terrain_id.map(|id| id.as_u64()).unwrap_or(0);
    h = h.rotate_left(7);
    h ^= if show_vertices { 1 } else { 0 };
    h = h.rotate_left(7);
    h ^= if show_edges { 2 } else { 0 };
    h = h.rotate_left(7);
    h ^= terrain_signature;
    for col in world_from_terrain.to_cols_array_2d() {
        for value in col {
            h ^= value.to_bits() as u64;
            h = h.rotate_left(9).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    h
}

fn terrain_chunk_to_runtime_mesh(chunk: &TerrainChunk) -> Option<RuntimeMeshData> {
    let vertices = chunk.vertices();
    let triangles = chunk.triangles();
    if vertices.is_empty() || triangles.is_empty() {
        return None;
    }

    let mut normals = vec![Vec3::ZERO; vertices.len()];
    let mut indices = Vec::with_capacity(triangles.len() * 3);
    for tri in triangles {
        if tri.a >= vertices.len() || tri.b >= vertices.len() || tri.c >= vertices.len() {
            return None;
        }
        let a = Vec3::new(
            vertices[tri.a].position.x,
            vertices[tri.a].position.y,
            vertices[tri.a].position.z,
        );
        let b = Vec3::new(
            vertices[tri.b].position.x,
            vertices[tri.b].position.y,
            vertices[tri.b].position.z,
        );
        let c = Vec3::new(
            vertices[tri.c].position.x,
            vertices[tri.c].position.y,
            vertices[tri.c].position.z,
        );
        let mut ib = tri.b as u32;
        let mut ic = tri.c as u32;
        let mut n = (b - a).cross(c - a);

        // Terrain is top-visible; enforce non-negative Y-facing winding.
        if n.y < 0.0 {
            std::mem::swap(&mut ib, &mut ic);
            n = -n;
        }

        if n.length_squared() > 1.0e-10 && n.is_finite() {
            normals[tri.a] += n;
            normals[tri.b] += n;
            normals[tri.c] += n;
        }
        indices.push(tri.a as u32);
        indices.push(ib);
        indices.push(ic);
    }

    let out_vertices: Vec<RuntimeMeshVertex> = vertices
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let n = if normals[i].length_squared() > 1.0e-10 {
                normals[i].normalize()
            } else {
                Vec3::Y
            };
            RuntimeMeshVertex {
                position: [v.position.x, v.position.y, v.position.z],
                normal: [n.x, n.y, n.z],
            }
        })
        .collect();

    Some(RuntimeMeshData {
        vertices: out_vertices,
        indices,
    })
}

fn terrain_chunk_hash(chunk: &TerrainChunk) -> u64 {
    let mut h = 0x9E37_79B9_7F4A_7C15u64;
    h ^= chunk.vertices().len() as u64;
    h = h.rotate_left(27).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= chunk.triangles().len() as u64;
    h = h.rotate_left(27).wrapping_mul(0x94D0_49BB_1331_11EB);
    for v in chunk.vertices() {
        h ^= v.position.x.to_bits() as u64;
        h = h.rotate_left(13).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        h ^= v.position.y.to_bits() as u64;
        h = h.rotate_left(13).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        h ^= v.position.z.to_bits() as u64;
        h = h.rotate_left(13).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    for tri in chunk.triangles() {
        h ^= tri.a as u64;
        h = h.rotate_left(11).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= tri.b as u64;
        h = h.rotate_left(11).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= tri.c as u64;
        h = h.rotate_left(11).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    }
    h
}

fn debug_edge_thickness(edge_len: f32) -> f32 {
    (0.02 + edge_len * 0.0035).clamp(0.03, 0.22)
}

fn debug_vertex_size(avg_edge_len: f32) -> f32 {
    (0.08 + avg_edge_len * 0.009).clamp(0.12, 0.75)
}

fn terrain_debug_point_node(node: NodeID, index: u32) -> NodeID {
    // Synthetic retained debug ID namespace: top byte 0xD1 for points.
    NodeID::from_u64((0xD1u64 << 56) ^ (node.as_u64() << 16) ^ index as u64)
}

fn terrain_debug_edge_node(node: NodeID, index: u32) -> NodeID {
    // Synthetic retained debug ID namespace: top byte 0xD2 for edges.
    NodeID::from_u64((0xD2u64 << 56) ^ (node.as_u64() << 16) ^ index as u64)
}

fn derived_particle_budget(spawn_rate: f32, lifetime_max: f32) -> u32 {
    if spawn_rate <= 0.0 || lifetime_max <= 0.0 {
        return 1;
    }
    let budget = (spawn_rate * lifetime_max).ceil() as u32 + 2;
    budget.clamp(1, 1_000_000)
}

fn resolve_particle_sim_mode(
    override_mode: ParticleEmitterSimMode3D,
    default_mode: perro_project::ParticleSimDefault,
) -> ParticleSimulationMode3D {
    match override_mode {
        ParticleEmitterSimMode3D::Default => match default_mode {
            perro_project::ParticleSimDefault::Cpu => ParticleSimulationMode3D::Cpu,
            perro_project::ParticleSimDefault::GpuVertex => ParticleSimulationMode3D::GpuVertex,
            perro_project::ParticleSimDefault::GpuCompute => ParticleSimulationMode3D::GpuCompute,
        },
        ParticleEmitterSimMode3D::Cpu => ParticleSimulationMode3D::Cpu,
        ParticleEmitterSimMode3D::GpuVertex => ParticleSimulationMode3D::GpuVertex,
        ParticleEmitterSimMode3D::GpuCompute => ParticleSimulationMode3D::GpuCompute,
    }
}

fn resolve_particle_render_mode(mode: ParticleType) -> ParticleRenderMode3D {
    match mode {
        ParticleType::Point => ParticleRenderMode3D::Point,
        ParticleType::Billboard => ParticleRenderMode3D::Billboard,
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

fn resolve_particle_profile(runtime: &mut Runtime, source: &str) -> Option<ParticleProfile3D> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    if let Some(path) = runtime.render_3d.particle_path_cache.get(source) {
        return Some(path.clone());
    }
    let parsed = if runtime.provider_mode() == crate::runtime_project::ProviderMode::Static {
        if let Some(lookup) = runtime
            .project()
            .and_then(|project| project.static_particle_lookup)
            && let Some(profile) = lookup(source)
        {
            profile.clone()
        } else if let Some(inline) = source.strip_prefix("inline://") {
            parse_pparticle_source(inline)?
        } else {
            let bytes = perro_io::load_asset(source).ok()?;
            let text = std::str::from_utf8(&bytes).ok()?;
            parse_pparticle_source(text)?
        }
    } else if let Some(inline) = source.strip_prefix("inline://") {
        parse_pparticle_source(inline)?
    } else {
        let bytes = perro_io::load_asset(source).ok()?;
        let text = std::str::from_utf8(&bytes).ok()?;
        parse_pparticle_source(text)?
    };
    runtime
        .render_3d
        .particle_path_cache
        .insert(source.to_string(), parsed.clone());
    Some(parsed)
}

fn parse_pparticle_source(source: &str) -> Option<ParticleProfile3D> {
    let mut profile = ParticleProfile3D::default();
    let mut preset: Option<String> = None;
    let mut preset_param_a = 1.0f32;
    let mut preset_param_b = 1.0f32;
    let mut preset_param_c = 0.0f32;
    let mut preset_param_d = 0.0f32;
    let mut expr_x = String::from("0.0");
    let mut expr_y = String::from("0.0");
    let mut expr_z = String::from("0.0");
    let mut has_expr_x = false;
    let mut has_expr_y = false;
    let mut has_expr_z = false;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let (key, value) = line.split_once('=')?;
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "preset" => {
                preset = Some(value.to_ascii_lowercase());
            }
            "preset_param_a" => {
                preset_param_a = value.parse::<f32>().ok().unwrap_or(preset_param_a);
            }
            "preset_param_b" => {
                preset_param_b = value.parse::<f32>().ok().unwrap_or(preset_param_b);
            }
            "preset_param_c" => {
                preset_param_c = value.parse::<f32>().ok().unwrap_or(preset_param_c);
            }
            "preset_param_d" => {
                preset_param_d = value.parse::<f32>().ok().unwrap_or(preset_param_d);
            }
            "x" => expr_x = value.to_string(),
            "y" => expr_y = value.to_string(),
            "z" => expr_z = value.to_string(),
            "force" => {
                if let Some(v) = parse_vec3_literal(value) {
                    profile.force = v;
                }
            }
            "force_x" => {
                let v = value.parse::<f32>().ok()?;
                profile.force[0] = v;
            }
            "force_y" => {
                let v = value.parse::<f32>().ok()?;
                profile.force[1] = v;
            }
            "force_z" => {
                let v = value.parse::<f32>().ok()?;
                profile.force[2] = v;
            }
            "lifetime_min" => {
                profile.lifetime_min = value.parse::<f32>().ok().unwrap_or(profile.lifetime_min);
            }
            "lifetime_max" => {
                profile.lifetime_max = value.parse::<f32>().ok().unwrap_or(profile.lifetime_max);
            }
            "speed_min" => {
                profile.speed_min = value.parse::<f32>().ok().unwrap_or(profile.speed_min);
            }
            "speed_max" => {
                profile.speed_max = value.parse::<f32>().ok().unwrap_or(profile.speed_max);
            }
            "spread_radians" => {
                profile.spread_radians =
                    value.parse::<f32>().ok().unwrap_or(profile.spread_radians);
            }
            "size" => {
                profile.size = value.parse::<f32>().ok().unwrap_or(profile.size);
            }
            "size_min" => {
                profile.size_min = value.parse::<f32>().ok().unwrap_or(profile.size_min);
            }
            "size_max" => {
                profile.size_max = value.parse::<f32>().ok().unwrap_or(profile.size_max);
            }
            "color_start" => {
                if let Some(v) = parse_vec4_literal(value) {
                    profile.color_start = v;
                }
            }
            "color_end" => {
                if let Some(v) = parse_vec4_literal(value) {
                    profile.color_end = v;
                }
            }
            "emissive" => {
                if let Some(v) = parse_vec3_literal(value) {
                    profile.emissive = v;
                }
            }
            "spin" => {
                profile.spin_angular_velocity = value
                    .parse::<f32>()
                    .ok()
                    .unwrap_or(profile.spin_angular_velocity);
            }
            _ => {}
        }
        match key.as_str() {
            "x" => has_expr_x = true,
            "y" => has_expr_y = true,
            "z" => has_expr_z = true,
            _ => {}
        }
    }
    profile.path = match preset.as_deref() {
        None => ParticlePath3D::None,
        Some("ballistic") => ParticlePath3D::Ballistic,
        Some("spiral") => ParticlePath3D::Spiral {
            angular_velocity: preset_param_a,
            radius: preset_param_b.abs(),
        },
        Some("orbit_y") => ParticlePath3D::OrbitY {
            angular_velocity: preset_param_a,
            radius: preset_param_b.abs(),
        },
        Some("noise_drift") => ParticlePath3D::NoiseDrift {
            amplitude: preset_param_a.abs(),
            frequency: preset_param_b.abs(),
        },
        Some("flat_disk") => ParticlePath3D::FlatDisk {
            radius: preset_param_a.abs(),
        },
        Some(_) => ParticlePath3D::None,
    };
    let _ = (preset_param_c, preset_param_d);
    if has_expr_x || has_expr_y || has_expr_z {
        profile.expr_x_ops = Some(Cow::Owned(compile_expression(&expr_x).ok()?.ops().to_vec()));
        profile.expr_y_ops = Some(Cow::Owned(compile_expression(&expr_y).ok()?.ops().to_vec()));
        profile.expr_z_ops = Some(Cow::Owned(compile_expression(&expr_z).ok()?.ops().to_vec()));
    }
    Some(profile)
}

fn parse_vec3_literal(raw: &str) -> Option<[f32; 3]> {
    let raw = raw.trim();
    let inner = raw.strip_prefix('(')?.strip_suffix(')')?;
    let mut it = inner.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([it.next()??, it.next()??, it.next()??])
}

fn parse_vec4_literal(raw: &str) -> Option<[f32; 4]> {
    let raw = raw.trim();
    let inner = raw.strip_prefix('(')?.strip_suffix(')')?;
    let mut it = inner.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([it.next()??, it.next()??, it.next()??, it.next()??])
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_render_3d_tests.rs"]
mod tests;
