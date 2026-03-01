use super::Runtime;
use crate::material_schema;
use glam::{Mat4, Vec3};
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_nodes::{
    CameraProjection, SceneNodeData,
    particle_emitter_3d::{ParticleEmitterRenderMode3D, ParticleEmitterSimMode3D},
};
use perro_particle_math::compile_expression;
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, CameraProjectionState, Command3D, Material3D,
    ParticlePath3D, ParticleRenderMode3D, ParticleSimulationMode3D, PointLight3DState,
    ParticleProfile3D, PointParticles3DState,
    RayLight3DState, RenderCommand, RenderRequestID, ResourceCommand, SpotLight3DState,
};
use perro_terrain::{ChunkCoord, TerrainData};
use std::borrow::Cow;

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
            if let Some((mesh, material, model)) = mesh_data {
                if effective_visible {
                    if let Some((mesh, material)) =
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
                }
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
            {
                if effective_visible {
                    if !self.ensure_terrain_instance_data(node) {
                        continue;
                    }
                    self.queue_render_command(RenderCommand::ThreeD(Command3D::DrawTerrain {
                        node,
                        model: world_from_terrain.to_cols_array_2d(),
                    }));
                    let active_terrain_id = if terrain_id.is_nil() {
                        self.nodes.get(node).and_then(|scene_node| match &scene_node.data {
                            SceneNodeData::TerrainInstance3D(terrain) => Some(terrain.terrain),
                            _ => None,
                        })
                    } else {
                        Some(terrain_id)
                    };
                    if (show_debug_vertices || show_debug_edges)
                        && let Some(id) = active_terrain_id
                        && let Some(data) = self.terrain_store.get(id)
                    {
                        let debug_commands = Self::build_terrain_debug_draws(
                            node,
                            data,
                            world_from_terrain,
                            show_debug_vertices,
                            show_debug_edges,
                        );
                        for command in debug_commands {
                            self.queue_render_command(command);
                        }
                    }
                    visible_now.insert(node);
                }
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
            self.queue_render_command(RenderCommand::ThreeD(Command3D::RemoveNode { node }));
        }
    }

    fn build_terrain_debug_draws(
        node: NodeID,
        terrain: &TerrainData,
        world_from_terrain: Mat4,
        show_vertices: bool,
        show_edges: bool,
    ) -> Vec<RenderCommand> {
        let mut out = Vec::new();
        let point_size = 0.16;
        let edge_thickness = 0.035;
        for (coord, chunk) in terrain.chunks() {
            let vertices = chunk.vertices();
            if show_vertices {
                for vertex in vertices {
                    let world = world_from_terrain
                        .transform_point3(terrain_chunk_local_to_world(vertex.position, coord, terrain));
                    out.push(RenderCommand::ThreeD(Command3D::DrawDebugPoint3D {
                        node,
                        position: world.to_array(),
                        size: point_size,
                    }));
                }
            }
            if show_edges {
                let mut edges = ahash::AHashSet::<(usize, usize)>::new();
                for tri in chunk.triangles() {
                    let pairs = [(tri.a, tri.b), (tri.b, tri.c), (tri.c, tri.a)];
                    for (a, b) in pairs {
                        let key = if a <= b { (a, b) } else { (b, a) };
                        if !edges.insert(key) {
                            continue;
                        }
                        let start = world_from_terrain.transform_point3(
                            terrain_chunk_local_to_world(vertices[a].position, coord, terrain),
                        );
                        let end = world_from_terrain.transform_point3(
                            terrain_chunk_local_to_world(vertices[b].position, coord, terrain),
                        );
                        out.push(RenderCommand::ThreeD(Command3D::DrawDebugLine3D {
                            node,
                            start: start.to_array(),
                            end: end.to_array(),
                            thickness: edge_thickness,
                        }));
                    }
                }
            }
        }
        out
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

fn terrain_chunk_local_to_world(local: perro_structs::Vector3, coord: ChunkCoord, terrain: &TerrainData) -> Vec3 {
    let size = terrain.chunk_size_meters();
    let center_x = coord.x as f32 * size + size * 0.5;
    let center_z = coord.z as f32 * size + size * 0.5;
    Vec3::new(local.x + center_x, local.y, local.z + center_z)
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

fn resolve_particle_render_mode(mode: ParticleEmitterRenderMode3D) -> ParticleRenderMode3D {
    match mode {
        ParticleEmitterRenderMode3D::Point => ParticleRenderMode3D::Point,
        ParticleEmitterRenderMode3D::Billboard => ParticleRenderMode3D::Billboard,
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
                profile.spread_radians = value
                    .parse::<f32>()
                    .ok()
                    .unwrap_or(profile.spread_radians);
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
