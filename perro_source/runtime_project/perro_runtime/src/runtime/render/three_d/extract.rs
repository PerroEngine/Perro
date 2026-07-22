use super::*;

#[path = "extract/support.rs"]
mod support;

impl Runtime {
    pub(crate) fn mesh_instance_render_ready(&self, node_id: NodeID) -> bool {
        let Some(node) = self.nodes.get(node_id) else {
            return false;
        };
        let (mesh, surfaces) = match &node.data {
            SceneNodeData::MeshInstance3D(mesh_instance) => {
                (mesh_instance.mesh, mesh_instance.surfaces.as_slice())
            }
            SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                (mesh_instance.mesh, mesh_instance.surfaces.as_slice())
            }
            _ => return false,
        };
        let only_nil_materials = surfaces
            .iter()
            .all(|surface| surface.material.is_none_or(|id| id.is_nil()));
        if mesh.is_nil() {
            return only_nil_materials && !self.render_3d.mesh_sources.contains_key(&node_id);
        }
        if !self.resource_api.is_mesh_loaded(mesh) {
            return false;
        }
        surfaces.iter().all(|surface| {
            surface
                .material
                .is_none_or(|id| id.is_nil() || self.resource_api.is_material_loaded(id))
        })
    }

    pub fn extract_render_3d_commands(&mut self) {
        let bootstrap_scan = self.render_3d.prev_visible.is_empty()
            && self.render_3d.retained_ambient_lights.is_empty()
            && self.render_3d.retained_skies.is_empty()
            && self.render_3d.retained_ray_lights.is_empty()
            && self.render_3d.retained_point_lights.is_empty()
            && self.render_3d.retained_spot_lights.is_empty()
            && self.render_3d.retained_decals.is_empty()
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

        let active_camera = self.active_render_camera_3d();
        let camera_changed = self.render_3d.last_camera.as_ref() != active_camera.as_ref();
        let previous_camera_render_mask = self
            .render_3d
            .last_camera
            .as_ref()
            .map(|camera| camera.render_mask)
            .unwrap_or(BitMask::NONE);
        let camera_render_mask = active_camera
            .as_ref()
            .map(|camera| camera.render_mask)
            .unwrap_or(BitMask::NONE);
        let camera_render_mask_changed = previous_camera_render_mask != camera_render_mask;

        if camera_changed {
            if let Some(camera) = &active_camera {
                self.resource_api.set_audio_listener_3d(
                    camera.position,
                    camera.rotation,
                    camera.audio_options.clone(),
                );
                self.queue_render_command(RenderCommand::ThreeD(Box::new(Command3D::SetCamera {
                    camera: camera.clone(),
                })));
            } else {
                let camera = fallback_camera_3d_state();
                self.resource_api.set_audio_listener_3d(
                    camera.position,
                    camera.rotation,
                    camera.audio_options.clone(),
                );
                self.queue_render_command(RenderCommand::ThreeD(Box::new(Command3D::SetCamera {
                    camera,
                })));
            }
            self.render_3d.last_camera = active_camera.clone();
        }

        let mut dirty_ids = std::mem::take(&mut self.render_3d.dirty_ids_scratch);
        dirty_ids.clear();
        dirty_ids.extend(
            self.dirty
                .dirty_indices()
                .iter()
                .filter_map(|&raw_index| self.nodes.slot_get(raw_index as usize).map(|(id, _)| id)),
        );
        let include_all_nodes = self.render_3d.full_scan_pending()
            || bootstrap_scan
            || camera_changed
            || camera_render_mask_changed;
        let mut all_ids = std::mem::take(&mut self.render_3d.all_ids_scratch);
        all_ids.clear();
        if include_all_nodes {
            all_ids.extend(self.nodes.iter().map(|(id, _)| id));
        }
        let nodes = &self.nodes;
        let mut traversal_ids = self.render_3d.collect_traversal(
            dirty_ids.iter().copied(),
            all_ids.iter().copied(),
            bootstrap_scan || camera_changed || camera_render_mask_changed,
            |node, out| {
                if let Some(node_ref) = nodes.get(node) {
                    out.extend(node_ref.get_children_ids().iter().copied());
                }
            },
        );
        dirty_ids.clear();
        all_ids.clear();
        self.render_3d.dirty_ids_scratch = dirty_ids;
        self.render_3d.all_ids_scratch = all_ids;

        let mut traversal_seen = std::mem::take(&mut self.render_3d.traversal_seen);
        traversal_seen.clear();
        traversal_seen.extend(traversal_ids.iter().copied());
        let mut dirty_skeletons = std::mem::take(&mut self.render_3d.dirty_skeletons_scratch);
        dirty_skeletons.clear();
        dirty_skeletons.extend(traversal_ids.iter().copied().filter(|id| {
            self.nodes
                .get(*id)
                .is_some_and(|node| matches!(node.data, SceneNodeData::Skeleton3D(_)))
        }));
        // Keep the skeleton->mesh reverse index current for mesh instances that
        // moved into the traversal this frame (adds + skeleton rebinds are dirty,
        // so they appear here). Removals are handled in note_removed_node.
        if self.render_3d.skeleton_mesh_index_built {
            for id in traversal_ids.iter().copied() {
                let skeleton = match self.nodes.get(id) {
                    Some(node) => match &node.data {
                        SceneNodeData::MeshInstance3D(mesh) => Some(mesh.skeleton),
                        _ => None,
                    },
                    None => None,
                };
                if let Some(skeleton) = skeleton {
                    self.render_3d.index_mesh_skeleton(id, skeleton);
                }
            }
        }
        if !dirty_skeletons.is_empty() {
            if self.render_3d.skeleton_mesh_index_built {
                // Fast path: pull the (few) skinned meshes bound to each dirty
                // skeleton from the reverse index. O(skinned) not O(all nodes).
                let mesh_map = std::mem::take(&mut self.render_3d.skeleton_mesh_map);
                for skeleton in &dirty_skeletons {
                    if let Some(bucket) = mesh_map.get(skeleton) {
                        for &mesh_id in bucket {
                            if self.nodes.get(mesh_id).is_some() && traversal_seen.insert(mesh_id) {
                                traversal_ids.push(mesh_id);
                            }
                        }
                    }
                }
                self.render_3d.skeleton_mesh_map = mesh_map;
            } else {
                // Fallback until the index is built: full arena scan, and build
                // the index from the same pass so later frames take the fast path.
                self.render_3d.skeleton_mesh_map.clear();
                self.render_3d.mesh_skeleton_map.clear();
                for (id, node) in self.nodes.iter() {
                    let SceneNodeData::MeshInstance3D(mesh) = &node.data else {
                        continue;
                    };
                    let skeleton = mesh.skeleton;
                    if !skeleton.is_nil() {
                        self.render_3d.mesh_skeleton_map.insert(id, skeleton);
                        self.render_3d
                            .skeleton_mesh_map
                            .entry(skeleton)
                            .or_default()
                            .insert(id);
                    }
                    if dirty_skeletons.contains(&skeleton) && traversal_seen.insert(id) {
                        traversal_ids.push(id);
                    }
                }
                self.render_3d.skeleton_mesh_index_built = true;
            }
        } else if !self.render_3d.skeleton_mesh_index_built {
            // No dirty skeletons this frame, but the index is not yet built (e.g.
            // fresh scene). Build it from a single arena pass so the very first
            // animating frame can use the fast path.
            self.render_3d.skeleton_mesh_map.clear();
            self.render_3d.mesh_skeleton_map.clear();
            for (id, node) in self.nodes.iter() {
                let SceneNodeData::MeshInstance3D(mesh) = &node.data else {
                    continue;
                };
                let skeleton = mesh.skeleton;
                if !skeleton.is_nil() {
                    self.render_3d.mesh_skeleton_map.insert(id, skeleton);
                    self.render_3d
                        .skeleton_mesh_map
                        .entry(skeleton)
                        .or_default()
                        .insert(id);
                }
            }
            self.render_3d.skeleton_mesh_index_built = true;
        }
        dirty_skeletons.clear();
        self.render_3d.dirty_skeletons_scratch = dirty_skeletons;
        let mut visible_now = self.render_3d.begin_visible_pass();
        let mut skeleton_cache = std::mem::take(&mut self.render_3d.skeleton_cache_scratch);
        skeleton_cache.clear();
        let mut skeleton_global_scratch =
            std::mem::take(&mut self.render_3d.skeleton_global_scratch);
        skeleton_global_scratch.clear();
        let mut skeleton_palette_scratch =
            std::mem::take(&mut self.render_3d.skeleton_palette_scratch);
        skeleton_palette_scratch.clear();
        let mut dense_instance_pose_scratch =
            std::mem::take(&mut self.render_3d.dense_instance_pose_scratch);
        dense_instance_pose_scratch.clear();

        // Loop-invariant: active camera + viewport are fixed for the whole
        // traversal. Compute once instead of per Sprite3D/VideoPlayer3D/Label3D.
        let overlay_camera = active_camera.unwrap_or_else(fallback_camera_3d_state);
        let overlay_viewport = self.input.viewport_size();
        // Reuse one compact blocker list for all world overlays. Building it
        // per overlay made Label3D spawn bursts scan + allocate for the full
        // node arena once per label.
        let mut overlay_occluders = std::mem::take(&mut self.render_3d.overlay_occluders_scratch);
        overlay_occluders.clear();
        overlay_occluders.extend(self.nodes.iter().filter_map(|(candidate, scene_node)| {
            matches!(
                scene_node.data,
                SceneNodeData::MeshInstance3D(_) | SceneNodeData::MultiMeshInstance3D(_)
            )
            .then_some(candidate)
        }));

        for node in traversal_ids.iter().copied() {
            visible_now.remove(&node);
            let effective_visible =
                self.is_effectively_visible(node) && !self.is_under_sub_view(node);
            let ambient_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::AmbientLight3D(light)
                    if light.active
                        && light.visible
                        && effective_visible
                        && render_mask_matches(camera_render_mask, light.render_layers) =>
                {
                    Some((light.color, light.intensity, light.cast_shadows))
                }
                _ => None,
            });
            if let Some((color, intensity, cast_shadows)) = ambient_light_data {
                let light = AmbientLight3DState {
                    color: Runtime::color_modulate_rgb(color, self.effective_self_modulate(node)),
                    intensity: intensity.max(0.0),
                    cast_shadows,
                };
                if self.render_3d.retained_ambient_lights.get(&node).copied() != Some(light) {
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(
                        Command3D::SetAmbientLight { node, light },
                    )));
                    self.render_3d.retained_ambient_lights.insert(node, light);
                }
                visible_now.insert(node);
            }

            let sky_src = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Sky3D(sky)
                    if sky.active
                        && sky.visible
                        && effective_visible
                        && render_mask_matches(camera_render_mask, sky.render_layers) =>
                {
                    Some(sky)
                }
                _ => None,
            });
            if let Some(sky) = sky_src {
                let unchanged = self
                    .render_3d
                    .retained_skies
                    .get(&node)
                    .is_some_and(|retained| sky_3d_state_matches(retained, sky));
                if !unchanged {
                    let sky = Sky3DState {
                        day_colors: Arc::from(sky.palette.day_colors.as_ref()),
                        evening_colors: Arc::from(sky.palette.evening_colors.as_ref()),
                        night_colors: Arc::from(sky.palette.night_colors.as_ref()),
                        horizon_colors: Arc::from(sky.palette.horizon_colors.as_ref()),
                        time: SkyTime3DState {
                            time_of_day: sky.time.time_of_day,
                            paused: sky.time.paused,
                            scale: sky.time.scale,
                        },
                        shaders: Arc::from(
                            sky.shaders
                                .iter()
                                .map(|shader| SkyShaderPass3DState {
                                    path: shader.path.clone(),
                                    params: Arc::from(shader.params.as_ref()),
                                })
                                .collect::<Vec<_>>(),
                        ),
                        environment: sky.environment.as_ref().map(|environment| {
                            EnvironmentMap3DState {
                                source: environment.source.clone(),
                                intensity: environment.intensity,
                                rotation_degrees: environment.rotation_degrees,
                            }
                        }),
                    };
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(Command3D::SetSky {
                        node,
                        sky: Box::new(sky.clone()),
                    })));
                    self.render_3d.retained_skies.insert(node, sky);
                }
                visible_now.insert(node);
            }

            let stream_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::CameraStream3D(stream) => Some((
                    effective_visible
                        && stream.visible
                        && stream.stream.enabled
                        && render_mask_matches(camera_render_mask, stream.render_layers),
                    stream.stream.clone(),
                    stream.transform,
                    stream.size,
                    stream.tint,
                )),
                _ => None,
            });
            if let Some((visible, stream, local_transform, size, tint)) = stream_data {
                if visible {
                    if let Some(stream_state) = self.camera_stream_state(node, &stream) {
                        let tint =
                            Runtime::color_modulate(tint, self.effective_self_modulate(node));
                        let model = self
                            .get_render_global_transform_3d(node)
                            .unwrap_or(local_transform)
                            .to_mat4()
                            .to_cols_array_2d();
                        self.queue_render_command(RenderCommand::CameraStream(
                            CameraStreamCommand::Upsert {
                                node,
                                state: Box::new(stream_state.clone()),
                            },
                        ));
                        self.queue_render_command(RenderCommand::ThreeD(Box::new(
                            Command3D::UpsertCameraStream {
                                node,
                                stream: Box::new(stream_state),
                                quad: CameraStream3DState { model, size, tint },
                            },
                        )));
                        visible_now.insert(node);
                    } else {
                        self.queue_render_command(RenderCommand::CameraStream(
                            CameraStreamCommand::RemoveNode { node },
                        ));
                        self.remove_retained_render_3d_node(node);
                    }
                } else {
                    self.queue_render_command(RenderCommand::CameraStream(
                        CameraStreamCommand::RemoveNode { node },
                    ));
                    self.remove_retained_render_3d_node(node);
                }
            }

            let sub_view_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::SubView3D(view) => Some((
                    effective_visible
                        && view.visible
                        && view.sub_view.enabled
                        && render_mask_matches(camera_render_mask, view.render_layers),
                    view.sub_view.clone(),
                    view.transform,
                    view.size,
                    view.tint,
                )),
                _ => None,
            });
            if let Some((visible, view, local_transform, size, tint)) = sub_view_data {
                if visible {
                    if let Some(stream_state) = self.sub_view_state(node, &view, None) {
                        let tint =
                            Runtime::color_modulate(tint, self.effective_self_modulate(node));
                        let model = self
                            .get_render_global_transform_3d(node)
                            .unwrap_or(local_transform)
                            .to_mat4()
                            .to_cols_array_2d();
                        self.queue_render_command(RenderCommand::CameraStream(
                            CameraStreamCommand::Upsert {
                                node,
                                state: Box::new(stream_state.clone()),
                            },
                        ));
                        self.queue_render_command(RenderCommand::ThreeD(Box::new(
                            Command3D::UpsertCameraStream {
                                node,
                                stream: Box::new(stream_state),
                                quad: CameraStream3DState {
                                    model,
                                    size: [size.x.max(0.001), size.y.max(0.001)],
                                    tint,
                                },
                            },
                        )));
                        visible_now.insert(node);
                    } else {
                        self.queue_render_command(RenderCommand::CameraStream(
                            CameraStreamCommand::RemoveNode { node },
                        ));
                        self.remove_retained_render_3d_node(node);
                    }
                } else {
                    self.queue_render_command(RenderCommand::CameraStream(
                        CameraStreamCommand::RemoveNode { node },
                    ));
                    self.remove_retained_render_3d_node(node);
                }
            }

            let water_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::WaterBody3D(water)
                    if water.visible
                        && effective_visible
                        && render_mask_matches(camera_render_mask, water.render_layers) =>
                {
                    Some((water.transform, water.water))
                }
                _ => None,
            });
            if let Some((local_transform, water)) = water_data {
                // resolve water global transform once; reused for model +
                // coastline + impacts (was recomputed 3x).
                let water_global = self.get_render_global_transform_3d(node);
                let model = water_global
                    .unwrap_or(local_transform)
                    .to_mat4()
                    .to_cols_array_2d();
                let coastline_shapes = self.collect_water_coastline_shapes_3d(&water, water_global);
                let queries = self.collect_water_queries_3d(node);
                let impacts = self.collect_water_impacts_3d(node, &water, water_global);
                let links = self.collect_water_links_3d(node, &water);
                let modulate = self.effective_self_modulate(node);
                self.queue_render_command(RenderCommand::ThreeD(Box::new(
                    Command3D::UpsertWater {
                        node,
                        water: Box::new(Water3DState {
                            model,
                            paused: false,
                            simulation_time: self.time.elapsed,
                            simulation_delta: self.time.delta.max(0.0),
                            size: water_render_size(water),
                            shape: water_shape_state(water.shape),
                            resolution: water.resolution,
                            render_resolution: water.render_resolution,
                            depth: water.shape.depth(water.depth),
                            flow: [water.flow.x, water.flow.y],
                            wind: [water.wind.x, water.wind.y],
                            idle_mode: water_idle_mode_state(water.idle_mode),
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
                            deep_color: Runtime::color_modulate(water.optics.deep_color, modulate),
                            shallow_color: Runtime::color_modulate(
                                water.optics.shallow_color,
                                modulate,
                            ),
                            shallow_depth: water.optics.shallow_depth,
                            sky_bias_ratio: water.optics.sky_bias.ratio(),
                            transparency: water.visual.transparency,
                            reflectivity: water.visual.reflectivity,
                            roughness: water.visual.roughness,
                            fresnel_power: water.visual.fresnel_power,
                            normal_strength: water.visual.normal_strength,
                            ripple_scale: water.visual.ripple_scale,
                            foam_color: Runtime::color_modulate(water.visual.foam_color, modulate),
                            foam_amount: water.visual.foam_amount,
                            crest_foam_threshold: water.visual.crest_foam_threshold,
                            caustic_strength: water.visual.caustic_strength,
                            refraction_strength: water.visual.refraction_strength,
                            scattering_strength: water.visual.scattering_strength,
                            distance_fog_strength: water.visual.distance_fog_strength,
                            coastline_foam_color: Runtime::color_modulate(
                                water.coastline.foam_color,
                                modulate,
                            ),
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
                        }),
                    },
                )));
                visible_now.insert(node);
            }

            let ray_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::RayLight3D(light)
                    if light.active
                        && light.visible
                        && effective_visible
                        && render_mask_matches(camera_render_mask, light.render_layers) =>
                {
                    Some((
                        light.transform,
                        light.color,
                        light.intensity,
                        light.cast_shadows,
                        light.shadow_strength,
                        light.shadow_depth_bias,
                        light.shadow_normal_bias,
                    ))
                }
                _ => None,
            });
            if let Some((
                local_transform,
                color,
                intensity,
                cast_shadows,
                shadow_strength,
                shadow_depth_bias,
                shadow_normal_bias,
            )) = ray_light_data
            {
                let color = Runtime::color_modulate_rgb(color, self.effective_self_modulate(node));
                let global = self
                    .get_render_global_transform_3d(node)
                    .unwrap_or(local_transform);
                let light = RayLight3DState {
                    direction: quaternion_forward(global.rotation),
                    color,
                    intensity: intensity.max(0.0),
                    cast_shadows,
                    shadow_strength,
                    shadow_depth_bias,
                    shadow_normal_bias,
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
                    if light.active
                        && light.visible
                        && effective_visible
                        && render_mask_matches(camera_render_mask, light.render_layers) =>
                {
                    Some((
                        light.transform,
                        light.color,
                        light.intensity,
                        light.range,
                        light.cast_shadows,
                        light.shadow_strength,
                        light.shadow_depth_bias,
                        light.shadow_normal_bias,
                    ))
                }
                _ => None,
            });
            if let Some((
                local_transform,
                color,
                intensity,
                range,
                cast_shadows,
                shadow_strength,
                shadow_depth_bias,
                shadow_normal_bias,
            )) = point_light_data
            {
                let color = Runtime::color_modulate_rgb(color, self.effective_self_modulate(node));
                let global = self
                    .get_render_global_transform_3d(node)
                    .unwrap_or(local_transform);
                let light = PointLight3DState {
                    position: [global.position.x, global.position.y, global.position.z],
                    color,
                    intensity: intensity.max(0.0),
                    range: range.max(0.001),
                    cast_shadows,
                    shadow_strength,
                    shadow_depth_bias,
                    shadow_normal_bias,
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
                    if light.active
                        && light.visible
                        && effective_visible
                        && render_mask_matches(camera_render_mask, light.render_layers) =>
                {
                    Some((
                        light.transform,
                        light.color,
                        light.intensity,
                        light.range,
                        light.inner_angle_radians,
                        light.outer_angle_radians,
                        light.cast_shadows,
                        light.shadow_strength,
                        light.shadow_depth_bias,
                        light.shadow_normal_bias,
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
                shadow_strength,
                shadow_depth_bias,
                shadow_normal_bias,
            )) = spot_light_data
            {
                let color = Runtime::color_modulate_rgb(color, self.effective_self_modulate(node));
                let global = self
                    .get_render_global_transform_3d(node)
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
                    shadow_strength,
                    shadow_depth_bias,
                    shadow_normal_bias,
                };
                if self.render_3d.retained_spot_lights.get(&node).copied() != Some(light) {
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(
                        Command3D::SetSpotLight { node, light },
                    )));
                    self.render_3d.retained_spot_lights.insert(node, light);
                }
                visible_now.insert(node);
            }

            let decal_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Decal3D(decal)
                    if decal.active
                        && decal.visible
                        && effective_visible
                        && render_mask_matches(camera_render_mask, decal.render_layers) =>
                {
                    Some((
                        decal.transform,
                        decal.size,
                        (
                            decal.albedo_texture,
                            decal.normal_texture,
                            decal.emission_texture,
                        ),
                        decal.modulate,
                        decal.surface,
                        decal.distance_fade,
                        decal.sort_priority,
                    ))
                }
                _ => None,
            });
            if let Some((
                local_transform,
                size,
                (albedo_texture, normal_texture, emission_texture),
                modulate,
                surface,
                distance_fade,
                sort_priority,
            )) = decal_data
            {
                let modulate =
                    Runtime::color_modulate(modulate, self.effective_self_modulate(node));
                let global = self
                    .get_render_global_transform_3d(node)
                    .unwrap_or(local_transform);
                let decal = Decal3DState {
                    position: global.position,
                    rotation: global.rotation,
                    size: Vector3::new(
                        (size.x * global.scale.x).max(0.001),
                        (size.y * global.scale.y).max(0.001),
                        (size.z * global.scale.z).max(0.001),
                    ),
                    albedo_texture,
                    normal_texture,
                    emission_texture,
                    modulate,
                    albedo_mix: surface.albedo_mix.clamp(0.0, 1.0),
                    emission_energy: surface.emission_energy.max(0.0),
                    normal_strength: surface.normal_strength.max(0.0),
                    normal_fade: surface.normal_fade.clamp(0.0, 1.0),
                    distance_fade_begin: distance_fade.begin.max(0.0),
                    distance_fade_length: distance_fade.length.max(0.001),
                    sort_priority,
                };
                if self.render_3d.retained_decals.get(&node).copied() != Some(decal) {
                    self.render_3d.retained_decals.insert(node, decal);
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(
                        Command3D::SetDecal {
                            node,
                            decal: Box::new(decal),
                        },
                    )));
                }
                visible_now.insert(node);
            }

            let sprite_3d_data =
                self.nodes
                    .get(node)
                    .and_then(|scene_node| match &scene_node.data {
                        SceneNodeData::Sprite3D(sprite) => Some((
                            effective_visible
                                && sprite.visible
                                && render_mask_matches(camera_render_mask, sprite.render_layers),
                            sprite.transform,
                            sprite.texture,
                            sprite.size,
                            sprite.texture_region,
                            sprite.flip_x,
                            sprite.flip_y,
                            self.effective_self_modulate(node),
                        )),
                        SceneNodeData::VideoPlayer3D(video) => Some((
                            effective_visible
                                && video.visible
                                && render_mask_matches(camera_render_mask, video.render_layers),
                            video.transform,
                            video.video.texture,
                            video.size,
                            None,
                            video.flip_x,
                            video.flip_y,
                            Runtime::color_modulate(video.tint, self.effective_self_modulate(node)),
                        )),
                        _ => None,
                    });
            if let Some((
                visible,
                local_transform,
                texture,
                size,
                texture_region,
                flip_x,
                flip_y,
                modulate,
            )) = sprite_3d_data
            {
                if visible {
                    let Some(texture) = self.resolve_sprite_texture(node, texture) else {
                        self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode {
                            node,
                        }));
                        continue;
                    };
                    let transform = self
                        .get_render_global_transform_3d(node)
                        .unwrap_or(local_transform);
                    let occluded = self.world_overlay_point_occluded_3d(
                        node,
                        transform.position,
                        &overlay_camera,
                        &overlay_occluders,
                    );
                    if occluded {
                        self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode {
                            node,
                        }));
                    } else if let Some(rect) =
                        world_rect_3d(transform, size, &overlay_camera, overlay_viewport)
                    {
                        let (uv_min, uv_max) = sprite_3d_uv(texture_region, flip_x, flip_y);
                        self.queue_render_command(RenderCommand::Ui(UiCommand::UpsertImage {
                            node,
                            rect,
                            clip_rect: viewport_clip_3d(overlay_viewport),
                            texture,
                            tint: modulate,
                            uv_min,
                            uv_max,
                            scale_mode: UiImageScaleState::Stretch,
                            h_align: UiTextAlignState::Center,
                            v_align: UiTextAlignState::Center,
                            aspect_ratio: 1.0,
                            corner_radii: Default::default(),
                        }));
                        visible_now.insert(node);
                    }
                } else {
                    self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode { node }));
                }
            }

            let label_3d_data =
                self.nodes
                    .get(node)
                    .and_then(|scene_node| match &scene_node.data {
                        SceneNodeData::Label3D(label) => Some((
                            effective_visible
                                && label.visible
                                && render_mask_matches(camera_render_mask, label.render_layers),
                            label.transform,
                            label.size,
                            label.lock_orientation,
                            label.backface_cull,
                            label.visible_through_objects,
                            label.backdrop_color,
                            label.corner_radii,
                            label.padding,
                            label.text.clone(),
                            label.color,
                            label.font_size,
                            label.font.clone(),
                            label.h_align,
                            label.v_align,
                            self.effective_self_modulate(node),
                        )),
                        _ => None,
                    });
            if let Some((
                visible,
                local_transform,
                size,
                lock_orientation,
                backface_cull,
                visible_through_objects,
                backdrop_color,
                corner_radii,
                padding,
                text,
                color,
                font_size,
                font,
                h_align,
                v_align,
                modulate,
            )) = label_3d_data
            {
                if visible {
                    let transform = self
                        .get_render_global_transform_3d(node)
                        .unwrap_or(local_transform);
                    if lock_orientation
                        && backface_cull
                        && !world_rect_front_facing_3d(transform, &overlay_camera)
                    {
                        self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode {
                            node,
                        }));
                        continue;
                    }
                    {
                        // Both label kinds ride the projected-quad path with a
                        // canonical (camera-independent) layout rect, so the
                        // painter re-projects its cached tessellation instead
                        // of re-shaping text every frame; billboards get a
                        // camera-facing quad, and the quad path's near-plane
                        // clipping replaces the old rect projection (which
                        // could blow up for labels closer than the near plane).
                        let quad_transform = if lock_orientation {
                            transform
                        } else {
                            label_billboard_transform_3d(transform, &overlay_camera)
                        };
                        let Some(projected_quad) = label_projected_quad_3d(
                            quad_transform,
                            size,
                            &overlay_camera,
                            overlay_viewport,
                        ) else {
                            self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode {
                                node,
                            }));
                            continue;
                        };
                        let rect = label_3d_canonical_layout_rect(size, font_size);
                        let content_size = label_3d_content_size(rect.size, padding);
                        self.queue_render_command(RenderCommand::Ui(UiCommand::UpsertLabel {
                            node,
                            rect,
                            clip_rect: viewport_clip_3d(overlay_viewport),
                            text,
                            color: Runtime::color_modulate(color, modulate),
                            font_size: font_size.max(0.001).min(content_size[1]),
                            font,
                            wrap_width: Some(content_size[0]),
                            h_align: text_align_state_3d(h_align),
                            v_align: text_align_state_3d(v_align),
                            backdrop_color: Runtime::color_modulate(backdrop_color, modulate),
                            corner_radii: perro_render_bridge::UiCornerRadiiState {
                                tl: corner_radii.tl,
                                tr: corner_radii.tr,
                                br: corner_radii.br,
                                bl: corner_radii.bl,
                            },
                            padding: [padding.left, padding.top, padding.right, padding.bottom],
                            projected_quad: Some(projected_quad),
                            depth_test: !visible_through_objects,
                            fit_content: true,
                        }));
                        visible_now.insert(node);
                    }
                } else {
                    self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode { node }));
                }
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
                perro_structs::Color,
                Option<NodeID>,
                Option<bool>,
                LODOptions3D,
                MeshBlendOptions3D,
                (bool, bool, bool),
                bool,
                bool,
                Arc<[f32]>,
                LocalMeshInstanceData,
            );
            let mesh_header = if effective_visible {
                self.nodes
                    .get(node)
                    .and_then(|scene_node| match &scene_node.data {
                        SceneNodeData::MeshInstance3D(mesh)
                            if render_mask_matches(camera_render_mask, mesh.render_layers) =>
                        {
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
                                },
                                (mesh.flip_x, mesh.flip_y, mesh.flip_z),
                                mesh.cast_shadows,
                                mesh.receive_shadows,
                                Arc::<[f32]>::from(mesh.blend_shape_weights.clone()),
                            ))
                        }
                        SceneNodeData::MultiMeshInstance3D(mesh)
                            if render_mask_matches(camera_render_mask, mesh.render_layers) =>
                        {
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
                                },
                                (mesh.flip_x, mesh.flip_y, mesh.flip_z),
                                mesh.cast_shadows,
                                mesh.receive_shadows,
                                Arc::<[f32]>::from(mesh.blend_shape_weights.clone()),
                            ))
                        }
                        _ => None,
                    })
            } else {
                None
            };
            let mesh_header = mesh_header.and_then(
                |(
                    mesh,
                    skeleton,
                    meshlet_override,
                    lod,
                    blend,
                    flip,
                    cast_shadows,
                    receive_shadows,
                    blend_shape_weights,
                )| {
                    self.resolve_render_mesh_id(node, mesh).map(|mesh| {
                        (
                            mesh,
                            skeleton,
                            meshlet_override,
                            lod,
                            blend,
                            flip,
                            cast_shadows,
                            receive_shadows,
                            blend_shape_weights,
                        )
                    })
                },
            );
            let mesh_data: Option<LocalMeshData> = mesh_header.and_then(
                |(
                    resolved_mesh,
                    skeleton,
                    meshlet_override,
                    lod,
                    blend,
                    flip,
                    cast_shadows,
                    receive_shadows,
                    blend_shape_weights,
                )| {
                    let effective_self_modulate = self.effective_self_modulate(node);
                    self.nodes
                        .get(node)
                        .and_then(|scene_node| match &scene_node.data {
                            SceneNodeData::MeshInstance3D(_mesh) => Some((
                                resolved_mesh,
                                effective_self_modulate,
                                skeleton,
                                meshlet_override,
                                lod,
                                blend,
                                flip,
                                cast_shadows,
                                receive_shadows,
                                blend_shape_weights.clone(),
                                LocalMeshInstanceData::Single,
                            )),
                            SceneNodeData::MultiMeshInstance3D(mesh) => Some((
                                resolved_mesh,
                                effective_self_modulate,
                                skeleton,
                                meshlet_override,
                                lod,
                                blend,
                                flip,
                                cast_shadows,
                                receive_shadows,
                                blend_shape_weights.clone(),
                                LocalMeshInstanceData::Dense {
                                    instance_scale: mesh.instance_scale.max(0.0001),
                                    poses: {
                                        let signature = dense_instance_signature(&mesh.instances);
                                        if let Some(cached) =
                                            self.render_3d.dense_instance_pose_cache.get(&node)
                                            && cached.signature == signature
                                        {
                                            cached.poses.clone()
                                        } else {
                                            dense_instance_pose_scratch.clear();
                                            if dense_instance_pose_scratch.capacity()
                                                < mesh.instances.len()
                                            {
                                                dense_instance_pose_scratch.reserve(
                                                    mesh.instances.len()
                                                        - dense_instance_pose_scratch.capacity(),
                                                );
                                            }
                                            dense_instance_pose_scratch.extend(
                                                mesh.instances.iter().map(|instance| {
                                                    DenseInstancePose3D {
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
                                                            .unwrap_or_else(|| Arc::from([])),
                                                    }
                                                }),
                                            );
                                            let poses: Arc<[DenseInstancePose3D]> =
                                                Arc::from(dense_instance_pose_scratch.as_slice());
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
                            )),
                            _ => None,
                        })
                },
            );
            if let Some((
                mesh,
                effective_self_modulate,
                skeleton,
                meshlet_override,
                lod,
                blend,
                flip,
                cast_shadows,
                receive_shadows,
                blend_shape_weights,
                local_instances,
            )) = mesh_data
                && effective_visible
                && let Some((mesh, resolved_surfaces)) =
                    self.resolve_mesh_surfaces_modulated(node, mesh, effective_self_modulate)
            {
                let node_global = self
                    .get_render_global_transform_3d(node)
                    .unwrap_or(perro_structs::Transform3D::IDENTITY)
                    .to_mat4()
                    * mirror_matrix_3d(flip.0, flip.1, flip.2);
                let retained_instances = match &local_instances {
                    LocalMeshInstanceData::Single => {
                        crate::runtime::state::RetainedMeshInstanceState::Single(
                            node_global.to_cols_array_2d(),
                        )
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
                    crate::runtime::state::RetainedMeshInstanceState::Single(_) => false,
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
                    } else if build_skeleton_palette(
                        &self.nodes,
                        skeleton,
                        &mut skeleton_global_scratch,
                        &mut skeleton_palette_scratch,
                    )
                    .is_some()
                    {
                        let palette = SkeletonPalette {
                            matrices: Arc::from(skeleton_palette_scratch.as_slice()),
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
                    lod,
                    blend,
                    cast_shadows,
                    receive_shadows,
                    blend_shape_weights: blend_shape_weights.clone(),
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
                            blend_shape_weights: blend_shape_weights.clone(),
                            meshlet_override,
                            lod,
                            blend,
                            cast_shadows,
                            receive_shadows,
                        },
                        crate::runtime::state::RetainedMeshInstanceState::Single(model) => {
                            Command3D::Draw {
                                mesh,
                                surfaces: resolved_surfaces,
                                node,
                                model,
                                skeleton: skeleton_palette,
                                blend_shape_weights: blend_shape_weights.clone(),
                                meshlet_override,
                                lod,
                                blend,
                                cast_shadows,
                                receive_shadows,
                            }
                        }
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
                            blend_shape_weights: blend_shape_weights.clone(),
                            meshlet_override,
                            lod,
                            blend,
                            cast_shadows,
                            receive_shadows,
                        },
                        crate::runtime::state::RetainedMeshInstanceState::Matrices(
                            instance_mats,
                        ) => Command3D::DrawMulti {
                            mesh,
                            surfaces: resolved_surfaces,
                            node,
                            instance_mats,
                            skeleton: skeleton_palette,
                            blend_shape_weights: blend_shape_weights.clone(),
                            meshlet_override,
                            lod,
                            blend,
                            cast_shadows,
                            receive_shadows,
                        },
                    };
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(draw_command)));
                    self.render_3d.retained_mesh_draws.insert(node, draw_state);
                }
                visible_now.insert(node);
            }
            if effective_visible
                && !visible_now.contains(&node)
                && self.render_3d.retained_mesh_draws.contains_key(&node)
                && self.mesh_draw_has_pending_asset(node)
            {
                visible_now.insert(node);
            }
            let collision_shape_debug_data =
                self.nodes
                    .get(node)
                    .and_then(|scene_node| match &scene_node.data {
                        SceneNodeData::CollisionShape3D(shape)
                            if shape.debug
                                && effective_visible
                                && render_mask_matches(camera_render_mask, shape.render_layers) =>
                        {
                            Some((shape.shape.clone(), shape.transform, scene_node.parent))
                        }
                        _ => None,
                    });
            if let Some((shape, local_transform, parent)) = collision_shape_debug_data {
                let (shape, world_from_shape) = if is_physics_body_3d(self, parent) {
                    let parent_global = self
                        .get_render_global_transform_3d(parent)
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
                        .get_render_global_transform_3d(node)
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
                && self.nodes.get(node).is_some_and(|scene_node| {
                    matches!(
                        &scene_node.data,
                        SceneNodeData::ParticleEmitter3D(emitter)
                            if render_mask_matches(camera_render_mask, emitter.render_layers)
                    )
                })
            {
                let profile = resolve_particle_profile(self, &emitter_profile).unwrap_or_default();
                let lifetime_min = profile.lifetime_min.max(0.001);
                let lifetime_max = profile.lifetime_max.max(lifetime_min);
                if let Some(node_mut) = self.nodes.get_mut_untracked(node)
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
                let modulate = self.effective_self_modulate(node);
                let particle_model = self
                    .get_render_global_transform_3d(node)
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
                            color_start: Runtime::color_modulate(profile.color_start, modulate),
                            color_end: Runtime::color_modulate(profile.color_end, modulate),
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
        for node in self.render_3d.collect_removed_visible_nodes(&visible_now) {
            self.remove_retained_render_3d_node(node);
        }
        traversal_seen.clear();
        self.render_3d.traversal_seen = traversal_seen;
        self.render_3d.overlay_occluders_scratch = overlay_occluders;
        self.render_3d
            .finish_visible_pass(traversal_ids, visible_now);
        skeleton_cache.clear();
        self.render_3d.skeleton_cache_scratch = skeleton_cache;
        skeleton_global_scratch.clear();
        self.render_3d.skeleton_global_scratch = skeleton_global_scratch;
        skeleton_palette_scratch.clear();
        self.render_3d.skeleton_palette_scratch = skeleton_palette_scratch;
        dense_instance_pose_scratch.clear();
        self.render_3d.dense_instance_pose_scratch = dense_instance_pose_scratch;
    }
}
