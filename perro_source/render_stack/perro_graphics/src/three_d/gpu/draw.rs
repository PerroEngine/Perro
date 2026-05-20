use super::*;

pub(super) struct DrawBatchPush {
    pub(super) render_path: RenderPath3D,
    pub(super) mesh: MeshRange,
    pub(super) instance_start: u32,
    pub(super) instance_count: u32,
    pub(super) double_sided: bool,
    pub(super) material_kind: MaterialPipelineKind,
    pub(super) alpha_mode: u8,
    pub(super) base_color_texture_slot: u32,
    pub(super) local_bounds: ([f32; 3], f32),
    pub(super) occlusion_query: Option<u32>,
    pub(super) disable_hiz_occlusion: bool,
    pub(super) casts_shadows: bool,
    pub(super) mesh_blend: bool,
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
        material_kind,
        alpha_mode,
        base_color_texture_slot,
        local_bounds,
        occlusion_query,
        disable_hiz_occlusion,
        casts_shadows,
        mesh_blend,
        mesh_blend_depth,
        blend_layers,
        blend_mask,
    } = batch;
    if instance_count == 0 {
        return;
    }
    let state_key =
        draw_batch_state_key(render_path, false, double_sided, alpha_mode, &material_kind);
    let render_state = render_state_key(
        state_key,
        base_color_texture_slot,
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
            && prev.double_sided == double_sided
            && prev.material_kind == material_kind
            && prev.alpha_mode == alpha_mode
            && !prev.draw_on_top
            && prev.base_color_texture_slot == base_color_texture_slot
            && prev.occlusion_query.is_none()
            && prev.casts_shadows == casts_shadows
            && prev.mesh_blend == mesh_blend
            && prev.mesh_blend_depth == mesh_blend_depth
            && prev.blend_layers == blend_layers
            && prev.blend_mask == blend_mask;
        if same_mesh && same_batch_state && prev_end == instance_start {
            prev.instance_count = prev.instance_count.saturating_add(instance_count);
            prev.disable_hiz_occlusion |= disable_hiz_occlusion;
            // Multiple instances do not share one tight bound in this path.
            if prev.instance_count > 1 {
                prev.local_center = [0.0, 0.0, 0.0];
                prev.local_radius = 1.0e9;
                prev.disable_hiz_occlusion = true;
            } else {
                prev.local_center = local_center;
                prev.local_radius = local_radius.max(0.0);
            }
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
        double_sided,
        material_kind,
        alpha_mode,
        draw_on_top: false,
        base_color_texture_slot,
        local_center,
        local_radius: local_radius.max(0.0),
        occlusion_query,
        disable_hiz_occlusion,
        casts_shadows,
        mesh_blend,
        mesh_blend_depth,
        blend_layers,
        blend_mask,
        order_index,
    });
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
    pub(super) material: MaterialInstanceGpu,
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
fn pack_mesh_blend_params(blend: MeshBlendOptions3D) -> u32 {
    pack_u8_lanes(
        quantize_unorm8_range(blend.distance, 16.0),
        quantize_unorm8_range(blend.min_distance, 16.0),
        quantize_unorm8(blend.noise_factor),
        quantize_unorm8_range(blend.noise_scale, 64.0),
    )
}

pub(super) fn resolve_mesh_blends(draws: &[Draw3DInstance], out: &mut Vec<ResolvedMeshBlend>) {
    out.clear();
    out.resize(draws.len(), ResolvedMeshBlend::default());

    for (index, draw) in draws.iter().enumerate() {
        if !draw.blend.active() || !matches!(draw.kind, Draw3DKind::Mesh(_)) {
            continue;
        }
        let self_interacts = draw
            .dense_multimesh
            .as_ref()
            .map(|dense| dense.instances.len() > 1)
            .unwrap_or_else(|| draw.instance_mats.len() > 1);
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
    if params.flat_shading {
        material_flags |= MATERIAL_FLAG_FLAT_SHADING;
    }
    if params.base_color_texture != MATERIAL_TEXTURE_NONE {
        material_flags |= MATERIAL_FLAG_HAS_BASE_COLOR_TEXTURE;
    }
    let blend_active = resolved_mesh_blend_active(mesh_blend);
    let packed_blend_params = if blend_active && !debug_view {
        if resolved_mesh_blend_screen_blending(mesh_blend) {
            material_flags |= MATERIAL_FLAG_MESH_BLEND;
        }
        if resolved_mesh_blend_normal_blending(mesh_blend) {
            material_flags |= MATERIAL_FLAG_NORMAL_BLEND;
        }
        mesh_blend.packed_params
    } else {
        0
    };

    BuiltInstanceParts {
        transform: TransformInstanceGpu {
            model_row_0: [model[0][0], model[1][0], model[2][0], model[3][0]],
            model_row_1: [model[0][1], model[1][1], model[2][1], model[3][1]],
            model_row_2: [model[0][2], model[1][2], model[2][2], model[3][2]],
        },
        material: MaterialInstanceGpu {
            packed_color: pack_unorm4x8(color),
            packed_pbr_params_0,
            packed_pbr_params_1: packed_pbr_params_1 | packed_blend_params,
            packed_emissive: pack_unorm4x8([
                emissive_factor[0],
                emissive_factor[1],
                emissive_factor[2],
                1.0,
            ]),
            packed_material_params: pack_material_params(
                params.alpha_mode,
                params.alpha_cutoff,
                params.double_sided,
                material_flags,
            ),
        },
        rigid_meta: RigidInstanceMetaGpu {
            custom_params: [custom_params_offset, custom_params_len],
        },
        skinned_meta: SkinnedInstanceMetaGpu {
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
            RenderBatchKind::Alpha | RenderBatchKind::MeshBlend | RenderBatchKind::Overlay => {
                a.order_index.cmp(&b.order_index)
            }
        })
}

#[inline]
pub(super) fn draw_batches_sorted(batches: &[DrawBatch]) -> bool {
    batches
        .windows(2)
        .all(|pair| compare_draw_batch_keys(&pair[0], &pair[1]) != Ordering::Greater)
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
    material_kind: &MaterialPipelineKind,
) -> u64 {
    let path_bits = match path {
        RenderPath3D::Rigid => 0u64,
        RenderPath3D::Skinned => 1u64,
    };
    let top_bits = u64::from(draw_on_top) << 1;
    let sided_bits = u64::from(double_sided) << 2;
    let alpha_bits = u64::from(alpha_mode == 2) << 3;
    let rank_bits = (material_pipeline_kind_rank(material_kind) as u64) << 4;
    let custom_bits = match material_kind {
        MaterialPipelineKind::Custom(token) => (*token as u64) << 9,
        _ => 0u64,
    };
    path_bits | top_bits | sided_bits | alpha_bits | rank_bits | custom_bits
}

#[inline]
pub(super) fn render_state_key(
    pipeline_key: u64,
    texture_slot: u32,
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
        && a.meshlet_override == b.meshlet_override
        && a.lod == b.lod
        && a.blend == b.blend
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
        for (index, batch) in self.draw_batches.iter().enumerate() {
            match batch.render_state.batch_kind {
                RenderBatchKind::Opaque => self.opaque_batch_indices.push(index),
                RenderBatchKind::Alpha => self.alpha_batch_indices.push(index),
                RenderBatchKind::MeshBlend => self.mesh_blend_batch_indices.push(index),
                RenderBatchKind::Overlay => self.overlay_batch_indices.push(index),
            }
            if !batch.draw_on_top && batch.casts_shadows && batch.alpha_mode == 0 {
                self.shadow_batch_indices.push(index);
            }
            if !batch.draw_on_top && batch.alpha_mode == 0 && !batch.mesh_blend {
                self.depth_prepass_batch_indices.push(index);
            }
            if !batch.draw_on_top
                && batch.alpha_mode == 0
                && !batch.mesh_blend
                && batch.mesh_blend_depth
            {
                self.mesh_blend_depth_batch_indices.push(index);
            }
        }
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
        };
        let built = build_instance(
            glam::Mat4::IDENTITY.to_cols_array_2d(),
            &material,
            base_args,
        );
        let flags = (built.material.packed_material_params >> 3) & 0x1fff;
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
        let flags = (built.material.packed_material_params >> 3) & 0x1fff;
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
            },
        );
        let flags = (built.material.packed_material_params >> 3) & 0x1fff;
        assert_eq!(flags & MATERIAL_FLAG_MESH_BLEND, 0);
        assert_ne!(flags & MATERIAL_FLAG_NORMAL_BLEND, 0);
    }
}
