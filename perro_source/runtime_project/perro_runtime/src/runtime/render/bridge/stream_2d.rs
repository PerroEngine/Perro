use super::*;

/// Per-refresh 2D collection scratch: every lane the fused member walk fills,
/// taken from / restored to the retained render state so hot rebuild loops
/// reuse capacity instead of allocating fresh containers.
pub(super) struct Stream2DCollect {
    pub(super) sprites: Vec<Sprite2DCommand>,
    pub(super) lights: Vec<Light2DState>,
    pub(super) particles: Vec<(NodeID, PointParticles2DState)>,
    pub(super) waters: Vec<(NodeID, Water2DState)>,
    /// take-pattern scratch for nested sub-view rect computes (see
    /// `prepare_nested_sub_views`); avoids fresh hash containers per node.
    pub(super) rect_computed: AHashMap<NodeID, ComputedUiRect>,
    pub(super) rect_scales: AHashMap<NodeID, Vector2>,
    pub(super) rect_auto: AHashSet<NodeID>,
}

/// Phase-1 payload 4 members that need `&mut self` work after the node borrow
/// ends (transform localization, water sub-collectors, profile resolve).
enum Stream2DLight {
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

impl Runtime {
    pub(super) fn take_stream_2d_collect(&mut self) -> Stream2DCollect {
        // take-pattern scratch: rebuilt every stream refresh, keep capacity.
        let mut out = Stream2DCollect {
            sprites: std::mem::take(&mut self.render_2d.stream_sprites_scratch),
            lights: std::mem::take(&mut self.render_2d.stream_lights_scratch),
            particles: std::mem::take(&mut self.render_2d.stream_particles_scratch),
            waters: std::mem::take(&mut self.render_2d.stream_waters_scratch),
            rect_computed: std::mem::take(&mut self.render_ui.nested_rect_computed_scratch),
            rect_scales: std::mem::take(&mut self.render_ui.nested_rect_scales_scratch),
            rect_auto: std::mem::take(&mut self.render_ui.nested_rect_auto_scratch),
        };
        out.sprites.clear();
        out.lights.clear();
        out.particles.clear();
        out.waters.clear();
        out
    }

    pub(super) fn restore_stream_2d_collect(&mut self, mut out: Stream2DCollect) {
        out.sprites.clear();
        out.lights.clear();
        out.particles.clear();
        out.waters.clear();
        out.rect_computed.clear();
        out.rect_scales.clear();
        out.rect_auto.clear();
        self.render_2d.stream_sprites_scratch = out.sprites;
        self.render_2d.stream_lights_scratch = out.lights;
        self.render_2d.stream_particles_scratch = out.particles;
        self.render_2d.stream_waters_scratch = out.waters;
        self.render_ui.nested_rect_computed_scratch = out.rect_computed;
        self.render_ui.nested_rect_scales_scratch = out.rect_scales;
        self.render_ui.nested_rect_auto_scratch = out.rect_auto;
    }

    /// One member, every 2D lane. The driver already applied the shared
    /// guards (self-skip, effective visibility, isolated-child skip); a node
    /// is exactly one type, so this replaces 4 full member walks with one
    /// type dispatch per node.
    pub(super) fn stream_member_2d(
        &mut self,
        node: NodeID,
        stream_node: NodeID,
        camera_mask: BitMask,
        stream_resolution: Option<[u32; 2]>,
        localize: &StreamLocalize2D,
        out: &mut Stream2DCollect,
    ) {
        // Camera-stream quads only compose into a sub-view target. Letting one
        // camera stream sample another stream from the same watched world can
        // form A -> B -> A feedback; main-world stream quads stay on the main
        // renderer path instead.
        if self.is_sub_view_node(stream_node) {
            if let Some((stream, rect)) = self.nodes.get(node).and_then(|node_ref| {
                let SceneNodeData::UiCameraStream(stream) = &node_ref.data else {
                    return None;
                };
                if !stream.visible || !stream.stream.enabled {
                    return None;
                }
                let rect = self.nested_ui_sub_view_rect(
                    stream_node,
                    node,
                    stream_resolution?,
                    &mut out.rect_computed,
                    &mut out.rect_scales,
                    &mut out.rect_auto,
                )?;
                Some((stream.as_ref().clone(), rect))
            }) {
                let model = perro_structs::Transform2D::new(
                    rect.center,
                    stream.base.transform.rotation,
                    stream.base.transform.scale,
                )
                .to_mat3()
                .to_cols_array_2d();
                out.sprites.push(Sprite2DCommand {
                    texture: Self::camera_stream_texture_id(node),
                    model,
                    tint: Runtime::color_modulate(stream.tint, self.effective_self_modulate(node)),
                    uv_min: [0.0, 0.0],
                    uv_max: [1.0, 1.0],
                    uv_normalized: true,
                    size: [rect.size.x.max(0.001), rect.size.y.max(0.001)],
                    z_index: stream.base.layout.z_index,
                });
                return;
            }
            if let Some((texture, transform, tint, aspect, z_index)) = self
                .nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::CameraStream2D(stream)
                        if stream.visible
                            && stream.stream.enabled
                            && stream_render_mask_matches(camera_mask, stream.render_layers) =>
                    {
                        Some((
                            Self::camera_stream_texture_id(node),
                            stream.transform,
                            stream.tint,
                            if stream.stream.aspect_ratio > 0.0 {
                                stream.stream.aspect_ratio
                            } else {
                                stream.stream.resolution.x as f32
                                    / stream.stream.resolution.y.max(1) as f32
                            },
                            stream.z_index,
                        ))
                    }
                    _ => None,
                })
            {
                let model = self
                    .stream_localized_transform_2d(node, localize)
                    .unwrap_or(transform)
                    .to_mat3()
                    .to_cols_array_2d();
                out.sprites.push(Sprite2DCommand {
                    texture,
                    model,
                    tint: Runtime::color_modulate(tint, self.effective_self_modulate(node)),
                    uv_min: [0.0, 0.0],
                    uv_max: [1.0, 1.0],
                    uv_normalized: true,
                    size: [aspect.max(0.001), 1.0],
                    z_index,
                });
                return;
            }
        }
        // nested ui sub-view quad.
        if let Some((view, rect)) = self.nodes.get(node).and_then(|node_ref| {
            let SceneNodeData::UiSubView(view) = &node_ref.data else {
                return None;
            };
            if !view.visible || !view.enabled {
                return None;
            }
            let rect = self.nested_ui_sub_view_rect(
                stream_node,
                node,
                stream_resolution?,
                &mut out.rect_computed,
                &mut out.rect_scales,
                &mut out.rect_auto,
            )?;
            Some((view.as_ref().clone(), rect))
        }) {
            let model = perro_structs::Transform2D::new(
                rect.center,
                view.base.transform.rotation,
                view.base.transform.scale,
            )
            .to_mat3()
            .to_cols_array_2d();
            out.sprites.push(Sprite2DCommand {
                texture: Self::camera_stream_texture_id(node),
                model,
                tint: Runtime::color_modulate(view.tint, self.effective_self_modulate(node)),
                uv_min: [0.0, 0.0],
                uv_max: [1.0, 1.0],
                uv_normalized: true,
                size: [rect.size.x.max(0.001), rect.size.y.max(0.001)],
                z_index: view.base.layout.z_index,
            });
            return;
        }
        // nested 2d sub-view quad.
        if let Some((transform, size, z_index, tint)) =
            self.nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::SubView2D(view) if view.visible && view.sub_view.enabled => {
                        Some((view.transform, view.size, view.z_index, view.tint))
                    }
                    _ => None,
                })
        {
            let model = self
                .stream_localized_transform_2d(node, localize)
                .unwrap_or(transform)
                .to_mat3()
                .to_cols_array_2d();
            out.sprites.push(Sprite2DCommand {
                texture: Self::camera_stream_texture_id(node),
                model,
                tint: Runtime::color_modulate(tint, self.effective_self_modulate(node)),
                uv_min: [0.0, 0.0],
                uv_max: [1.0, 1.0],
                uv_normalized: true,
                size: [size.x.max(0.001), size.y.max(0.001)],
                z_index,
            });
            return;
        }
        // sprites.
        if let Some((texture, region, transform, z_index)) =
            self.nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::Sprite2D(sprite)
                        if sprite.visible
                            && stream_render_mask_matches(camera_mask, sprite.render_layers) =>
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
                            && stream_render_mask_matches(camera_mask, sprite.render_layers) =>
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
        {
            let Some(texture) = self.resolve_sprite_texture(node, texture) else {
                return;
            };
            let (uv_min, uv_max, size) = stream_sprite_region_uv(region);
            let model = self
                .stream_localized_transform_2d(node, localize)
                .unwrap_or(transform)
                .to_mat3()
                .to_cols_array_2d();
            out.sprites.push(Sprite2DCommand {
                texture,
                model,
                tint: self.effective_self_modulate(node),
                uv_min,
                uv_max,
                uv_normalized: false,
                size,
                z_index,
            });
            return;
        }
        // tilemaps. Header only (no tiles clone): tiles are read by reference
        // in the shared per-node cache pass below.
        let tilemap_meta = self
            .nodes
            .get(node)
            .and_then(|node_ref| match &node_ref.data {
                SceneNodeData::TileMap2D(tilemap)
                    if tilemap.visible
                        && stream_render_mask_matches(camera_mask, tilemap.render_layers) =>
                {
                    Some((tilemap.tileset.clone(), tilemap.transform))
                }
                _ => None,
            });
        if let Some((tileset_source, local_transform)) = tilemap_meta {
            if let Some(tileset) = resolve_tileset_2d(self, &tileset_source)
                && let Some(texture) = self.resolve_tilemap_texture(node, tileset.texture.as_ref())
            {
                let global_transform = self
                    .stream_localized_transform_2d(node, localize)
                    .unwrap_or(local_transform);
                let base_model = global_transform.to_mat3().to_cols_array_2d();
                let tint = self.effective_self_modulate(node);
                if let Some(scene_node) = self.nodes.get(node)
                    && let SceneNodeData::TileMap2D(tilemap) = &scene_node.data
                {
                    let signature = crate::runtime::render_2d::tilemap_render_signature(
                        texture,
                        &base_model,
                        tint,
                        &tileset,
                        tilemap,
                    );
                    let cache = &mut self.render_2d.tilemap_render_cache;
                    let sprites = match cache.get(&node) {
                        Some(cached) if cached.signature == signature => cached.sprites.clone(),
                        _ => {
                            let sprites = build_tilemap_sprites(TilemapSpriteBuild {
                                texture,
                                width: tilemap.width,
                                height: tilemap.height,
                                z_index: tilemap.z_index,
                                empty_tile: tilemap.empty_tile,
                                tint,
                                base_model,
                                tiles: &tilemap.tiles,
                                tileset: &tileset,
                            });
                            let shadow_casters = if tilemap.collision_enabled {
                                crate::runtime::render_2d::build_tilemap_shadow_casters(
                                    tilemap,
                                    global_transform,
                                    &tileset,
                                )
                            } else {
                                Vec::new()
                            };
                            let entry = crate::runtime::state::TilemapRenderCache {
                                signature,
                                sprites: arc_slice_from_vec(sprites),
                                shadow_casters: arc_slice_from_vec(shadow_casters),
                            };
                            let sprites = entry.sprites.clone();
                            cache.insert(node, entry);
                            sprites
                        }
                    };
                    out.sprites.extend_from_slice(&sprites);
                }
            }
            return;
        }
        // particles.
        if let Some((
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
        )) = self
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
            })
        {
            let profile = resolve_particle_profile_2d(self, &profile_source).unwrap_or_default();
            let lifetime_min = profile.lifetime_min.max(0.001);
            let lifetime_max = profile.lifetime_max.max(lifetime_min);
            let model = self
                .stream_localized_transform_2d(node, localize)
                .unwrap_or(transform)
                .to_mat3()
                .to_cols_array_2d();
            out.particles.push((
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
            return;
        }
        // waters.
        if let Some((local_transform, z_index, water)) =
            self.nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::WaterBody2D(water)
                        if water.visible
                            && stream_render_mask_matches(camera_mask, water.render_layers) =>
                    {
                        Some((water.transform, water.z_index, water.water))
                    }
                    _ => None,
                })
        {
            let water_global = self.stream_localized_transform_2d(node, localize);
            let model = water_global
                .unwrap_or(local_transform)
                .to_mat3()
                .to_cols_array_2d();
            let coastline_shapes = self.collect_water_coastline_shapes_2d(&water, water_global);
            let queries = self.collect_water_queries_2d(node);
            let impacts = self.collect_water_impacts_2d(node, &water, water_global);
            let links = self.collect_water_links_2d(node, &water);
            out.waters.push((
                node,
                Water2DState {
                    model,
                    z_index,
                    paused: false,
                    simulation_time: self.time.elapsed,
                    simulation_delta: self.time.delta.max(0.0),
                    size: water_render_size_2d(water),
                    shape: water_shape_state_2d(water.shape),
                    quality: water.quality,
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
            return;
        }
        // lights.
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
                    Some(Stream2DLight::Ambient {
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
                    Some(Stream2DLight::Ray {
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
                    Some(Stream2DLight::Point {
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
                    Some(Stream2DLight::Spot {
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
            Some(Stream2DLight::Ambient { color, intensity }) => {
                out.lights.push(Light2DState::Ambient(AmbientLight2DState {
                    color: color.to_rgb(),
                    intensity: intensity.max(0.0),
                }));
            }
            Some(Stream2DLight::Ray {
                transform,
                color,
                intensity,
                z_index,
                cast_shadows,
                shadow_softness,
                shadow_samples,
            }) => {
                let global = self
                    .stream_localized_transform_2d(node, localize)
                    .unwrap_or(transform);
                out.lights.push(Light2DState::Ray(RayLight2DState {
                    direction: direction_from_rotation_2d(global.rotation),
                    color: color.to_rgb(),
                    intensity: intensity.max(0.0),
                    z_index,
                    cast_shadows,
                    shadow_softness: shadow_softness_2d(shadow_softness),
                    shadow_samples: shadow_samples.clamp(1, 16),
                }));
            }
            Some(Stream2DLight::Point {
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
                    .stream_localized_transform_2d(node, localize)
                    .unwrap_or(transform);
                out.lights.push(Light2DState::Point(PointLight2DState {
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
            Some(Stream2DLight::Spot {
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
                    .stream_localized_transform_2d(node, localize)
                    .unwrap_or(transform);
                out.lights.push(Light2DState::Spot(SpotLight2DState {
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
}
