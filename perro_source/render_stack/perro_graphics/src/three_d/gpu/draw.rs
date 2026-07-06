use super::*;

pub(super) struct DrawBatchPush {
    pub(super) render_path: RenderPath3D,
    pub(super) mesh: MeshRange,
    pub(super) instance_start: u32,
    pub(super) instance_count: u32,
    pub(super) double_sided: bool,
    pub(super) packed_lod: bool,
    pub(super) material_kind: MaterialPipelineKind,
    pub(super) alpha_mode: u8,
    pub(super) base_color_texture_slot: u32,
    pub(super) material_texture_key: MaterialTextureKey,
    pub(super) local_bounds: ([f32; 3], f32),
    pub(super) occlusion_query: Option<u32>,
    pub(super) disable_hiz_occlusion: bool,
    pub(super) casts_shadows: bool,
    pub(super) receives_shadows: bool,
    pub(super) mesh_blend: bool,
    pub(super) mesh_blend_screen: bool,
    pub(super) mesh_blend_params: u32,
    pub(super) mesh_blend_depth: bool,
    pub(super) blend_layers: u32,
    pub(super) blend_mask: u32,
}

pub(super) fn push_draw_batch(draw_batches: &mut Vec<DrawBatch>, batch: DrawBatchPush) {
    let DrawBatchPush {
        render_path,
        mesh,
        instance_start,
        instance_count,
        double_sided,
        packed_lod,
        material_kind,
        alpha_mode,
        base_color_texture_slot,
        material_texture_key,
        local_bounds,
        occlusion_query,
        disable_hiz_occlusion,
        casts_shadows,
        receives_shadows,
        mesh_blend,
        mesh_blend_screen,
        mesh_blend_params,
        mesh_blend_depth,
        blend_layers,
        blend_mask,
    } = batch;
    if instance_count == 0 {
        return;
    }
    let state_key = draw_batch_state_key(
        render_path,
        false,
        double_sided,
        alpha_mode,
        packed_lod,
        &material_kind,
    );
    let render_state = render_state_key(
        state_key,
        material_texture_key.state_hash(),
        mesh.index_start,
        mesh.base_vertex,
        false,
        alpha_mode,
        mesh_blend,
    );
    let order_index = next_draw_batch_order(draw_batches);
    let (local_center, local_radius) = local_bounds;
    if occlusion_query.is_none()
        && let Some(prev) = draw_batches.last_mut()
    {
        let prev_end = prev.instance_start.saturating_add(prev.instance_count);
        let same_mesh = prev.mesh.index_start == mesh.index_start
            && prev.mesh.index_count == mesh.index_count
            && prev.mesh.base_vertex == mesh.base_vertex;
        let same_batch_state = prev.state_key == state_key
            && prev.path == render_path
            && prev.packed_lod == packed_lod
            && prev.double_sided == double_sided
            && prev.material_kind == material_kind
            && prev.alpha_mode == alpha_mode
            && !prev.draw_on_top
            && prev.base_color_texture_slot == base_color_texture_slot
            && prev.material_texture_key == material_texture_key
            && prev.occlusion_query.is_none()
            && prev.casts_shadows == casts_shadows
            && prev.receives_shadows == receives_shadows
            && prev.mesh_blend == mesh_blend
            && prev.mesh_blend_screen == mesh_blend_screen
            && prev.mesh_blend_params == mesh_blend_params
            && prev.mesh_blend_depth == mesh_blend_depth
            && prev.blend_layers == blend_layers
            && prev.blend_mask == blend_mask;
        if same_mesh && same_batch_state && prev_end == instance_start {
            prev.instance_count = prev.instance_count.saturating_add(instance_count);
            prev.disable_hiz_occlusion |= disable_hiz_occlusion;
            // Same mesh (checked above), so both bounds share one local space;
            // keep the tight enclosing sphere. The cull upload expands it into a
            // merged per-instance world sphere for multi-instance batches.
            let (center, radius) = enclose_local_spheres(
                (prev.local_center, prev.local_radius),
                (local_center, local_radius.max(0.0)),
            );
            prev.local_center = center;
            prev.local_radius = radius;
            return;
        }
    }
    draw_batches.push(DrawBatch {
        state_key,
        render_state,
        mesh,
        instance_start,
        instance_count,
        path: render_path,
        packed_lod,
        double_sided,
        material_kind,
        alpha_mode,
        draw_on_top: false,
        base_color_texture_slot,
        material_texture_key,
        local_center,
        local_radius: local_radius.max(0.0),
        occlusion_query,
        disable_hiz_occlusion,
        casts_shadows,
        receives_shadows,
        mesh_blend,
        mesh_blend_screen,
        mesh_blend_params,
        mesh_blend_depth,
        blend_layers,
        blend_mask,
        order_index,
    });
}

// Normalized rgb in the color lanes, max-component / EMISSIVE_PACK_MAX in the
// alpha lane, so emissive keeps HDR magnitude through the unorm8 pack.
#[inline]
pub(super) fn pack_emissive_hdr(rgb: [f32; 3]) -> u32 {
    let lin = crate::srgb_to_linear_rgb(rgb);
    let m = lin[0].max(lin[1]).max(lin[2]);
    if !m.is_finite() || m <= 1.0e-6 {
        return 0;
    }
    let scale = (m / crate::EMISSIVE_PACK_MAX).clamp(0.0, 1.0);
    pack_unorm4x8([lin[0] / m, lin[1] / m, lin[2] / m, scale])
}

#[inline]
fn next_draw_batch_order(draw_batches: &[DrawBatch]) -> u32 {
    draw_batches
        .last()
        .map(|batch| batch.order_index.saturating_add(1))
        .unwrap_or(0)
}

#[derive(Clone, Copy)]
pub(super) struct BuiltInstanceParts {
    pub(super) transform: TransformInstanceGpu,
    pub(super) rigid_meta: RigidInstanceMetaGpu,
    pub(super) skinned_meta: SkinnedInstanceMetaGpu,
}

#[derive(Clone, Copy)]
pub(super) struct BuildInstanceArgs {
    pub(super) debug_view: bool,
    pub(super) debug_color: [f32; 4],
    pub(super) mesh_blend: ResolvedMeshBlend,
    pub(super) skeleton_start: u32,
    pub(super) skeleton_count: u32,
    pub(super) custom_params_offset: u32,
    pub(super) custom_params_len: u32,
    pub(super) packed_lod_param_id: u32,
    pub(super) receive_shadows: bool,
}

#[derive(Clone, Copy, Default)]
pub(super) struct ResolvedMeshBlend {
    pub(super) packed_params: u32,
    pub(super) packed_flags: u32,
    pub(super) depth_receiver: bool,
}

const RESOLVED_MESH_BLEND_ACTIVE: u32 = 1u32 << 0;
const RESOLVED_MESH_BLEND_NORMAL_BLEND: u32 = 1u32 << 1;
const RESOLVED_MESH_BLEND_SCREEN_BLEND: u32 = 1u32 << 3;
// Set during prepare when the screen-space seam pass will handle this draw;
// the source then renders opaque and the in-material depth fade stays off.
const RESOLVED_MESH_BLEND_SCREEN_PASS: u32 = 1u32 << 4;

#[inline]
fn pack_resolved_mesh_blend_flags(blend: MeshBlendOptions3D) -> u32 {
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
pub(super) fn resolved_mesh_blend_active(blend: ResolvedMeshBlend) -> bool {
    (blend.packed_flags & RESOLVED_MESH_BLEND_ACTIVE) != 0
}

#[inline]
pub(super) fn resolved_mesh_blend_depth_receiver(blend: ResolvedMeshBlend) -> bool {
    blend.depth_receiver
}

#[inline]
fn resolved_mesh_blend_normal_blending(blend: ResolvedMeshBlend) -> bool {
    (blend.packed_flags & RESOLVED_MESH_BLEND_NORMAL_BLEND) != 0
}

#[inline]
fn resolved_mesh_blend_screen_blending(blend: ResolvedMeshBlend) -> bool {
    (blend.packed_flags & RESOLVED_MESH_BLEND_SCREEN_BLEND) != 0
}

#[inline]
pub(super) fn resolved_mesh_blend_screen_pass(blend: ResolvedMeshBlend) -> bool {
    (blend.packed_flags & RESOLVED_MESH_BLEND_SCREEN_PASS) != 0
}

#[inline]
pub(super) fn promote_mesh_blend_screen_pass(blend: &mut ResolvedMeshBlend) {
    if (blend.packed_flags & RESOLVED_MESH_BLEND_ACTIVE) != 0
        && (blend.packed_flags & RESOLVED_MESH_BLEND_SCREEN_BLEND) != 0
    {
        blend.packed_flags |= RESOLVED_MESH_BLEND_SCREEN_PASS;
    }
}

#[inline]
fn pack_mesh_blend_params(blend: MeshBlendOptions3D) -> u32 {
    pack_u8_lanes(
        quantize_unorm8_range(blend.distance, 16.0),
        quantize_unorm8_range(blend.min_distance, 16.0),
        quantize_unorm8(blend.noise_factor),
        quantize_unorm8_range(blend.noise_scale, 64.0),
    )
}

pub(super) fn resolve_mesh_blends(draws: &[Draw3DInstance], out: &mut Vec<ResolvedMeshBlend>) {
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

fn resolve_mesh_blends_quadratic(draws: &[Draw3DInstance], out: &mut [ResolvedMeshBlend]) {
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
fn add_layer_pair_counts(
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
fn layer_pair_has_other_or_self_interact(
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
fn draw_self_interacts(draw: &Draw3DInstance) -> bool {
    draw.dense_multimesh
        .as_ref()
        .map(|dense| dense.instances.len() > 1)
        .unwrap_or_else(|| draw.instance_mats.len() > 1)
}

#[inline]
pub(super) fn quantize_unorm8(v: f32) -> u32 {
    ((v.clamp(0.0, 1.0) * 255.0) + 0.5).floor() as u32
}

#[inline]
pub(super) fn quantize_unorm8_range(v: f32, max: f32) -> u32 {
    if max <= 0.0 {
        return 0;
    }
    quantize_unorm8(v / max)
}

#[inline]
pub(super) fn pack_u8_lanes(x: u32, y: u32, z: u32, w: u32) -> u32 {
    (x & 0xff) | ((y & 0xff) << 8) | ((z & 0xff) << 16) | ((w & 0xff) << 24)
}

#[inline]
pub(super) fn pack_standard_pbr_params(
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
pub(super) fn pack_toon_pbr_params(band_count: u32, rim_strength: f32, outline_width: f32) -> u32 {
    pack_u8_lanes(
        band_count.clamp(1, 255),
        quantize_unorm8_range(rim_strength, PACKED_TOON_RIM_STRENGTH_MAX),
        quantize_unorm8_range(outline_width, PACKED_TOON_OUTLINE_WIDTH_MAX),
        0,
    )
}

#[inline]
pub(super) fn pack_material_params(
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
pub(super) fn build_instance(
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
    if receive_shadows && !matches!(material, Material3D::Unlit(_)) {
        material_flags |= MATERIAL_FLAG_RECEIVE_SHADOWS;
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
pub(super) fn model_cols_from_affine_rows(inst: &TransformInstanceGpu) -> [[f32; 4]; 4] {
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

// Pack one column-major bone matrix into its 3 affine rows for the skeleton
// palette upload (the shaders never read the w row).
#[inline]
pub(super) fn skeleton_bone_rows(cols: &[[f32; 4]; 4]) -> [[f32; 4]; 3] {
    [
        [cols[0][0], cols[1][0], cols[2][0], cols[3][0]],
        [cols[0][1], cols[1][1], cols[2][1], cols[3][1]],
        [cols[0][2], cols[1][2], cols[2][2], cols[3][2]],
    ]
}

// Smallest sphere enclosing two spheres that share one space.
pub(super) fn enclose_spheres(a: (Vec3, f32), b: (Vec3, f32)) -> (Vec3, f32) {
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
pub(super) fn enclose_local_spheres(a: ([f32; 3], f32), b: ([f32; 3], f32)) -> ([f32; 3], f32) {
    let (center, radius) = enclose_spheres(
        (Vec3::from(a.0), a.1.max(0.0)),
        (Vec3::from(b.0), b.1.max(0.0)),
    );
    (center.to_array(), radius)
}

// World-space bounding sphere of one instance's local sphere.
pub(super) fn instance_world_sphere(
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
pub(super) fn batch_merged_world_sphere(
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
pub(super) fn multi_instance_cull_rows(
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
pub(super) fn encode_custom_param_value_packed(
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

pub(super) fn apply_surface_binding(
    mut material: Material3D,
    surface: &MeshSurfaceBinding3D,
) -> Material3D {
    apply_modulate(&mut material, surface.modulate);
    apply_overrides(&mut material, &surface.overrides);
    material
}

pub(super) fn apply_modulate(material: &mut Material3D, modulate: perro_structs::Color) {
    let modulate = modulate.to_float_slice();
    match material {
        Material3D::Standard(m) => {
            for (dst, src) in m.base_color_factor.iter_mut().zip(modulate) {
                *dst *= src;
            }
        }
        Material3D::Unlit(m) => {
            for (dst, src) in m.base_color_factor.iter_mut().zip(modulate) {
                *dst *= src;
            }
        }
        Material3D::Toon(m) => {
            for (dst, src) in m.base_color_factor.iter_mut().zip(modulate) {
                *dst *= src;
            }
        }
        Material3D::Custom(_) => {}
    }
}

pub(super) fn apply_overrides(material: &mut Material3D, overrides: &[MaterialParamOverride3D]) {
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

pub(super) fn apply_flat_shading_override(
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

pub(super) fn override_bool(value: &MaterialParamOverrideValue3D) -> Option<bool> {
    match value {
        MaterialParamOverrideValue3D::Bool(v) => Some(*v),
        MaterialParamOverrideValue3D::I32(v) => Some(*v != 0),
        MaterialParamOverrideValue3D::F32(v) => Some(*v > 0.5),
        _ => None,
    }
}

#[inline]
pub(super) fn compare_draw_batch_keys(a: &DrawBatch, b: &DrawBatch) -> Ordering {
    a.render_state
        .batch_kind
        .cmp(&b.render_state.batch_kind)
        .then_with(|| match a.render_state.batch_kind {
            RenderBatchKind::Opaque => a
                .render_state
                .pipeline_key
                .cmp(&b.render_state.pipeline_key)
                .then_with(|| {
                    a.render_state
                        .texture_slot
                        .cmp(&b.render_state.texture_slot)
                })
                .then_with(|| {
                    a.render_state
                        .mesh_index_start
                        .cmp(&b.render_state.mesh_index_start)
                })
                .then_with(|| {
                    a.render_state
                        .mesh_base_vertex
                        .cmp(&b.render_state.mesh_base_vertex)
                })
                .then_with(|| a.instance_start.cmp(&b.instance_start)),
            RenderBatchKind::Alpha | RenderBatchKind::MeshBlend | RenderBatchKind::Overlay => a
                .order_index
                .cmp(&b.order_index)
                .then_with(|| a.instance_start.cmp(&b.instance_start)),
        })
}

#[inline]
pub(super) fn draw_batches_sorted(batches: &[DrawBatch]) -> bool {
    batches
        .windows(2)
        .all(|pair| compare_draw_batch_keys(&pair[0], &pair[1]) != Ordering::Greater)
}

#[inline]
pub(super) fn multimesh_batch_sort_key(
    batch: &MultiMeshBatch,
) -> (bool, bool, bool, u8, u32, u32, u32) {
    let custom_token = match batch.material_kind {
        MaterialPipelineKind::Custom(token) => token,
        _ => 0,
    };
    (
        batch.mesh_blend,
        batch.casts_shadows,
        batch.double_sided,
        material_pipeline_kind_rank(&batch.material_kind),
        custom_token,
        batch.mesh.index_start,
        batch.draw_param_index,
    )
}

#[inline]
pub(super) fn multimesh_batches_sorted(batches: &[MultiMeshBatch]) -> bool {
    batches
        .windows(2)
        .all(|pair| multimesh_batch_sort_key(&pair[0]) <= multimesh_batch_sort_key(&pair[1]))
}

#[inline]
pub(super) fn material_pipeline_kind_rank(kind: &MaterialPipelineKind) -> u8 {
    match kind {
        MaterialPipelineKind::Standard => 0,
        MaterialPipelineKind::Unlit => 1,
        MaterialPipelineKind::Toon => 2,
        MaterialPipelineKind::Custom(_) => 3,
    }
}

#[inline]
pub(super) fn draw_batch_state_key(
    path: RenderPath3D,
    draw_on_top: bool,
    double_sided: bool,
    alpha_mode: u8,
    packed_lod: bool,
    material_kind: &MaterialPipelineKind,
) -> u64 {
    let path_bits = match path {
        RenderPath3D::Rigid => 0u64,
        RenderPath3D::Skinned => 1u64,
        RenderPath3D::MultiMesh => 2u64,
    };
    let top_bits = u64::from(draw_on_top) << 1;
    let sided_bits = u64::from(double_sided) << 2;
    let alpha_bits = u64::from(alpha_mode == 2) << 3;
    let rank_bits = (material_pipeline_kind_rank(material_kind) as u64) << 4;
    let packed_bits = u64::from(packed_lod) << 8;
    let custom_bits = match material_kind {
        MaterialPipelineKind::Custom(token) => (*token as u64) << 9,
        _ => 0u64,
    };
    path_bits | top_bits | sided_bits | alpha_bits | rank_bits | packed_bits | custom_bits
}

#[inline]
pub(super) fn render_state_key(
    pipeline_key: u64,
    texture_slot: u64,
    mesh_index_start: u32,
    mesh_base_vertex: i32,
    draw_on_top: bool,
    alpha_mode: u8,
    mesh_blend: bool,
) -> RenderStateKey {
    let batch_kind = if draw_on_top {
        RenderBatchKind::Overlay
    } else if mesh_blend {
        RenderBatchKind::MeshBlend
    } else if alpha_mode != 0 {
        RenderBatchKind::Alpha
    } else {
        RenderBatchKind::Opaque
    };
    RenderStateKey {
        pipeline_key,
        texture_slot,
        mesh_index_start,
        mesh_base_vertex,
        batch_kind,
    }
}

#[inline]
pub(super) fn same_draw_except_model(a: &Draw3DInstance, b: &Draw3DInstance) -> bool {
    a.node == b.node
        && a.kind == b.kind
        && a.surfaces == b.surfaces
        && a.skeleton == b.skeleton
        && a.blend_shape_weights == b.blend_shape_weights
        && a.dense_multimesh == b.dense_multimesh
        && a.meshlet_override == b.meshlet_override
        && a.lod == b.lod
        && a.blend == b.blend
        && a.cast_shadows == b.cast_shadows
        && a.receive_shadows == b.receive_shadows
}

/// Cheap identity check for a dense multimesh's instance pose list. The retained
/// producer reuses the same `Arc` when poses do not change, so `Arc::ptr_eq`
/// hits the fast path; the deep compare stays as a correctness fallback.
#[inline]
pub(super) fn same_dense_instances(a: &DenseMultiMeshDraw3D, b: &DenseMultiMeshDraw3D) -> bool {
    a.instance_scale == b.instance_scale
        && (Arc::ptr_eq(&a.instances, &b.instances) || a.instances == b.instances)
}

/// True when `a`/`b` are the same multimesh draw except possibly `node_model`.
/// Instances (poses + scale) and material/blend must be unchanged; only the
/// draw's world transform may differ. Such a draw is patchable in the
/// transform-only path (instances are relative to the draw model in the shader,
/// so only the `MultiMeshDrawParamGpu` model rows need a rewrite).
#[inline]
pub(super) fn same_multimesh_except_node_model(a: &Draw3DInstance, b: &Draw3DInstance) -> bool {
    let (Some(dense_a), Some(dense_b)) = (a.dense_multimesh.as_ref(), b.dense_multimesh.as_ref())
    else {
        return false;
    };
    a.node == b.node
        && a.kind == b.kind
        && a.surfaces == b.surfaces
        && a.skeleton == b.skeleton
        && a.blend_shape_weights == b.blend_shape_weights
        && a.meshlet_override == b.meshlet_override
        && a.lod == b.lod
        && a.blend == b.blend
        && a.cast_shadows == b.cast_shadows
        && a.receive_shadows == b.receive_shadows
        && same_dense_instances(dense_a, dense_b)
}

/// Per-draw classification for the transform-only fast path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TransformOnlyDrawKind {
    /// Single-instance regular draw; only its model row may differ.
    RegularSingle,
    /// Dense multimesh w/ unchanged poses; only its node_model may differ.
    Multimesh,
}

/// Classify one draw pair for the transform-only path, or `None` if the pair
/// forces a full rebuild (topology/material/instance-count change).
#[inline]
pub(super) fn classify_transform_only_draw(
    prev: &Draw3DInstance,
    next: &Draw3DInstance,
) -> Option<TransformOnlyDrawKind> {
    if next.dense_multimesh.is_some() {
        if same_multimesh_except_node_model(prev, next) {
            return Some(TransformOnlyDrawKind::Multimesh);
        }
        return None;
    }
    // Multimesh cannot flip to a regular draw and stay transform-only.
    if prev.dense_multimesh.is_some() {
        return None;
    }
    if prev.instance_mats.len() == 1
        && next.instance_mats.len() == 1
        && same_draw_except_model(prev, next)
    {
        return Some(TransformOnlyDrawKind::RegularSingle);
    }
    None
}

/// Whole-scene decision: every draw pair must classify, and at least one draw
/// must actually be present. Returns per-draw kinds when the transform-only
/// path is valid.
pub(super) fn classify_transform_only_scene(
    prev: &[Draw3DInstance],
    next: &[Draw3DInstance],
    out: &mut Vec<TransformOnlyDrawKind>,
) -> bool {
    out.clear();
    if prev.len() != next.len() || next.is_empty() {
        return false;
    }
    for (p, n) in prev.iter().zip(next.iter()) {
        match classify_transform_only_draw(p, n) {
            Some(kind) => out.push(kind),
            None => {
                out.clear();
                return false;
            }
        }
    }
    true
}

impl Gpu3D {
    pub(super) fn rebuild_batch_views(&mut self) {
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
            // below the cutoff for mode 1. Blend (2) stays out.
            let derived_depth_safe = !batch.material_kind.uses_custom_shader();
            if derived_depth_safe
                && !batch.draw_on_top
                && batch.casts_shadows
                && batch.alpha_mode != 2
            {
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
pub(super) fn debug_color(seed: u64) -> [f32; 4] {
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;

    let h = ((x & 0xFFFF) as f32) / 65535.0;
    hsv_to_rgb(h, 0.75, 0.95)
}

pub(super) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 4] {
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

    fn draw(node: u64, layers: BitMask, mask: BitMask, instances: usize) -> Draw3DInstance {
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

    #[test]
    fn blend_resolve_requires_matching_target() {
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
    fn blend_resolve_respects_default_all_layers() {
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
    fn blend_resolve_uses_receiver_layers_without_receiver_fade() {
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
    fn blend_resolve_treats_all_mask_as_ignore_all() {
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
    fn blend_resolve_allows_multimesh_self_interaction() {
        let draws = [draw(1, BitMask::with([3]), BitMask::NONE, 2)];
        let mut out = Vec::new();
        resolve_mesh_blends(&draws, &mut out);
        assert!(resolved_mesh_blend_active(out[0]));
    }

    #[test]
    fn blend_resolve_bucket_path_handles_large_sparse_layers() {
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
    fn blend_resolve_preserves_normal_blending_flag() {
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
    fn blend_resolve_keeps_normal_blending_opt_in() {
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
    fn blend_resolve_uses_source_params() {
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
    fn material_params_sets_normal_blend_flag_only_when_resolved() {
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
    fn material_params_allow_normal_blend_without_screen_alpha() {
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
            },
        );
        let flags = (built.rigid_meta.material.packed_material_params >> 3) & 0x1fff;
        assert_eq!(flags & MATERIAL_FLAG_MESH_BLEND, 0);
        assert_ne!(flags & MATERIAL_FLAG_NORMAL_BLEND, 0);
    }

    fn dense_pose(pos: [f32; 3]) -> perro_render_bridge::DenseInstancePose3D {
        perro_render_bridge::DenseInstancePose3D {
            position: pos,
            scale: [1.0, 1.0, 1.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            has_blend_shape_weight_override: false,
            blend_shape_weights: Arc::from([]),
        }
    }

    fn multimesh_draw(
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
    fn transform_only_scene_takes_fast_path_w_multimesh_and_moved_regular() {
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
    fn transform_only_scene_falls_back_when_multimesh_poses_change() {
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
    fn same_dense_instances_hits_arc_ptr_fast_path() {
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

    fn meshlet_push(index_start: u32, instance_start: u32, instance_count: u32) -> DrawBatchPush {
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
    fn push_draw_batch_never_merges_shared_span_meshlet_batches() {
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
    fn mirrored_winding_flag_tracks_odd_negative_axes() {
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

    fn bounds_batch(instance_start: u32, instance_count: u32, radius: f32) -> DrawBatch {
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

    fn instance_at(pos: [f32; 3]) -> TransformInstanceGpu {
        TransformInstanceGpu {
            model_row_0: [1.0, 0.0, 0.0, pos[0]],
            model_row_1: [0.0, 1.0, 0.0, pos[1]],
            model_row_2: [0.0, 0.0, 1.0, pos[2]],
        }
    }

    #[test]
    fn enclose_spheres_contains_both_inputs() {
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
    fn batch_merged_world_sphere_covers_every_instance() {
        let transforms = [
            instance_at([-10.0, 0.0, 0.0]),
            instance_at([0.0, 0.0, 0.0]),
            instance_at([10.0, 0.0, 0.0]),
        ];
        let batch = bounds_batch(0, 3, 1.0);
        let (center, radius) = batch_merged_world_sphere(&batch, &transforms).unwrap();
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
    fn multi_instance_cull_rows_emit_world_sphere_with_identity_model() {
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
