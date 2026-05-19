//! 3D scene extraction into render bridge commands.

use super::Runtime;
use glam::{Mat4, Quat, Vec3};
use perro_ids::{MaterialID, MeshID, NodeID, parse_hashed_source_uri, string_to_u64};
use perro_nodes::{
    CameraProjection, MeshSurfaceBinding, SceneNodeData, Shape3D,
    particle_emitter_3d::{ParticleEmitterSimMode3D, ParticleType},
    water_impact_strength,
};
use perro_particle_math::compile_expression;
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, CameraProjectionState, Command3D, DenseInstancePose3D,
    LODOptions3D, Material3D, MaterialParamOverride3D, MeshBlendOptions3D, MeshSurfaceBinding3D,
    ParticlePath3D, ParticleProfile3D, ParticleRenderMode3D, ParticleSimulationMode3D,
    PointLight3DState, PointParticles3DState, RayLight3DState, RenderCommand, ResourceCommand,
    SkeletonPalette, Sky3DState, SkyTime3DState, SpotLight3DState, Water3DState,
    WaterBodyQueryState, WaterCoastlineShape3D, WaterIdleModeState, WaterImpact3D, WaterLinkState,
    WaterShapeState,
};
use perro_resource_api::sub_apis::MaterialAPI;
use perro_runtime_render::{material_3d_request, mesh_3d_request};
use perro_structs::BitMask;
use std::borrow::Cow;
use std::sync::Arc;

const PARTICLE_PATH_CACHE_MAX: usize = 256;

#[path = "three_d/helpers.rs"]
mod helpers;
use helpers::*;

impl Runtime {
    pub fn extract_render_3d_commands(&mut self) {
        self.reset_water_scan_cache_3d();
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
            }
            self.render_3d.last_camera = active_camera;
        }

        let dirty_ids = self
            .dirty
            .dirty_indices()
            .iter()
            .filter_map(|&raw_index| self.nodes.slot_get(raw_index as usize).map(|(id, _)| id))
            .collect::<Vec<_>>();
        let all_ids = self.nodes.iter().map(|(id, _)| id).collect::<Vec<_>>();
        let nodes = &self.nodes;
        let mut traversal_ids = self.render_3d.collect_traversal(
            dirty_ids,
            all_ids,
            bootstrap_scan || camera_render_mask_changed,
            |node, out| {
                if let Some(node_ref) = nodes.get(node) {
                    out.extend(node_ref.get_children_ids().iter().copied());
                }
            },
        );
        let mut traversal_seen = traversal_ids
            .iter()
            .copied()
            .collect::<ahash::AHashSet<_>>();
        let dirty_skeletons = traversal_ids
            .iter()
            .copied()
            .filter(|id| {
                self.nodes
                    .get(*id)
                    .is_some_and(|node| matches!(node.data, SceneNodeData::Skeleton3D(_)))
            })
            .collect::<ahash::AHashSet<_>>();
        if !dirty_skeletons.is_empty() {
            for (id, node) in self.nodes.iter() {
                let SceneNodeData::MeshInstance3D(mesh) = &node.data else {
                    continue;
                };
                if dirty_skeletons.contains(&mesh.skeleton) && traversal_seen.insert(id) {
                    traversal_ids.push(id);
                }
            }
        }
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
                SceneNodeData::Sky3D(sky)
                    if sky.active
                        && sky.visible
                        && effective_visible
                        && render_mask_matches(camera_render_mask, sky.render_layers) =>
                {
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
                let model = self
                    .get_global_transform_3d(node)
                    .unwrap_or(local_transform)
                    .to_mat4()
                    .to_cols_array_2d();
                let coastline_shapes = self.collect_water_coastline_shapes_3d(node, &water);
                let queries = self.collect_water_queries_3d(node);
                let impacts = self.collect_water_impacts_3d(node, &water);
                let links = self.collect_water_links_3d(node, &water);
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
                LODOptions3D,
                MeshBlendOptions3D,
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
                            ))
                        }
                        _ => None,
                    })
            } else {
                None
            };
            let mesh_header =
                mesh_header.and_then(|(mesh, skeleton, meshlet_override, lod, blend)| {
                    self.resolve_render_mesh_id(node, mesh)
                        .map(|mesh| (mesh, skeleton, meshlet_override, lod, blend))
                });
            let mesh_data: Option<LocalMeshData> =
                mesh_header.and_then(|(resolved_mesh, skeleton, meshlet_override, lod, blend)| {
                    self.nodes
                        .get(node)
                        .and_then(|scene_node| match &scene_node.data {
                            SceneNodeData::MeshInstance3D(mesh) => Some((
                                resolved_mesh,
                                mesh.surfaces.clone(),
                                skeleton,
                                meshlet_override,
                                lod,
                                blend,
                                LocalMeshInstanceData::Single,
                            )),
                            SceneNodeData::MultiMeshInstance3D(mesh) => Some((
                                resolved_mesh,
                                mesh.surfaces.clone(),
                                skeleton,
                                meshlet_override,
                                lod,
                                blend,
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
                                                            instance.0.x,
                                                            instance.0.y,
                                                            instance.0.z,
                                                        ],
                                                        rotation: [
                                                            instance.1.x,
                                                            instance.1.y,
                                                            instance.1.z,
                                                            instance.1.w,
                                                        ],
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
                });
            if let Some((mesh, surfaces, skeleton, meshlet_override, lod, blend, local_instances)) =
                mesh_data
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
                            lod,
                            blend,
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
                            lod,
                            blend,
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
                            lod,
                            blend,
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
        self.render_3d.dense_instance_pose_cache.remove(&node);
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

    fn active_render_camera_3d(&mut self) -> Option<Camera3DState> {
        let mut found = None;
        for (node, scene_node) in self.nodes.iter() {
            let SceneNodeData::Camera3D(camera) = &scene_node.data else {
                continue;
            };
            if !camera.active || !self.is_effectively_visible(node) {
                continue;
            }
            found = Some((
                node,
                camera.transform,
                camera.projection.clone(),
                camera.render_mask,
                camera.post_processing.clone(),
                camera.audio_options.clone(),
            ));
        }
        let (node, local_transform, projection, render_mask, post_processing, audio_options) =
            found?;
        let global = self
            .get_global_transform_3d(node)
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

    fn resolve_render_mesh_assets(
        &mut self,
        node: NodeID,
        mut mesh: MeshID,
        mut surfaces: Vec<MeshSurfaceBinding>,
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
                continue;
            }

            let request = material_3d_request(node, surface_index as u32);
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
                if let Some(node) = self.nodes.get_mut(node) {
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

    fn resolve_render_mesh_id(&mut self, node: NodeID, mut mesh: MeshID) -> Option<MeshID> {
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
            let request = mesh_3d_request(node);
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
        Some(mesh)
    }

    fn collect_water_coastline_shapes_3d(
        &mut self,
        water_id: NodeID,
        water: &perro_nodes::WaterSurfaceParams,
    ) -> Arc<[WaterCoastlineShape3D]> {
        let Some(water_global) = self.get_global_transform_3d(water_id) else {
            return Arc::from([]);
        };
        let water_half = water.shape.surface_size() * 0.5;
        let water_top = water_global.position.y;
        let surface_band = water.coastline.foam_width.max(0.35) * 0.65;
        let surface_epsilon = surface_band.max(0.05) * 0.2;
        let mut shapes = Vec::new();
        let body_ids: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                matches!(
                    node.data,
                    SceneNodeData::StaticBody3D(_) | SceneNodeData::RigidBody3D(_)
                )
                .then_some(id)
            })
            .collect();
        for body_id in body_ids {
            let Some((enabled, layers, mask, children, scale_bias)) =
                self.nodes.get(body_id).and_then(|node| match &node.data {
                    SceneNodeData::StaticBody3D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        node.children_slice().to_vec(),
                        1.02f32,
                    )),
                    SceneNodeData::RigidBody3D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        node.children_slice().to_vec(),
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
            let Some(_body_global) = self.get_global_transform_3d(body_id) else {
                continue;
            };
            for child_id in children {
                let Some(shape_kind) = self.nodes.get(child_id).and_then(|child| {
                    let SceneNodeData::CollisionShape3D(shape) = &child.data else {
                        return None;
                    };
                    Some(shape.shape.clone())
                }) else {
                    continue;
                };
                let Some(shape_global) = self.get_global_transform_3d(child_id) else {
                    continue;
                };
                let local = shape_global.position - water_global.position;
                if local.x.abs() > water_half.x + 512.0 || local.z.abs() > water_half.y + 512.0 {
                    continue;
                }
                let scale = shape_global.scale;
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
                            scale.x.abs(),
                            scale.y.abs(),
                            scale.z.abs(),
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
        Arc::from(shapes)
    }

    fn collect_water_queries_3d(&mut self, water_id: NodeID) -> Arc<[WaterBodyQueryState]> {
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

    fn collect_water_impacts_3d(
        &mut self,
        water_id: NodeID,
        water: &perro_nodes::WaterSurfaceParams,
    ) -> Arc<[WaterImpact3D]> {
        let Some(water_global) = self.get_global_transform_3d(water_id) else {
            return Arc::from([]);
        };
        let water_inv = water_global.to_mat4().inverse();
        let half = water.shape.surface_size() * 0.5;
        let body_ids = self.cached_rigid_body_ids_3d().to_vec();
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
            let Some(body_global) = self.get_global_transform_3d(body_id) else {
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
            if submerged <= 0.0 || submerged > target * 2.25 || rel_down <= 0.35 {
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
                    strength: (contact.foam_amount * 5.8).max(0.35),
                    radius: contact.radius,
                    cavitation: contact.foam_amount * 0.2,
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

    fn collect_water_links_3d(
        &mut self,
        water_id: NodeID,
        water: &perro_nodes::WaterSurfaceParams,
    ) -> Arc<[WaterLinkState]> {
        let Some(water_global) = self.get_global_transform_3d(water_id) else {
            return Arc::from([]);
        };
        let other_ids = self.cached_water_ids_3d().to_vec();
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
            let Some(other_global) = self.get_global_transform_3d(other_id) else {
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
        Arc::from(links)
    }
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

#[inline]
fn render_mask_matches(camera_mask: BitMask, render_layers: BitMask) -> bool {
    !camera_mask.intersects(render_layers)
}

fn water_idle_mode_state(mode: perro_nodes::WaterIdleMode) -> WaterIdleModeState {
    match mode {
        perro_nodes::WaterIdleMode::Calm => WaterIdleModeState::Calm,
        perro_nodes::WaterIdleMode::Sine => WaterIdleModeState::Sine,
        perro_nodes::WaterIdleMode::Chop => WaterIdleModeState::Chop,
        perro_nodes::WaterIdleMode::Storm => WaterIdleModeState::Storm,
        perro_nodes::WaterIdleMode::River => WaterIdleModeState::River,
    }
}

fn water_shape_state(shape: perro_nodes::WaterShape) -> WaterShapeState {
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

fn water_render_size(water: perro_nodes::WaterSurfaceParams) -> [f32; 2] {
    let size = water.shape.surface_size();
    [size.x, size.y]
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

#[cfg(test)]
#[path = "../../../tests/unit/runtime_render_3d_tests.rs"]
mod tests;
