use super::*;

impl Runtime {
    pub(super) fn collect_camera_stream_sprites_2d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[Sprite2DCommand]> {
        let mut out = Vec::new();
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node
                || !self.is_effectively_visible(node)
                || self.stream_skips_isolated_child(node, stream_node)
            {
                continue;
            }
            let Some((texture, region, transform, z_index)) =
                self.nodes
                    .get(node)
                    .and_then(|node_ref| match &node_ref.data {
                        SceneNodeData::Sprite2D(sprite)
                            if sprite.visible
                                && stream_render_mask_matches(
                                    camera_mask,
                                    sprite.render_layers,
                                ) =>
                        {
                            Some((
                                sprite.texture,
                                sprite.texture_region,
                                sprite.transform,
                                sprite.z_index,
                            ))
                        }
                        SceneNodeData::AnimatedSprite2D(sprite)
                            if sprite.visible
                                && stream_render_mask_matches(
                                    camera_mask,
                                    sprite.render_layers,
                                ) =>
                        {
                            Some((
                                sprite.texture,
                                sprite.current_texture_region(),
                                sprite.transform,
                                sprite.z_index,
                            ))
                        }
                        _ => None,
                    })
            else {
                let tilemap_data = self
                    .nodes
                    .get(node)
                    .and_then(|node_ref| match &node_ref.data {
                        SceneNodeData::TileMap2D(tilemap)
                            if tilemap.visible
                                && stream_render_mask_matches(
                                    camera_mask,
                                    tilemap.render_layers,
                                ) =>
                        {
                            Some((
                                tilemap.tileset.clone(),
                                tilemap.width,
                                tilemap.height,
                                tilemap.empty_tile,
                                tilemap.tiles.clone(),
                                tilemap.transform,
                                tilemap.z_index,
                            ))
                        }
                        _ => None,
                    });
                if let Some((
                    tileset_source,
                    width,
                    height,
                    empty_tile,
                    tiles,
                    local_transform,
                    z_index,
                )) = tilemap_data
                    && let Some(tileset) = resolve_tileset_2d(self, &tileset_source)
                    && let Some(texture) =
                        self.resolve_tilemap_texture(node, tileset.texture.as_ref())
                {
                    let base_model = self
                        .stream_render_transform_2d(node, stream_node)
                        .unwrap_or(local_transform)
                        .to_mat3()
                        .to_cols_array_2d();
                    out.extend(build_tilemap_sprites(TilemapSpriteBuild {
                        texture,
                        width,
                        height,
                        z_index,
                        empty_tile,
                        tint: self.effective_self_modulate(node),
                        base_model,
                        tiles: &tiles,
                        tileset: &tileset,
                    }));
                }
                continue;
            };
            let Some(texture) = self.resolve_sprite_texture(node, texture) else {
                continue;
            };
            let (uv_min, uv_max, size) = stream_sprite_region_uv(region);
            let model = self
                .stream_render_transform_2d(node, stream_node)
                .unwrap_or(transform)
                .to_mat3()
                .to_cols_array_2d();
            out.push(Sprite2DCommand {
                texture,
                model,
                tint: self.effective_self_modulate(node),
                uv_min,
                uv_max,
                uv_normalized: false,
                size,
                z_index,
            });
        }
        Arc::from(out)
    }

    pub(super) fn collect_camera_stream_lights_2d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[Light2DState]> {
        enum StreamLight2DData {
            Ambient {
                color: Color,
                intensity: f32,
            },
            Ray {
                transform: perro_structs::Transform2D,
                color: Color,
                intensity: f32,
                z_index: i32,
                cast_shadows: bool,
                shadow_softness: f32,
                shadow_samples: u32,
            },
            Point {
                transform: perro_structs::Transform2D,
                color: Color,
                intensity: f32,
                range: f32,
                z_index: i32,
                cast_shadows: bool,
                shadow_softness: f32,
                shadow_samples: u32,
            },
            Spot {
                transform: perro_structs::Transform2D,
                color: Color,
                intensity: f32,
                range: f32,
                inner_angle_radians: f32,
                outer_angle_radians: f32,
                z_index: i32,
                cast_shadows: bool,
                shadow_softness: f32,
                shadow_samples: u32,
            },
        }
        let mut out = Vec::new();
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node
                || !self.is_effectively_visible(node)
                || self.stream_skips_isolated_child(node, stream_node)
            {
                continue;
            }
            let data = self
                .nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::AmbientLight2D(light)
                        if light.visible
                            && light.active
                            && light.intensity > 0.0
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight2DData::Ambient {
                            color: light.color,
                            intensity: light.intensity,
                        })
                    }
                    SceneNodeData::RayLight2D(light)
                        if light.visible
                            && light.active
                            && light.intensity > 0.0
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight2DData::Ray {
                            transform: light.transform,
                            color: light.color,
                            intensity: light.intensity,
                            z_index: light.z_index,
                            cast_shadows: light.cast_shadows,
                            shadow_softness: light.shadow_softness,
                            shadow_samples: light.shadow_samples,
                        })
                    }
                    SceneNodeData::PointLight2D(light)
                        if light.visible
                            && light.active
                            && light.intensity > 0.0
                            && light.range > 0.0
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight2DData::Point {
                            transform: light.transform,
                            color: light.color,
                            intensity: light.intensity,
                            range: light.range,
                            z_index: light.z_index,
                            cast_shadows: light.cast_shadows,
                            shadow_softness: light.shadow_softness,
                            shadow_samples: light.shadow_samples,
                        })
                    }
                    SceneNodeData::SpotLight2D(light)
                        if light.visible
                            && light.active
                            && light.intensity > 0.0
                            && light.range > 0.0
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight2DData::Spot {
                            transform: light.transform,
                            color: light.color,
                            intensity: light.intensity,
                            range: light.range,
                            inner_angle_radians: light.inner_angle_radians,
                            outer_angle_radians: light.outer_angle_radians,
                            z_index: light.z_index,
                            cast_shadows: light.cast_shadows,
                            shadow_softness: light.shadow_softness,
                            shadow_samples: light.shadow_samples,
                        })
                    }
                    _ => None,
                });
            match data {
                Some(StreamLight2DData::Ambient { color, intensity }) => {
                    out.push(Light2DState::Ambient(AmbientLight2DState {
                        color: color.to_rgb(),
                        intensity: intensity.max(0.0),
                    }));
                }
                Some(StreamLight2DData::Ray {
                    transform,
                    color,
                    intensity,
                    z_index,
                    cast_shadows,
                    shadow_softness,
                    shadow_samples,
                }) => {
                    let global = self
                        .stream_render_transform_2d(node, stream_node)
                        .unwrap_or(transform);
                    out.push(Light2DState::Ray(RayLight2DState {
                        direction: direction_from_rotation_2d(global.rotation),
                        color: color.to_rgb(),
                        intensity: intensity.max(0.0),
                        z_index,
                        cast_shadows,
                        shadow_softness: shadow_softness_2d(shadow_softness),
                        shadow_samples: shadow_samples.clamp(1, 16),
                    }));
                }
                Some(StreamLight2DData::Point {
                    transform,
                    color,
                    intensity,
                    range,
                    z_index,
                    cast_shadows,
                    shadow_softness,
                    shadow_samples,
                }) => {
                    let global = self
                        .stream_render_transform_2d(node, stream_node)
                        .unwrap_or(transform);
                    out.push(Light2DState::Point(PointLight2DState {
                        position: [global.position.x, global.position.y],
                        color: color.to_rgb(),
                        intensity: intensity.max(0.0),
                        range: range.max(0.001),
                        z_index,
                        cast_shadows,
                        shadow_softness: shadow_softness_2d(shadow_softness),
                        shadow_samples: shadow_samples.clamp(1, 16),
                    }));
                }
                Some(StreamLight2DData::Spot {
                    transform,
                    color,
                    intensity,
                    range,
                    inner_angle_radians,
                    outer_angle_radians,
                    z_index,
                    cast_shadows,
                    shadow_softness,
                    shadow_samples,
                }) => {
                    let global = self
                        .stream_render_transform_2d(node, stream_node)
                        .unwrap_or(transform);
                    out.push(Light2DState::Spot(SpotLight2DState {
                        position: [global.position.x, global.position.y],
                        direction: direction_from_rotation_2d(global.rotation),
                        color: color.to_rgb(),
                        intensity: intensity.max(0.0),
                        range: range.max(0.001),
                        inner_angle_radians: inner_angle_radians.max(0.0),
                        outer_angle_radians: outer_angle_radians.max(inner_angle_radians),
                        z_index,
                        cast_shadows,
                        shadow_softness: shadow_softness_2d(shadow_softness),
                        shadow_samples: shadow_samples.clamp(1, 16),
                    }));
                }
                None => {}
            }
        }
        Arc::from(out)
    }

    pub(super) fn collect_camera_stream_point_particles_2d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[(NodeID, PointParticles2DState)]> {
        let mut out = Vec::new();
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node
                || !self.is_effectively_visible(node)
                || self.stream_skips_isolated_child(node, stream_node)
            {
                continue;
            }
            let data = self
                .nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::ParticleEmitter2D(emitter)
                        if emitter.visible
                            && stream_render_mask_matches(camera_mask, emitter.render_layers) =>
                    {
                        Some((
                            emitter.profile.clone(),
                            emitter.sim_mode,
                            emitter.transform,
                            emitter.z_index,
                            emitter.active,
                            emitter.looping,
                            emitter.prewarm,
                            emitter.spawn_rate,
                            emitter.seed,
                            emitter.params.clone(),
                            emitter.internal_simulation_time,
                        ))
                    }
                    _ => None,
                });
            let Some((
                profile_source,
                sim_mode,
                transform,
                z_index,
                active,
                looping,
                prewarm,
                spawn_rate,
                seed,
                params,
                simulation_time,
            )) = data
            else {
                continue;
            };
            let profile = resolve_particle_profile_2d(self, &profile_source).unwrap_or_default();
            let lifetime_min = profile.lifetime_min.max(0.001);
            let lifetime_max = profile.lifetime_max.max(lifetime_min);
            let model = self
                .stream_render_transform_2d(node, stream_node)
                .unwrap_or(transform)
                .to_mat3()
                .to_cols_array_2d();
            out.push((
                node,
                PointParticles2DState {
                    model,
                    z_index,
                    active,
                    looping,
                    prewarm,
                    alive_budget: derived_particle_budget(spawn_rate.max(0.0), lifetime_max),
                    emission_rate: spawn_rate.max(0.0),
                    lifetime_min,
                    lifetime_max,
                    speed_min: profile.speed_min.max(0.0),
                    speed_max: profile.speed_max.max(profile.speed_min.max(0.0)),
                    spread_radians: profile.spread_radians.clamp(0.0, std::f32::consts::TAU),
                    size: profile.size.max(1.0),
                    size_min: profile.size_min.max(0.01),
                    size_max: profile.size_max.max(profile.size_min.max(0.01)),
                    force: profile.force,
                    color_start: profile.color_start,
                    color_end: profile.color_end,
                    seed,
                    params,
                    simulation_time,
                    simulation_delta: 0.0,
                    profile,
                    sim_mode: resolve_particle_sim_mode_2d(sim_mode),
                },
            ));
        }
        Arc::from(out)
    }

    pub(super) fn collect_camera_stream_waters_2d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[(NodeID, Water2DState)]> {
        let mut out = Vec::new();
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node
                || !self.is_effectively_visible(node)
                || self.stream_skips_isolated_child(node, stream_node)
            {
                continue;
            }
            let data = self
                .nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::WaterBody2D(water)
                        if water.visible
                            && stream_render_mask_matches(camera_mask, water.render_layers) =>
                    {
                        Some((water.transform, water.z_index, water.water))
                    }
                    _ => None,
                });
            let Some((local_transform, z_index, water)) = data else {
                continue;
            };
            let water_global = self.stream_render_transform_2d(node, stream_node);
            let model = water_global
                .unwrap_or(local_transform)
                .to_mat3()
                .to_cols_array_2d();
            let coastline_shapes = self.collect_water_coastline_shapes_2d(&water, water_global);
            let queries = self.collect_water_queries_2d(node);
            let impacts = self.collect_water_impacts_2d(node, &water, water_global);
            let links = self.collect_water_links_2d(node, &water);
            out.push((
                node,
                Water2DState {
                    model,
                    z_index,
                    paused: false,
                    simulation_time: self.time.elapsed,
                    simulation_delta: self.time.delta.max(0.0),
                    size: water_render_size_2d(water),
                    shape: water_shape_state_2d(water.shape),
                    resolution: water.resolution,
                    render_resolution: water.render_resolution,
                    depth: water.shape.depth(water.depth),
                    flow: [water.flow.x, water.flow.y],
                    wind: [water.wind.x, water.wind.y],
                    idle_mode: water_idle_mode_state_2d(water.idle_mode),
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
        Arc::from(out)
    }
}
