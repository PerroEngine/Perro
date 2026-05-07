use super::Runtime;
use crate::material_schema;
use glam::{Mat4, Quat, Vec3};
use perro_ids::{MaterialID, MeshID, NodeID, parse_hashed_source_uri, string_to_u64};
use perro_nodes::{
    CameraProjection, MeshSurfaceBinding, SceneNodeData, Shape3D,
    particle_emitter_3d::{ParticleEmitterSimMode3D, ParticleType},
};
use perro_particle_math::compile_expression;
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, CameraProjectionState, Command3D, DenseInstancePose3D,
    Material3D, MaterialParamOverride3D, MeshSurfaceBinding3D, ParticlePath3D, ParticleProfile3D,
    ParticleRenderMode3D, ParticleSimulationMode3D, PointLight3DState, PointParticles3DState,
    RayLight3DState, RenderCommand, RenderRequestID, ResourceCommand, SkeletonPalette, Sky3DState,
    SkyTime3DState, SpotLight3DState,
};
use std::borrow::Cow;
use std::sync::Arc;

const PARTICLE_PATH_CACHE_MAX: usize = 256;

impl Runtime {
    fn mesh_request(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x3E)
    }

    fn material_request(node: NodeID, surface_index: u32) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 16) | ((surface_index as u64) << 8) | 0x3F)
    }
    pub fn extract_render_3d_commands(&mut self) {
        let bootstrap_scan = self.render_3d.prev_visible.is_empty()
            && self.render_3d.retained_ambient_lights.is_empty()
            && self.render_3d.retained_skies.is_empty()
            && self.render_3d.retained_ray_lights.is_empty()
            && self.render_3d.retained_point_lights.is_empty()
            && self.render_3d.retained_spot_lights.is_empty()
            && self.render_3d.retained_mesh_draws.is_empty()
            && self.render_3d.collision_debug_state.is_empty()
            && self.render_3d.last_camera.is_none();
        let has_extraction_work = self.dirty.has_any_dirty()
            || self.dirty.has_pending_transform_roots()
            || !self.render_3d.removed_nodes.is_empty()
            || self.render_3d.force_full_scan_once
            || bootstrap_scan;
        if !has_extraction_work {
            return;
        }

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();

        let mut traversal_ids = std::mem::take(&mut self.render_3d.traversal_ids);
        traversal_ids.clear();
        traversal_ids.extend(
            self.dirty
                .dirty_indices()
                .iter()
                .filter_map(|&raw_index| self.nodes.slot_get(raw_index as usize).map(|(id, _)| id)),
        );
        if self.render_3d.force_full_scan_once {
            traversal_ids.extend(self.nodes.iter().map(|(id, _)| id));
            self.render_3d.force_full_scan_once = false;
        }
        if traversal_ids.is_empty() && bootstrap_scan {
            traversal_ids.extend(self.nodes.iter().map(|(id, _)| id));
        }
        let mut traversal_seen = std::mem::take(&mut self.render_3d.traversal_seen);
        traversal_seen.clear();
        traversal_ids.retain(|id| traversal_seen.insert(*id));
        #[cfg(feature = "profile")]
        {
            self.render_3d.profile_last_seed_nodes =
                traversal_ids.len().min(u32::MAX as usize) as u32;
        }
        let mut traversal_cursor = 0usize;
        while traversal_cursor < traversal_ids.len() {
            let node = traversal_ids[traversal_cursor];
            traversal_cursor += 1;
            if let Some(node_ref) = self.nodes.get(node) {
                for &child in node_ref.get_children_ids() {
                    if traversal_seen.insert(child) {
                        traversal_ids.push(child);
                    }
                }
            }
        }
        #[cfg(feature = "profile")]
        {
            self.render_3d.profile_last_affected_nodes =
                traversal_ids.len().min(u32::MAX as usize) as u32;
        }
        let mut visible_now = std::mem::take(&mut self.render_3d.visible_now);
        visible_now.clear();
        visible_now.extend(self.render_3d.prev_visible.iter().copied());
        let mut removed_nodes = std::mem::take(&mut self.render_3d.removed_nodes);
        for node in removed_nodes.drain(..) {
            visible_now.remove(&node);
        }
        self.render_3d.removed_nodes = removed_nodes;
        let mut skeleton_cache = std::mem::take(&mut self.render_3d.skeleton_cache_scratch);
        skeleton_cache.clear();

        for node in traversal_ids.iter().copied() {
            visible_now.remove(&node);
            let effective_visible = self.is_effectively_visible(node);
            let camera_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Camera3D(camera) if camera.active && effective_visible => Some((
                    camera.transform,
                    match &camera.projection {
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
                    Arc::from(camera.post_processing.as_slice()),
                )),
                _ => None,
            });
            if let Some((local_transform, projection, post_processing)) = camera_data {
                let global = self
                    .get_global_transform_3d(node)
                    .unwrap_or(local_transform);
                let camera = Camera3DState {
                    position: [global.position.x, global.position.y, global.position.z],
                    rotation: [
                        global.rotation.x,
                        global.rotation.y,
                        global.rotation.z,
                        global.rotation.w,
                    ],
                    projection,
                    post_processing,
                };
                if self.render_3d.last_camera.as_ref() != Some(&camera) {
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(
                        Command3D::SetCamera {
                            camera: camera.clone(),
                        },
                    )));
                    self.render_3d.last_camera = Some(camera);
                }
            }

            let ambient_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::AmbientLight3D(light)
                    if light.active && light.visible && effective_visible =>
                {
                    Some(AmbientLight3DState {
                        color: light.color,
                        intensity: light.intensity.max(0.0),
                        cast_shadows: light.cast_shadows,
                    })
                }
                _ => None,
            });
            if let Some(light) = ambient_light_data {
                if self.render_3d.retained_ambient_lights.get(&node).copied() != Some(light) {
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(
                        Command3D::SetAmbientLight { node, light },
                    )));
                    self.render_3d.retained_ambient_lights.insert(node, light);
                }
                visible_now.insert(node);
            }

            let sky_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Sky3D(sky) if sky.active && sky.visible && effective_visible => {
                    Some(Sky3DState {
                        day_colors: Arc::from(sky.day_colors.as_ref()),
                        evening_colors: Arc::from(sky.evening_colors.as_ref()),
                        night_colors: Arc::from(sky.night_colors.as_ref()),
                        sky_angle: sky.sky_angle,
                        time: SkyTime3DState {
                            time_of_day: sky.time.time_of_day,
                            paused: sky.time.paused,
                            scale: sky.time.scale,
                        },
                        cloud_size: sky.clouds.size,
                        cloud_density: sky.clouds.density,
                        cloud_variance: sky.clouds.variance,
                        cloud_wind_vector: sky.clouds.wind_vector,
                        star_size: sky.stars.size,
                        star_scatter: sky.stars.scatter,
                        star_gleam: sky.stars.gleam,
                        moon_size: sky.moon.size,
                        sun_size: sky.sun.size,
                        style_blend: sky.style.blend_factor(),
                        sky_shader: sky.sky_shader.clone(),
                    })
                }
                _ => None,
            });
            if let Some(sky) = sky_data {
                if self.render_3d.retained_skies.get(&node) != Some(&sky) {
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(Command3D::SetSky {
                        node,
                        sky: Box::new(sky.clone()),
                    })));
                    self.render_3d.retained_skies.insert(node, sky);
                }
                visible_now.insert(node);
            }

            let ray_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::RayLight3D(light)
                    if light.active && light.visible && effective_visible =>
                {
                    Some((
                        light.transform,
                        light.color,
                        light.intensity,
                        light.cast_shadows,
                    ))
                }
                _ => None,
            });
            if let Some((local_transform, color, intensity, cast_shadows)) = ray_light_data {
                let global = self
                    .get_global_transform_3d(node)
                    .unwrap_or(local_transform);
                let light = RayLight3DState {
                    direction: quaternion_forward(global.rotation),
                    color,
                    intensity: intensity.max(0.0),
                    cast_shadows,
                };
                if self.render_3d.retained_ray_lights.get(&node).copied() != Some(light) {
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(
                        Command3D::SetRayLight { node, light },
                    )));
                    self.render_3d.retained_ray_lights.insert(node, light);
                }
                visible_now.insert(node);
            }

            let point_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::PointLight3D(light)
                    if light.active && light.visible && effective_visible =>
                {
                    Some((
                        light.transform,
                        light.color,
                        light.intensity,
                        light.range,
                        light.cast_shadows,
                    ))
                }
                _ => None,
            });
            if let Some((local_transform, color, intensity, range, cast_shadows)) = point_light_data
            {
                let global = self
                    .get_global_transform_3d(node)
                    .unwrap_or(local_transform);
                let light = PointLight3DState {
                    position: [global.position.x, global.position.y, global.position.z],
                    color,
                    intensity: intensity.max(0.0),
                    range: range.max(0.001),
                    cast_shadows,
                };
                if self.render_3d.retained_point_lights.get(&node).copied() != Some(light) {
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(
                        Command3D::SetPointLight { node, light },
                    )));
                    self.render_3d.retained_point_lights.insert(node, light);
                }
                visible_now.insert(node);
            }

            let spot_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::SpotLight3D(light)
                    if light.active && light.visible && effective_visible =>
                {
                    Some((
                        light.transform,
                        light.color,
                        light.intensity,
                        light.range,
                        light.inner_angle_radians,
                        light.outer_angle_radians,
                        light.cast_shadows,
                    ))
                }
                _ => None,
            });
            if let Some((
                local_transform,
                color,
                intensity,
                range,
                inner_angle_radians,
                outer_angle_radians,
                cast_shadows,
            )) = spot_light_data
            {
                let global = self
                    .get_global_transform_3d(node)
                    .unwrap_or(local_transform);
                let light = SpotLight3DState {
                    position: [global.position.x, global.position.y, global.position.z],
                    direction: quaternion_forward(global.rotation),
                    color,
                    intensity: intensity.max(0.0),
                    range: range.max(0.001),
                    inner_angle_radians: inner_angle_radians.max(0.0),
                    outer_angle_radians: outer_angle_radians.max(inner_angle_radians),
                    cast_shadows,
                };
                if self.render_3d.retained_spot_lights.get(&node).copied() != Some(light) {
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(
                        Command3D::SetSpotLight { node, light },
                    )));
                    self.render_3d.retained_spot_lights.insert(node, light);
                }
                visible_now.insert(node);
            }

            enum LocalMeshInstanceData {
                Single,
                Dense {
                    instance_scale: f32,
                    poses: Arc<[DenseInstancePose3D]>,
                },
            }
            type LocalMeshData = (
                MeshID,
                Vec<MeshSurfaceBinding>,
                Option<NodeID>,
                Option<bool>,
                LocalMeshInstanceData,
            );
            let mesh_data: Option<LocalMeshData> =
                self.nodes.get(node).and_then(|node| match &node.data {
                    SceneNodeData::MeshInstance3D(mesh) => Some((
                        mesh.mesh,
                        mesh.surfaces.clone(),
                        Some(mesh.skeleton),
                        mesh.meshlet_override,
                        LocalMeshInstanceData::Single,
                    )),
                    SceneNodeData::MultiMeshInstance3D(mesh) => Some((
                        mesh.mesh,
                        mesh.surfaces.clone(),
                        None,
                        mesh.meshlet_override,
                        LocalMeshInstanceData::Dense {
                            instance_scale: mesh.instance_scale.max(0.0001),
                            poses: Arc::from(
                                mesh.instances
                                    .iter()
                                    .map(|instance| DenseInstancePose3D {
                                        position: [instance.0.x, instance.0.y, instance.0.z],
                                        rotation: [
                                            instance.1.x,
                                            instance.1.y,
                                            instance.1.z,
                                            instance.1.w,
                                        ],
                                    })
                                    .collect::<Vec<_>>()
                                    .into_boxed_slice(),
                            ),
                        },
                    )),
                    _ => None,
                });
            if let Some((mesh, surfaces, skeleton, meshlet_override, local_instances)) = mesh_data
                && effective_visible
                && let Some((mesh, resolved_surfaces)) =
                    self.resolve_render_mesh_assets(node, mesh, surfaces)
            {
                let node_global = self
                    .get_global_transform_3d(node)
                    .unwrap_or(perro_structs::Transform3D::IDENTITY)
                    .to_mat4();
                let retained_instances = match &local_instances {
                    LocalMeshInstanceData::Single => {
                        crate::runtime::state::RetainedMeshInstanceState::Matrices(Arc::from([
                            (node_global * Mat4::IDENTITY).to_cols_array_2d(),
                        ]))
                    }
                    LocalMeshInstanceData::Dense {
                        poses,
                        instance_scale,
                    } => crate::runtime::state::RetainedMeshInstanceState::Dense {
                        node_model: node_global.to_cols_array_2d(),
                        instance_scale: *instance_scale,
                        poses: poses.clone(),
                    },
                };
                let empty = match &retained_instances {
                    crate::runtime::state::RetainedMeshInstanceState::Matrices(mats) => {
                        mats.is_empty()
                    }
                    crate::runtime::state::RetainedMeshInstanceState::Dense { poses, .. } => {
                        poses.is_empty()
                    }
                };
                if empty {
                    self.render_3d.retained_mesh_draws.remove(&node);
                    continue;
                }
                let skeleton_palette = if let Some(skeleton) = skeleton
                    && !skeleton.is_nil()
                {
                    if let Some(cached) = skeleton_cache.get(&skeleton) {
                        Some(cached.clone())
                    } else if let Some(palette) = build_skeleton_palette(&self.nodes, skeleton) {
                        let palette = SkeletonPalette {
                            matrices: Arc::from(palette.into_boxed_slice()),
                        };
                        skeleton_cache.insert(skeleton, palette.clone());
                        Some(palette)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let draw_state = crate::runtime::state::RetainedMeshDrawState {
                    mesh,
                    surfaces: resolved_surfaces.clone(),
                    instances: retained_instances.clone(),
                    skeleton: skeleton_palette.clone(),
                    meshlet_override,
                };
                if self.render_3d.retained_mesh_draws.get(&node) != Some(&draw_state) {
                    let draw_command = match retained_instances {
                        crate::runtime::state::RetainedMeshInstanceState::Dense {
                            node_model,
                            instance_scale,
                            poses,
                        } => Command3D::DrawMultiDense {
                            mesh,
                            surfaces: resolved_surfaces,
                            node,
                            node_model,
                            instance_scale,
                            instances: poses,
                            meshlet_override,
                        },
                        crate::runtime::state::RetainedMeshInstanceState::Matrices(
                            instance_mats,
                        ) if instance_mats.len() <= 1 => Command3D::Draw {
                            mesh,
                            surfaces: resolved_surfaces,
                            node,
                            model: instance_mats
                                .first()
                                .copied()
                                .unwrap_or(Mat4::IDENTITY.to_cols_array_2d()),
                            skeleton: skeleton_palette,
                            meshlet_override,
                        },
                        crate::runtime::state::RetainedMeshInstanceState::Matrices(
                            instance_mats,
                        ) => Command3D::DrawMulti {
                            mesh,
                            surfaces: resolved_surfaces,
                            node,
                            instance_mats,
                            skeleton: skeleton_palette,
                            meshlet_override,
                        },
                    };
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(draw_command)));
                    self.render_3d.retained_mesh_draws.insert(node, draw_state);
                }
                visible_now.insert(node);
            }
            let collision_shape_debug_data =
                self.nodes
                    .get(node)
                    .and_then(|scene_node| match &scene_node.data {
                        SceneNodeData::CollisionShape3D(shape)
                            if shape.debug && effective_visible =>
                        {
                            Some((shape.shape.clone(), shape.transform, scene_node.parent))
                        }
                        _ => None,
                    });
            if let Some((shape, local_transform, parent)) = collision_shape_debug_data {
                let (shape, world_from_shape) = if is_physics_body_3d(self, parent) {
                    let parent_global = self
                        .get_global_transform_3d(parent)
                        .unwrap_or(perro_structs::Transform3D::IDENTITY);
                    let parent_no_scale = transform_no_scale_mat4(parent_global);
                    let local_no_scale = transform_no_scale_mat4(local_transform);
                    let inherited_scale = perro_structs::Vector3::new(
                        local_transform.scale.x * parent_global.scale.x,
                        local_transform.scale.y * parent_global.scale.y,
                        local_transform.scale.z * parent_global.scale.z,
                    );
                    (
                        shape_scaled_by_local_scale(shape, inherited_scale),
                        parent_no_scale * local_no_scale,
                    )
                } else {
                    let world = self
                        .get_global_transform_3d(node)
                        .unwrap_or(local_transform)
                        .to_mat4();
                    (shape, world)
                };
                let signature = collision_debug_signature(&shape, world_from_shape);
                let prev = self.render_3d.collision_debug_state.get(&node).copied();
                let needs_rebuild = prev
                    .map(|state| state.signature != signature)
                    .unwrap_or(true);
                if needs_rebuild {
                    if let Some(prev) = prev {
                        Self::queue_remove_collision_debug_nodes(self, node, 0, prev.edge_count);
                    }
                    let edge_count = Self::queue_collision_shape_debug_draws(
                        self,
                        node,
                        shape,
                        world_from_shape,
                    );
                    self.render_3d.collision_debug_state.insert(
                        node,
                        crate::runtime::CollisionDebugState {
                            signature,
                            edge_count,
                        },
                    );
                }
                visible_now.insert(node);
            } else if let Some(prev) = self.render_3d.collision_debug_state.remove(&node) {
                Self::queue_remove_collision_debug_nodes(self, node, 0, prev.edge_count);
            }

            let point_emitter_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::ParticleEmitter3D(emitter) => Some((
                    emitter.profile.clone(),
                    emitter.sim_mode,
                    emitter.render_mode,
                    emitter.transform,
                    emitter.active,
                    emitter.looping,
                    emitter.prewarm,
                    emitter.spawn_rate,
                    emitter.seed,
                    emitter.params.clone(),
                    emitter.internal_simulation_time,
                )),
                _ => None,
            });
            if effective_visible
                && let Some((
                    emitter_profile,
                    emitter_sim_mode,
                    emitter_render_mode,
                    emitter_transform,
                    emitter_active,
                    emitter_looping,
                    emitter_prewarm,
                    emitter_spawn_rate,
                    emitter_seed,
                    emitter_params,
                    emitter_simulation_time,
                )) = point_emitter_data
            {
                let profile = resolve_particle_profile(self, &emitter_profile).unwrap_or_default();
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
                let sim_mode = resolve_particle_sim_mode(emitter_sim_mode, default_sim_mode);
                let render_mode = resolve_particle_render_mode(emitter_render_mode);
                let particle_model = self
                    .get_global_transform_3d(node)
                    .unwrap_or(emitter_transform)
                    .to_mat4()
                    .to_cols_array_2d();
                self.queue_render_command(RenderCommand::ThreeD(Box::new(
                    Command3D::UpsertPointParticles {
                        node,
                        particles: Box::new(PointParticles3DState {
                            model: particle_model,
                            active: emitter_active,
                            looping: emitter_looping,
                            prewarm: emitter_prewarm,
                            lifetime_min,
                            lifetime_max,
                            alive_budget: derived_particle_budget(
                                emitter_spawn_rate.max(0.0),
                                lifetime_max,
                            ),
                            emission_rate: emitter_spawn_rate.max(0.0),
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
                            seed: emitter_seed,
                            params: emitter_params,
                            simulation_time: emitter_simulation_time.max(0.0),
                            simulation_delta: self.time.delta.max(0.0),
                            profile,
                            sim_mode,
                            render_mode,
                        }),
                    },
                )));
                visible_now.insert(node);
            }
        }
        self.remove_no_longer_visible_render_3d_nodes(&visible_now);
        std::mem::swap(&mut self.render_3d.prev_visible, &mut visible_now);
        visible_now.clear();
        self.render_3d.visible_now = visible_now;

        traversal_ids.clear();
        self.render_3d.traversal_ids = traversal_ids;
        traversal_seen.clear();
        self.render_3d.traversal_seen = traversal_seen;
        skeleton_cache.clear();
        self.render_3d.skeleton_cache_scratch = skeleton_cache;
    }

    fn remove_no_longer_visible_render_3d_nodes(&mut self, visible_now: &ahash::AHashSet<NodeID>) {
        for node in self.render_3d.prev_visible.iter().copied() {
            if !visible_now.contains(&node) {
                self.render_3d.removed_nodes.push(node);
            }
        }
        while let Some(node) = self.render_3d.removed_nodes.pop() {
            if let Some(prev) = self.render_3d.collision_debug_state.remove(&node) {
                Self::queue_remove_collision_debug_nodes(self, node, 0, prev.edge_count);
            }
            self.render_3d.retained_ambient_lights.remove(&node);
            self.render_3d.retained_skies.remove(&node);
            self.render_3d.retained_ray_lights.remove(&node);
            self.render_3d.retained_point_lights.remove(&node);
            self.render_3d.retained_spot_lights.remove(&node);
            self.render_3d.retained_mesh_draws.remove(&node);
            self.queue_render_command(RenderCommand::ThreeD(Box::new(Command3D::RemoveNode {
                node,
            })));
        }
    }

    fn queue_collision_shape_debug_draws(
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
                },
            )));
            edge_count = edge_count.saturating_add(1);
        }
        edge_count
    }

    fn queue_remove_collision_debug_nodes(
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

    fn resolve_render_mesh_assets(
        &mut self,
        node: NodeID,
        mut mesh: MeshID,
        mut surfaces: Vec<MeshSurfaceBinding>,
    ) -> Option<(MeshID, std::sync::Arc<[MeshSurfaceBinding3D]>)> {
        let canonical = self.resource_api.canonical_mesh_id(mesh);
        if canonical != mesh {
            mesh = canonical;
            if let Some(node) = self.nodes.get_mut(node) {
                match &mut node.data {
                    SceneNodeData::MeshInstance3D(mesh_instance) => {
                        mesh_instance.mesh = mesh;
                    }
                    SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                        mesh_instance.mesh = mesh;
                    }
                    _ => {}
                }
            }
        }

        if !mesh.is_nil() && self.resource_api.is_mesh_id_pending(mesh) {
            // Runtime script/resource paths can assign a non-nil MeshID before the
            // render backend finishes CreateMesh; defer draw until ready.
            return None;
        }

        if mesh.is_nil() {
            let request = Self::mesh_request(node);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Mesh(id) => {
                        mesh = id;
                        if let Some(node) = self.nodes.get_mut(node) {
                            match &mut node.data {
                                SceneNodeData::MeshInstance3D(mesh_instance) => {
                                    mesh_instance.mesh = id;
                                }
                                SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                                    mesh_instance.mesh = id;
                                }
                                _ => {}
                            }
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

        for surface_index in 0..surfaces.len().max(1) {
            if surfaces.len() <= surface_index {
                surfaces.push(MeshSurfaceBinding::default());
            }
            let material = surfaces[surface_index]
                .material
                .unwrap_or(MaterialID::nil());
            if !material.is_nil() {
                continue;
            }

            let request = Self::material_request(node, surface_index as u32);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Material(id) => {
                        surfaces[surface_index].material = Some(id);
                        if let Some(node) = self.nodes.get_mut(node) {
                            match &mut node.data {
                                SceneNodeData::MeshInstance3D(mesh_instance) => {
                                    mesh_instance.set_surface_material(surface_index, Some(id));
                                }
                                SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                                    mesh_instance.ensure_surface_mut(surface_index).material =
                                        Some(id);
                                }
                                _ => {}
                            }
                        }
                        continue;
                    }
                    crate::RuntimeRenderResult::Failed(_)
                    | crate::RuntimeRenderResult::Texture(_)
                    | crate::RuntimeRenderResult::Mesh(_) => {}
                }
            }

            let source = self
                .render_3d
                .material_surface_sources
                .get(&node)
                .and_then(|sources| sources.get(surface_index))
                .cloned()
                .flatten();
            let material = self
                .render_3d
                .material_surface_overrides
                .get(&node)
                .and_then(|overrides| overrides.get(surface_index))
                .cloned()
                .flatten()
                .or_else(|| {
                    source
                        .as_ref()
                        .and_then(|src| load_material_from_source(self, src))
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

        let converted: Vec<MeshSurfaceBinding3D> = surfaces
            .into_iter()
            .map(|surface| MeshSurfaceBinding3D {
                material: surface.material,
                overrides: surface
                    .overrides
                    .into_iter()
                    .map(|ovr| MaterialParamOverride3D {
                        name: ovr.name,
                        value: ovr.value,
                    })
                    .collect::<Vec<_>>()
                    .into(),
                modulate: surface.modulate,
            })
            .collect();
        Some((mesh, std::sync::Arc::from(converted)))
    }
}
fn build_skeleton_palette(
    nodes: &crate::cns::NodeArena,
    skeleton_id: NodeID,
) -> Option<Vec<[[f32; 4]; 4]>> {
    let skeleton_node = nodes.get(skeleton_id)?;
    let skeleton = match &skeleton_node.data {
        SceneNodeData::Skeleton3D(skeleton) => skeleton,
        _ => return None,
    };
    if skeleton.bones.is_empty() {
        return None;
    }

    let mut global = vec![Mat4::IDENTITY; skeleton.bones.len()];
    for (i, bone) in skeleton.bones.iter().enumerate() {
        let local = bone.rest.to_mat4();
        if bone.parent >= 0 {
            let parent = bone.parent as usize;
            if parent < global.len() {
                global[i] = global[parent] * local;
            } else {
                global[i] = local;
            }
        } else {
            global[i] = local;
        }
    }

    let mut out = Vec::with_capacity(skeleton.bones.len());
    for (i, bone) in skeleton.bones.iter().enumerate() {
        let joint = global[i] * bone.inv_bind.to_mat4();
        out.push(joint.to_cols_array_2d());
    }
    Some(out)
}

fn collision_debug_edge_node(node: NodeID, index: u32) -> NodeID {
    // Synthetic retained debug ID namespace: top byte 0xD3 for collision edges.
    NodeID::from_u64((0xD3u64 << 56) ^ (node.as_u64() << 16) ^ index as u64)
}

fn is_physics_body_3d(runtime: &Runtime, node: NodeID) -> bool {
    runtime.nodes.get(node).is_some_and(|scene_node| {
        matches!(
            scene_node.data,
            SceneNodeData::StaticBody3D(_)
                | SceneNodeData::RigidBody3D(_)
                | SceneNodeData::Area3D(_)
        )
    })
}

fn transform_no_scale_mat4(transform: perro_structs::Transform3D) -> Mat4 {
    let rotation = Quat::from_xyzw(
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
    );
    Mat4::from_scale_rotation_translation(
        Vec3::ONE,
        rotation,
        Vec3::new(
            transform.position.x,
            transform.position.y,
            transform.position.z,
        ),
    )
}

fn shape_scaled_by_local_scale(shape: Shape3D, scale: perro_structs::Vector3) -> Shape3D {
    let sx = scale.x.abs().max(0.0001);
    let sy = scale.y.abs().max(0.0001);
    let sz = scale.z.abs().max(0.0001);
    match shape {
        Shape3D::Cube { size } => Shape3D::Cube {
            size: perro_structs::Vector3::new(size.x * sx, size.y * sy, size.z * sz),
        },
        Shape3D::Sphere { radius } => Shape3D::Sphere {
            radius: radius * sx.max(sy).max(sz),
        },
        Shape3D::Capsule {
            radius,
            half_height,
        } => Shape3D::Capsule {
            radius: radius * sx.max(sz),
            half_height: half_height * sy,
        },
        Shape3D::Cylinder {
            radius,
            half_height,
        } => Shape3D::Cylinder {
            radius: radius * sx.max(sz),
            half_height: half_height * sy,
        },
        Shape3D::Cone {
            radius,
            half_height,
        } => Shape3D::Cone {
            radius: radius * sx.max(sz),
            half_height: half_height * sy,
        },
        Shape3D::TriPrism { size } => Shape3D::TriPrism {
            size: perro_structs::Vector3::new(size.x * sx, size.y * sy, size.z * sz),
        },
        Shape3D::TriangularPyramid { size } => Shape3D::TriangularPyramid {
            size: perro_structs::Vector3::new(size.x * sx, size.y * sy, size.z * sz),
        },
        Shape3D::SquarePyramid { size } => Shape3D::SquarePyramid {
            size: perro_structs::Vector3::new(size.x * sx, size.y * sy, size.z * sz),
        },
        Shape3D::TriMesh { source } => Shape3D::TriMesh { source },
    }
}

fn collision_debug_signature(shape: &Shape3D, world_from_shape: Mat4) -> u64 {
    let mut h = 0xC011_1510_0D3B_9A77u64;
    hash_shape3d(&mut h, shape);
    for col in world_from_shape.to_cols_array_2d() {
        for value in col {
            h ^= value.to_bits() as u64;
            h = h.rotate_left(9).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    h
}

fn hash_shape3d(h: &mut u64, shape: &Shape3D) {
    match shape {
        Shape3D::Cube { size } => {
            *h ^= 1;
            mix_hash_f32(h, size.x);
            mix_hash_f32(h, size.y);
            mix_hash_f32(h, size.z);
        }
        Shape3D::Sphere { radius } => {
            *h ^= 2;
            mix_hash_f32(h, *radius);
        }
        Shape3D::Capsule {
            radius,
            half_height,
        } => {
            *h ^= 3;
            mix_hash_f32(h, *radius);
            mix_hash_f32(h, *half_height);
        }
        Shape3D::Cylinder {
            radius,
            half_height,
        } => {
            *h ^= 4;
            mix_hash_f32(h, *radius);
            mix_hash_f32(h, *half_height);
        }
        Shape3D::Cone {
            radius,
            half_height,
        } => {
            *h ^= 5;
            mix_hash_f32(h, *radius);
            mix_hash_f32(h, *half_height);
        }
        Shape3D::TriPrism { size } => {
            *h ^= 6;
            mix_hash_f32(h, size.x);
            mix_hash_f32(h, size.y);
            mix_hash_f32(h, size.z);
        }
        Shape3D::TriangularPyramid { size } => {
            *h ^= 7;
            mix_hash_f32(h, size.x);
            mix_hash_f32(h, size.y);
            mix_hash_f32(h, size.z);
        }
        Shape3D::SquarePyramid { size } => {
            *h ^= 8;
            mix_hash_f32(h, size.x);
            mix_hash_f32(h, size.y);
            mix_hash_f32(h, size.z);
        }
        Shape3D::TriMesh { source } => {
            *h ^= 9;
            for b in source.as_bytes() {
                *h ^= *b as u64;
                *h = h.rotate_left(11).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            }
        }
    }
}

#[inline]
fn mix_hash_f32(h: &mut u64, value: f32) {
    *h ^= value.to_bits() as u64;
    *h = h.rotate_left(11).wrapping_mul(0xBF58_476D_1CE4_E5B9);
}

fn collision_shape_wire_segments(shape: Shape3D) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    match shape {
        Shape3D::Cube { size } => {
            let hx = size.x.abs().max(0.0001) * 0.5;
            let hy = size.y.abs().max(0.0001) * 0.5;
            let hz = size.z.abs().max(0.0001) * 0.5;
            let points = [
                Vec3::new(-hx, -hy, -hz),
                Vec3::new(hx, -hy, -hz),
                Vec3::new(hx, hy, -hz),
                Vec3::new(-hx, hy, -hz),
                Vec3::new(-hx, -hy, hz),
                Vec3::new(hx, -hy, hz),
                Vec3::new(hx, hy, hz),
                Vec3::new(-hx, hy, hz),
            ];
            let edges = [
                (0usize, 1usize),
                (1, 2),
                (2, 3),
                (3, 0),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 4),
                (0, 4),
                (1, 5),
                (2, 6),
                (3, 7),
            ];
            push_indexed_edges(&mut out, &points, &edges);
        }
        Shape3D::Sphere { radius } => {
            let r = radius.abs().max(0.0001);
            append_circle_segments(
                &mut out,
                Vec3::ZERO,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, r, 0.0),
                20,
            );
            append_circle_segments(
                &mut out,
                Vec3::ZERO,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            append_circle_segments(
                &mut out,
                Vec3::ZERO,
                Vec3::new(0.0, r, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
        }
        Shape3D::Capsule {
            radius,
            half_height,
        } => {
            let r = radius.abs().max(0.0001);
            let h = half_height.abs().max(0.0001);
            let top = Vec3::new(0.0, h, 0.0);
            let bot = Vec3::new(0.0, -h, 0.0);
            append_circle_segments(
                &mut out,
                top,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            append_circle_segments(
                &mut out,
                bot,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            out.push((Vec3::new(r, -h, 0.0), Vec3::new(r, h, 0.0)));
            out.push((Vec3::new(-r, -h, 0.0), Vec3::new(-r, h, 0.0)));
            out.push((Vec3::new(0.0, -h, r), Vec3::new(0.0, h, r)));
            out.push((Vec3::new(0.0, -h, -r), Vec3::new(0.0, h, -r)));
            append_arc_segments(
                &mut out,
                top,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, r, 0.0),
                std::f32::consts::PI,
                16,
            );
            append_arc_segments(
                &mut out,
                top,
                Vec3::new(0.0, 0.0, r),
                Vec3::new(0.0, r, 0.0),
                std::f32::consts::PI,
                16,
            );
            append_arc_segments(
                &mut out,
                bot,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, -r, 0.0),
                std::f32::consts::PI,
                16,
            );
            append_arc_segments(
                &mut out,
                bot,
                Vec3::new(0.0, 0.0, r),
                Vec3::new(0.0, -r, 0.0),
                std::f32::consts::PI,
                16,
            );
        }
        Shape3D::Cylinder {
            radius,
            half_height,
        } => {
            let r = radius.abs().max(0.0001);
            let h = half_height.abs().max(0.0001);
            append_circle_segments(
                &mut out,
                Vec3::new(0.0, h, 0.0),
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            append_circle_segments(
                &mut out,
                Vec3::new(0.0, -h, 0.0),
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            out.push((Vec3::new(r, -h, 0.0), Vec3::new(r, h, 0.0)));
            out.push((Vec3::new(-r, -h, 0.0), Vec3::new(-r, h, 0.0)));
            out.push((Vec3::new(0.0, -h, r), Vec3::new(0.0, h, r)));
            out.push((Vec3::new(0.0, -h, -r), Vec3::new(0.0, h, -r)));
        }
        Shape3D::Cone {
            radius,
            half_height,
        } => {
            let r = radius.abs().max(0.0001);
            let h = half_height.abs().max(0.0001);
            append_circle_segments(
                &mut out,
                Vec3::new(0.0, -h, 0.0),
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            let apex = Vec3::new(0.0, h, 0.0);
            out.push((Vec3::new(r, -h, 0.0), apex));
            out.push((Vec3::new(-r, -h, 0.0), apex));
            out.push((Vec3::new(0.0, -h, r), apex));
            out.push((Vec3::new(0.0, -h, -r), apex));
        }
        Shape3D::TriPrism { size } => {
            let hw = size.x.abs().max(0.0001) * 0.5;
            let hh = size.y.abs().max(0.0001) * 0.5;
            let hd = size.z.abs().max(0.0001) * 0.5;
            let points = [
                Vec3::new(-hw, -hh, -hd),
                Vec3::new(hw, -hh, -hd),
                Vec3::new(0.0, hh, -hd),
                Vec3::new(-hw, -hh, hd),
                Vec3::new(hw, -hh, hd),
                Vec3::new(0.0, hh, hd),
            ];
            let edges = [
                (0usize, 1usize),
                (1, 2),
                (2, 0),
                (3, 4),
                (4, 5),
                (5, 3),
                (0, 3),
                (1, 4),
                (2, 5),
            ];
            push_indexed_edges(&mut out, &points, &edges);
        }
        Shape3D::TriangularPyramid { size } => {
            let hw = size.x.abs().max(0.0001) * 0.5;
            let hh = size.y.abs().max(0.0001) * 0.5;
            let hd = size.z.abs().max(0.0001) * 0.5;
            let points = [
                Vec3::new(-hw, -hh, -hd),
                Vec3::new(hw, -hh, -hd),
                Vec3::new(0.0, -hh, hd),
                Vec3::new(0.0, hh, 0.0),
            ];
            let edges = [(0usize, 1usize), (1, 2), (2, 0), (0, 3), (1, 3), (2, 3)];
            push_indexed_edges(&mut out, &points, &edges);
        }
        Shape3D::SquarePyramid { size } => {
            let hw = size.x.abs().max(0.0001) * 0.5;
            let hh = size.y.abs().max(0.0001) * 0.5;
            let hd = size.z.abs().max(0.0001) * 0.5;
            let points = [
                Vec3::new(-hw, -hh, -hd),
                Vec3::new(hw, -hh, -hd),
                Vec3::new(hw, -hh, hd),
                Vec3::new(-hw, -hh, hd),
                Vec3::new(0.0, hh, 0.0),
            ];
            let edges = [
                (0usize, 1usize),
                (1, 2),
                (2, 3),
                (3, 0),
                (0, 4),
                (1, 4),
                (2, 4),
                (3, 4),
            ];
            push_indexed_edges(&mut out, &points, &edges);
        }
        Shape3D::TriMesh { .. } => {}
    }
    out
}

fn push_indexed_edges(out: &mut Vec<(Vec3, Vec3)>, points: &[Vec3], edges: &[(usize, usize)]) {
    for (a, b) in edges.iter().copied() {
        if let (Some(pa), Some(pb)) = (points.get(a), points.get(b)) {
            out.push((*pa, *pb));
        }
    }
}

fn append_circle_segments(
    out: &mut Vec<(Vec3, Vec3)>,
    center: Vec3,
    axis_u: Vec3,
    axis_v: Vec3,
    segments: usize,
) {
    if segments < 3 {
        return;
    }
    let mut prev = center + axis_u;
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let a = std::f32::consts::TAU * t;
        let p = center + axis_u * a.cos() + axis_v * a.sin();
        out.push((prev, p));
        prev = p;
    }
}

fn append_arc_segments(
    out: &mut Vec<(Vec3, Vec3)>,
    center: Vec3,
    axis_u: Vec3,
    axis_v: Vec3,
    arc_radians: f32,
    segments: usize,
) {
    if segments == 0 {
        return;
    }
    let mut prev = center + axis_u;
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let a = arc_radians * t;
        let p = center + axis_u * a.cos() + axis_v * a.sin();
        out.push((prev, p));
        prev = p;
    }
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
    // Use glam's SIMD quaternion-vector rotate path.
    let q = Quat::from_xyzw(rotation.x, rotation.y, rotation.z, rotation.w);
    let q = if q.is_finite() && q.length_squared() > 1.0e-6 {
        q.normalize()
    } else {
        Quat::IDENTITY
    };
    let forward = q * Vec3::NEG_Z;
    [forward.x, forward.y, forward.z]
}

#[cfg(test)]
fn quaternion_forward_scalar_legacy(rotation: perro_structs::Quaternion) -> [f32; 3] {
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
    if let Some(hash) = parse_hashed_source_uri(source) {
        return runtime
            .project()
            .and_then(|project| project.static_material_lookup)
            .map(|lookup| lookup(hash).clone());
    }

    let normalized = normalize_source_slashes(source);
    let (path, _fragment) = split_source_fragment(source);
    let (normalized_path, _) = split_source_fragment(normalized.as_ref());
    if let Some(lookup) = runtime
        .project()
        .and_then(|project| project.static_material_lookup)
    {
        return Some(lookup(string_to_u64(source)).clone());
    }

    if path.ends_with(".pmat") || path.ends_with(".glb") || path.ends_with(".gltf") {
        return material_schema::load_from_source(source)
            .or_else(|| material_schema::load_from_source(path));
    }
    if normalized_path.ends_with(".pmat")
        || normalized_path.ends_with(".glb")
        || normalized_path.ends_with(".gltf")
    {
        return material_schema::load_from_source(normalized.as_ref())
            .or_else(|| material_schema::load_from_source(normalized_path));
    }

    None
}

fn normalize_source_slashes(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains('\\') {
        std::borrow::Cow::Owned(source.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(source)
    }
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

fn resolve_particle_profile(runtime: &mut Runtime, source: &str) -> Option<ParticleProfile3D> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    if let Some(path) = runtime.render_3d.particle_path_cache.get(source) {
        return Some(path.clone());
    }
    let parsed = if runtime.provider_mode() == crate::runtime_project::ProviderMode::Static {
        if let Some(inline) = source.strip_prefix("inline://") {
            parse_pparticle_source(inline)?
        } else if let Some(lookup) = runtime
            .project()
            .and_then(|project| project.static_particle_lookup)
        {
            let source_hash =
                parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
            lookup(source_hash).clone()
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
    if !runtime.render_3d.particle_path_cache.contains_key(source) {
        while runtime.render_3d.particle_path_cache.len() >= PARTICLE_PATH_CACHE_MAX {
            let Some(evict_key) = runtime.render_3d.particle_path_cache_order.pop_front() else {
                break;
            };
            runtime
                .render_3d
                .particle_path_cache
                .remove(evict_key.as_str());
        }
        runtime
            .render_3d
            .particle_path_cache_order
            .push_back(source.to_string());
    }
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
