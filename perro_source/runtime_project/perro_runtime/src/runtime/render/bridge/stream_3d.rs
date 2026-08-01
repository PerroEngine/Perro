use super::*;

/// Per-refresh 3D collection scratch: every lane the fused member walk fills,
/// taken from / restored to the retained render state so hot rebuild loops
/// reuse capacity instead of allocating fresh containers.
pub(super) struct Stream3DCollect {
    pub(super) draws: Vec<CameraStreamDraw3DState>,
    pub(super) ray_lights: Vec<(NodeID, RayLight3DState)>,
    pub(super) point_lights: Vec<(NodeID, PointLight3DState)>,
    pub(super) spot_lights: Vec<(NodeID, SpotLight3DState)>,
    /// min-NodeID wins ambient/sky (was sorted-first-wins).
    pub(super) best_ambient: Option<(NodeID, AmbientLight3DState)>,
    pub(super) best_sky: Option<(NodeID, Sky3DState)>,
    pub(super) particles: Vec<(NodeID, PointParticles3DState)>,
    pub(super) waters: Vec<(NodeID, Water3DState)>,
    pub(super) skeleton_global: Vec<glam::Mat4>,
    pub(super) skeleton_palette: Vec<[[f32; 4]; 3]>,
    /// Recycled surface buffer: a node's surface list is copied in here for
    /// material resolve instead of cloning a fresh Vec per mesh.
    pub(super) surfaces: Vec<perro_nodes::MeshSurfaceBinding>,
}

/// Deterministic lowest-id order for the capped light arrays (matches old
/// sorted-scratch fill; slot cap keeps lowest-id lights), then drain the
/// collected lights into the fixed lighting slots.
pub(super) fn finish_stream_lighting_3d(out: &mut Stream3DCollect) -> CameraStreamLighting3DState {
    let mut lighting = CameraStreamLighting3DState::default();
    out.ray_lights.sort_unstable_by_key(|(id, _)| id.as_u64());
    out.point_lights.sort_unstable_by_key(|(id, _)| id.as_u64());
    out.spot_lights.sort_unstable_by_key(|(id, _)| id.as_u64());
    lighting.ambient_light = out.best_ambient.take().map(|(_, light)| light);
    lighting.sky = out.best_sky.take().map(|(_, sky)| sky);
    for (slot, (_, light)) in lighting.ray_lights.iter_mut().zip(out.ray_lights.drain(..)) {
        *slot = Some(light);
    }
    for (slot, (_, light)) in lighting
        .point_lights
        .iter_mut()
        .zip(out.point_lights.drain(..))
    {
        *slot = Some(light);
    }
    for (slot, (_, light)) in lighting
        .spot_lights
        .iter_mut()
        .zip(out.spot_lights.drain(..))
    {
        *slot = Some(light);
    }
    lighting
}

/// Phase-1 payload 4 members that need `&mut self` work after the node borrow
/// ends (transform localization, water sub-collectors, profile resolve).
enum Stream3DItem {
    Ambient(AmbientLight3DState),
    Sky(Sky3DState),
    Ray {
        transform: perro_structs::Transform3D,
        color: Color,
        intensity: f32,
        cast_shadows: bool,
        shadow_strength: f32,
        shadow_depth_bias: f32,
        shadow_normal_bias: f32,
    },
    Point {
        transform: perro_structs::Transform3D,
        color: Color,
        intensity: f32,
        range: f32,
        cast_shadows: bool,
        shadow_strength: f32,
        shadow_depth_bias: f32,
        shadow_normal_bias: f32,
    },
    Spot {
        transform: perro_structs::Transform3D,
        color: Color,
        intensity: f32,
        range: f32,
        inner_angle_radians: f32,
        outer_angle_radians: f32,
        cast_shadows: bool,
        shadow_strength: f32,
        shadow_depth_bias: f32,
        shadow_normal_bias: f32,
    },
    Particles {
        profile_source: perro_ids::ParticleProfileRef,
        sim_mode: perro_nodes::ParticleEmitterSimMode3D,
        render_mode: perro_nodes::ParticleType,
        transform: perro_structs::Transform3D,
        active: bool,
        looping: bool,
        prewarm: bool,
        spawn_rate: f32,
        seed: u32,
        params: Vec<f32>,
        simulation_time: f32,
    },
    Water {
        transform: perro_structs::Transform3D,
        water: perro_nodes::WaterSurfaceParams,
    },
}

impl Runtime {
    pub(super) fn take_stream_3d_collect(&mut self) -> Stream3DCollect {
        // take-pattern scratch: rebuilt every stream refresh, keep capacity.
        let mut out = Stream3DCollect {
            draws: std::mem::take(&mut self.render_3d.stream_draws_scratch),
            ray_lights: std::mem::take(&mut self.render_3d.stream_ray_lights_scratch),
            point_lights: std::mem::take(&mut self.render_3d.stream_point_lights_scratch),
            spot_lights: std::mem::take(&mut self.render_3d.stream_spot_lights_scratch),
            best_ambient: None,
            best_sky: None,
            particles: std::mem::take(&mut self.render_3d.stream_particles_scratch),
            waters: std::mem::take(&mut self.render_3d.stream_waters_scratch),
            skeleton_global: std::mem::take(&mut self.render_3d.skeleton_global_scratch),
            skeleton_palette: std::mem::take(&mut self.render_3d.skeleton_palette_scratch),
            surfaces: std::mem::take(&mut self.mesh_surface_scratch),
        };
        out.draws.clear();
        out.ray_lights.clear();
        out.point_lights.clear();
        out.spot_lights.clear();
        out.particles.clear();
        out.waters.clear();
        out.surfaces.clear();
        out
    }

    pub(super) fn restore_stream_3d_collect(&mut self, mut out: Stream3DCollect) {
        out.draws.clear();
        out.ray_lights.clear();
        out.point_lights.clear();
        out.spot_lights.clear();
        out.particles.clear();
        out.waters.clear();
        out.surfaces.clear();
        self.render_3d.stream_draws_scratch = out.draws;
        self.render_3d.stream_ray_lights_scratch = out.ray_lights;
        self.render_3d.stream_point_lights_scratch = out.point_lights;
        self.render_3d.stream_spot_lights_scratch = out.spot_lights;
        self.render_3d.stream_particles_scratch = out.particles;
        self.render_3d.stream_waters_scratch = out.waters;
        self.render_3d.skeleton_global_scratch = out.skeleton_global;
        self.render_3d.skeleton_palette_scratch = out.skeleton_palette;
        self.mesh_surface_scratch = out.surfaces;
    }

    /// One member, every 3D lane. The driver already applied the shared
    /// guards (self-skip, effective visibility, isolated-child skip); a node
    /// is exactly one type, so this replaces 4 full member walks with one
    /// type dispatch per node.
    pub(super) fn stream_member_3d(
        &mut self,
        node: NodeID,
        camera_mask: BitMask,
        localize: &StreamLocalize3D,
        out: &mut Stream3DCollect,
    ) {
        // nested sub-view quad.
        if let Some((transform, size, tint)) =
            self.nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::SubView3D(view) if view.visible && view.sub_view.enabled => {
                        Some((view.transform, view.size, view.tint))
                    }
                    _ => None,
                })
        {
            let model = self
                .stream_localized_transform_3d(node, localize)
                .unwrap_or(transform)
                .to_mat4()
                .to_cols_array_2d();
            out.draws.push(CameraStreamDraw3DState::CameraStreamQuad {
                texture: Self::camera_stream_texture_id(node),
                tint: Runtime::color_modulate(tint, self.effective_self_modulate(node)),
                node,
                model,
                size: [size.x.max(0.001), size.y.max(0.001)],
            });
            return;
        }
        // mesh / multimesh draws.
        if let Some((
            mesh,
            skeleton,
            meshlet_override,
            lod,
            blend,
            cast_shadows,
            receive_shadows,
            instance_kind,
        )) = self
            .nodes
            .get(node)
            .and_then(|node_ref| match &node_ref.data {
                SceneNodeData::MeshInstance3D(mesh)
                    if mesh.visible
                        && stream_render_mask_matches(camera_mask, mesh.render_layers) =>
                {
                    out.surfaces.clear();
                    out.surfaces.extend(mesh.surfaces.iter().cloned());
                    Some((
                        mesh.mesh,
                        Some(mesh.skeleton),
                        mesh.meshlet_override,
                        LODOptions3D {
                            min_lod: mesh.lod.min_lod,
                            max_lod: mesh.lod.max_lod,
                        },
                        MeshBlendOptions3D {
                            enabled: mesh.blend.enabled,
                            screen_blending: mesh.blend.screen_blending,
                            normal_blending: mesh.blend.normal_blending,
                            blend_layers: mesh.blend.blend_layers,
                            blend_mask: mesh.blend.blend_mask,
                            distance: mesh.blend.distance,
                            min_distance: mesh.blend.min_distance,
                            noise_factor: mesh.blend.noise_factor,
                            noise_scale: mesh.blend.noise_scale,
                            slope_factor: mesh.blend.slope_factor,
                            strength: mesh.blend.strength,
                            salt_instances: mesh.blend.salt_instances,
                        },
                        mesh.cast_shadows,
                        mesh.receive_shadows,
                        StreamMeshInstanceKind::Single,
                    ))
                }
                SceneNodeData::MultiMeshInstance3D(mesh)
                    if mesh.visible
                        && stream_render_mask_matches(camera_mask, mesh.render_layers) =>
                {
                    out.surfaces.clear();
                    out.surfaces.extend(mesh.surfaces.iter().cloned());
                    Some((
                        mesh.mesh,
                        None,
                        mesh.meshlet_override,
                        LODOptions3D {
                            min_lod: mesh.lod.min_lod,
                            max_lod: mesh.lod.max_lod,
                        },
                        MeshBlendOptions3D {
                            enabled: mesh.blend.enabled,
                            screen_blending: mesh.blend.screen_blending,
                            normal_blending: mesh.blend.normal_blending,
                            blend_layers: mesh.blend.blend_layers,
                            blend_mask: mesh.blend.blend_mask,
                            distance: mesh.blend.distance,
                            min_distance: mesh.blend.min_distance,
                            noise_factor: mesh.blend.noise_factor,
                            noise_scale: mesh.blend.noise_scale,
                            slope_factor: mesh.blend.slope_factor,
                            strength: mesh.blend.strength,
                            salt_instances: mesh.blend.salt_instances,
                        },
                        mesh.cast_shadows,
                        mesh.receive_shadows,
                        StreamMeshInstanceKind::Dense {
                            instance_scale: mesh.instance_scale.max(0.0001),
                            // Shares the main extract path's per-node
                            // signature cache: unchanged instance sets hand
                            // back the retained Arc (refcount bump) instead of
                            // re-collecting dense poses + the per-instance
                            // blend-weight Arcs every stream refresh. Poses
                            // are stream-independent (raw local instance
                            // data), so one entry serves the main pass and
                            // every stream.
                            poses: {
                                let signature = dense_instance_signature(&mesh.instances);
                                if let Some(cached) =
                                    self.render_3d.dense_instance_pose_cache.get(&node)
                                    && cached.signature == signature
                                {
                                    cached.poses.clone()
                                } else {
                                    // slice iter is TrustedLen: collects into
                                    // the Arc directly (no Vec round trip).
                                    let poses: Arc<[DenseInstancePose3D]> = mesh
                                        .instances
                                        .iter()
                                        .map(|instance| DenseInstancePose3D {
                                            position: [
                                                instance.transform.position.x,
                                                instance.transform.position.y,
                                                instance.transform.position.z,
                                            ],
                                            scale: [
                                                instance.transform.scale.x,
                                                instance.transform.scale.y,
                                                instance.transform.scale.z,
                                            ],
                                            rotation: [
                                                instance.transform.rotation.x,
                                                instance.transform.rotation.y,
                                                instance.transform.rotation.z,
                                                instance.transform.rotation.w,
                                            ],
                                            has_blend_shape_weight_override: instance
                                                .blend_shape_weights
                                                .is_some(),
                                            blend_shape_weights: instance
                                                .blend_shape_weights
                                                .clone()
                                                .map(Arc::<[f32]>::from)
                                                .unwrap_or_else(empty_arc_slice),
                                        })
                                        .collect();
                                    self.render_3d.dense_instance_pose_cache.insert(
                                        node,
                                        crate::runtime::state::DenseInstancePoseCache {
                                            signature,
                                            poses: poses.clone(),
                                        },
                                    );
                                    poses
                                }
                            },
                        },
                    ))
                }
                _ => None,
            })
        {
            let mut surfaces_scratch = std::mem::take(&mut out.surfaces);
            let resolved =
                self.resolve_render_mesh_assets_scratch(node, mesh, &mut surfaces_scratch);
            out.surfaces = surfaces_scratch;
            let Some((mesh, surfaces)) = resolved else {
                return;
            };
            let model = self
                .stream_localized_transform_3d(node, localize)
                .unwrap_or(perro_structs::Transform3D::IDENTITY)
                .to_mat4()
                .to_cols_array_2d();
            let mut skeleton_global = std::mem::take(&mut out.skeleton_global);
            let mut skeleton_palette_scratch = std::mem::take(&mut out.skeleton_palette);
            let skeleton_palette = skeleton.and_then(|skeleton| {
                (!skeleton.is_nil()).then(|| {
                    self.stream_skeleton_palette(
                        skeleton,
                        &mut skeleton_global,
                        &mut skeleton_palette_scratch,
                    )
                })?
            });
            out.skeleton_global = skeleton_global;
            out.skeleton_palette = skeleton_palette_scratch;
            match instance_kind {
                StreamMeshInstanceKind::Single => out.draws.push(CameraStreamDraw3DState::Draw {
                    mesh,
                    surfaces,
                    node,
                    model,
                    skeleton: skeleton_palette,
                    meshlet_override,
                    lod,
                    blend,
                    cast_shadows,
                    receive_shadows,
                }),
                StreamMeshInstanceKind::Dense {
                    instance_scale,
                    poses,
                } => out.draws.push(CameraStreamDraw3DState::DrawMultiDense {
                    mesh,
                    surfaces,
                    node,
                    node_model: model,
                    instance_scale,
                    instances: poses,
                    meshlet_override,
                    lod,
                    blend,
                    cast_shadows,
                    receive_shadows,
                }),
            }
            return;
        }
        // lights / sky / particles / waters.
        let data = self
            .nodes
            .get(node)
            .and_then(|node_ref| match &node_ref.data {
                SceneNodeData::AmbientLight3D(light)
                    if light.visible
                        && light.active
                        && stream_render_mask_matches(camera_mask, light.render_layers) =>
                {
                    Some(Stream3DItem::Ambient(AmbientLight3DState {
                        color: light.color.to_rgb(),
                        intensity: light.intensity.max(0.0),
                        cast_shadows: light.cast_shadows,
                    }))
                }
                SceneNodeData::Sky3D(sky)
                    if sky.visible
                        && sky.active
                        && stream_render_mask_matches(camera_mask, sky.render_layers) =>
                {
                    // The main extraction retains an identical state per sky
                    // node; when it still matches, share its Arcs instead of
                    // rebuilding palettes + shader passes.
                    let state = self
                        .render_3d
                        .retained_skies
                        .get(&node)
                        .filter(|retained| sky_3d_state_matches(retained, sky))
                        .map(|retained| (**retained).clone())
                        .unwrap_or_else(|| Sky3DState {
                            day_colors: Arc::from(sky.palette.day_colors.as_ref()),
                            evening_colors: Arc::from(sky.palette.evening_colors.as_ref()),
                            night_colors: Arc::from(sky.palette.night_colors.as_ref()),
                            horizon_colors: Arc::from(sky.palette.horizon_colors.as_ref()),
                            time: SkyTime3DState {
                                time_of_day: sky.time.time_of_day,
                                paused: sky.time.paused,
                                scale: sky.time.scale,
                            },
                            // slice iter is TrustedLen: collects into the Arc
                            // directly (no Vec round trip).
                            shaders: sky
                                .shaders
                                .iter()
                                .map(|shader| SkyShaderPass3DState {
                                    path: shader.path.clone(),
                                    params: Arc::from(shader.params.as_ref()),
                                })
                                .collect(),
                            environment: sky.environment.as_ref().map(|environment| {
                                EnvironmentMap3DState {
                                    source: environment.source.clone(),
                                    intensity: environment.intensity,
                                    rotation_degrees: environment.rotation_degrees,
                                }
                            }),
                        });
                    Some(Stream3DItem::Sky(state))
                }
                SceneNodeData::RayLight3D(light)
                    if light.visible
                        && light.active
                        && stream_render_mask_matches(camera_mask, light.render_layers) =>
                {
                    Some(Stream3DItem::Ray {
                        transform: light.transform,
                        color: light.color,
                        intensity: light.intensity,
                        cast_shadows: light.cast_shadows,
                        shadow_strength: light.shadow_strength,
                        shadow_depth_bias: light.shadow_depth_bias,
                        shadow_normal_bias: light.shadow_normal_bias,
                    })
                }
                SceneNodeData::PointLight3D(light)
                    if light.visible
                        && light.active
                        && light.range > 0.0
                        && stream_render_mask_matches(camera_mask, light.render_layers) =>
                {
                    Some(Stream3DItem::Point {
                        transform: light.transform,
                        color: light.color,
                        intensity: light.intensity,
                        range: light.range,
                        cast_shadows: light.cast_shadows,
                        shadow_strength: light.shadow_strength,
                        shadow_depth_bias: light.shadow_depth_bias,
                        shadow_normal_bias: light.shadow_normal_bias,
                    })
                }
                SceneNodeData::SpotLight3D(light)
                    if light.visible
                        && light.active
                        && light.range > 0.0
                        && stream_render_mask_matches(camera_mask, light.render_layers) =>
                {
                    Some(Stream3DItem::Spot {
                        transform: light.transform,
                        color: light.color,
                        intensity: light.intensity,
                        range: light.range,
                        inner_angle_radians: light.inner_angle_radians,
                        outer_angle_radians: light.outer_angle_radians,
                        cast_shadows: light.cast_shadows,
                        shadow_strength: light.shadow_strength,
                        shadow_depth_bias: light.shadow_depth_bias,
                        shadow_normal_bias: light.shadow_normal_bias,
                    })
                }
                SceneNodeData::ParticleEmitter3D(emitter)
                    if emitter.visible
                        && stream_render_mask_matches(camera_mask, emitter.render_layers) =>
                {
                    Some(Stream3DItem::Particles {
                        profile_source: emitter.profile.clone(),
                        sim_mode: emitter.sim_mode,
                        render_mode: emitter.render_mode,
                        transform: emitter.transform,
                        active: emitter.active,
                        looping: emitter.looping,
                        prewarm: emitter.prewarm,
                        spawn_rate: emitter.spawn_rate,
                        seed: emitter.seed,
                        params: emitter.params.clone(),
                        simulation_time: emitter.internal_simulation_time,
                    })
                }
                SceneNodeData::WaterBody3D(water)
                    if water.visible
                        && stream_render_mask_matches(camera_mask, water.render_layers) =>
                {
                    Some(Stream3DItem::Water {
                        transform: water.transform,
                        water: water.water,
                    })
                }
                _ => None,
            });
        match data {
            Some(Stream3DItem::Ambient(light)) => {
                if out
                    .best_ambient
                    .as_ref()
                    .is_none_or(|(id, _)| node.as_u64() < id.as_u64())
                {
                    out.best_ambient = Some((node, light));
                }
            }
            Some(Stream3DItem::Sky(sky)) => {
                if out
                    .best_sky
                    .as_ref()
                    .is_none_or(|(id, _)| node.as_u64() < id.as_u64())
                {
                    out.best_sky = Some((node, sky));
                }
            }
            Some(Stream3DItem::Ray {
                transform,
                color,
                intensity,
                cast_shadows,
                shadow_strength,
                shadow_depth_bias,
                shadow_normal_bias,
            }) => {
                let global = self
                    .stream_localized_transform_3d(node, localize)
                    .unwrap_or(transform);
                out.ray_lights.push((
                    node,
                    RayLight3DState {
                        direction: stream_quaternion_forward(global.rotation),
                        color: color.to_rgb(),
                        intensity: intensity.max(0.0),
                        cast_shadows,
                        shadow_strength,
                        shadow_depth_bias,
                        shadow_normal_bias,
                    },
                ));
            }
            Some(Stream3DItem::Point {
                transform,
                color,
                intensity,
                range,
                cast_shadows,
                shadow_strength,
                shadow_depth_bias,
                shadow_normal_bias,
            }) => {
                let global = self
                    .stream_localized_transform_3d(node, localize)
                    .unwrap_or(transform);
                out.point_lights.push((
                    node,
                    PointLight3DState {
                        position: [global.position.x, global.position.y, global.position.z],
                        color: color.to_rgb(),
                        intensity: intensity.max(0.0),
                        range: range.max(0.001),
                        cast_shadows,
                        shadow_strength,
                        shadow_depth_bias,
                        shadow_normal_bias,
                    },
                ));
            }
            Some(Stream3DItem::Spot {
                transform,
                color,
                intensity,
                range,
                inner_angle_radians,
                outer_angle_radians,
                cast_shadows,
                shadow_strength,
                shadow_depth_bias,
                shadow_normal_bias,
            }) => {
                let global = self
                    .stream_localized_transform_3d(node, localize)
                    .unwrap_or(transform);
                out.spot_lights.push((
                    node,
                    SpotLight3DState {
                        position: [global.position.x, global.position.y, global.position.z],
                        direction: stream_quaternion_forward(global.rotation),
                        color: color.to_rgb(),
                        intensity: intensity.max(0.0),
                        range: range.max(0.001),
                        inner_angle_radians: inner_angle_radians.max(0.0),
                        outer_angle_radians: outer_angle_radians.max(inner_angle_radians),
                        cast_shadows,
                        shadow_strength,
                        shadow_depth_bias,
                        shadow_normal_bias,
                    },
                ));
            }
            Some(Stream3DItem::Particles {
                profile_source,
                sim_mode,
                render_mode,
                transform,
                active,
                looping,
                prewarm,
                spawn_rate,
                seed,
                params,
                simulation_time,
            }) => {
                let profile =
                    resolve_particle_profile_3d(self, &profile_source).unwrap_or_default();
                let lifetime_min = profile.lifetime_min.max(0.001);
                let lifetime_max = profile.lifetime_max.max(lifetime_min);
                let default_sim_mode = self
                    .project()
                    .map(|project| project.config.particle_sim_default)
                    .unwrap_or(perro_project::ParticleSimDefault::Cpu);
                let model = self
                    .stream_localized_transform_3d(node, localize)
                    .unwrap_or(transform)
                    .to_mat4()
                    .to_cols_array_2d();
                out.particles.push((
                    node,
                    PointParticles3DState {
                        model,
                        active,
                        looping,
                        prewarm,
                        lifetime_min,
                        lifetime_max,
                        alive_budget: derived_particle_budget_3d(spawn_rate.max(0.0), lifetime_max),
                        emission_rate: spawn_rate.max(0.0),
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
                        seed,
                        params,
                        simulation_time: simulation_time.max(0.0),
                        simulation_delta: self.time.delta.max(0.0),
                        profile,
                        sim_mode: resolve_particle_sim_mode_3d(sim_mode, default_sim_mode),
                        render_mode: resolve_particle_render_mode_3d(render_mode),
                    },
                ));
            }
            Some(Stream3DItem::Water {
                transform: local_transform,
                water,
            }) => {
                let water_global = self.stream_localized_transform_3d(node, localize);
                let model = water_global
                    .unwrap_or(local_transform)
                    .to_mat4()
                    .to_cols_array_2d();
                let coastline_shapes = self.collect_water_coastline_shapes_3d(&water, water_global);
                let queries = self.collect_water_queries_3d(node);
                let impacts = self.collect_water_impacts_3d(node, &water, water_global);
                let links = self.collect_water_links_3d(node, &water);
                out.waters.push((
                    node,
                    Water3DState {
                        model,
                        paused: false,
                        simulation_time: self.time.elapsed,
                        simulation_delta: self.time.delta.max(0.0),
                        size: water_render_size_3d(water),
                        shape: water_shape_state_3d(water.shape),
                        resolution: water.resolution,
                        render_resolution: water.render_resolution,
                        depth: water.shape.depth(water.depth),
                        flow: [water.flow.x, water.flow.y],
                        wind: [water.wind.x, water.wind.y],
                        idle_mode: water_idle_mode_state_3d(water.idle_mode),
                        wave_speed: water.wave.speed,
                        wave_scale: water.wave.scale,
                        wave_length: water.wave.length,
                        damping: water.wave.damping,
                        wake_strength: water.physics.wake_strength,
                        foam_strength: water.physics.foam_strength,
                        sample_readback_rate: water.physics.sample_readback_rate,
                        lod_near_distance: water.lod.near_distance,
                        lod_mid_distance: water.lod.mid_distance,
                        lod_far_distance: water.lod.far_distance,
                        lod_min_resolution: water.lod.min_resolution,
                        collision_layers: water.collision_layers,
                        collision_mask: water.collision_mask,
                        deep_color: water.optics.deep_color,
                        shallow_color: water.optics.shallow_color,
                        shallow_depth: water.optics.shallow_depth,
                        sky_bias_ratio: water.optics.sky_bias.ratio(),
                        transparency: water.visual.transparency,
                        reflectivity: water.visual.reflectivity,
                        roughness: water.visual.roughness,
                        fresnel_power: water.visual.fresnel_power,
                        normal_strength: water.visual.normal_strength,
                        ripple_scale: water.visual.ripple_scale,
                        foam_color: water.visual.foam_color,
                        foam_amount: water.visual.foam_amount,
                        crest_foam_threshold: water.visual.crest_foam_threshold,
                        caustic_strength: water.visual.caustic_strength,
                        refraction_strength: water.visual.refraction_strength,
                        scattering_strength: water.visual.scattering_strength,
                        distance_fog_strength: water.visual.distance_fog_strength,
                        coastline_foam_color: water.coastline.foam_color,
                        coastline_foam_strength: water.coastline.foam_strength,
                        coastline_foam_width: water.coastline.foam_width,
                        coastline_cutoff_softness: water.coastline.cutoff_softness,
                        coastline_wave_reflection: water.coastline.wave_reflection,
                        coastline_wave_damping: water.coastline.wave_damping,
                        coastline_edge_noise: water.coastline.edge_noise,
                        debug: water.debug,
                        links,
                        queries,
                        impacts,
                        coastline_shapes,
                    },
                ));
            }
            None => {}
        }
    }
}
