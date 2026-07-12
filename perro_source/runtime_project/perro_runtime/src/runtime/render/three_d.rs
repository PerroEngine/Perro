//! 3D scene extraction into render bridge commands.

use super::Runtime;
use glam::{Mat4, Quat, Vec3};
use perro_ids::{
    MaterialID, MeshID, NodeID, ParticleProfileRef, parse_hashed_source_uri, string_to_u64,
};
use perro_nodes::{
    CameraProjection, MeshSurfaceBinding, SceneNodeData, Shape3D,
    particle_emitter_3d::{ParticleEmitterSimMode3D, ParticleType},
    water_impact_strength,
};
use perro_particle_math::compile_expression;
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, CameraProjectionState, CameraStream3DState,
    CameraStreamCommand, Command3D, Decal3DState, DenseInstancePose3D, LODOptions3D, Material3D,
    MaterialParamOverride3D, MeshBlendOptions3D, MeshSurfaceBinding3D, ParticlePath3D,
    ParticleProfile3D, ParticleRenderMode3D, ParticleSimulationMode3D, PointLight3DState,
    PointParticles3DState, RayLight3DState, RenderCommand, ResourceCommand, SkeletonPalette,
    Sky3DState, SkyShaderPass3DState, SkyTime3DState, SpotLight3DState, UiCommand,
    UiImageScaleState, UiRectState, UiTextAlignState, Water3DState, WaterBodyQueryState,
    WaterCoastlineShape3D, WaterIdleModeState, WaterImpact3D, WaterLinkState, WaterShapeState,
};
use perro_resource_api::sub_apis::{MaterialAPI, MeshAPI, TextureAPI};
use perro_runtime_render::{TextDecalTextureCache, material_3d_request, mesh_3d_request};
use perro_structs::{BitMask, Color, Vector2, Vector3};
use std::borrow::Cow;
use std::sync::Arc;

const PARTICLE_PATH_CACHE_MAX: usize = 256;

type Camera3DPick = (
    (u64, u32, u32),
    NodeID,
    perro_structs::Transform3D,
    CameraProjection,
    BitMask,
    perro_structs::PostProcessSet,
    perro_structs::AudioListenerOptions,
);

struct TextDecalRasterParams<'a> {
    node: NodeID,
    text: &'a str,
    size: Vector3,
    font_size: f32,
    h_align: perro_ui::UiTextAlign,
    v_align: perro_ui::UiTextAlign,
    texture_resolution: u32,
    color: Color,
    outline_width: f32,
    outline_color: Color,
}

#[inline]
fn mirror_matrix_3d(flip_x: bool, flip_y: bool, flip_z: bool) -> Mat4 {
    Mat4::from_scale(Vec3::new(
        if flip_x { -1.0 } else { 1.0 },
        if flip_y { -1.0 } else { 1.0 },
        if flip_z { -1.0 } else { 1.0 },
    ))
}

#[path = "three_d/helpers.rs"]
mod helpers;
use helpers::*;
pub(crate) use helpers::{
    build_skeleton_palette, derived_particle_budget as derived_particle_budget_3d,
    resolve_particle_profile, resolve_particle_render_mode, resolve_particle_sim_mode,
};

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
        let overlay_camera = active_camera
            .clone()
            .unwrap_or_else(fallback_camera_3d_state);
        let overlay_viewport = self.input.viewport_size();

        for node in traversal_ids.iter().copied() {
            visible_now.remove(&node);
            let effective_visible = self.is_effectively_visible(node);
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

            let text_decal_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::TextDecal3D(decal)
                    if decal.active
                        && decal.visible
                        && effective_visible
                        && render_mask_matches(camera_render_mask, decal.render_layers) =>
                {
                    Some((
                        decal.transform,
                        decal.size,
                        decal.text.clone(),
                        decal.color,
                        decal.font_size,
                        decal.h_align,
                        decal.v_align,
                        decal.texture_resolution,
                        (decal.outline_width, decal.outline_color),
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
                text,
                color,
                font_size,
                h_align,
                v_align,
                texture_resolution,
                (outline_width, outline_color),
                surface,
                distance_fade,
                sort_priority,
            )) = text_decal_data
            {
                let albedo_texture = self.text_decal_texture(TextDecalRasterParams {
                    node,
                    text: text.as_ref(),
                    size,
                    font_size,
                    h_align,
                    v_align,
                    texture_resolution,
                    color,
                    outline_width,
                    outline_color,
                });
                // Text and outline colors are baked into the raster (so the
                // outline keeps its own tint); only the color's alpha and the
                // node modulate scale the decal.
                let modulate = Runtime::color_modulate(
                    Color::new(1.0, 1.0, 1.0, color.a.to_u8() as f32 / 255.0),
                    self.effective_self_modulate(node),
                );
                let global = self
                    .get_render_global_transform_3d(node)
                    .unwrap_or(local_transform);
                let emission_texture = if surface.emission_energy > 0.0 {
                    albedo_texture
                } else {
                    perro_ids::TextureID::nil()
                };
                let decal = Decal3DState {
                    position: global.position,
                    rotation: global.rotation,
                    size: Vector3::new(
                        (size.x * global.scale.x).max(0.001),
                        (size.y * global.scale.y).max(0.001),
                        (size.z * global.scale.z).max(0.001),
                    ),
                    albedo_texture,
                    normal_texture: perro_ids::TextureID::nil(),
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
                            label.text.clone(),
                            label.color,
                            label.font_size,
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
                text,
                color,
                font_size,
                h_align,
                v_align,
                modulate,
            )) = label_3d_data
            {
                if visible {
                    let transform = self
                        .get_render_global_transform_3d(node)
                        .unwrap_or(local_transform);
                    let occluded = self.world_overlay_point_occluded_3d(
                        node,
                        transform.position,
                        &overlay_camera,
                    );
                    if occluded {
                        self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode {
                            node,
                        }));
                    } else if let Some(rect) =
                        world_rect_3d(transform, size, &overlay_camera, overlay_viewport)
                    {
                        self.queue_render_command(RenderCommand::Ui(UiCommand::UpsertLabel {
                            node,
                            rect,
                            clip_rect: viewport_clip_3d(overlay_viewport),
                            text,
                            color: Runtime::color_modulate(color, modulate),
                            font_size: font_size.max(0.001),
                            wrap_width: label_3d_wrap_width(size, font_size),
                            h_align: text_align_state_3d(h_align),
                            v_align: text_align_state_3d(v_align),
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

    fn remove_retained_render_3d_node(&mut self, node: NodeID) {
        self.render_3d.camera_activation_order.remove(&node);
        self.render_3d.dense_instance_pose_cache.remove(&node);
        self.render_3d.mesh_sources.remove(&node);
        self.render_3d.material_surface_sources.remove(&node);
        self.render_3d.material_surface_overrides.remove(&node);
        if let Some(cache) = self.render_3d.text_decal_texture_cache.remove(&node) {
            TextureAPI::drop_texture(self.resource_api.as_ref(), cache.texture);
        }
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

    fn text_decal_texture(&mut self, params: TextDecalRasterParams<'_>) -> perro_ids::TextureID {
        let signature = text_decal_signature(&params);
        if let Some(cache) = self.render_3d.text_decal_texture_cache.get(&params.node)
            && cache.signature == signature
        {
            return cache.texture;
        }
        let (rgba, width, height) = raster_text_decal(&params);
        let texture =
            TextureAPI::create_texture_from_rgba(self.resource_api.as_ref(), width, height, &rgba);
        if let Some(old) = self
            .render_3d
            .text_decal_texture_cache
            .insert(params.node, TextDecalTextureCache { signature, texture })
        {
            TextureAPI::drop_texture(self.resource_api.as_ref(), old.texture);
        }
        texture
    }

    fn world_overlay_point_occluded_3d(
        &mut self,
        node: NodeID,
        point: Vector3,
        camera: &Camera3DState,
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

        let candidates: Vec<NodeID> = self
            .nodes
            .iter()
            .filter_map(|(candidate, scene_node)| {
                if candidate == node {
                    return None;
                }
                matches!(
                    scene_node.data,
                    SceneNodeData::MeshInstance3D(_) | SceneNodeData::MultiMeshInstance3D(_)
                )
                .then_some(candidate)
            })
            .collect();
        for candidate in candidates {
            let Some((visible, layers)) = self.nodes.get(candidate).and_then(|scene_node| {
                let visible = self.is_effectively_visible(candidate);
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

    fn active_render_camera_3d(&mut self) -> Option<Camera3DState> {
        let mut found: Option<Camera3DPick> = None;
        for (node, scene_node) in self.nodes.iter() {
            let SceneNodeData::Camera3D(camera) = &scene_node.data else {
                continue;
            };
            if !camera.active || !self.is_effectively_visible(node) {
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
                    color: [0.15, 0.95, 0.95, 1.0],
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

    pub(crate) fn resolve_render_mesh_assets(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        mut surfaces: Vec<MeshSurfaceBinding>,
    ) -> Option<(MeshID, std::sync::Arc<[MeshSurfaceBinding3D]>)> {
        self.resolve_render_mesh_assets_scratch(node, mesh, &mut surfaces)
    }

    // Resolve a mesh's surface materials into a render-bridge binding list using a
    // caller-owned `surfaces` buffer. Taking `&mut Vec` lets the per-frame
    // extraction path recycle one scratch allocation instead of cloning a fresh
    // Vec per moving mesh (see resolve_mesh_surfaces_modulated).
    fn resolve_render_mesh_assets_scratch(
        &mut self,
        node: NodeID,
        mut mesh: MeshID,
        surfaces: &mut Vec<MeshSurfaceBinding>,
    ) -> Option<(MeshID, std::sync::Arc<[MeshSurfaceBinding3D]>)> {
        mesh = self.resolve_render_mesh_id(node, mesh)?;

        for surface_index in 0..surfaces.len().max(1) {
            if surfaces.len() <= surface_index {
                surfaces.push(MeshSurfaceBinding::default());
            }
            let material = surfaces[surface_index]
                .material
                .unwrap_or(MaterialID::nil());
            if !material.is_nil() {
                if self.resource_api.is_material_id_pending(material) {
                    return None;
                }
                continue;
            }

            let request = material_3d_request(node, surface_index as u32);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Material(id) => {
                        surfaces[surface_index].material = Some(id);
                        if let Some(node) = self.nodes.get_mut_untracked(node) {
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
            let material_override = self
                .render_3d
                .material_surface_overrides
                .get(&node)
                .and_then(|overrides| overrides.get(surface_index))
                .cloned()
                .flatten();
            if material_override.is_none()
                && let Some(source) = source.as_deref()
                && let Some(id) = (!source.trim().is_empty())
                    .then(|| self.resource_api.load_material_source(source))
                && !id.is_nil()
            {
                surfaces[surface_index].material = Some(id);
                if let Some(node) = self.nodes.get_mut_untracked(node) {
                    match &mut node.data {
                        SceneNodeData::MeshInstance3D(mesh_instance) => {
                            mesh_instance.set_surface_material(surface_index, Some(id));
                        }
                        SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                            mesh_instance.ensure_surface_mut(surface_index).material = Some(id);
                        }
                        _ => {}
                    }
                }
                continue;
            }

            if source.is_none() {
                let id = if let Some(material) = material_override.clone() {
                    self.resource_api.shared_inline_material_id(material)
                } else {
                    self.resource_api.default_material_id()
                };
                surfaces[surface_index].material = Some(id);
                if let Some(node) = self.nodes.get_mut_untracked(node) {
                    match &mut node.data {
                        SceneNodeData::MeshInstance3D(mesh_instance) => {
                            mesh_instance.set_surface_material(surface_index, Some(id));
                        }
                        SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                            mesh_instance.ensure_surface_mut(surface_index).material = Some(id);
                        }
                        _ => {}
                    }
                }
                continue;
            }

            let material = material_override.unwrap_or_else(Material3D::default);
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

        if self.render_3d.material_surface_sources.get(&node).is_none()
            && self
                .render_3d
                .material_surface_overrides
                .get(&node)
                .is_none()
            && surfaces.iter().all(|surface| surface.overrides.is_empty())
            && let Some(retained) = self.render_3d.retained_mesh_draws.get(&node)
            && retained.mesh == mesh
            && simple_surfaces_match(surfaces.as_slice(), &retained.surfaces)
        {
            return Some((mesh, retained.surfaces.clone()));
        }

        let converted: Vec<MeshSurfaceBinding3D> = surfaces
            .iter()
            .map(|surface| MeshSurfaceBinding3D {
                material: surface.material,
                overrides: surface
                    .overrides
                    .iter()
                    .map(|ovr| MaterialParamOverride3D {
                        name: ovr.name.clone(),
                        value: ovr.value.clone(),
                    })
                    .collect::<Vec<_>>()
                    .into(),
                modulate: surface.modulate,
            })
            .collect();
        Some((mesh, std::sync::Arc::from(converted)))
    }

    // Build the modulated surface list for `node` into a recycled scratch buffer
    // and resolve its materials. WHITE modulate skips the per-surface fold.
    fn resolve_mesh_surfaces_modulated(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        modulate: perro_structs::Color,
    ) -> Option<(MeshID, std::sync::Arc<[MeshSurfaceBinding3D]>)> {
        let mut surfaces = std::mem::take(&mut self.mesh_surface_scratch);
        surfaces.clear();
        if let Some(scene_node) = self.nodes.get(node) {
            match &scene_node.data {
                SceneNodeData::MeshInstance3D(mesh) => {
                    surfaces.extend(mesh.surfaces.iter().cloned());
                }
                SceneNodeData::MultiMeshInstance3D(mesh) => {
                    surfaces.extend(mesh.surfaces.iter().cloned());
                }
                _ => {}
            }
        }
        if modulate != perro_structs::Color::WHITE {
            for surface in &mut surfaces {
                surface.modulate = Self::color_modulate(surface.modulate, modulate);
            }
        }
        let result = self.resolve_render_mesh_assets_scratch(node, mesh, &mut surfaces);
        surfaces.clear();
        self.mesh_surface_scratch = surfaces;
        result
    }

    pub(crate) fn mesh_draw_has_pending_asset(&self, node: NodeID) -> bool {
        self.nodes
            .get(node)
            .is_some_and(|scene_node| match &scene_node.data {
                SceneNodeData::MeshInstance3D(mesh) => {
                    (!mesh.mesh.is_nil() && self.resource_api.is_mesh_id_pending(mesh.mesh))
                        || mesh.surfaces.iter().any(|surface| {
                            surface.material.is_some_and(|material| {
                                self.resource_api.is_material_id_pending(material)
                            })
                        })
                }
                SceneNodeData::MultiMeshInstance3D(mesh) => {
                    (!mesh.mesh.is_nil() && self.resource_api.is_mesh_id_pending(mesh.mesh))
                        || mesh.surfaces.iter().any(|surface| {
                            surface.material.is_some_and(|material| {
                                self.resource_api.is_material_id_pending(material)
                            })
                        })
                }
                _ => false,
            })
    }

    pub(crate) fn invalidate_3d_mesh_draws_using_material(&mut self, material: MaterialID) {
        if material.is_nil() {
            return;
        }
        let mut nodes = Vec::new();
        for (node, scene_node) in self.nodes.iter() {
            let uses_material = match &scene_node.data {
                SceneNodeData::MeshInstance3D(mesh) => mesh
                    .surfaces
                    .iter()
                    .any(|surface| surface.material == Some(material)),
                SceneNodeData::MultiMeshInstance3D(mesh) => mesh
                    .surfaces
                    .iter()
                    .any(|surface| surface.material == Some(material)),
                _ => false,
            };
            if uses_material {
                nodes.push(node);
            }
        }
        for (node, draw) in self.render_3d.retained_mesh_draws.iter() {
            if draw
                .surfaces
                .iter()
                .any(|surface| surface.material == Some(material))
                && !nodes.contains(node)
            {
                nodes.push(*node);
            }
        }
        for node in nodes {
            self.render_3d.retained_mesh_draws.remove(&node);
            self.mark_needs_rerender(node);
        }
    }

    pub(crate) fn resolve_render_mesh_id(
        &mut self,
        node: NodeID,
        mut mesh: MeshID,
    ) -> Option<MeshID> {
        let canonical = self.resource_api.canonical_mesh_id(mesh);
        if canonical != mesh {
            mesh = canonical;
            if let Some(node) = self.nodes.get_mut_untracked(node) {
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
            let request = mesh_3d_request(node);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Mesh(id) => {
                        mesh = id;
                        if let Some(node) = self.nodes.get_mut_untracked(node) {
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
        Some(mesh)
    }

    pub(crate) fn collect_water_coastline_shapes_3d(
        &mut self,
        water: &perro_nodes::WaterSurfaceParams,
        water_global: Option<perro_structs::Transform3D>,
    ) -> Arc<[WaterCoastlineShape3D]> {
        let Some(water_global) = water_global else {
            return Arc::from([]);
        };
        let water_half = water.shape.surface_size() * 0.5;
        let water_top = water_global.position.y;
        let surface_band = water.coastline.foam_width.max(0.35) * 0.65;
        let surface_epsilon = surface_band.max(0.05) * 0.2;
        let mut shapes = Vec::new();
        // cached candidate ids (static/rigid/character bodies), gated on
        // physics_revision -> no per-tick full-arena scan. take out to iterate
        // while calling &mut self transform lookups, then restore.
        self.cached_water_collision_body_ids_3d();
        let body_ids = std::mem::take(&mut self.water_collision_body_ids_3d_cache);
        for body_id in body_ids.iter().copied() {
            let Some((enabled, layers, mask, scale_bias)) =
                self.nodes.get(body_id).and_then(|node| match &node.data {
                    SceneNodeData::StaticBody3D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        1.02f32,
                    )),
                    SceneNodeData::RigidBody3D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        1.00f32,
                    )),
                    SceneNodeData::CharacterBody3D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        1.00f32,
                    )),
                    _ => None,
                })
            else {
                continue;
            };
            if !enabled
                || water.collision_mask.intersects(layers)
                || mask.intersects(water.collision_layers)
            {
                continue;
            }
            let Some(_body_global) = self.get_render_global_transform_3d(body_id) else {
                continue;
            };
            // defer children clone until after enabled/mask filter passes.
            let Some(children) = self
                .nodes
                .get(body_id)
                .map(|node| node.children_slice().to_vec())
            else {
                continue;
            };
            for child_id in children {
                let Some((shape_kind, flip)) = self.nodes.get(child_id).and_then(|child| {
                    let SceneNodeData::CollisionShape3D(shape) = &child.data else {
                        return None;
                    };
                    Some((
                        shape.shape.clone(),
                        (shape.flip_x, shape.flip_y, shape.flip_z),
                    ))
                }) else {
                    continue;
                };
                let Some(shape_global) = self.get_render_global_transform_3d(child_id) else {
                    continue;
                };
                let local = shape_global.position - water_global.position;
                if local.x.abs() > water_half.x + 512.0 || local.z.abs() > water_half.y + 512.0 {
                    continue;
                }
                let scale = shape_global.scale;
                let mesh_scale = signed_collision_shape_scale(scale, flip);
                match &shape_kind {
                    Shape3D::Cube { size }
                    | Shape3D::TriPrism { size }
                    | Shape3D::TriangularPyramid { size }
                    | Shape3D::SquarePyramid { size } => {
                        let half = perro_structs::Vector3::new(
                            size.x.abs() * scale.x.abs() * 0.5,
                            size.y.abs() * scale.y.abs() * 0.5,
                            size.z.abs() * scale.z.abs() * 0.5,
                        );
                        let min_y = shape_global.position.y - half.y;
                        let max_y = shape_global.position.y + half.y;
                        let crosses_surface = min_y <= water_top + surface_epsilon
                            && max_y >= water_top - surface_epsilon;
                        if !crosses_surface
                            || max_y < water_top - surface_band
                            || min_y > water_top + surface_band
                        {
                            continue;
                        }
                        shapes.push(WaterCoastlineShape3D::Box {
                            center: [local.x, local.y, local.z],
                            half_extents: [half.x * scale_bias, half.y, half.z * scale_bias],
                            axis_x: water_local_axis_xz(
                                water_global,
                                shape_global,
                                perro_structs::Vector3::new(1.0, 0.0, 0.0),
                            ),
                            axis_z: water_local_axis_xz(
                                water_global,
                                shape_global,
                                perro_structs::Vector3::new(0.0, 0.0, 1.0),
                            ),
                        });
                    }
                    Shape3D::Sphere { radius } => {
                        let radius =
                            radius.abs() * scale.x.abs().max(scale.y.abs()).max(scale.z.abs());
                        let min_y = shape_global.position.y - radius;
                        let max_y = shape_global.position.y + radius;
                        let crosses_surface = min_y <= water_top + surface_epsilon
                            && max_y >= water_top - surface_epsilon;
                        if !crosses_surface
                            || max_y < water_top - surface_band
                            || min_y > water_top + surface_band
                        {
                            continue;
                        }
                        shapes.push(WaterCoastlineShape3D::Sphere {
                            center: [local.x, local.y, local.z],
                            radius: radius * scale_bias,
                        });
                    }
                    Shape3D::Capsule {
                        radius,
                        half_height,
                    }
                    | Shape3D::Cylinder {
                        radius,
                        half_height,
                    }
                    | Shape3D::Cone {
                        radius,
                        half_height,
                    } => {
                        let radius = radius.abs() * scale.x.abs().max(scale.z.abs());
                        let half_height = half_height.abs() * scale.y.abs();
                        let min_y = shape_global.position.y - half_height;
                        let max_y = shape_global.position.y + half_height;
                        let crosses_surface = min_y <= water_top + surface_epsilon
                            && max_y >= water_top - surface_epsilon;
                        if !crosses_surface
                            || max_y < water_top - surface_band
                            || min_y > water_top + surface_band
                        {
                            continue;
                        }
                        shapes.push(WaterCoastlineShape3D::Cylinder {
                            center: [local.x, local.y, local.z],
                            radius: radius * scale_bias,
                            half_height,
                        });
                    }
                    Shape3D::TriMesh { source } => {
                        let source_hash = parse_hashed_source_uri(source)
                            .unwrap_or_else(|| string_to_u64(source));
                        let Some(bytes) = self
                            .project()
                            .and_then(|project| project.static_collision_trimesh_lookup)
                            .map(|lookup| lookup(source_hash))
                            .filter(|bytes| !bytes.is_empty())
                        else {
                            continue;
                        };
                        let Some((vertices, triangles)) = perro_physics::decode_pmesh_trimesh(
                            bytes,
                            mesh_scale.x,
                            mesh_scale.y,
                            mesh_scale.z,
                        ) else {
                            continue;
                        };
                        for tri in triangles {
                            let Some(a) = vertices.get(tri[0] as usize) else {
                                continue;
                            };
                            let Some(b) = vertices.get(tri[1] as usize) else {
                                continue;
                            };
                            let Some(c) = vertices.get(tri[2] as usize) else {
                                continue;
                            };
                            let ay = shape_global.position.y + a.y;
                            let by = shape_global.position.y + b.y;
                            let cy = shape_global.position.y + c.y;
                            let min_y = ay.min(by).min(cy);
                            let max_y = ay.max(by).max(cy);
                            let crosses_surface = min_y <= water_top + surface_epsilon
                                && max_y >= water_top - surface_epsilon;
                            if !crosses_surface
                                || max_y < water_top - surface_band
                                || min_y > water_top + surface_band
                            {
                                continue;
                            }
                            let centroid_x = (a.x + b.x + c.x) / 3.0;
                            let centroid_z = (a.z + b.z + c.z) / 3.0;
                            let shrink = |x: f32, y: f32, z: f32| -> [f32; 3] {
                                [
                                    local.x + centroid_x + (x - centroid_x) * scale_bias,
                                    local.y + y,
                                    local.z + centroid_z + (z - centroid_z) * scale_bias,
                                ]
                            };
                            shapes.push(WaterCoastlineShape3D::Triangle {
                                points: [
                                    shrink(a.x, a.y, a.z),
                                    shrink(b.x, b.y, b.z),
                                    shrink(c.x, c.y, c.z),
                                ],
                            });
                        }
                    }
                }
            }
        }
        self.water_collision_body_ids_3d_cache = body_ids;
        Arc::from(shapes)
    }

    pub(crate) fn collect_water_queries_3d(
        &mut self,
        water_id: NodeID,
    ) -> Arc<[WaterBodyQueryState]> {
        let Some(queries) = self.pending_water_queries_3d.get(&water_id) else {
            return Arc::from([]);
        };
        Arc::from(
            queries
                .iter()
                .map(|query| WaterBodyQueryState {
                    water: water_id,
                    body: query.body,
                    point: query.point,
                    local: [query.local.x, query.local.y],
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )
    }

    pub(crate) fn collect_water_impacts_3d(
        &mut self,
        water_id: NodeID,
        water: &perro_nodes::WaterSurfaceParams,
        water_global: Option<perro_structs::Transform3D>,
    ) -> Arc<[WaterImpact3D]> {
        let Some(water_global) = water_global else {
            return Arc::from([]);
        };
        let water_inv = water_global.to_mat4().inverse();
        let half = water.shape.surface_size() * 0.5;
        self.cached_rigid_body_ids_3d();
        let body_ids = std::mem::take(&mut self.water_rigid_body_ids_3d_cache);
        let mut impacts = Vec::new();
        for body_id in body_ids.iter().copied() {
            let Some((layers, mask, mass, density, velocity)) =
                self.nodes.get(body_id).and_then(|node| {
                    let SceneNodeData::RigidBody3D(body) = &node.data else {
                        return None;
                    };
                    Some((
                        body.collision_layers,
                        body.collision_mask,
                        body.mass,
                        body.density,
                        body.linear_velocity,
                    ))
                })
            else {
                continue;
            };
            if water.collision_mask.intersects(layers) || mask.intersects(water.collision_layers) {
                continue;
            }
            let Some(body_global) = self.get_render_global_transform_3d(body_id) else {
                continue;
            };
            let radius = mass.sqrt().max(1.0);
            let local = water_local_point_3d(water_inv, body_global.position);
            if !water
                .shape
                .contains_surface(perro_structs::Vector2::new(local.x, local.z))
                || local.y > radius
                || local.y < -water.shape.depth(water.depth)
            {
                continue;
            }
            let local_xz = perro_structs::Vector2::new(local.x, local.z);
            let cached_sample = crate::runtime::physics::lookup_water_body_sample(
                &self.water_body_samples,
                water_id,
                body_id,
                0,
                local_xz,
                self.time.elapsed,
            );
            let local = perro_structs::Vector3::new(local_xz.x, local.y, local_xz.y);
            let sample = crate::runtime::physics::water_physics_sample_for_body_cached(
                water,
                local_xz,
                self.time.elapsed,
                cached_sample,
                self.water_samples.get(&water_id).copied(),
            );
            let target = crate::runtime::physics::water_target_submerged(density);
            let submerged = sample.height - local.y;
            let rel_down = sample.velocity.y - velocity.y;
            // fast bodies cross the surface band in one tick; widen the window
            // by entry speed so high-velocity drops still register a splash
            let entry_window = (target * 2.25).max(rel_down.max(0.0) * (1.0 / 30.0) + target);
            if submerged <= 0.0 || submerged > entry_window || rel_down <= 1.1 {
                continue;
            }
            let velocity_2d = perro_structs::Vector2::new(velocity.x, velocity.z);
            let vertical_impact =
                (-velocity.y).max(0.0) * (1.0 - (local.y.abs() / radius).clamp(0.0, 1.0));
            let impact_velocity =
                perro_structs::Vector2::new(velocity_2d.length(), vertical_impact);
            let impact_strength =
                water_impact_strength(mass, impact_velocity, water.physics.wake_strength);
            let surface_contact = 1.0 - (local.y.abs() / radius).clamp(0.0, 1.0);
            let displacement_strength =
                mass.sqrt() * water.physics.wake_strength.max(0.0) * surface_contact * 0.42;
            let strength = impact_strength.max(displacement_strength.min(18.0));
            if strength <= 0.0 {
                continue;
            }
            impacts.push(WaterImpact3D {
                position: [local.x, local.y, local.z],
                velocity: [velocity.x, velocity.y, velocity.z],
                strength: strength * 1.18,
                radius: radius * 0.5,
                cavitation: (vertical_impact * 0.035 + surface_contact * 0.08).clamp(0.0, 1.0),
            });
        }
        self.water_rigid_body_ids_3d_cache = body_ids;
        for impact in self.force_water_impacts_3d.iter() {
            let local = water_local_point_3d(water_inv, impact.position);
            if local.x.abs() > half.x + impact.radius
                || local.z.abs() > half.y + impact.radius
                || local.y > impact.radius
                || local.y < -water.shape.depth(water.depth) - impact.radius
            {
                continue;
            }
            impacts.push(WaterImpact3D {
                position: [local.x, local.y, local.z],
                velocity: [impact.force.x, impact.force.y, impact.force.z],
                strength: impact.strength,
                radius: impact.radius,
                cavitation: impact.cavitation,
            });
        }
        if let Some(contacts) = self.water_contacts_3d.get(&water_id) {
            for contact in contacts {
                let local = water_local_point_3d(water_inv, contact.position);
                if local.x.abs() > half.x + contact.radius
                    || local.z.abs() > half.y + contact.radius
                    || local.y > contact.radius
                    || local.y < -water.shape.depth(water.depth) - contact.radius
                {
                    continue;
                }
                impacts.push(WaterImpact3D {
                    position: [local.x, local.y, local.z],
                    velocity: [contact.velocity.x, contact.velocity.y, contact.velocity.z],
                    strength: (contact.foam_amount * 5.8).max(0.22),
                    radius: contact.radius,
                    cavitation: (contact.foam_amount * 0.30).min(1.0),
                });
            }
        }
        for link in self.collect_water_links_3d(water_id, water).iter() {
            for impact in self.force_water_impacts_3d.iter() {
                let local = water_local_point_3d(water_inv, impact.position);
                if water
                    .shape
                    .contains_surface(perro_structs::Vector2::new(local.x, local.z))
                {
                    continue;
                }
                let pad = link.blend_width + impact.radius;
                if local.x < link.overlap_min[0] - pad
                    || local.x > link.overlap_max[0] + pad
                    || local.z < link.overlap_min[1] - pad
                    || local.z > link.overlap_max[1] + pad
                {
                    continue;
                }
                let weight =
                    water_link_overlap_weight(perro_structs::Vector2::new(local.x, local.z), link);
                if weight <= 0.0 {
                    continue;
                }
                impacts.push(WaterImpact3D {
                    position: [local.x, local.y, local.z],
                    velocity: [impact.force.x, impact.force.y, impact.force.z],
                    strength: impact.strength * link.wave_transfer * weight,
                    radius: impact.radius,
                    cavitation: impact.cavitation * weight,
                });
            }
        }
        Arc::from(impacts)
    }

    pub(crate) fn collect_water_links_3d(
        &mut self,
        water_id: NodeID,
        water: &perro_nodes::WaterSurfaceParams,
    ) -> Arc<[WaterLinkState]> {
        let Some(water_global) = self.get_render_global_transform_3d(water_id) else {
            return Arc::from([]);
        };
        self.cached_water_ids_3d();
        let other_ids = std::mem::take(&mut self.water_ids_3d_cache);
        let mut links = Vec::new();
        for other_id in other_ids.iter().copied() {
            if other_id == water_id {
                continue;
            }
            let Some(other_water) = self.nodes.get(other_id).and_then(|node| {
                let SceneNodeData::WaterBody3D(other) = &node.data else {
                    return None;
                };
                Some(other.water)
            }) else {
                continue;
            };
            let Some(other_global) = self.get_render_global_transform_3d(other_id) else {
                continue;
            };
            if water
                .link
                .link_mask
                .intersects(other_water.link.link_layers)
                || other_water
                    .link
                    .link_mask
                    .intersects(water.link.link_layers)
            {
                continue;
            }
            let Some((overlap_min, overlap_max)) =
                water_overlap_bounds_3d(water, water_global, other_water, other_global)
            else {
                continue;
            };
            let extent = (overlap_max.x - overlap_min.x).min(overlap_max.y - overlap_min.y);
            let blend_width = if water.link.blend_width > 0.0 {
                water.link.blend_width
            } else {
                (extent * 0.5).max(0.5)
            };
            links.push(WaterLinkState {
                other: other_id,
                overlap_min: [overlap_min.x, overlap_min.y],
                overlap_max: [overlap_max.x, overlap_max.y],
                blend_width,
                wave_transfer: water.link.wave_transfer.min(other_water.link.wave_transfer),
                flow_transfer: water.link.flow_transfer.min(other_water.link.flow_transfer),
            });
        }
        self.water_ids_3d_cache = other_ids;
        Arc::from(links)
    }
}

fn signed_collision_shape_scale(
    scale: perro_structs::Vector3,
    flip: (bool, bool, bool),
) -> perro_structs::Vector3 {
    perro_structs::Vector3::new(
        if flip.0 {
            -scale.x.abs()
        } else {
            scale.x.abs()
        },
        if flip.1 {
            -scale.y.abs()
        } else {
            scale.y.abs()
        },
        if flip.2 {
            -scale.z.abs()
        } else {
            scale.z.abs()
        },
    )
}

fn camera_projection_state(projection: &CameraProjection) -> CameraProjectionState {
    match projection {
        CameraProjection::Perspective {
            fov_y_degrees,
            near,
            far,
        } => CameraProjectionState::Perspective {
            fov_y_degrees: *fov_y_degrees,
            near: *near,
            far: *far,
        },
        CameraProjection::Orthographic { size, near, far } => CameraProjectionState::Orthographic {
            size: *size,
            near: *near,
            far: *far,
        },
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
    }
}

fn fallback_camera_3d_state() -> Camera3DState {
    Camera3DState {
        position: [0.0, 0.0, 6.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        projection: CameraProjectionState::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 1000.0,
        },
        render_mask: BitMask::NONE,
        post_processing: Arc::from([]),
        audio_options: perro_structs::AudioListenerOptions::new(),
    }
}

fn viewport_clip_3d(viewport: Vector2) -> [f32; 4] {
    [0.0, 0.0, viewport.x.max(1.0), viewport.y.max(1.0)]
}

fn text_align_state_3d(align: perro_ui::UiTextAlign) -> UiTextAlignState {
    match align {
        perro_ui::UiTextAlign::Start => UiTextAlignState::Start,
        perro_ui::UiTextAlign::Center => UiTextAlignState::Center,
        perro_ui::UiTextAlign::End => UiTextAlignState::End,
    }
}

fn text_decal_signature(params: &TextDecalRasterParams<'_>) -> u64 {
    let font_size = sanitize_text_decal_font_size(params.font_size);
    string_to_u64(&format!(
        "text-decal|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        params.text,
        params.size.x.to_bits(),
        params.size.y.to_bits(),
        font_size.to_bits(),
        text_decal_align_id(params.h_align),
        text_decal_align_id(params.v_align),
        params.texture_resolution,
        params.color.r.to_u8(),
        params.color.g.to_u8(),
        params.color.b.to_u8(),
        sanitize_outline_width(params.outline_width).to_bits(),
        params.outline_color.r.to_u8(),
        params.outline_color.g.to_u8(),
        params.outline_color.b.to_u8(),
        params.outline_color.a.to_u8(),
        2u8
    ))
}

fn sanitize_outline_width(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(0.0, 64.0)
    } else {
        0.0
    }
}

fn text_decal_align_id(align: perro_ui::UiTextAlign) -> u8 {
    match align {
        perro_ui::UiTextAlign::Start => 0,
        perro_ui::UiTextAlign::Center => 1,
        perro_ui::UiTextAlign::End => 2,
    }
}

fn sanitize_text_decal_font_size(font_size: f32) -> f32 {
    if font_size.is_finite() {
        font_size.max(1.0)
    } else {
        1.0
    }
}

fn raster_text_decal(params: &TextDecalRasterParams<'_>) -> (Vec<u8>, u32, u32) {
    let resolution = params.texture_resolution.clamp(16, 4096);
    let size_x = if params.size.x.is_finite() {
        params.size.x.abs().max(0.001)
    } else {
        1.0
    };
    let size_y = if params.size.y.is_finite() {
        params.size.y.abs().max(0.001)
    } else {
        1.0
    };
    let aspect = (size_x / size_y).clamp(0.0625, 16.0);
    let font_size = sanitize_text_decal_font_size(params.font_size);
    let (width, height) = if aspect >= 1.0 {
        (
            resolution,
            ((resolution as f32) / aspect).round().max(1.0) as u32,
        )
    } else {
        (
            ((resolution as f32) * aspect).round().max(1.0) as u32,
            resolution,
        )
    };
    let pixel_count = width as usize * height as usize;
    // Rasterize glyph coverage into a single-channel mask, then compose
    // colors afterwards. Keeping color out of the coverage pass lets the
    // outline dilate the same mask, and lets transparent texels carry the
    // fill/outline RGB so linear filtering never bleeds black fringes in.
    let mut mask = vec![0u8; pixel_count];
    if !params.text.is_empty() {
        if let Some(font_data) = load_text_decal_font_data()
            && let Ok(font) = ab_glyph::FontRef::try_from_slice(&font_data)
        {
            raster_text_decal_font(TextDecalFontRaster {
                mask: &mut mask,
                width,
                height,
                font: &font,
                text: params.text,
                font_size,
                h_align: params.h_align,
                v_align: params.v_align,
            });
        } else {
            raster_text_decal_blocks(
                &mut mask,
                width,
                height,
                params.text,
                params.h_align,
                params.v_align,
            );
        }
    }
    let text_rgb = color_rgb8(params.color);
    let outline_px = sanitize_outline_width(params.outline_width).round() as usize;
    let outline_alpha = params.outline_color.a.to_u8() as f32 / 255.0;
    let mut rgba = vec![0u8; pixel_count * 4];
    if outline_px == 0 || outline_alpha <= 0.0 {
        // No outline: every texel carries the fill RGB, alpha = coverage.
        for (pixel, coverage) in mask.iter().enumerate() {
            let idx = pixel * 4;
            rgba[idx] = text_rgb[0];
            rgba[idx + 1] = text_rgb[1];
            rgba[idx + 2] = text_rgb[2];
            rgba[idx + 3] = *coverage;
        }
        return (rgba, width, height);
    }
    let outline_rgb = color_rgb8(params.outline_color);
    let dilated = dilate_mask(&mask, width as usize, height as usize, outline_px);
    for pixel in 0..pixel_count {
        let fill = mask[pixel] as f32 / 255.0;
        let outline = (dilated[pixel] as f32 / 255.0) * outline_alpha;
        // Fill layer over outline layer (straight alpha "over").
        let alpha = fill + outline * (1.0 - fill);
        let idx = pixel * 4;
        if alpha <= 0.0 {
            // Transparent texels take the outline RGB (it is always the
            // outermost boundary) so filtering stays fringe-free.
            rgba[idx] = outline_rgb[0];
            rgba[idx + 1] = outline_rgb[1];
            rgba[idx + 2] = outline_rgb[2];
            continue;
        }
        let fill_weight = fill / alpha;
        let outline_weight = 1.0 - fill_weight;
        rgba[idx] = mix_channel(outline_rgb[0], text_rgb[0], fill_weight, outline_weight);
        rgba[idx + 1] = mix_channel(outline_rgb[1], text_rgb[1], fill_weight, outline_weight);
        rgba[idx + 2] = mix_channel(outline_rgb[2], text_rgb[2], fill_weight, outline_weight);
        rgba[idx + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    (rgba, width, height)
}

#[inline]
fn color_rgb8(color: Color) -> [u8; 3] {
    [color.r.to_u8(), color.g.to_u8(), color.b.to_u8()]
}

#[inline]
fn mix_channel(under: u8, over: u8, over_weight: f32, under_weight: f32) -> u8 {
    (over as f32 * over_weight + under as f32 * under_weight)
        .round()
        .clamp(0.0, 255.0) as u8
}

// Grayscale dilation with a (2r+1)² box (Chebyshev disc), as two separable
// sliding-window max passes — O(width × height) regardless of radius.
fn dilate_mask(mask: &[u8], width: usize, height: usize, radius: usize) -> Vec<u8> {
    if radius == 0 || mask.is_empty() {
        return mask.to_vec();
    }
    let mut horizontal = vec![0u8; mask.len()];
    for row in 0..height {
        sliding_window_max(
            &mask[row * width..(row + 1) * width],
            radius,
            &mut horizontal[row * width..(row + 1) * width],
        );
    }
    let mut out = vec![0u8; mask.len()];
    let mut column_in = vec![0u8; height];
    let mut column_out = vec![0u8; height];
    for col in 0..width {
        for row in 0..height {
            column_in[row] = horizontal[row * width + col];
        }
        sliding_window_max(&column_in, radius, &mut column_out);
        for row in 0..height {
            out[row * width + col] = column_out[row];
        }
    }
    out
}

// out[i] = max(input[i-radius ..= i+radius]) via a monotonic index deque.
fn sliding_window_max(input: &[u8], radius: usize, out: &mut [u8]) {
    let mut deque: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    for i in 0..input.len() + radius {
        if i < input.len() {
            while deque.back().is_some_and(|&back| input[back] <= input[i]) {
                deque.pop_back();
            }
            deque.push_back(i);
        }
        if i >= radius {
            let target = i - radius;
            while deque.front().is_some_and(|&front| front + radius < target) {
                deque.pop_front();
            }
            if let Some(&front) = deque.front() {
                out[target] = input[front];
            }
        }
    }
}

fn load_text_decal_font_data() -> Option<Vec<u8>> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let query = fontdb::Query {
        families: &[
            fontdb::Family::SansSerif,
            fontdb::Family::Name("Arial"),
            fontdb::Family::Name("DejaVu Sans"),
            fontdb::Family::Name("Noto Sans"),
        ],
        ..fontdb::Query::default()
    };
    let id = db.query(&query)?;
    let face = db.face(id)?;
    match &face.source {
        fontdb::Source::Binary(data) => Some(data.as_ref().as_ref().to_vec()),
        fontdb::Source::File(path) => std::fs::read(path).ok(),
        fontdb::Source::SharedFile(path, _) => std::fs::read(path).ok(),
    }
}

struct TextDecalFontRaster<'a, 'font> {
    mask: &'a mut [u8],
    width: u32,
    height: u32,
    font: &'font ab_glyph::FontRef<'font>,
    text: &'a str,
    font_size: f32,
    h_align: perro_ui::UiTextAlign,
    v_align: perro_ui::UiTextAlign,
}

fn raster_text_decal_font(params: TextDecalFontRaster<'_, '_>) {
    use ab_glyph::{Font, ScaleFont};

    let scale = ab_glyph::PxScale::from(
        params
            .font_size
            .clamp(1.0, params.height.max(params.width) as f32),
    );
    let scaled = params.font.as_scaled(scale);
    let line_height = scaled.height().max(params.font_size.max(1.0));
    let lines: Vec<&str> = params.text.lines().collect();
    let total_height = line_height * lines.len().max(1) as f32;
    let start_y = match params.v_align {
        perro_ui::UiTextAlign::Start => 0.0,
        perro_ui::UiTextAlign::Center => (params.height as f32 - total_height).max(0.0) * 0.5,
        perro_ui::UiTextAlign::End => (params.height as f32 - total_height).max(0.0),
    };
    for (line_index, line) in lines.iter().enumerate() {
        let mut line_width = 0.0;
        let mut prev = None;
        for ch in line.chars() {
            let glyph_id = scaled.glyph_id(ch);
            if let Some(prev_id) = prev {
                line_width += scaled.kern(prev_id, glyph_id);
            }
            line_width += scaled.h_advance(glyph_id);
            prev = Some(glyph_id);
        }
        let mut cursor_x = match params.h_align {
            perro_ui::UiTextAlign::Start => 0.0,
            perro_ui::UiTextAlign::Center => (params.width as f32 - line_width).max(0.0) * 0.5,
            perro_ui::UiTextAlign::End => (params.width as f32 - line_width).max(0.0),
        };
        let baseline = start_y + scaled.ascent() + line_index as f32 * line_height;
        let mut prev = None;
        for ch in line.chars() {
            let glyph_id = scaled.glyph_id(ch);
            if let Some(prev_id) = prev {
                cursor_x += scaled.kern(prev_id, glyph_id);
            }
            let glyph =
                glyph_id.with_scale_and_position(scale, ab_glyph::point(cursor_x, baseline));
            if let Some(outlined) = params.font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|x, y, coverage| {
                    let px = x as i32 + bounds.min.x.floor() as i32;
                    let py = y as i32 + bounds.min.y.floor() as i32;
                    if px < 0 || py < 0 || px >= params.width as i32 || py >= params.height as i32 {
                        return;
                    }
                    let idx = py as usize * params.width as usize + px as usize;
                    let alpha = (coverage * 255.0).round().clamp(0.0, 255.0) as u8;
                    params.mask[idx] = params.mask[idx].max(alpha);
                });
            }
            cursor_x += scaled.h_advance(glyph_id);
            prev = Some(glyph_id);
        }
    }
}

fn raster_text_decal_blocks(
    mask: &mut [u8],
    width: u32,
    height: u32,
    text: &str,
    h_align: perro_ui::UiTextAlign,
    v_align: perro_ui::UiTextAlign,
) {
    let lines: Vec<&str> = text.lines().collect();
    let cols = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1)
        .max(1);
    let rows = lines.len().max(1);
    let cell_w = (width as usize / cols.max(1)).max(1);
    let cell_h = (height as usize / rows.max(1)).max(1);
    let block = cell_w.min(cell_h).max(1);
    let total_h = rows * block;
    let start_y = match v_align {
        perro_ui::UiTextAlign::Start => 0,
        perro_ui::UiTextAlign::Center => (height as usize).saturating_sub(total_h) / 2,
        perro_ui::UiTextAlign::End => (height as usize).saturating_sub(total_h),
    };
    for (row, line) in lines.iter().enumerate() {
        let line_w = line.chars().count() * block;
        let start_x = match h_align {
            perro_ui::UiTextAlign::Start => 0,
            perro_ui::UiTextAlign::Center => (width as usize).saturating_sub(line_w) / 2,
            perro_ui::UiTextAlign::End => (width as usize).saturating_sub(line_w),
        };
        for (col, ch) in line.chars().enumerate() {
            if ch.is_whitespace() {
                continue;
            }
            let x0 = start_x + col * block;
            let y0 = start_y + row * block;
            let pad = (block / 6).max(1);
            for y in y0 + pad..(y0 + block).saturating_sub(pad).min(height as usize) {
                for x in x0 + pad..(x0 + block).saturating_sub(pad).min(width as usize) {
                    mask[y * width as usize + x] = 255;
                }
            }
        }
    }
}

fn label_3d_wrap_width(size: Vector2, font_size: f32) -> Option<f32> {
    if !size.x.is_finite() || !size.y.is_finite() || !font_size.is_finite() {
        return None;
    }
    let height = size.y.abs().max(0.001);
    let aspect = (size.x.abs() / height).max(1.0);
    Some((aspect * font_size.max(1.0)).max(1.0))
}

fn sprite_3d_uv(
    texture_region: Option<[f32; 4]>,
    flip_x: bool,
    flip_y: bool,
) -> ([f32; 2], [f32; 2]) {
    let (mut min, mut max) = if let Some([x, y, w, h]) = texture_region {
        ([x, y], [x + w, y + h])
    } else {
        ([0.0, 0.0], [1.0, 1.0])
    };
    if flip_x {
        std::mem::swap(&mut min[0], &mut max[0]);
    }
    if flip_y {
        std::mem::swap(&mut min[1], &mut max[1]);
    }
    (min, max)
}

fn world_rect_3d(
    transform: perro_structs::Transform3D,
    size: Vector2,
    camera: &Camera3DState,
    viewport: Vector2,
) -> Option<UiRectState> {
    let view_proj = camera_view_proj_3d(camera, viewport);
    let center = Vec3::new(
        transform.position.x,
        transform.position.y,
        transform.position.z,
    );
    let rotation = Quat::from_xyzw(
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
    );
    let rotation = if rotation.is_finite() && rotation.length_squared() > 1.0e-6 {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    };
    let right = rotation * Vec3::X * (size.x * transform.scale.x.abs() * 0.5);
    let up = rotation * Vec3::Y * (size.y * transform.scale.y.abs() * 0.5);
    let center_screen = project_world_to_ui(center, view_proj, viewport)?;
    let right_screen = project_world_to_ui(center + right, view_proj, viewport)?;
    let up_screen = project_world_to_ui(center + up, view_proj, viewport)?;
    let width = ((right_screen[0] - center_screen[0]).hypot(right_screen[1] - center_screen[1])
        * 2.0)
        .max(0.001);
    let height =
        ((up_screen[0] - center_screen[0]).hypot(up_screen[1] - center_screen[1]) * 2.0).max(0.001);
    Some(UiRectState {
        center: center_screen,
        size: [width, height],
        pivot: [0.5, 0.5],
        rotation_radians: 0.0,
        z_index: 0,
    })
}

fn project_world_to_ui(world: Vec3, view_proj: Mat4, viewport: Vector2) -> Option<[f32; 2]> {
    let clip = view_proj * world.extend(1.0);
    if !clip.is_finite() || clip.w <= 1.0e-6 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.z < -1.0 || ndc.z > 1.0 {
        return None;
    }
    Some([
        ndc.x * viewport.x.max(1.0) * 0.5,
        ndc.y * viewport.y.max(1.0) * 0.5,
    ])
}

fn camera_view_proj_3d(camera: &Camera3DState, viewport: Vector2) -> Mat4 {
    let aspect = viewport.x.max(1.0) / viewport.y.max(1.0);
    let proj = projection_matrix_3d(camera.projection, aspect);
    let pos = Vec3::from(camera.position);
    let rot = Quat::from_xyzw(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
        camera.rotation[3],
    );
    let rot = if rot.is_finite() && rot.length_squared() > 1.0e-6 {
        rot.normalize()
    } else {
        Quat::IDENTITY
    };
    proj * Mat4::from_rotation_translation(rot, pos).inverse()
}

fn projection_matrix_3d(projection: CameraProjectionState, aspect: f32) -> Mat4 {
    match projection {
        CameraProjectionState::Perspective {
            fov_y_degrees,
            near,
            far,
        } => Mat4::perspective_rh(
            perspective_fov_y_radians_3d(fov_y_degrees),
            aspect.max(1.0e-6),
            sanitize_near_3d(near),
            sanitize_far_3d(far, sanitize_near_3d(near)),
        ),
        CameraProjectionState::Orthographic { size, near, far } => {
            let half_h = if size.is_finite() {
                (size.abs() * 0.5).max(1.0e-3)
            } else {
                5.0
            };
            let half_w = half_h * aspect.max(1.0e-6);
            let near = sanitize_near_3d(near);
            let far = sanitize_far_3d(far, near);
            Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, near, far)
        }
        CameraProjectionState::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => {
            let near = sanitize_near_3d(near);
            let far = sanitize_far_3d(far, near);
            let (left, right) = sanitize_range_3d(left, right, -1.0, 1.0);
            let (bottom, top) = sanitize_range_3d(bottom, top, -1.0, 1.0);
            Mat4::frustum_rh(left, right, bottom, top, near, far)
        }
    }
}

fn perspective_fov_y_radians_3d(fov_y_degrees: f32) -> f32 {
    if fov_y_degrees.is_finite() {
        fov_y_degrees
            .to_radians()
            .clamp(10.0f32.to_radians(), 120.0f32.to_radians())
    } else {
        60.0f32.to_radians()
    }
}

fn sanitize_near_3d(near: f32) -> f32 {
    if near.is_finite() {
        near.max(1.0e-3)
    } else {
        0.1
    }
}

fn sanitize_far_3d(far: f32, near: f32) -> f32 {
    if far.is_finite() {
        far.max(near + 1.0e-3)
    } else {
        (near + 1000.0).max(near + 1.0e-3)
    }
}

fn sanitize_range_3d(min: f32, max: f32, fallback_min: f32, fallback_max: f32) -> (f32, f32) {
    let mut a = if min.is_finite() { min } else { fallback_min };
    let mut b = if max.is_finite() { max } else { fallback_max };
    if (b - a).abs() < 1.0e-6 {
        a = fallback_min;
        b = fallback_max;
    }
    if b < a {
        std::mem::swap(&mut a, &mut b);
    }
    (a, b)
}

#[inline]
fn render_mask_matches(camera_mask: BitMask, render_layers: BitMask) -> bool {
    !camera_mask.intersects(render_layers)
}

pub(crate) fn water_idle_mode_state(mode: perro_nodes::WaterIdleMode) -> WaterIdleModeState {
    match mode {
        perro_nodes::WaterIdleMode::Calm => WaterIdleModeState::Calm,
        perro_nodes::WaterIdleMode::Sine => WaterIdleModeState::Sine,
        perro_nodes::WaterIdleMode::Chop => WaterIdleModeState::Chop,
        perro_nodes::WaterIdleMode::Storm => WaterIdleModeState::Storm,
        perro_nodes::WaterIdleMode::River => WaterIdleModeState::River,
    }
}

pub(crate) fn water_shape_state(shape: perro_nodes::WaterShape) -> WaterShapeState {
    match shape {
        perro_nodes::WaterShape::Circle { radius } => WaterShapeState::Circle { radius },
        perro_nodes::WaterShape::Cylinder {
            radius,
            half_height,
        } => WaterShapeState::Cylinder {
            radius,
            half_height,
        },
        _ => WaterShapeState::Rect,
    }
}

pub(crate) fn water_render_size(water: perro_nodes::WaterSurfaceParams) -> [f32; 2] {
    let size = water.shape.surface_size();
    [size.x, size.y]
}

fn simple_surfaces_match(
    surfaces: &[MeshSurfaceBinding],
    retained: &[MeshSurfaceBinding3D],
) -> bool {
    surfaces.len() == retained.len()
        && surfaces
            .iter()
            .zip(retained.iter())
            .all(|(surface, retained)| {
                surface.material == retained.material
                    && surface.modulate == retained.modulate
                    && surface.overrides.is_empty()
                    && retained.overrides.is_empty()
            })
}

fn water_local_point_3d(
    inv_transform: glam::Mat4,
    point: perro_structs::Vector3,
) -> perro_structs::Vector3 {
    inv_transform.transform_point3(point.into()).into()
}

fn water_global_point_3d(
    transform: perro_structs::Transform3D,
    point: perro_structs::Vector3,
) -> perro_structs::Vector3 {
    transform.to_mat4().transform_point3(point.into()).into()
}

fn water_local_axis_xz(
    water_transform: perro_structs::Transform3D,
    shape_transform: perro_structs::Transform3D,
    axis: perro_structs::Vector3,
) -> [f32; 2] {
    let world_axis = shape_transform.rotation.rotate_vector3(axis);
    let local_axis = water_transform
        .rotation
        .inverse()
        .rotate_vector3(world_axis);
    let len = (local_axis.x * local_axis.x + local_axis.z * local_axis.z)
        .sqrt()
        .max(0.0001);
    [local_axis.x / len, local_axis.z / len]
}

fn water_surface_corners(size: perro_structs::Vector2) -> [perro_structs::Vector3; 4] {
    let half = size * 0.5;
    [
        perro_structs::Vector3::new(-half.x, 0.0, -half.y),
        perro_structs::Vector3::new(half.x, 0.0, -half.y),
        perro_structs::Vector3::new(-half.x, 0.0, half.y),
        perro_structs::Vector3::new(half.x, 0.0, half.y),
    ]
}

fn water_overlap_bounds_3d(
    water: &perro_nodes::WaterSurfaceParams,
    water_transform: perro_structs::Transform3D,
    other: perro_nodes::WaterSurfaceParams,
    other_transform: perro_structs::Transform3D,
) -> Option<(perro_structs::Vector2, perro_structs::Vector2)> {
    let water_inv = water_transform.to_mat4().inverse();
    let other_inv = other_transform.to_mat4().inverse();
    let mut points = Vec::new();
    for corner in water_surface_corners(other.shape.surface_size()) {
        let world = water_global_point_3d(other_transform, corner);
        let local = water_local_point_3d(water_inv, world);
        let surface = perro_structs::Vector2::new(local.x, local.z);
        if water.shape.contains_surface(surface) {
            points.push(surface);
        }
    }
    for corner in water_surface_corners(water.shape.surface_size()) {
        let world = water_global_point_3d(water_transform, corner);
        let other_local = water_local_point_3d(other_inv, world);
        if other
            .shape
            .contains_surface(perro_structs::Vector2::new(other_local.x, other_local.z))
        {
            points.push(perro_structs::Vector2::new(corner.x, corner.z));
        }
    }
    let other_center = water_local_point_3d(water_inv, other_transform.position);
    let other_center_surface = perro_structs::Vector2::new(other_center.x, other_center.z);
    if water.shape.contains_surface(other_center_surface) {
        points.push(other_center_surface);
    }
    let water_center_in_other = water_local_point_3d(other_inv, water_transform.position);
    if other.shape.contains_surface(perro_structs::Vector2::new(
        water_center_in_other.x,
        water_center_in_other.z,
    )) {
        points.push(perro_structs::Vector2::ZERO);
    }
    if points.is_empty() {
        return None;
    }
    let mut min = points[0];
    let mut max = points[0];
    for point in points.into_iter().skip(1) {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
    }
    (min.x < max.x && min.y < max.y).then_some((min, max))
}

fn water_link_overlap_weight(local: perro_structs::Vector2, link: &WaterLinkState) -> f32 {
    let cx = ((link.overlap_min[0] + link.overlap_max[0]) * 0.5 - local.x).abs();
    let cy = ((link.overlap_min[1] + link.overlap_max[1]) * 0.5 - local.y).abs();
    let hx = (link.overlap_max[0] - link.overlap_min[0]).abs() * 0.5 + link.blend_width;
    let hy = (link.overlap_max[1] - link.overlap_min[1]).abs() * 0.5 + link.blend_width;
    let edge = (1.0 - (cx / hx.max(0.001))).min(1.0 - (cy / hy.max(0.001)));
    let t = edge.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Compares a retained `Sky3DState` against a live `Sky3D` node without
/// allocating a new `Sky3DState` first. Mirrors the derived `PartialEq` on
/// `Sky3DState` field-for-field, so callers can skip the SetSky command and
/// its Arc allocations when nothing actually changed.
fn sky_3d_state_matches(retained: &Sky3DState, sky: &perro_nodes::Sky3D) -> bool {
    retained.day_colors[..] == sky.palette.day_colors[..]
        && retained.evening_colors[..] == sky.palette.evening_colors[..]
        && retained.night_colors[..] == sky.palette.night_colors[..]
        && retained.horizon_colors[..] == sky.palette.horizon_colors[..]
        && retained.time.time_of_day == sky.time.time_of_day
        && retained.time.paused == sky.time.paused
        && retained.time.scale == sky.time.scale
        && retained.shaders.len() == sky.shaders.len()
        && retained
            .shaders
            .iter()
            .zip(sky.shaders.iter())
            .all(|(retained_shader, shader)| {
                retained_shader.path == shader.path
                    && retained_shader.params[..] == shader.params[..]
            })
}

#[cfg(test)]
#[path = "../../../tests/unit/runtime_render_3d_tests.rs"]
mod tests;
