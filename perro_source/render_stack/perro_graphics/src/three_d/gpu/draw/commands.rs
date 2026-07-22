use super::*;

impl Gpu3D {
    pub(in super::super) fn rebuild_batch_views(&mut self) {
        self.opaque_batch_indices.clear();
        self.alpha_batch_indices.clear();
        self.mesh_blend_batch_indices.clear();
        self.overlay_batch_indices.clear();
        self.shadow_batch_indices.clear();
        self.depth_prepass_batch_indices.clear();
        self.mesh_blend_depth_batch_indices.clear();
        self.perf_counters.draw_batches = self.draw_batches.len() as u32;
        let mut has_shadow_casters = false;
        let mut mesh_blend_depth_active = false;
        for (index, batch) in self.draw_batches.iter().enumerate() {
            match batch.render_state.batch_kind {
                RenderBatchKind::Opaque => self.opaque_batch_indices.push(index),
                RenderBatchKind::Alpha => self.alpha_batch_indices.push(index),
                RenderBatchKind::MeshBlend => self.mesh_blend_batch_indices.push(index),
                RenderBatchKind::Overlay => self.overlay_batch_indices.push(index),
            }
            if !batch.draw_on_top && batch.casts_shadows && batch.alpha_mode != 2 {
                has_shadow_casters = true;
            }
            if batch.mesh_blend {
                mesh_blend_depth_active = true;
            }
            // Opaque (0) and cutout (1) feed depth; the depth shaders discard
            // below the cutoff for mode 1. Blend (2) stays out. Custom
            // materials qualify only when hook-free and opaque (see
            // batch_depth_safe); pipeline_for_batch's prepass_covered
            // predicate mirrors the prepass condition below.
            let derived_depth_safe = batch_depth_safe(batch, &self.custom_pipeline_vertex_hooks);
            if batch_casts_into_shadow_map(batch, &self.custom_pipeline_vertex_hooks) {
                self.shadow_batch_indices.push(index);
            }
            if derived_depth_safe
                && !batch.draw_on_top
                && batch.alpha_mode != 2
                && !batch.mesh_blend
            {
                self.depth_prepass_batch_indices.push(index);
            }
            if derived_depth_safe
                && !batch.draw_on_top
                && batch.alpha_mode != 2
                && !batch.mesh_blend
                && batch.mesh_blend_depth
            {
                self.mesh_blend_depth_batch_indices.push(index);
            }
        }
        if !mesh_blend_depth_active {
            mesh_blend_depth_active = self.multimesh_batches.iter().any(|batch| batch.mesh_blend);
        }
        if !has_shadow_casters {
            // Multimesh casters render into shadow layers too; mesh_blend
            // batches are excluded (matching the rigid alpha_mode==2 exclusion).
            has_shadow_casters = self
                .multimesh_batches
                .iter()
                .any(|batch| batch.casts_shadows && !batch.mesh_blend);
        }
        self.has_shadow_casters = has_shadow_casters;
        self.mesh_blend_depth_active = mesh_blend_depth_active;
    }
}

#[inline]
pub(in super::super) fn debug_color(seed: u64) -> [f32; 4] {
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;

    let h = ((x & 0xFFFF) as f32) / 65535.0;
    hsv_to_rgb(h, 0.75, 0.95)
}

pub(in super::super) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 4] {
    let h = (h.fract() * 6.0).max(0.0);
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [r, g, b, 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_ids::{MeshID, NodeID};
    use perro_structs::BitMask;
    use std::sync::Arc;

    pub(super) fn draw(
        node: u64,
        layers: BitMask,
        mask: BitMask,
        instances: usize,
    ) -> Draw3DInstance {
        Draw3DInstance {
            node: NodeID::from_parts(node as u32, 0),
            kind: Draw3DKind::Mesh(MeshID::from_parts(1, 0)),
            surfaces: Arc::from([]),
            instance_mats: (0..instances)
                .map(|_| glam::Mat4::IDENTITY.to_cols_array_2d())
                .collect::<Vec<_>>()
                .into(),
            blend_shape_weights: Arc::from([]),
            debug_color: None,
            skeleton: None,
            dense_multimesh: None,
            meshlet_override: None,
            lod: LODOptions3D::default(),
            blend: MeshBlendOptions3D {
                enabled: true,
                screen_blending: true,
                normal_blending: false,
                blend_layers: layers,
                blend_mask: mask,
                distance: 0.25,
                min_distance: 0.0,
                noise_factor: 0.0,
                noise_scale: 1.0,
            },
            cast_shadows: true,
            receive_shadows: true,
        }
    }

    pub(super) fn assert_rgba_near(actual: [f32; 4], expected: [f32; 4]) {
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1.0e-5, "{actual:?} vs {expected:?}");
        }
    }

    pub(super) fn shadow_test_batch(
        material_kind: MaterialPipelineKind,
        alpha_mode: u8,
    ) -> DrawBatch {
        let state_key = draw_batch_state_key(
            RenderPath3D::Rigid,
            false,
            false,
            alpha_mode,
            false,
            &material_kind,
        );
        let material_texture_key = MaterialTextureKey::from_base(0);
        DrawBatch {
            state_key,
            render_state: render_state_key(
                state_key,
                material_texture_key.state_hash(),
                0,
                0,
                false,
                alpha_mode,
                false,
            ),
            mesh: perro_graphics_assets::MeshRange {
                index_start: 0,
                index_count: 3,
                base_vertex: 0,
            },
            instance_start: 0,
            instance_count: 1,
            path: RenderPath3D::Rigid,
            packed_lod: false,
            double_sided: false,
            material_kind,
            alpha_mode,
            draw_on_top: false,
            base_color_texture_slot: 0,
            material_texture_key,
            local_center: [0.0, 0.0, 0.0],
            local_radius: 2.0,
            occlusion_query: None,
            disable_hiz_occlusion: false,
            casts_shadows: true,
            receives_shadows: true,
            mesh_blend: false,
            mesh_blend_screen: false,
            mesh_blend_params: 0,
            mesh_blend_depth: false,
            blend_layers: BitMask::ALL.bits(),
            blend_mask: BitMask::NONE.bits(),
            order_index: 0,
        }
    }

    #[test]
    pub(super) fn custom_material_shadow_casting_follows_vertex_hook_and_alpha() {
        let mut hooks = AHashMap::new();
        hooks.insert(7u32, false); // hook-free custom shader
        hooks.insert(9u32, true); // shader defines shade_vertex

        // Built-ins cast in opaque and cutout modes, unchanged.
        let standard = shadow_test_batch(MaterialPipelineKind::Standard, 0);
        assert!(batch_casts_into_shadow_map(&standard, &hooks));
        let standard_mask = shadow_test_batch(MaterialPipelineKind::Standard, 1);
        assert!(batch_casts_into_shadow_map(&standard_mask, &hooks));

        // (a) Opaque custom without a vertex hook now casts.
        let custom_plain = shadow_test_batch(MaterialPipelineKind::Custom(7), 0);
        assert!(batch_casts_into_shadow_map(&custom_plain, &hooks));
        assert!(batch_depth_safe(&custom_plain, &hooks));

        // (b) A shade_vertex custom stays out: the depth-only pass would
        // render its undisplaced geometry.
        let custom_hooked = shadow_test_batch(MaterialPipelineKind::Custom(9), 0);
        assert!(!batch_casts_into_shadow_map(&custom_hooked, &hooks));
        assert!(!batch_depth_safe(&custom_hooked, &hooks));

        // (c) Mask-mode custom stays out: its fragment alpha can diverge from
        // the base-texture cutout the shared depth shaders replicate.
        let custom_mask = shadow_test_batch(MaterialPipelineKind::Custom(7), 1);
        assert!(!batch_casts_into_shadow_map(&custom_mask, &hooks));

        // Unknown token (pipeline never ensured): conservative, no cast.
        let custom_unknown = shadow_test_batch(MaterialPipelineKind::Custom(42), 0);
        assert!(!batch_casts_into_shadow_map(&custom_unknown, &hooks));
    }

    #[test]
    pub(super) fn modulate_white_passthrough() {
        let base = [0.8, 0.3, 0.1, 0.9];
        assert_rgba_near(modulate_color_mix(base, [1.0; 4]), base);

        let mut material = Material3D::Standard(perro_render_bridge::StandardMaterial3D {
            base_color_factor: base,
            ..Default::default()
        });
        assert!(!apply_modulate(&mut material, perro_structs::Color::WHITE));
        assert_eq!(material.standard_params().base_color_factor, base);
    }

    #[test]
    pub(super) fn modulate_bias_flag_tracks_chromatic_modulate() {
        let mut material = Material3D::default();
        assert!(!apply_modulate(
            &mut material,
            perro_structs::Color::new(0.5, 0.5, 0.5, 1.0)
        ));
        let mut material = Material3D::default();
        assert!(apply_modulate(
            &mut material,
            perro_structs::Color::new(0.2, 1.0, 0.2, 1.0)
        ));

        let args = BuildInstanceArgs {
            debug_view: false,
            debug_color: [1.0, 1.0, 1.0, 1.0],
            mesh_blend: ResolvedMeshBlend::default(),
            skeleton_start: 0,
            skeleton_count: 0,
            custom_params_offset: 0,
            custom_params_len: 0,
            packed_lod_param_id: 0,
            receive_shadows: true,
            modulate_bias: true,
        };
        let built = build_instance(glam::Mat4::IDENTITY.to_cols_array_2d(), &material, args);
        let flags = (built.rigid_meta.material.packed_material_params >> 3) & 0x1fff;
        assert_ne!(flags & MATERIAL_FLAG_MODULATE_BIAS, 0);
    }

    #[test]
    pub(super) fn standard_texture_slots_set_all_gpu_sample_flags() {
        let material = Material3D::Standard(perro_render_bridge::StandardMaterial3D {
            base_color_texture: 1,
            metallic_roughness_texture: 2,
            normal_texture: 3,
            occlusion_texture: 4,
            emissive_texture: 5,
            ..Default::default()
        });
        let built = build_instance(
            glam::Mat4::IDENTITY.to_cols_array_2d(),
            &material,
            BuildInstanceArgs {
                debug_view: false,
                debug_color: [1.0; 4],
                mesh_blend: ResolvedMeshBlend::default(),
                skeleton_start: 0,
                skeleton_count: 0,
                custom_params_offset: 0,
                custom_params_len: 0,
                packed_lod_param_id: 0,
                receive_shadows: true,
                modulate_bias: false,
            },
        );
        let flags = (built.rigid_meta.material.packed_material_params >> 3) & 0x1fff;
        let texture_flags = MATERIAL_FLAG_HAS_BASE_COLOR_TEXTURE
            | MATERIAL_FLAG_HAS_METALLIC_ROUGHNESS_TEXTURE
            | MATERIAL_FLAG_HAS_NORMAL_TEXTURE
            | MATERIAL_FLAG_HAS_OCCLUSION_TEXTURE
            | MATERIAL_FLAG_HAS_EMISSIVE_TEXTURE;
        assert_eq!(flags & texture_flags, texture_flags);
    }

    #[test]
    pub(super) fn modulate_grey_stays_pure_multiply() {
        let out = modulate_color_mix([0.8, 0.3, 0.1, 1.0], [0.5, 0.5, 0.5, 1.0]);
        assert_rgba_near(out, [0.4, 0.15, 0.05, 1.0]);
    }

    #[test]
    pub(super) fn modulate_white_base_takes_modulate_color_exactly() {
        let green = [0.0, 1.0, 0.0, 1.0];
        assert_rgba_near(modulate_color_mix([1.0; 4], green), green);
    }

    #[test]
    pub(super) fn modulate_opposing_hue_biases_toward_modulate() {
        // Red base x green modulate: pure multiply collapses to black;
        // bias keeps a hint of green at the base color's luminance.
        let out = modulate_color_mix([1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0]);
        assert!(out[0].abs() < 1.0e-5 && out[2].abs() < 1.0e-5);
        assert!(out[1] > 0.0, "green channel survives: {out:?}");
        assert!(out[1] < 0.1, "bias stays slight: {out:?}");
        assert_rgba_near(out, [0.0, MODULATE_TINT_BIAS * 0.2126, 0.0, 1.0]);
    }

    #[test]
    pub(super) fn modulate_alpha_multiplies_straight() {
        let out = modulate_color_mix([1.0, 1.0, 1.0, 0.5], [0.0, 1.0, 0.0, 0.5]);
        assert!((out[3] - 0.25).abs() < 1.0e-5);
    }

    #[test]
    pub(super) fn blend_resolve_requires_matching_target() {
        let draws = [draw(1, BitMask::with([1]), BitMask::without([2]), 1)];
        let mut out = Vec::new();
        resolve_mesh_blends(&draws, &mut out);
        assert!(!resolved_mesh_blend_active(out[0]));

        let draws = [
            draw(1, BitMask::with([2]), BitMask::NONE, 1),
            draw(2, BitMask::with([1]), BitMask::without([2]), 1),
        ];
        resolve_mesh_blends(&draws, &mut out);
        assert!(resolved_mesh_blend_active(out[0]));
        assert!(resolved_mesh_blend_active(out[1]));
        assert!(resolved_mesh_blend_depth_receiver(out[0]));
        assert!(resolved_mesh_blend_depth_receiver(out[1]));

        let draws = [
            draw(1, BitMask::with([1]), BitMask::without([2]), 1),
            draw(2, BitMask::with([2]), BitMask::without([1]), 1),
        ];
        resolve_mesh_blends(&draws, &mut out);
        assert!(resolved_mesh_blend_active(out[0]));
        assert!(resolved_mesh_blend_active(out[1]));
    }

    #[test]
    pub(super) fn blend_resolve_respects_default_all_layers() {
        let mut draws = [
            draw(1, BitMask::ALL, BitMask::NONE, 1),
            draw(2, BitMask::with([2]), BitMask::NONE, 1),
        ];
        draws[0].blend.enabled = false;

        let mut out = Vec::new();
        resolve_mesh_blends(&draws, &mut out);

        assert!(resolved_mesh_blend_active(out[1]));
        assert!(resolved_mesh_blend_depth_receiver(out[0]));
    }

    #[test]
    pub(super) fn blend_resolve_uses_receiver_layers_without_receiver_fade() {
        let mut draws = [
            draw(1, BitMask::with([1]), BitMask::NONE, 1),
            draw(2, BitMask::with([2]), BitMask::without([1]), 1),
        ];
        draws[0].blend.enabled = false;
        let mut out = Vec::new();
        resolve_mesh_blends(&draws, &mut out);
        assert!(!resolved_mesh_blend_active(out[0]));
        assert!(resolved_mesh_blend_active(out[1]));
    }

    #[test]
    pub(super) fn blend_resolve_treats_all_mask_as_ignore_all() {
        let draws = [
            draw(1, BitMask::with([1]), BitMask::ALL, 1),
            draw(2, BitMask::with([2]), BitMask::NONE, 1),
        ];
        let mut out = Vec::new();
        resolve_mesh_blends(&draws, &mut out);
        assert!(!resolved_mesh_blend_active(out[0]));
        assert!(!resolved_mesh_blend_active(out[1]));
        assert!(
            !MeshBlendOptions3D {
                enabled: true,
                screen_blending: true,
                normal_blending: false,
                blend_layers: BitMask::with([1]),
                blend_mask: BitMask::ALL,
                distance: 0.25,
                min_distance: 0.0,
                noise_factor: 0.0,
                noise_scale: 1.0,
            }
            .active()
        );
    }

    #[test]
    pub(super) fn blend_resolve_allows_multimesh_self_interaction() {
        let draws = [draw(1, BitMask::with([3]), BitMask::NONE, 2)];
        let mut out = Vec::new();
        resolve_mesh_blends(&draws, &mut out);
        assert!(resolved_mesh_blend_active(out[0]));
    }

    #[test]
    pub(super) fn blend_resolve_bucket_path_handles_large_sparse_layers() {
        let mut draws = Vec::new();
        for i in 0..300 {
            draws.push(draw(
                i,
                BitMask::with([((i % 8) + 1) as u8]),
                BitMask::NONE,
                1,
            ));
        }
        let mut out = Vec::new();
        resolve_mesh_blends(&draws, &mut out);

        assert_eq!(out.len(), draws.len());
        assert!(out.iter().any(|blend| resolved_mesh_blend_active(*blend)));
        assert!(
            out.iter()
                .any(|blend| resolved_mesh_blend_depth_receiver(*blend))
        );
    }

    #[test]
    pub(super) fn blend_resolve_preserves_normal_blending_flag() {
        let mut draws = [
            draw(1, BitMask::with([1]), BitMask::NONE, 1),
            draw(2, BitMask::with([2]), BitMask::NONE, 1),
        ];
        draws[0].blend.enabled = false;
        draws[1].blend.normal_blending = true;

        let mut out = Vec::new();
        resolve_mesh_blends(&draws, &mut out);

        assert!(!resolved_mesh_blend_active(out[0]));
        assert!(resolved_mesh_blend_active(out[1]));
        assert!(resolved_mesh_blend_normal_blending(out[1]));
    }

    #[test]
    pub(super) fn blend_resolve_keeps_normal_blending_opt_in() {
        let draws = [
            draw(1, BitMask::with([1]), BitMask::NONE, 1),
            draw(2, BitMask::with([2]), BitMask::NONE, 1),
        ];
        let mut out = Vec::new();
        resolve_mesh_blends(&draws, &mut out);

        assert!(resolved_mesh_blend_active(out[1]));
        assert!(!resolved_mesh_blend_normal_blending(out[1]));
    }

    #[test]
    pub(super) fn blend_resolve_uses_source_params() {
        let mut draws = [
            draw(1, BitMask::with([1]), BitMask::NONE, 1),
            draw(2, BitMask::with([2]), BitMask::NONE, 1),
        ];
        draws[0].blend.distance = 1.0;
        draws[0].blend.min_distance = 0.2;
        draws[0].blend.noise_factor = 0.4;
        draws[0].blend.noise_scale = 8.0;
        draws[1].blend.distance = 3.0;
        draws[1].blend.min_distance = 0.6;
        draws[1].blend.noise_factor = 0.8;
        draws[1].blend.noise_scale = 24.0;

        let mut out = Vec::new();
        resolve_mesh_blends(&draws, &mut out);

        assert_eq!(
            out[1].packed_params,
            pack_u8_lanes(
                quantize_unorm8_range(3.0, 16.0),
                quantize_unorm8_range(0.6, 16.0),
                quantize_unorm8(0.8),
                quantize_unorm8_range(24.0, 64.0),
            )
        );
    }

    #[test]
    pub(super) fn material_params_sets_normal_blend_flag_only_when_resolved() {
        let material = perro_render_bridge::Material3D::default();
        let base_args = BuildInstanceArgs {
            debug_view: false,
            debug_color: [1.0, 1.0, 1.0, 1.0],
            mesh_blend: ResolvedMeshBlend {
                packed_params: 1,
                packed_flags: RESOLVED_MESH_BLEND_ACTIVE
                    | RESOLVED_MESH_BLEND_SCREEN_BLEND
                    | RESOLVED_MESH_BLEND_NORMAL_BLEND,
                depth_receiver: false,
            },
            skeleton_start: 0,
            skeleton_count: 0,
            custom_params_offset: 0,
            custom_params_len: 0,
            packed_lod_param_id: 0,
            receive_shadows: true,
            modulate_bias: false,
        };
        let built = build_instance(
            glam::Mat4::IDENTITY.to_cols_array_2d(),
            &material,
            base_args,
        );
        let flags = (built.rigid_meta.material.packed_material_params >> 3) & 0x1fff;
        assert_ne!(flags & MATERIAL_FLAG_MESH_BLEND, 0);
        assert_ne!(flags & MATERIAL_FLAG_NORMAL_BLEND, 0);

        let inactive = BuildInstanceArgs {
            mesh_blend: ResolvedMeshBlend {
                packed_params: 1,
                packed_flags: RESOLVED_MESH_BLEND_NORMAL_BLEND,
                depth_receiver: false,
            },
            ..base_args
        };
        let built = build_instance(glam::Mat4::IDENTITY.to_cols_array_2d(), &material, inactive);
        let flags = (built.rigid_meta.material.packed_material_params >> 3) & 0x1fff;
        assert_eq!(flags & MATERIAL_FLAG_NORMAL_BLEND, 0);
    }

    #[test]
    pub(super) fn material_params_allow_normal_blend_without_screen_alpha() {
        let material = perro_render_bridge::Material3D::default();
        let built = build_instance(
            glam::Mat4::IDENTITY.to_cols_array_2d(),
            &material,
            BuildInstanceArgs {
                debug_view: false,
                debug_color: [1.0, 1.0, 1.0, 1.0],
                mesh_blend: ResolvedMeshBlend {
                    packed_params: 1,
                    packed_flags: RESOLVED_MESH_BLEND_ACTIVE | RESOLVED_MESH_BLEND_NORMAL_BLEND,
                    depth_receiver: false,
                },
                skeleton_start: 0,
                skeleton_count: 0,
                custom_params_offset: 0,
                custom_params_len: 0,
                packed_lod_param_id: 0,
                receive_shadows: true,
                modulate_bias: false,
            },
        );
        let flags = (built.rigid_meta.material.packed_material_params >> 3) & 0x1fff;
        assert_eq!(flags & MATERIAL_FLAG_MESH_BLEND, 0);
        assert_ne!(flags & MATERIAL_FLAG_NORMAL_BLEND, 0);
    }

    pub(super) fn dense_pose(pos: [f32; 3]) -> perro_render_bridge::DenseInstancePose3D {
        perro_render_bridge::DenseInstancePose3D {
            position: pos,
            scale: [1.0, 1.0, 1.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            has_blend_shape_weight_override: false,
            blend_shape_weights: Arc::from([]),
        }
    }

    pub(super) fn multimesh_draw(
        node: u64,
        node_model: [[f32; 4]; 4],
        instances: Arc<[perro_render_bridge::DenseInstancePose3D]>,
    ) -> Draw3DInstance {
        let mut d = draw(node, BitMask::NONE, BitMask::NONE, 0);
        d.blend.enabled = false;
        d.instance_mats = Arc::from([]);
        d.dense_multimesh = Some(DenseMultiMeshDraw3D {
            node_model,
            instance_scale: 1.0,
            instances,
        });
        d
    }

    #[test]
    pub(super) fn transform_only_scene_takes_fast_path_w_multimesh_and_moved_regular() {
        // Scene: one dense multimesh (unchanged poses, same Arc) + one regular
        // single-instance draw whose model moved. Expect the transform-only
        // fast path to be taken, classifying each draw correctly.
        let poses: Arc<[_]> = Arc::from([dense_pose([0.0, 0.0, 0.0]), dense_pose([1.0, 0.0, 0.0])]);
        let identity = glam::Mat4::IDENTITY.to_cols_array_2d();
        let moved = glam::Mat4::from_translation(glam::Vec3::new(5.0, 0.0, 0.0)).to_cols_array_2d();

        let prev = vec![
            multimesh_draw(1, identity, poses.clone()),
            draw(2, BitMask::NONE, BitMask::NONE, 1),
        ];
        let mut next = vec![
            // Node moved: node_model differs but same pose Arc.
            multimesh_draw(1, moved, poses.clone()),
            draw(2, BitMask::NONE, BitMask::NONE, 1),
        ];
        next[1].instance_mats = Arc::from([moved]);
        next[0].blend.enabled = false;
        next[1].blend.enabled = false;
        // prev[1] blend was left enabled by `draw`; disable to match next.
        // (blend must be equal for same_draw_except_model.)
        let mut prev = prev;
        prev[1].blend.enabled = false;
        prev[0].blend.enabled = false;

        let mut kinds = Vec::new();
        assert!(classify_transform_only_scene(&prev, &next, &mut kinds));
        assert_eq!(
            kinds,
            vec![
                TransformOnlyDrawKind::Multimesh,
                TransformOnlyDrawKind::RegularSingle,
            ]
        );
    }

    #[test]
    pub(super) fn transform_only_scene_falls_back_when_multimesh_poses_change() {
        let poses_a: Arc<[_]> = Arc::from([dense_pose([0.0, 0.0, 0.0])]);
        let poses_b: Arc<[_]> = Arc::from([dense_pose([9.0, 0.0, 0.0])]);
        let identity = glam::Mat4::IDENTITY.to_cols_array_2d();
        let prev = vec![multimesh_draw(1, identity, poses_a)];
        let next = vec![multimesh_draw(1, identity, poses_b)];
        let mut kinds = Vec::new();
        // Different pose contents (and different Arc) force a full rebuild.
        assert!(!classify_transform_only_scene(&prev, &next, &mut kinds));
        assert!(kinds.is_empty());
    }

    #[test]
    pub(super) fn same_dense_instances_hits_arc_ptr_fast_path() {
        let poses: Arc<[_]> = Arc::from([dense_pose([0.0, 0.0, 0.0])]);
        let a = DenseMultiMeshDraw3D {
            node_model: glam::Mat4::IDENTITY.to_cols_array_2d(),
            instance_scale: 1.0,
            instances: poses.clone(),
        };
        let b = DenseMultiMeshDraw3D {
            node_model: glam::Mat4::from_translation(glam::Vec3::X).to_cols_array_2d(),
            instance_scale: 1.0,
            instances: poses.clone(),
        };
        // node_model differs but instances share the Arc: patchable.
        assert!(same_dense_instances(&a, &b));
    }

    #[test]
    pub(super) fn draws_semantically_unchanged_accepts_noisy_revision_bump() {
        let prev = vec![draw(1, BitMask::NONE, BitMask::NONE, 1)];
        let next = prev.clone();
        assert!(draws_semantically_unchanged(10, 11, &prev, &next));
    }

    #[test]
    pub(super) fn draws_semantically_unchanged_rejects_cold_empty_cache() {
        assert!(!draws_semantically_unchanged(u64::MAX, 1, &[], &[]));
    }

    #[test]
    pub(super) fn draws_semantically_unchanged_rejects_data_change() {
        let prev = vec![draw(1, BitMask::NONE, BitMask::NONE, 1)];
        let next = vec![draw(2, BitMask::NONE, BitMask::NONE, 1)];
        assert!(!draws_semantically_unchanged(10, 11, &prev, &next));
    }

    pub(super) fn meshlet_push(
        index_start: u32,
        instance_start: u32,
        instance_count: u32,
    ) -> DrawBatchPush {
        DrawBatchPush {
            render_path: RenderPath3D::Rigid,
            mesh: MeshRange {
                index_start,
                index_count: 12,
                base_vertex: 0,
            },
            instance_start,
            instance_count,
            double_sided: false,
            packed_lod: false,
            material_kind: MaterialPipelineKind::Standard,
            alpha_mode: 0,
            base_color_texture_slot: 0,
            material_texture_key: MaterialTextureKey::from_base(0),
            local_bounds: ([0.0, 0.0, 0.0], 1.0),
            occlusion_query: None,
            disable_hiz_occlusion: false,
            casts_shadows: true,
            receives_shadows: true,
            mesh_blend: false,
            mesh_blend_screen: false,
            mesh_blend_params: 0,
            mesh_blend_depth: false,
            blend_layers: 0,
            blend_mask: 0,
        }
    }

    #[test]
    pub(super) fn push_draw_batch_never_merges_shared_span_meshlet_batches() {
        // Meshlet batches of one draw share the same instance span but differ by
        // mesh.index_start. They must NOT merge: same_mesh is false, and their
        // regions are not adjacent (prev_end != instance_start), so each stays a
        // distinct batch pointing at the shared span.
        let mut batches = Vec::new();
        push_draw_batch(&mut batches, meshlet_push(0, 0, 1));
        push_draw_batch(&mut batches, meshlet_push(30, 0, 1));
        push_draw_batch(&mut batches, meshlet_push(60, 0, 1));
        assert_eq!(batches.len(), 3);
        for batch in &batches {
            assert_eq!(batch.instance_start, 0);
            assert_eq!(batch.instance_count, 1);
        }
        assert_eq!(batches[0].mesh.index_start, 0);
        assert_eq!(batches[1].mesh.index_start, 30);
        assert_eq!(batches[2].mesh.index_start, 60);

        // Adjacent same-mesh batches DO still merge (regression guard).
        let mut merged = Vec::new();
        push_draw_batch(&mut merged, meshlet_push(0, 0, 1));
        push_draw_batch(&mut merged, meshlet_push(0, 1, 1));
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].instance_count, 2);
    }

    #[test]
    pub(super) fn mirrored_winding_flag_tracks_odd_negative_axes() {
        let material = perro_render_bridge::Material3D::default();
        let args = BuildInstanceArgs {
            debug_view: false,
            debug_color: [1.0, 1.0, 1.0, 1.0],
            mesh_blend: ResolvedMeshBlend::default(),
            skeleton_start: 0,
            skeleton_count: 0,
            custom_params_offset: 0,
            custom_params_len: 0,
            packed_lod_param_id: 0,
            receive_shadows: true,
            modulate_bias: false,
        };
        let odd = build_instance(
            glam::Mat4::from_scale(glam::Vec3::new(-1.0, 1.0, 1.0)).to_cols_array_2d(),
            &material,
            args,
        );
        let odd_flags = (odd.rigid_meta.material.packed_material_params >> 3) & 0x1fff;
        assert_ne!(odd_flags & MATERIAL_FLAG_MIRRORED_WINDING, 0);
        assert_ne!(
            (odd.rigid_meta.material.packed_material_params >> 2) & 0x1,
            0
        );

        let even = build_instance(
            glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0)).to_cols_array_2d(),
            &material,
            args,
        );
        let even_flags = (even.rigid_meta.material.packed_material_params >> 3) & 0x1fff;
        assert_eq!(even_flags & MATERIAL_FLAG_MIRRORED_WINDING, 0);
        assert_eq!(
            (even.rigid_meta.material.packed_material_params >> 2) & 0x1,
            0
        );
    }

    pub(super) fn bounds_batch(instance_start: u32, instance_count: u32, radius: f32) -> DrawBatch {
        let material_kind = MaterialPipelineKind::Standard;
        let state_key =
            draw_batch_state_key(RenderPath3D::Rigid, false, false, 0, false, &material_kind);
        let material_texture_key = MaterialTextureKey::from_base(0);
        DrawBatch {
            state_key,
            render_state: render_state_key(
                state_key,
                material_texture_key.state_hash(),
                0,
                0,
                false,
                0,
                false,
            ),
            mesh: MeshRange {
                index_start: 0,
                index_count: 3,
                base_vertex: 0,
            },
            instance_start,
            instance_count,
            path: RenderPath3D::Rigid,
            packed_lod: false,
            double_sided: false,
            material_kind,
            alpha_mode: 0,
            draw_on_top: false,
            base_color_texture_slot: 0,
            material_texture_key,
            local_center: [0.0, 0.0, 0.0],
            local_radius: radius,
            occlusion_query: None,
            disable_hiz_occlusion: false,
            casts_shadows: true,
            receives_shadows: true,
            mesh_blend: false,
            mesh_blend_screen: false,
            mesh_blend_params: 0,
            mesh_blend_depth: false,
            blend_layers: BitMask::ALL.bits(),
            blend_mask: BitMask::NONE.bits(),
            order_index: 0,
        }
    }

    pub(super) fn instance_at(pos: [f32; 3]) -> TransformInstanceGpu {
        TransformInstanceGpu {
            model_row_0: [1.0, 0.0, 0.0, pos[0]],
            model_row_1: [0.0, 1.0, 0.0, pos[1]],
            model_row_2: [0.0, 0.0, 1.0, pos[2]],
        }
    }

    #[test]
    pub(super) fn enclose_spheres_contains_both_inputs() {
        let a = (Vec3::new(1.0, 2.0, 3.0), 2.0);
        let b = (Vec3::new(9.0, 9.0, 9.0), 4.0);
        let (center, radius) = enclose_spheres(a, b);
        assert!(center.distance(a.0) + a.1 <= radius + 1.0e-4);
        assert!(center.distance(b.0) + b.1 <= radius + 1.0e-4);
        // Containment cases return the bigger sphere unchanged.
        let inner = (Vec3::new(0.1, 0.0, 0.0), 1.0);
        let outer = (Vec3::ZERO, 5.0);
        assert_eq!(enclose_spheres(inner, outer), outer);
        assert_eq!(enclose_spheres(outer, inner), outer);
    }

    #[test]
    pub(super) fn batch_merged_world_sphere_covers_every_instance() {
        let transforms = [
            instance_at([-10.0, 0.0, 0.0]),
            instance_at([0.0, 0.0, 0.0]),
            instance_at([10.0, 0.0, 0.0]),
        ];
        let batch = bounds_batch(0, 3, 1.0);
        let (center, radius) =
            batch_merged_world_sphere(&batch, &transforms).expect("required value must be present");
        for inst in &transforms {
            let world = Vec3::new(
                inst.model_row_0[3],
                inst.model_row_1[3],
                inst.model_row_2[3],
            );
            assert!(center.distance(world) + 1.0 <= radius + 1.0e-4);
        }
        // Sentinel radius and out-of-range instance windows yield no bound.
        assert!(batch_merged_world_sphere(&bounds_batch(0, 3, 1.0e9), &transforms).is_none());
        assert!(batch_merged_world_sphere(&bounds_batch(2, 4, 1.0), &transforms).is_none());
    }

    #[test]
    pub(super) fn multi_instance_cull_rows_emit_world_sphere_with_identity_model() {
        let transforms = [instance_at([-5.0, 0.0, 0.0]), instance_at([5.0, 0.0, 0.0])];
        let batch = bounds_batch(0, 2, 1.0);
        let (static_row, dynamic_row) = multi_instance_cull_rows(&batch, &transforms);
        // Identity model: the shader treats the sphere as already world-space.
        assert_eq!(dynamic_row.model_0, [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(dynamic_row.model_1, [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(dynamic_row.model_2, [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(dynamic_row.model_3, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(static_row.cull_flags[0], 0, "hi-z stays enabled");
        let [x, y, z, r] = static_row.local_center_radius;
        assert!((x - 0.0).abs() < 1.0e-4 && y == 0.0 && z == 0.0);
        assert!((r - 6.0).abs() < 1.0e-4, "sphere spans both instances");

        // No usable bound (sentinel radius): always-visible + hi-z disabled.
        let (fallback_static, _) =
            multi_instance_cull_rows(&bounds_batch(0, 2, 1.0e9), &transforms);
        assert_eq!(fallback_static.local_center_radius, [0.0, 0.0, 0.0, 1.0e9]);
        assert_eq!(
            fallback_static.cull_flags[0],
            CULL_FLAG_DISABLE_HIZ_OCCLUSION
        );
    }
}
