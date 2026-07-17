use super::*;

#[inline]
pub(in super::super) fn pack_emissive_hdr(rgb: [f32; 3]) -> u32 {
    let lin = crate::srgb_to_linear_rgb(rgb);
    let m = lin[0].max(lin[1]).max(lin[2]);
    if !m.is_finite() || m <= 1.0e-6 {
        return 0;
    }
    let scale = (m / crate::EMISSIVE_PACK_MAX).clamp(0.0, 1.0);
    pack_unorm4x8([lin[0] / m, lin[1] / m, lin[2] / m, scale])
}

#[inline]
pub(super) fn next_draw_batch_order(draw_batches: &[DrawBatch]) -> u32 {
    draw_batches
        .last()
        .map(|batch| batch.order_index.saturating_add(1))
        .unwrap_or(0)
}

#[derive(Clone, Copy)]
pub(in super::super) struct BuiltInstanceParts {
    pub(in super::super) transform: TransformInstanceGpu,
    pub(in super::super) rigid_meta: RigidInstanceMetaGpu,
    pub(in super::super) skinned_meta: SkinnedInstanceMetaGpu,
}

#[derive(Clone, Copy)]
pub(in super::super) struct BuildInstanceArgs {
    pub(in super::super) debug_view: bool,
    pub(in super::super) debug_color: [f32; 4],
    pub(in super::super) mesh_blend: ResolvedMeshBlend,
    pub(in super::super) skeleton_start: u32,
    pub(in super::super) skeleton_count: u32,
    pub(in super::super) custom_params_offset: u32,
    pub(in super::super) custom_params_len: u32,
    pub(in super::super) packed_lod_param_id: u32,
    pub(in super::super) receive_shadows: bool,
    pub(in super::super) modulate_bias: bool,
}

#[derive(Clone, Copy, Default)]
pub(in super::super) struct ResolvedMeshBlend {
    pub(in super::super) packed_params: u32,
    pub(in super::super) packed_flags: u32,
    pub(in super::super) depth_receiver: bool,
}

pub(super) const RESOLVED_MESH_BLEND_ACTIVE: u32 = 1u32 << 0;
pub(super) const RESOLVED_MESH_BLEND_NORMAL_BLEND: u32 = 1u32 << 1;
pub(super) const RESOLVED_MESH_BLEND_SCREEN_BLEND: u32 = 1u32 << 3;
// Set during prepare when the screen-space seam pass will handle this draw;
// the source then renders opaque and the in-material depth fade stays off.
pub(super) const RESOLVED_MESH_BLEND_SCREEN_PASS: u32 = 1u32 << 4;

#[inline]
pub(super) fn pack_resolved_mesh_blend_flags(blend: MeshBlendOptions3D) -> u32 {
    let mut flags = RESOLVED_MESH_BLEND_ACTIVE;
    if blend.normal_blending {
        flags |= RESOLVED_MESH_BLEND_NORMAL_BLEND;
    }
    if blend.screen_blending {
        flags |= RESOLVED_MESH_BLEND_SCREEN_BLEND;
    }
    flags
}

#[inline]
pub(in super::super) fn resolved_mesh_blend_active(blend: ResolvedMeshBlend) -> bool {
    (blend.packed_flags & RESOLVED_MESH_BLEND_ACTIVE) != 0
}

#[inline]
pub(in super::super) fn resolved_mesh_blend_depth_receiver(blend: ResolvedMeshBlend) -> bool {
    blend.depth_receiver
}

#[inline]
pub(super) fn resolved_mesh_blend_normal_blending(blend: ResolvedMeshBlend) -> bool {
    (blend.packed_flags & RESOLVED_MESH_BLEND_NORMAL_BLEND) != 0
}

#[inline]
pub(super) fn resolved_mesh_blend_screen_blending(blend: ResolvedMeshBlend) -> bool {
    (blend.packed_flags & RESOLVED_MESH_BLEND_SCREEN_BLEND) != 0
}

#[inline]
pub(in super::super) fn resolved_mesh_blend_screen_pass(blend: ResolvedMeshBlend) -> bool {
    (blend.packed_flags & RESOLVED_MESH_BLEND_SCREEN_PASS) != 0
}

#[inline]
pub(in super::super) fn promote_mesh_blend_screen_pass(blend: &mut ResolvedMeshBlend) {
    if (blend.packed_flags & RESOLVED_MESH_BLEND_ACTIVE) != 0
        && (blend.packed_flags & RESOLVED_MESH_BLEND_SCREEN_BLEND) != 0
    {
        blend.packed_flags |= RESOLVED_MESH_BLEND_SCREEN_PASS;
    }
}

#[inline]
pub(super) fn pack_mesh_blend_params(blend: MeshBlendOptions3D) -> u32 {
    pack_u8_lanes(
        quantize_unorm8_range(blend.distance, 16.0),
        quantize_unorm8_range(blend.min_distance, 16.0),
        quantize_unorm8(blend.noise_factor),
        quantize_unorm8_range(blend.noise_scale, 64.0),
    )
}

pub(in super::super) fn resolve_mesh_blends(
    draws: &[Draw3DInstance],
    out: &mut Vec<ResolvedMeshBlend>,
) {
    const LAYER_BITS: usize = 32;
    const MESH_BLEND_BUCKET_MIN: usize = 256;

    out.clear();
    out.resize(draws.len(), ResolvedMeshBlend::default());

    if draws.len() < MESH_BLEND_BUCKET_MIN {
        resolve_mesh_blends_quadratic(draws, out);
        return;
    }

    let mut source_accept_counts = [[0u32; LAYER_BITS]; LAYER_BITS];
    let mut source_accept_single = [[usize::MAX; LAYER_BITS]; LAYER_BITS];
    let mut target_accept_counts = [[0u32; LAYER_BITS]; LAYER_BITS];
    let mut target_accept_single = [[usize::MAX; LAYER_BITS]; LAYER_BITS];

    for (index, draw) in draws.iter().enumerate() {
        if !matches!(draw.kind, Draw3DKind::Mesh(_)) {
            continue;
        }
        add_layer_pair_counts(
            draw.blend.blend_layers.bits(),
            !draw.blend.blend_mask.bits(),
            index,
            &mut target_accept_counts,
            &mut target_accept_single,
        );
        if draw.blend.active() {
            add_layer_pair_counts(
                !draw.blend.blend_mask.bits(),
                draw.blend.blend_layers.bits(),
                index,
                &mut source_accept_counts,
                &mut source_accept_single,
            );
        }
    }

    for (index, draw) in draws.iter().enumerate() {
        if !matches!(draw.kind, Draw3DKind::Mesh(_)) {
            continue;
        }
        if layer_pair_has_other_or_self_interact(
            draw.blend.blend_layers.bits(),
            !draw.blend.blend_mask.bits(),
            index,
            draws,
            &source_accept_counts,
            &source_accept_single,
        ) {
            out[index].depth_receiver = true;
        }
    }

    for (index, draw) in draws.iter().enumerate() {
        if !draw.blend.active() || !matches!(draw.kind, Draw3DKind::Mesh(_)) {
            continue;
        }
        if layer_pair_has_other_or_self_interact(
            !draw.blend.blend_mask.bits(),
            draw.blend.blend_layers.bits(),
            index,
            draws,
            &target_accept_counts,
            &target_accept_single,
        ) {
            out[index] = ResolvedMeshBlend {
                packed_params: pack_mesh_blend_params(draw.blend),
                packed_flags: pack_resolved_mesh_blend_flags(draw.blend),
                depth_receiver: out[index].depth_receiver,
            }
        }
    }
}

pub(super) fn resolve_mesh_blends_quadratic(
    draws: &[Draw3DInstance],
    out: &mut [ResolvedMeshBlend],
) {
    for (index, draw) in draws.iter().enumerate() {
        if !draw.blend.active() || !matches!(draw.kind, Draw3DKind::Mesh(_)) {
            continue;
        }
        let self_interacts = draw_self_interacts(draw);
        let own_layers = draw.blend.blend_layers.bits();
        let mut target_found = false;
        for (target_index, target) in draws.iter().enumerate() {
            if target_index == index && !self_interacts {
                continue;
            }
            if !matches!(target.kind, Draw3DKind::Mesh(_)) {
                continue;
            }
            let source_accepts_target =
                target.blend.blend_layers.bits() & !draw.blend.blend_mask.bits() != 0;
            let target_accepts_source = own_layers & !target.blend.blend_mask.bits() != 0;
            if source_accepts_target && target_accepts_source {
                target_found = true;
                out[target_index].depth_receiver = true;
            }
        }
        if target_found {
            out[index] = ResolvedMeshBlend {
                packed_params: pack_mesh_blend_params(draw.blend),
                packed_flags: pack_resolved_mesh_blend_flags(draw.blend),
                depth_receiver: out[index].depth_receiver,
            };
        }
    }
}

#[inline]
pub(super) fn add_layer_pair_counts(
    outer_bits: u32,
    inner_bits: u32,
    index: usize,
    counts: &mut [[u32; 32]; 32],
    single: &mut [[usize; 32]; 32],
) {
    let mut outer = outer_bits;
    while outer != 0 {
        let outer_bit = outer.trailing_zeros() as usize;
        outer &= outer - 1;
        let mut inner = inner_bits;
        while inner != 0 {
            let inner_bit = inner.trailing_zeros() as usize;
            inner &= inner - 1;
            counts[outer_bit][inner_bit] = counts[outer_bit][inner_bit].saturating_add(1);
            single[outer_bit][inner_bit] = index;
        }
    }
}

#[inline]
pub(super) fn layer_pair_has_other_or_self_interact(
    outer_bits: u32,
    inner_bits: u32,
    index: usize,
    draws: &[Draw3DInstance],
    counts: &[[u32; 32]; 32],
    single: &[[usize; 32]; 32],
) -> bool {
    let mut outer = outer_bits;
    while outer != 0 {
        let outer_bit = outer.trailing_zeros() as usize;
        outer &= outer - 1;
        let mut inner = inner_bits;
        while inner != 0 {
            let inner_bit = inner.trailing_zeros() as usize;
            inner &= inner - 1;
            let count = counts[outer_bit][inner_bit];
            if count > 1 {
                return true;
            }
            if count == 1 {
                let source_index = single[outer_bit][inner_bit];
                if source_index != index || draw_self_interacts(&draws[index]) {
                    return true;
                }
            }
        }
    }
    false
}

#[inline]
pub(super) fn draw_self_interacts(draw: &Draw3DInstance) -> bool {
    draw.dense_multimesh
        .as_ref()
        .map(|dense| dense.instances.len() > 1)
        .unwrap_or_else(|| draw.instance_mats.len() > 1)
}

#[inline]
pub(in super::super) fn quantize_unorm8(v: f32) -> u32 {
    ((v.clamp(0.0, 1.0) * 255.0) + 0.5).floor() as u32
}

#[inline]
pub(in super::super) fn quantize_unorm8_range(v: f32, max: f32) -> u32 {
    if max <= 0.0 {
        return 0;
    }
    quantize_unorm8(v / max)
}

#[inline]
pub(in super::super) fn pack_u8_lanes(x: u32, y: u32, z: u32, w: u32) -> u32 {
    (x & 0xff) | ((y & 0xff) << 8) | ((z & 0xff) << 16) | ((w & 0xff) << 24)
}

#[inline]
pub(in super::super) fn pack_standard_pbr_params(
    roughness: f32,
    metallic: f32,
    occlusion_strength: f32,
    normal_scale: f32,
) -> u32 {
    pack_u8_lanes(
        quantize_unorm8(roughness),
        quantize_unorm8(metallic),
        quantize_unorm8(occlusion_strength),
        quantize_unorm8_range(normal_scale, PACKED_STANDARD_NORMAL_SCALE_MAX),
    )
}

#[inline]
pub(in super::super) fn pack_toon_pbr_params(
    band_count: u32,
    rim_strength: f32,
    outline_width: f32,
) -> u32 {
    pack_u8_lanes(
        band_count.clamp(1, 255),
        quantize_unorm8_range(rim_strength, PACKED_TOON_RIM_STRENGTH_MAX),
        quantize_unorm8_range(outline_width, PACKED_TOON_OUTLINE_WIDTH_MAX),
        0,
    )
}

#[inline]
pub(in super::super) fn pack_material_params(
    alpha_mode: u8,
    alpha_cutoff: f32,
    double_sided: bool,
    flags: u32,
) -> u32 {
    let mode_bits = (alpha_mode as u32) & 0x3;
    let double_sided_bit = if double_sided { 1u32 } else { 0u32 };
    // bits: [0..1]=alpha_mode, [2]=double_sided, [3..15]=flags, [16..23]=alpha_cutoff u8
    let packed_flags = (flags & 0x1fff) << 3;
    let alpha_cutoff_bits = quantize_unorm8(alpha_cutoff) << 16;
    mode_bits | (double_sided_bit << 2) | packed_flags | alpha_cutoff_bits
}

#[inline]
pub(in super::super) fn build_instance(
    model: [[f32; 4]; 4],
    material: &perro_render_bridge::Material3D,
    args: BuildInstanceArgs,
) -> BuiltInstanceParts {
    let BuildInstanceArgs {
        debug_view,
        debug_color,
        mesh_blend,
        skeleton_start,
        skeleton_count,
        custom_params_offset,
        custom_params_len,
        packed_lod_param_id,
        receive_shadows,
        modulate_bias,
    } = args;
    let (color, packed_pbr_params_0, packed_pbr_params_1, emissive_factor, debug_flags) =
        if debug_view {
            (
                debug_color,
                pack_standard_pbr_params(0.5, 0.0, 1.0, 1.0),
                0,
                [0.0, 0.0, 0.0],
                MATERIAL_FLAG_MESHLET_DEBUG_VIEW,
            )
        } else {
            match material {
                Material3D::Standard(params) => (
                    params.base_color_factor,
                    pack_standard_pbr_params(
                        params.roughness_factor,
                        params.metallic_factor,
                        params.occlusion_strength,
                        params.normal_scale,
                    ),
                    0,
                    params.emissive_factor,
                    0u32,
                ),
                Material3D::Unlit(params) => {
                    (params.base_color_factor, 0, 0, params.emissive_factor, 0u32)
                }
                Material3D::Toon(params) => (
                    params.base_color_factor,
                    pack_toon_pbr_params(
                        params.band_count,
                        params.rim_strength,
                        params.outline_width,
                    ),
                    0,
                    params.emissive_factor,
                    0u32,
                ),
                Material3D::Custom(_) => {
                    let params = material.standard_params();
                    (
                        params.base_color_factor,
                        pack_standard_pbr_params(
                            params.roughness_factor,
                            params.metallic_factor,
                            params.occlusion_strength,
                            params.normal_scale,
                        ),
                        0,
                        params.emissive_factor,
                        0u32,
                    )
                }
            }
        };
    let params = material.standard_params();
    let mut material_flags = debug_flags;
    let mirrored_winding = Mat4::from_cols_array_2d(&model).determinant() < 0.0;
    if mirrored_winding {
        material_flags |= MATERIAL_FLAG_MIRRORED_WINDING;
    }
    if params.flat_shading {
        material_flags |= MATERIAL_FLAG_FLAT_SHADING;
    }
    if params.base_color_texture != MATERIAL_TEXTURE_NONE {
        material_flags |= MATERIAL_FLAG_HAS_BASE_COLOR_TEXTURE;
    }
    if matches!(material, Material3D::Standard(_)) {
        if params.metallic_roughness_texture != MATERIAL_TEXTURE_NONE {
            material_flags |= MATERIAL_FLAG_HAS_METALLIC_ROUGHNESS_TEXTURE;
        }
        if params.normal_texture != MATERIAL_TEXTURE_NONE {
            material_flags |= MATERIAL_FLAG_HAS_NORMAL_TEXTURE;
        }
        if params.occlusion_texture != MATERIAL_TEXTURE_NONE {
            material_flags |= MATERIAL_FLAG_HAS_OCCLUSION_TEXTURE;
        }
        if params.emissive_texture != MATERIAL_TEXTURE_NONE {
            material_flags |= MATERIAL_FLAG_HAS_EMISSIVE_TEXTURE;
        }
    }
    if receive_shadows && !matches!(material, Material3D::Unlit(_)) {
        material_flags |= MATERIAL_FLAG_RECEIVE_SHADOWS;
    }
    if modulate_bias {
        material_flags |= MATERIAL_FLAG_MODULATE_BIAS;
    }
    let blend_active = resolved_mesh_blend_active(mesh_blend);
    let packed_blend_params = if blend_active && !debug_view {
        // Screen-pass sources render opaque; the seam pass softens the
        // intersection instead of the in-material depth fade.
        if resolved_mesh_blend_screen_blending(mesh_blend)
            && !resolved_mesh_blend_screen_pass(mesh_blend)
        {
            material_flags |= MATERIAL_FLAG_MESH_BLEND;
        }
        if resolved_mesh_blend_normal_blending(mesh_blend) {
            material_flags |= MATERIAL_FLAG_NORMAL_BLEND;
        }
        mesh_blend.packed_params
    } else {
        0
    };

    let color_linear = crate::srgb_to_linear_rgb([color[0], color[1], color[2]]);
    let material = MaterialInstanceGpu {
        packed_color: pack_unorm4x8([color_linear[0], color_linear[1], color_linear[2], color[3]]),
        packed_pbr_params_0,
        packed_pbr_params_1: packed_pbr_params_1 | packed_blend_params,
        packed_emissive: pack_emissive_hdr(emissive_factor),
        packed_material_params: pack_material_params(
            params.alpha_mode,
            params.alpha_cutoff,
            params.double_sided || mirrored_winding,
            material_flags,
        ),
    };

    BuiltInstanceParts {
        transform: TransformInstanceGpu {
            model_row_0: [model[0][0], model[1][0], model[2][0], model[3][0]],
            model_row_1: [model[0][1], model[1][1], model[2][1], model[3][1]],
            model_row_2: [model[0][2], model[1][2], model[2][2], model[3][2]],
        },
        rigid_meta: RigidInstanceMetaGpu {
            material,
            custom_params: [custom_params_offset, custom_params_len],
            packed_lod_param_id,
        },
        skinned_meta: SkinnedInstanceMetaGpu {
            material,
            skeleton_params: [
                skeleton_start,
                skeleton_count,
                custom_params_offset,
                custom_params_len,
            ],
        },
    }
}

#[inline]
pub(in super::super) fn model_cols_from_affine_rows(inst: &TransformInstanceGpu) -> [[f32; 4]; 4] {
    [
        [
            inst.model_row_0[0],
            inst.model_row_1[0],
            inst.model_row_2[0],
            0.0,
        ],
        [
            inst.model_row_0[1],
            inst.model_row_1[1],
            inst.model_row_2[1],
            0.0,
        ],
        [
            inst.model_row_0[2],
            inst.model_row_1[2],
            inst.model_row_2[2],
            0.0,
        ],
        [
            inst.model_row_0[3],
            inst.model_row_1[3],
            inst.model_row_2[3],
            1.0,
        ],
    ]
}

// Smallest sphere enclosing two spheres that share one space.
pub(in super::super) fn enclose_spheres(a: (Vec3, f32), b: (Vec3, f32)) -> (Vec3, f32) {
    let delta = b.0 - a.0;
    let dist = delta.length();
    if !dist.is_finite() {
        return if a.1 >= b.1 { a } else { b };
    }
    if a.1 >= dist + b.1 {
        return a;
    }
    if b.1 >= dist + a.1 {
        return b;
    }
    let radius = (dist + a.1 + b.1) * 0.5;
    let center = if dist > 1.0e-6 {
        a.0 + delta * ((radius - a.1) / dist)
    } else {
        a.0
    };
    (center, radius)
}

#[inline]
pub(in super::super) fn enclose_local_spheres(
    a: ([f32; 3], f32),
    b: ([f32; 3], f32),
) -> ([f32; 3], f32) {
    let (center, radius) = enclose_spheres(
        (Vec3::from(a.0), a.1.max(0.0)),
        (Vec3::from(b.0), b.1.max(0.0)),
    );
    (center.to_array(), radius)
}

// World-space bounding sphere of one instance's local sphere.
pub(in super::super) fn instance_world_sphere(
    local_center: [f32; 3],
    local_radius: f32,
    inst: &TransformInstanceGpu,
) -> Option<(Vec3, f32)> {
    let model = Mat4::from_cols_array_2d(&model_cols_from_affine_rows(inst));
    if !model.is_finite() {
        return None;
    }
    let center = model * Vec4::new(local_center[0], local_center[1], local_center[2], 1.0);
    if !center.is_finite() {
        return None;
    }
    let sx = model.x_axis.truncate().length();
    let sy = model.y_axis.truncate().length();
    let sz = model.z_axis.truncate().length();
    let scale = sx.max(sy).max(sz).max(1.0e-6);
    Some((center.truncate(), local_radius.max(0.0) * scale))
}

// Merged world sphere over every instance of a batch. None when the instance
// range is out of bounds, any transform is non-finite, or the bound is too
// large to be useful for culling (debug batches carry a 1e9 sentinel radius).
pub(in super::super) fn batch_merged_world_sphere(
    batch: &DrawBatch,
    transforms: &[TransformInstanceGpu],
) -> Option<(Vec3, f32)> {
    if !batch.local_radius.is_finite() || batch.local_radius >= 1.0e8 {
        return None;
    }
    let start = batch.instance_start as usize;
    let end = start.checked_add(batch.instance_count as usize)?;
    if start >= end || end > transforms.len() {
        return None;
    }
    let mut merged: Option<(Vec3, f32)> = None;
    for inst in &transforms[start..end] {
        let sphere = instance_world_sphere(batch.local_center, batch.local_radius, inst)?;
        merged = Some(match merged {
            Some(current) => enclose_spheres(current, sphere),
            None => sphere,
        });
    }
    merged.filter(|(center, radius)| center.is_finite() && radius.is_finite() && *radius < 1.0e8)
}

// GPU cull rows for a multi-instance batch: one model matrix cannot bound every
// instance, so emit the merged world sphere with an identity model. Falls back
// to an always-visible, hi-z-disabled row when no usable bound exists.
pub(in super::super) fn multi_instance_cull_rows(
    batch: &DrawBatch,
    transforms: &[TransformInstanceGpu],
) -> (FrustumCullStaticGpu, FrustumCullDynamicGpu) {
    let (center_radius, flags) = match batch_merged_world_sphere(batch, transforms) {
        Some((center, radius)) => (
            [center.x, center.y, center.z, radius],
            if batch.disable_hiz_occlusion {
                CULL_FLAG_DISABLE_HIZ_OCCLUSION
            } else {
                0
            },
        ),
        None => ([0.0, 0.0, 0.0, 1.0e9], CULL_FLAG_DISABLE_HIZ_OCCLUSION),
    };
    (
        FrustumCullStaticGpu {
            local_center_radius: center_radius,
            cull_flags: [flags, 0, 0, 0],
        },
        FrustumCullDynamicGpu {
            model_0: [1.0, 0.0, 0.0, 0.0],
            model_1: [0.0, 1.0, 0.0, 0.0],
            model_2: [0.0, 0.0, 1.0, 0.0],
            model_3: [0.0, 0.0, 0.0, 1.0],
        },
    )
}

#[inline]
pub(in super::super) fn encode_custom_param_value_packed(
    value: &perro_render_bridge::CustomMaterialParamValue3D,
    out: &mut Vec<f32>,
) -> u32 {
    match value {
        perro_render_bridge::CustomMaterialParamValue3D::F32(v) => {
            out.push(*v);
            CUSTOM_PARAM_KIND_SCALAR
        }
        perro_render_bridge::CustomMaterialParamValue3D::I32(v) => {
            out.push(*v as f32);
            CUSTOM_PARAM_KIND_SCALAR
        }
        perro_render_bridge::CustomMaterialParamValue3D::Bool(v) => {
            out.push(if *v { 1.0 } else { 0.0 });
            CUSTOM_PARAM_KIND_SCALAR
        }
        perro_render_bridge::CustomMaterialParamValue3D::Vec2(v) => {
            out.extend_from_slice(v);
            CUSTOM_PARAM_KIND_VEC2
        }
        perro_render_bridge::CustomMaterialParamValue3D::Vec3(v) => {
            out.extend_from_slice(v);
            CUSTOM_PARAM_KIND_VEC3
        }
        perro_render_bridge::CustomMaterialParamValue3D::Vec4(v) => {
            out.extend_from_slice(v);
            CUSTOM_PARAM_KIND_VEC4
        }
    }
}

pub(in super::super) fn apply_surface_binding(
    mut material: Material3D,
    surface: &MeshSurfaceBinding3D,
) -> (Material3D, bool) {
    let modulate_bias = apply_modulate(&mut material, surface.modulate);
    apply_overrides(&mut material, &surface.overrides);
    (material, modulate_bias)
}

/// Pull strength toward the modulate hue on top of the straight multiply.
/// Scaled by modulate saturation, so white/grey modulates stay a pure
/// multiply (white = exact passthrough). Mirrored by the WGSL constant in
/// material_standard.wgsl, which extends the same bias past the base color
/// texture.
pub(super) const MODULATE_TINT_BIAS: f32 = 0.2;

pub(in super::super) fn modulate_bias_strength(modulate: [f32; 4]) -> f32 {
    let max_c = modulate[0].max(modulate[1]).max(modulate[2]);
    let min_c = modulate[0].min(modulate[1]).min(modulate[2]);
    MODULATE_TINT_BIAS * (max_c - min_c).clamp(0.0, 1.0)
}

pub(in super::super) fn modulate_color_mix(base: [f32; 4], modulate: [f32; 4]) -> [f32; 4] {
    let mul = [
        base[0] * modulate[0],
        base[1] * modulate[1],
        base[2] * modulate[2],
    ];
    let alpha = base[3] * modulate[3];
    let k = modulate_bias_strength(modulate);
    if k <= 0.0 {
        return [mul[0], mul[1], mul[2], alpha];
    }
    // Luminance-preserving target: modulate hue at the base color's
    // brightness. Keeps opposing hues (red base x green modulate) from
    // collapsing to black.
    let luma = 0.2126 * base[0] + 0.7152 * base[1] + 0.0722 * base[2];
    [
        mul[0] + (modulate[0] * luma - mul[0]) * k,
        mul[1] + (modulate[1] * luma - mul[1]) * k,
        mul[2] + (modulate[2] * luma - mul[2]) * k,
        alpha,
    ]
}

/// Returns true when a chromatic modulate was folded in, so the shader can
/// carry the bias past the base color texture (MATERIAL_FLAG_MODULATE_BIAS).
pub(in super::super) fn apply_modulate(
    material: &mut Material3D,
    modulate: perro_structs::Color,
) -> bool {
    if modulate == perro_structs::Color::WHITE {
        return false;
    }
    let modulate = modulate.to_float_slice();
    match material {
        Material3D::Standard(m) => {
            m.base_color_factor = modulate_color_mix(m.base_color_factor, modulate);
        }
        Material3D::Unlit(m) => {
            m.base_color_factor = modulate_color_mix(m.base_color_factor, modulate);
        }
        Material3D::Toon(m) => {
            m.base_color_factor = modulate_color_mix(m.base_color_factor, modulate);
        }
        Material3D::Custom(_) => return false,
    }
    modulate_bias_strength(modulate) > 0.0
}

pub(in super::super) fn apply_overrides(
    material: &mut Material3D,
    overrides: &[MaterialParamOverride3D],
) {
    if overrides.is_empty() {
        return;
    }
    match material {
        Material3D::Standard(standard) => {
            for ovr in overrides {
                apply_flat_shading_override(&ovr.name, &ovr.value, &mut standard.flat_shading);
            }
        }
        Material3D::Unlit(unlit) => {
            for ovr in overrides {
                apply_flat_shading_override(&ovr.name, &ovr.value, &mut unlit.flat_shading);
            }
        }
        Material3D::Toon(toon) => {
            for ovr in overrides {
                apply_flat_shading_override(&ovr.name, &ovr.value, &mut toon.flat_shading);
            }
        }
        Material3D::Custom(custom) => {
            let mut params = custom.params.clone().into_owned();
            for ovr in overrides {
                params.push(perro_render_bridge::CustomMaterialParam3D {
                    name: Some(ovr.name.clone()),
                    value: match ovr.value {
                        MaterialParamOverrideValue3D::F32(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::F32(v)
                        }
                        MaterialParamOverrideValue3D::I32(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::I32(v)
                        }
                        MaterialParamOverrideValue3D::Bool(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::Bool(v)
                        }
                        MaterialParamOverrideValue3D::Vec2(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::Vec2(v)
                        }
                        MaterialParamOverrideValue3D::Vec3(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::Vec3(v)
                        }
                        MaterialParamOverrideValue3D::Vec4(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::Vec4(v)
                        }
                    },
                });
            }
            custom.params = Cow::Owned(params);
        }
    }
}

pub(in super::super) fn apply_flat_shading_override(
    name: &str,
    value: &MaterialParamOverrideValue3D,
    flat_shading: &mut bool,
) {
    let Some(v) = override_bool(value) else {
        return;
    };
    match name {
        "flat_shading" | "flatShading" | "shade_flat" | "shadeFlat" => {
            *flat_shading = v;
        }
        "shade_smooth" | "shadeSmooth" => {
            *flat_shading = !v;
        }
        _ => {}
    }
}

pub(in super::super) fn override_bool(value: &MaterialParamOverrideValue3D) -> Option<bool> {
    match value {
        MaterialParamOverrideValue3D::Bool(v) => Some(*v),
        MaterialParamOverrideValue3D::I32(v) => Some(*v != 0),
        MaterialParamOverrideValue3D::F32(v) => Some(*v > 0.5),
        _ => None,
    }
}
