mod regular {
    // The preludes shared with the offline shader checker (`perro doctor`,
    // static builds) live in perro_wgsl so devtools can compose game shaders
    // without linking wgpu. Everything below is renderer-only.
    #[allow(unused_imports, reason = "prelude_wgsl only feeds parity tests")]
    pub use perro_wgsl::compose::{
        multimesh_wgsl, prelude_rigid_wgsl, prelude_skinned_wgsl, prelude_wgsl,
    };

    pub const MATERIAL_STANDARD_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/material_standard.wgsl");
    pub const MATERIAL_UNLIT_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/material_unlit.wgsl");
    pub const MATERIAL_TOON_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/material_toon.wgsl");
    pub const DEPTH_PREPASS_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/depth_prepass.wgsl");
    pub const DEPTH_PREPASS_RIGID_WGSL: &str = perro_macros::minified_wgsl!(concat!(
        include_str!("shaders/vertex_modifiers_depth.wgsl"),
        include_str!("shaders/depth_prepass_rigid.wgsl"),
    ));
    pub const DEPTH_PREPASS_SKINNED_WGSL: &str = perro_macros::minified_wgsl!(concat!(
        include_str!("shaders/vertex_modifiers_depth.wgsl"),
        include_str!("shaders/depth_prepass_skinned.wgsl"),
    ));
    pub const MESH_BLEND_SCREEN_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/mesh_blend_screen.wgsl");
}

mod culling {
    pub const FRUSTUM_CULL_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/frustum_cull.wgsl");
    pub const HIZ_DEPTH_COPY_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/hiz_depth_copy.wgsl");
    pub const HIZ_DOWNSAMPLE_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/hiz_downsample.wgsl");
    pub const HIZ_DOWNSAMPLE_SPD_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/hiz_downsample_spd.wgsl");
    pub const HIZ_OCCLUSION_CULL_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/hiz_occlusion_cull.wgsl");
    pub const MULTIMESH_CULL_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/multimesh_cull.wgsl");
    pub const INDIRECT_COMPACT_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/indirect_compact.wgsl");
}

use perro_wgsl::compose::{
    build_custom_material_shader_with_prelude as compose_custom_material,
    build_custom_multimesh_material_shader as compose_custom_multimesh,
};
pub use perro_wgsl::compose::{
    build_material_shader, build_material_shader_with_prelude, build_sky_shader,
    prelude_rigid_wgsl, prelude_skinned_wgsl, sanitize_reserved_meta_identifier,
};

/// Runtime lighting mode mapped onto the composition-level output mode.
#[inline]
fn material_output(
    lighting: perro_render_bridge::CustomMaterialLighting3D,
) -> perro_wgsl::compose::MaterialOutput {
    match lighting {
        perro_render_bridge::CustomMaterialLighting3D::Standard => {
            perro_wgsl::compose::MaterialOutput::Surface
        }
        _ => perro_wgsl::compose::MaterialOutput::Final,
    }
}

#[inline]
pub fn build_custom_material_shader_with_prelude(
    prelude_wgsl: &str,
    material_wgsl: &str,
    lighting: perro_render_bridge::CustomMaterialLighting3D,
) -> String {
    compose_custom_material(prelude_wgsl, material_wgsl, material_output(lighting))
}

#[inline]
pub fn build_custom_multimesh_material_shader(
    material_wgsl: &str,
    lighting: perro_render_bridge::CustomMaterialLighting3D,
) -> String {
    compose_custom_multimesh(material_wgsl, material_output(lighting))
}

#[inline]
pub fn build_sky_shader_with_passes(
    passes: &[(String, &[perro_structs::CustomPostParam])],
) -> String {
    let encoded = passes
        .iter()
        .map(|(source, params)| (source.clone(), encoded_sky_param_values(params)))
        .collect::<Vec<_>>();
    perro_wgsl::compose::build_sky_shader_with_passes(&encoded)
}

fn encoded_sky_param_values(
    params: &[perro_structs::CustomPostParam],
) -> perro_wgsl::compose::SkyPassParams {
    let mut out = [[0.0f32; 4]; 16];
    for (slot, param) in out.iter_mut().zip(params) {
        *slot = encode_custom_param_value(&param.value);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MaterialShaderFeatures(u16);

impl MaterialShaderFeatures {
    const BASE_TEXTURE: u16 = 1 << 0;
    const METALLIC_ROUGHNESS_TEXTURE: u16 = 1 << 1;
    const NORMAL_TEXTURE: u16 = 1 << 2;
    const OCCLUSION_TEXTURE: u16 = 1 << 3;
    const EMISSIVE_TEXTURE: u16 = 1 << 4;
    const RECEIVE_SHADOWS: u16 = 1 << 5;
    const ALPHA_SHIFT: u16 = 6;
    const VERTEX_MODIFIERS: u16 = 1 << 8;

    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        base_texture: bool,
        metallic_roughness_texture: bool,
        normal_texture: bool,
        occlusion_texture: bool,
        emissive_texture: bool,
        receive_shadows: bool,
        alpha_mode: u8,
        vertex_modifiers: bool,
    ) -> Self {
        let alpha_mode = if alpha_mode > 2 { 2 } else { alpha_mode };
        let mut bits = (alpha_mode as u16) << Self::ALPHA_SHIFT;
        if base_texture {
            bits |= Self::BASE_TEXTURE;
        }
        if metallic_roughness_texture {
            bits |= Self::METALLIC_ROUGHNESS_TEXTURE;
        }
        if normal_texture {
            bits |= Self::NORMAL_TEXTURE;
        }
        if occlusion_texture {
            bits |= Self::OCCLUSION_TEXTURE;
        }
        if emissive_texture {
            bits |= Self::EMISSIVE_TEXTURE;
        }
        if receive_shadows {
            bits |= Self::RECEIVE_SHADOWS;
        }
        if vertex_modifiers {
            bits |= Self::VERTEX_MODIFIERS;
        }
        Self(bits)
    }

    #[inline]
    pub(crate) const fn bits(self) -> u16 {
        self.0
    }

    #[inline]
    const fn contains(self, bit: u16) -> bool {
        self.0 & bit != 0
    }

    #[inline]
    const fn alpha_mode(self) -> u8 {
        ((self.0 >> Self::ALPHA_SHIFT) & 0x3) as u8
    }
}

pub(crate) fn create_standard_shader_module_rigid_variant(
    device: &wgpu::Device,
    kind: BuiltinShaderKind,
    features: MaterialShaderFeatures,
) -> wgpu::ShaderModule {
    create_builtin_shader_module_variant(
        device,
        regular::prelude_rigid_wgsl(),
        kind,
        features,
        "perro_mesh_builtin_rigid_variant",
    )
}

pub(crate) fn create_standard_shader_module_skinned_variant(
    device: &wgpu::Device,
    kind: BuiltinShaderKind,
    features: MaterialShaderFeatures,
) -> wgpu::ShaderModule {
    create_builtin_shader_module_variant(
        device,
        regular::prelude_skinned_wgsl(),
        kind,
        features,
        "perro_mesh_builtin_skinned_variant",
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinShaderKind {
    Standard,
    Unlit,
    Toon,
}

fn create_builtin_shader_module_variant(
    device: &wgpu::Device,
    prelude: &str,
    kind: BuiltinShaderKind,
    features: MaterialShaderFeatures,
    label: &'static str,
) -> wgpu::ShaderModule {
    let wgsl = build_builtin_material_shader_variant(prelude, kind, features);
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    })
}

fn build_builtin_material_shader_variant(
    prelude: &str,
    kind: BuiltinShaderKind,
    features: MaterialShaderFeatures,
) -> String {
    let material = match kind {
        BuiltinShaderKind::Standard => regular::MATERIAL_STANDARD_WGSL,
        BuiltinShaderKind::Unlit => regular::MATERIAL_UNLIT_WGSL,
        BuiltinShaderKind::Toon => regular::MATERIAL_TOON_WGSL,
    };
    let source = build_material_shader_with_prelude(prelude, material);
    [
        (
            "/*__PERRO_STD_BASE_TEXTURE__*/",
            features.contains(MaterialShaderFeatures::BASE_TEXTURE),
        ),
        (
            "/*__PERRO_STD_METALLIC_ROUGHNESS_TEXTURE__*/",
            features.contains(MaterialShaderFeatures::METALLIC_ROUGHNESS_TEXTURE),
        ),
        (
            "/*__PERRO_STD_NORMAL_TEXTURE__*/",
            features.contains(MaterialShaderFeatures::NORMAL_TEXTURE),
        ),
        (
            "/*__PERRO_STD_OCCLUSION_TEXTURE__*/",
            features.contains(MaterialShaderFeatures::OCCLUSION_TEXTURE),
        ),
        (
            "/*__PERRO_STD_EMISSIVE_TEXTURE__*/",
            features.contains(MaterialShaderFeatures::EMISSIVE_TEXTURE),
        ),
        (
            "/*__PERRO_STD_RECEIVE_SHADOWS__*/",
            features.contains(MaterialShaderFeatures::RECEIVE_SHADOWS),
        ),
        ("/*__PERRO_STD_ALPHA_MASK__*/", features.alpha_mode() == 1),
        ("/*__PERRO_STD_ALPHA_OPAQUE__*/", features.alpha_mode() == 0),
    ]
    .into_iter()
    .fold(source, |source, (marker, enabled)| {
        source.replace(marker, if enabled { "true ||" } else { "false &&" })
    })
    .replace(
        "/*__PERRO_BUILTIN_VERTEX_MODIFIERS__*/",
        if features.contains(MaterialShaderFeatures::VERTEX_MODIFIERS) {
            "false &&"
        } else {
            "true ||"
        },
    )
}

#[inline]
pub fn create_mesh_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_instanced"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader(regular::MATERIAL_STANDARD_WGSL).into(),
        ),
    })
}

#[inline]
pub fn create_mesh_shader_module_rigid(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_instanced_rigid"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::prelude_rigid_wgsl(),
                regular::MATERIAL_STANDARD_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_mesh_shader_module_rigid_packed_lod(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_instanced_rigid_packed_lod"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                &build_packed_lod_rigid_prelude(),
                regular::MATERIAL_STANDARD_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_unlit_shader_module_rigid(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_unlit_rigid"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::prelude_rigid_wgsl(),
                regular::MATERIAL_UNLIT_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_toon_shader_module_rigid(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_toon_rigid"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::prelude_rigid_wgsl(),
                regular::MATERIAL_TOON_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_mesh_shader_module_skinned(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_instanced_skinned"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::prelude_skinned_wgsl(),
                regular::MATERIAL_STANDARD_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_unlit_shader_module_skinned(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_unlit_skinned"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::prelude_skinned_wgsl(),
                regular::MATERIAL_UNLIT_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_toon_shader_module_skinned(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_toon_skinned"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::prelude_skinned_wgsl(),
                regular::MATERIAL_TOON_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_unlit_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_unlit"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader(regular::MATERIAL_UNLIT_WGSL).into(),
        ),
    })
}

#[inline]
pub fn create_toon_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_toon"),
        source: wgpu::ShaderSource::Wgsl(build_material_shader(regular::MATERIAL_TOON_WGSL).into()),
    })
}

// Packed-LOD patches run against whitespace-minified sources (perro_wgsl's
// optimize_source collapses every whitespace run to one space), so anchors
// must be single-line space-separated. A missing anchor panics instead of
// silently returning the unpatched (non-packed) module.
fn packed_lod_replace(source: String, anchor: &str, replacement: &str) -> String {
    assert!(
        source.contains(anchor),
        "packed-lod shader patch anchor drifted: `{anchor}`"
    );
    source.replace(anchor, replacement)
}

fn build_packed_lod_rigid_prelude() -> String {
    let base = regular::prelude_rigid_wgsl();
    let wgsl = packed_lod_replace(
        base.to_string(),
        "@group(0) @binding(5) var<storage, read> blend_shape_instances: array<BlendShapeInstance>;",
        "@group(0) @binding(5) var<storage, read> blend_shape_instances: array<BlendShapeInstance>; @group(0) @binding(6) var<storage, read> packed_lod_params: array<PackedLodParam>;",
    );
    let wgsl = packed_lod_replace(
        wgsl,
        "struct VertexInput { @location(0) pos: vec3<f32>,",
        "struct VertexInput { @location(0) pos: vec4<f32>,",
    );
    let wgsl = packed_lod_replace(
        wgsl,
        "@location(13) @interpolate(flat) custom_params: vec2<u32>, };",
        "@location(13) @interpolate(flat) custom_params: vec2<u32>, @location(14) @interpolate(flat) packed_lod_param_id: u32, };",
    );
    let wgsl = packed_lod_replace(
        wgsl,
        "struct BlendShapeDelta { position_delta: vec3<f32>, packed_normal_delta: u32, };",
        "struct PackedLodParam { pos_min: vec4<f32>, pos_extent: vec4<f32>, uv_min_extent: vec4<f32>, }; struct BlendShapeDelta { position_delta: vec3<f32>, packed_normal_delta: u32, };",
    );
    let wgsl = packed_lod_replace(wgsl, "var out_pos = v.pos;", "var out_pos = v.pos.xyz;");
    let wgsl = packed_lod_replace(
        wgsl,
        "return VertexInput(out_pos, vec4<f32>(normalize(out_normal), 0.0), v.uv, v.paint_uv);",
        "return VertexInput(vec4<f32>(out_pos, 0.0), vec4<f32>(normalize(out_normal), 0.0), v.uv, v.paint_uv);",
    );
    let wgsl = packed_lod_replace(
        wgsl,
        "let blended = perro_apply_blend_shapes(v, vertex_index, instance_index);",
        "let packed_lod = packed_lod_params[inst.packed_lod_param_id]; var decoded_v = v; decoded_v.pos = vec4<f32>(packed_lod.pos_min.xyz + v.pos.xyz * packed_lod.pos_extent.xyz, 0.0); decoded_v.uv = packed_lod.uv_min_extent.xy + v.uv * packed_lod.uv_min_extent.zw; let blended = perro_apply_blend_shapes(decoded_v, vertex_index, instance_index);",
    );
    let wgsl = packed_lod_replace(
        wgsl,
        "let p = vec4<f32>(blended.pos, 1.0);",
        "let p = vec4<f32>(blended.pos.xyz, 1.0);",
    );
    // The rigid prelude forwards the raw vertex uv; packed vertices store
    // normalized uv, so route the decoded uv through instead.
    let wgsl = packed_lod_replace(
        wgsl,
        "out.uv = v.uv; out.paint_uv = v.paint_uv;",
        "out.uv = decoded_v.uv; out.paint_uv = v.paint_uv;",
    );
    assert_ne!(wgsl, base, "packed-lod rigid prelude patch was a no-op");
    assert!(wgsl.contains("packed_lod_params[inst.packed_lod_param_id]"));
    wgsl
}

fn build_packed_lod_depth_rigid_wgsl() -> String {
    let base = regular::DEPTH_PREPASS_RIGID_WGSL;
    let wgsl = packed_lod_replace(
        base.to_string(),
        "@group(0) @binding(5) var<storage, read> blend_shape_instances: array<BlendShapeInstance>;",
        "@group(0) @binding(5) var<storage, read> blend_shape_instances: array<BlendShapeInstance>; @group(0) @binding(6) var<storage, read> packed_lod_params: array<PackedLodParam>;",
    );
    let wgsl = packed_lod_replace(
        wgsl,
        "struct VertexInput { @location(0) pos: vec3<f32>, @location(1) normal: vec4<f32>, }",
        "struct VertexInput { @location(0) pos: vec4<f32>, @location(1) normal: vec4<f32>, }",
    );
    let wgsl = packed_lod_replace(
        wgsl,
        "struct InstanceInput { @location(4) model_row_0: vec4<f32>, @location(5) model_row_1: vec4<f32>, @location(6) model_row_2: vec4<f32>, @location(7) @interpolate(flat) packed_color: u32, @location(11) @interpolate(flat) packed_material_params: u32, @location(13) @interpolate(flat) custom_range: vec2<u32>, }",
        "struct InstanceInput { @location(4) model_row_0: vec4<f32>, @location(5) model_row_1: vec4<f32>, @location(6) model_row_2: vec4<f32>, @location(7) @interpolate(flat) packed_color: u32, @location(11) @interpolate(flat) packed_material_params: u32, @location(13) @interpolate(flat) custom_range: vec2<u32>, @location(14) @interpolate(flat) packed_lod_param_id: u32, }",
    );
    let wgsl = packed_lod_replace(
        wgsl,
        "struct BlendShapeDelta { position_delta: vec3<f32>, packed_normal_delta: u32, }",
        "struct PackedLodParam { pos_min: vec4<f32>, pos_extent: vec4<f32>, uv_min_extent: vec4<f32>, } struct BlendShapeDelta { position_delta: vec3<f32>, packed_normal_delta: u32, }",
    );
    let wgsl = packed_lod_replace(wgsl, "var pos = v.pos;", "var pos = v.pos.xyz;");
    let wgsl = packed_lod_replace(
        wgsl,
        "return VertexInput(pos, vec4<f32>(normalize(normal), 0.0));",
        "return VertexInput(vec4<f32>(pos, 0.0), vec4<f32>(normalize(normal), 0.0));",
    );
    let wgsl = packed_lod_replace(
        wgsl,
        "let blended = apply_blend_shapes(v, vertex_index, instance_index);",
        "let packed_lod = packed_lod_params[inst.packed_lod_param_id]; var decoded_v = v; decoded_v.pos = vec4<f32>(packed_lod.pos_min.xyz + v.pos.xyz * packed_lod.pos_extent.xyz, 0.0); let blended = apply_blend_shapes(decoded_v, vertex_index, instance_index);",
    );
    let wgsl = packed_lod_replace(
        wgsl,
        "let p = vec4<f32>(blended.pos, 1.0);",
        "let p = vec4<f32>(blended.pos.xyz, 1.0);",
    );
    assert_ne!(wgsl, base, "packed-lod depth rigid patch was a no-op");
    assert!(wgsl.contains("packed_lod_params[inst.packed_lod_param_id]"));
    wgsl
}

// Mask entry point appended to the depth-prepass shaders: writes the batch's
// blend id so the screen-space seam pass can find mesh boundaries. The cutout
// discard mirrors the depth-prepass fs_main.
const MESH_BLEND_MASK_FS_WGSL: &str = "
@group(1) @binding(0)
var<uniform> mesh_blend_mask_id: vec4<u32>;

@fragment
fn fs_mask(in: VertexOutput) -> @location(0) u32 {
    let alpha_mode = in.packed_material_params & 0x3u;
    if alpha_mode == 1u {
        let alpha = unpack_unorm8(in.packed_color, 24u);
        let cutoff = unpack_unorm8(in.packed_material_params, 16u);
        if alpha < cutoff {
            discard;
        }
    }
    return mesh_blend_mask_id.x;
}
";

/// Max shadow depth layers one multiview pass may cover. 6 = a point light's
/// cube faces, the biggest layer set the shadow encoder forms; cascades (4) and
/// spot lights (<=4) fit under it. The multiview shadow shaders declare the
/// view-proj array at this length, so every multiview pipeline shares one
/// uniform layout no matter how many views its pass actually uses.
pub const MAX_MULTIVIEW_SHADOW_VIEWS: usize = 6;

// Group 1 the shadow depth pipelines leave free (the depth pipeline layouts
// bind only group 0). Holds the view-proj of every layer in the multiview set;
// `perro_mv_view` is the private lane vs_main seeds from `@builtin(view_index)`
// so the vertex-modifier helper -- which is shared with the single-view depth
// path and takes no view argument -- can read it without a signature change.
const MULTIVIEW_SHADOW_PRELUDE_WGSL: &str = "\
struct PerroMultiviewShadow { view_proj: array<mat4x4<f32>, 6>, };
@group(1) @binding(0) var<uniform> perro_mv_shadow: PerroMultiviewShadow;
var<private> perro_mv_view: u32 = 0u;
";

fn multiview_replace(source: String, anchor: &str, replacement: &str) -> String {
    assert!(
        source.contains(anchor),
        "multiview shadow shader patch anchor drifted: `{anchor}`"
    );
    source.replace(anchor, replacement)
}

/// Rewrite a single-view depth shader into its multiview shadow twin.
///
/// Three edits, all on the minified source (whitespace already collapsed to
/// single spaces, so the anchors below are the source lines joined by one
/// space -- see `perro_wgsl::optimize_source`):
/// 1. prepend the group-1 view-proj array + the private view lane;
/// 2. project through `perro_mv_shadow.view_proj[perro_mv_view]` instead of the
///    single `scene.view_proj`;
/// 3. take `@builtin(view_index)` in `vs_main` and seed the private lane.
///
/// Every anchor is asserted, so a drifting source fails the build instead of
/// silently emitting a shader that writes all views with view 0's matrix.
fn build_multiview_shadow_wgsl(base: &str) -> String {
    let wgsl = multiview_replace(
        base.to_string(),
        "var clip = scene.view_proj * vec4<f32>(vertex.position, 1.0);",
        "var clip = perro_mv_shadow.view_proj[perro_mv_view] * vec4<f32>(vertex.position, 1.0);",
    );
    let wgsl = multiview_replace(
        wgsl,
        "@builtin(instance_index) instance_index: u32) -> VertexOutput { ",
        "@builtin(instance_index) instance_index: u32, @builtin(view_index) perro_view_index: u32) \
         -> VertexOutput { perro_mv_view = perro_view_index; ",
    );
    let mut out = String::with_capacity(MULTIVIEW_SHADOW_PRELUDE_WGSL.len() + wgsl.len());
    out.push_str(MULTIVIEW_SHADOW_PRELUDE_WGSL);
    out.push_str(&wgsl);
    out
}

#[inline]
pub fn create_shadow_depth_multiview_shader_module_rigid(
    device: &wgpu::Device,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_shadow_depth_multiview_rigid"),
        source: wgpu::ShaderSource::Wgsl(
            build_multiview_shadow_wgsl(regular::DEPTH_PREPASS_RIGID_WGSL).into(),
        ),
    })
}

#[inline]
pub fn create_shadow_depth_multiview_shader_module_rigid_packed_lod(
    device: &wgpu::Device,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_shadow_depth_multiview_rigid_packed_lod"),
        source: wgpu::ShaderSource::Wgsl(
            build_multiview_shadow_wgsl(&build_packed_lod_depth_rigid_wgsl()).into(),
        ),
    })
}

#[inline]
pub fn create_shadow_depth_multiview_shader_module_skinned(
    device: &wgpu::Device,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_shadow_depth_multiview_skinned"),
        source: wgpu::ShaderSource::Wgsl(
            build_multiview_shadow_wgsl(regular::DEPTH_PREPASS_SKINNED_WGSL).into(),
        ),
    })
}

fn build_mesh_blend_mask_wgsl(base: &str) -> String {
    let mut out = String::with_capacity(base.len() + MESH_BLEND_MASK_FS_WGSL.len() + 1);
    out.push_str(base);
    out.push('\n');
    out.push_str(MESH_BLEND_MASK_FS_WGSL);
    out
}

#[inline]
pub fn create_mesh_blend_mask_shader_module_rigid(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_blend_mask_rigid"),
        source: wgpu::ShaderSource::Wgsl(
            build_mesh_blend_mask_wgsl(regular::DEPTH_PREPASS_RIGID_WGSL).into(),
        ),
    })
}

#[inline]
pub fn create_mesh_blend_mask_shader_module_rigid_packed_lod(
    device: &wgpu::Device,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_blend_mask_rigid_packed_lod"),
        source: wgpu::ShaderSource::Wgsl(
            build_mesh_blend_mask_wgsl(&build_packed_lod_depth_rigid_wgsl()).into(),
        ),
    })
}

#[inline]
pub fn create_mesh_blend_mask_shader_module_skinned(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_blend_mask_skinned"),
        source: wgpu::ShaderSource::Wgsl(
            build_mesh_blend_mask_wgsl(regular::DEPTH_PREPASS_SKINNED_WGSL).into(),
        ),
    })
}

#[inline]
pub fn create_mesh_blend_screen_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_blend_screen"),
        source: wgpu::ShaderSource::Wgsl(regular::MESH_BLEND_SCREEN_WGSL.into()),
    })
}

#[inline]
pub fn create_depth_prepass_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_depth_prepass"),
        source: wgpu::ShaderSource::Wgsl(regular::DEPTH_PREPASS_WGSL.into()),
    })
}

#[inline]
pub fn create_depth_prepass_shader_module_rigid(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_depth_prepass_rigid"),
        source: wgpu::ShaderSource::Wgsl(regular::DEPTH_PREPASS_RIGID_WGSL.into()),
    })
}

#[inline]
pub fn create_depth_prepass_shader_module_rigid_packed_lod(
    device: &wgpu::Device,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_depth_prepass_rigid_packed_lod"),
        source: wgpu::ShaderSource::Wgsl(build_packed_lod_depth_rigid_wgsl().into()),
    })
}

#[inline]
pub fn create_depth_prepass_shader_module_skinned(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_depth_prepass_skinned"),
        source: wgpu::ShaderSource::Wgsl(regular::DEPTH_PREPASS_SKINNED_WGSL.into()),
    })
}

// Per-instance mesh-blend mask salt for the multimesh path: same-type
// instances get distinct blend ids (base + source_instance % 7) so their
// overlaps read as seams in the screen-space blend pass. Patched here on the
// composed (whitespace-collapsed) source instead of in perro_wgsl so the
// offline compose stays mask-agnostic. The salt rides an extra flat varying;
// fs_main simply never reads it. The `% 7u` modulus and the gating on
// `mesh_blend_mask_id.y` mirror MULTIMESH_SOURCE_ID_STRIDE and the staged
// mask-id uniform in gpu/mesh_blend_screen.rs.
fn build_multimesh_shader_wgsl() -> String {
    let wgsl = sanitize_reserved_meta_identifier(regular::multimesh_wgsl())
        .replace(
            "@location(12) @interpolate(flat) packed_emissive: u32, }; struct FragmentInput {",
            "@location(12) @interpolate(flat) packed_emissive: u32, \
             @location(13) @interpolate(flat) mask_salt: u32, }; struct FragmentInput {",
        )
        .replace(
            ") -> VertexOutput { let inst = perro_fetch_instance(instance_index); \
             return perro_multimesh_vs_main_base(v, inst, vertex_index); }",
            ") -> VertexOutput { let inst = perro_fetch_instance(instance_index); \
             var out = perro_multimesh_vs_main_base(v, inst, vertex_index); \
             out.mask_salt = visible_indices[instance_index] % 7u; return out; }",
        )
        .replace(
            "fn fs_mask(in: VertexOutput) -> @location(0) u32 { return mesh_blend_mask_id.x; }",
            "fn fs_mask(in: VertexOutput) -> @location(0) u32 { \
             return mesh_blend_mask_id.x + select(0u, in.mask_salt, mesh_blend_mask_id.y != 0u); }",
        );
    debug_assert_eq!(
        wgsl.matches("mask_salt").count(),
        3,
        "multimesh mask salt patch anchors drifted"
    );
    wgsl
}

#[inline]
pub fn create_multimesh_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_multimesh"),
        source: wgpu::ShaderSource::Wgsl(build_multimesh_shader_wgsl().into()),
    })
}

#[inline]
pub fn create_sky_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_sky3d"),
        source: wgpu::ShaderSource::Wgsl(build_sky_shader().into()),
    })
}

#[inline]
pub fn create_sky_shader_module_from_source(
    device: &wgpu::Device,
    source: String,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_sky3d_custom"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

fn encode_custom_param_value(value: &perro_structs::CustomPostParamValue) -> [f32; 4] {
    match value {
        perro_structs::CustomPostParamValue::F32(v) => [*v, 0.0, 0.0, 0.0],
        perro_structs::CustomPostParamValue::I32(v) => [*v as f32, 0.0, 0.0, 0.0],
        perro_structs::CustomPostParamValue::Bool(v) => [if *v { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        perro_structs::CustomPostParamValue::Vec2(v) => [v[0], v[1], 0.0, 0.0],
        perro_structs::CustomPostParamValue::Vec3(v) => [v[0], v[1], v[2], 0.0],
        perro_structs::CustomPostParamValue::Vec4(v) => *v,
    }
}

#[inline]
pub fn create_frustum_cull_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_frustum_cull"),
        source: wgpu::ShaderSource::Wgsl(culling::FRUSTUM_CULL_WGSL.into()),
    })
}

#[inline]
pub fn create_indirect_compact_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_indirect_compact"),
        source: wgpu::ShaderSource::Wgsl(culling::INDIRECT_COMPACT_WGSL.into()),
    })
}

#[inline]
pub fn create_multimesh_cull_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_multimesh_cull"),
        source: wgpu::ShaderSource::Wgsl(culling::MULTIMESH_CULL_WGSL.into()),
    })
}

#[inline]
pub fn create_hiz_depth_copy_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_hiz_depth_copy"),
        source: wgpu::ShaderSource::Wgsl(culling::HIZ_DEPTH_COPY_WGSL.into()),
    })
}

#[inline]
pub fn create_hiz_downsample_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_hiz_downsample"),
        source: wgpu::ShaderSource::Wgsl(culling::HIZ_DOWNSAMPLE_WGSL.into()),
    })
}

#[inline]
pub fn create_hiz_downsample_spd_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_hiz_downsample_spd"),
        source: wgpu::ShaderSource::Wgsl(culling::HIZ_DOWNSAMPLE_SPD_WGSL.into()),
    })
}

#[inline]
pub fn create_hiz_occlusion_cull_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_hiz_occlusion_cull"),
        source: wgpu::ShaderSource::Wgsl(culling::HIZ_OCCLUSION_CULL_WGSL.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::{Pod, Zeroable};
    use naga::valid::{Capabilities, ValidationFlags, Validator};
    use std::sync::mpsc;

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct TestVertex {
        pos: [f32; 3],
        normal: [f32; 3],
        uv: [f32; 2],
    }

    fn parse_and_validate(wgsl: &str, label: &str) {
        let module =
            naga::front::wgsl::parse_str(wgsl).unwrap_or_else(|err| panic!("{label}: {err}"));
        Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|err| panic!("{label}: {err}"));
    }

    /// The multiview patch is three string replacements on minified WGSL, the
    /// exact shape that has silently no-opped before (newline anchors vs
    /// minified source). Each `multiview_replace` asserts its own anchor, so a
    /// drift panics here rather than shipping a shadow map where every view
    /// writes with view 0's matrix.
    #[test]
    fn multiview_shadow_wgsl_patches_and_validates() {
        for (label, base) in [
            ("rigid", regular::DEPTH_PREPASS_RIGID_WGSL.to_string()),
            ("skinned", regular::DEPTH_PREPASS_SKINNED_WGSL.to_string()),
            ("packed_lod", build_packed_lod_depth_rigid_wgsl()),
        ] {
            let wgsl = build_multiview_shadow_wgsl(&base);
            assert!(
                wgsl.contains("@builtin(view_index) perro_view_index: u32"),
                "{label}: vs_main never takes view_index"
            );
            assert!(
                wgsl.contains("perro_mv_view = perro_view_index;"),
                "{label}: private view lane never seeded"
            );
            assert!(
                wgsl.contains("perro_mv_shadow.view_proj[perro_mv_view]"),
                "{label}: projection still single-view"
            );
            assert!(
                !wgsl.contains("scene.view_proj *"),
                "{label}: a single-view projection survived the patch"
            );
            let module = naga::front::wgsl::parse_str(&wgsl)
                .unwrap_or_else(|err| panic!("multiview {label}: {err}"));
            Validator::new(ValidationFlags::all(), Capabilities::MULTIVIEW)
                .validate(&module)
                .unwrap_or_else(|err| panic!("multiview {label}: {err}"));
        }
    }

    /// The WGSL declares the view-proj array at a literal length; the Rust
    /// uniform is sized from the const. They must agree or the bind group is
    /// short and every multiview pipeline fails validation.
    #[test]
    fn multiview_shadow_prelude_matches_view_count() {
        assert!(
            MULTIVIEW_SHADOW_PRELUDE_WGSL
                .contains(&format!("array<mat4x4<f32>, {MAX_MULTIVIEW_SHADOW_VIEWS}>")),
            "prelude array length drifted from MAX_MULTIVIEW_SHADOW_VIEWS"
        );
    }

    /// The single-view depth prepass shares its source with the shadow path and
    /// must NOT gain a multiview binding: it runs on a plain one-layer target.
    #[test]
    fn single_view_depth_wgsl_stays_multiview_free() {
        for base in [
            regular::DEPTH_PREPASS_RIGID_WGSL,
            regular::DEPTH_PREPASS_SKINNED_WGSL,
        ] {
            assert!(!base.contains("view_index"));
            assert!(base.contains("scene.view_proj *"));
        }
    }

    #[test]
    fn three_d_material_wgsl_parses() {
        for prelude in [
            regular::prelude_wgsl(),
            regular::prelude_rigid_wgsl(),
            regular::prelude_skinned_wgsl(),
        ] {
            for material in [
                regular::MATERIAL_STANDARD_WGSL,
                regular::MATERIAL_UNLIT_WGSL,
                regular::MATERIAL_TOON_WGSL,
            ] {
                let wgsl = build_material_shader_with_prelude(prelude, material);
                naga::front::wgsl::parse_str(&wgsl).expect("3d material wgsl parses");
            }
        }
    }

    #[test]
    fn builtin_variants_resolve_all_runtime_feature_checks() {
        let none = MaterialShaderFeatures::new(false, false, false, false, false, false, 0, false);
        let all = MaterialShaderFeatures::new(true, true, true, true, true, true, 1, true);
        assert_ne!(none, all);

        for (prelude, label) in [
            (regular::prelude_rigid_wgsl(), "rigid"),
            (regular::prelude_skinned_wgsl(), "skinned"),
        ] {
            for kind in [
                BuiltinShaderKind::Standard,
                BuiltinShaderKind::Unlit,
                BuiltinShaderKind::Toon,
            ] {
                for (features, suffix) in [(none, "none"), (all, "all")] {
                    let wgsl = build_builtin_material_shader_variant(prelude, kind, features);
                    assert!(!wgsl.contains("/*__PERRO_"));
                    parse_and_validate(&wgsl, &format!("{label} {suffix} {kind:?} variant"));
                }
            }
        }

        let none_wgsl = build_builtin_material_shader_variant(
            regular::prelude_rigid_wgsl(),
            BuiltinShaderKind::Standard,
            none,
        );
        assert!(none_wgsl.contains("if false && material.has_base_color_texture"));
        assert!(none_wgsl.contains("if true || material.alpha_mode == 0u"));
        assert!(none_wgsl.contains("if true || out_in.custom_range.x < 2u"));

        let all_wgsl = build_builtin_material_shader_variant(
            regular::prelude_rigid_wgsl(),
            BuiltinShaderKind::Standard,
            all,
        );
        assert!(all_wgsl.contains("if true || material.has_base_color_texture"));
        assert!(all_wgsl.contains("if true || material.receive_shadows"));
        assert!(all_wgsl.contains("if false && out_in.custom_range.x < 2u"));
    }

    #[test]
    fn packed_lod_material_wgsl_keeps_paint_uv() {
        let prelude = build_packed_lod_rigid_prelude();
        // The patch must actually change the module; a drifted anchor used to
        // no-op silently and ship the non-packed prelude.
        assert_ne!(prelude, regular::prelude_rigid_wgsl());
        assert!(prelude.contains(
            "@group(0) @binding(6) var<storage, read> packed_lod_params: array<PackedLodParam>;"
        ));
        assert!(prelude.contains("packed_lod_params[inst.packed_lod_param_id]"));
        assert!(prelude.contains("struct VertexInput { @location(0) pos: vec4<f32>,"));
        assert!(prelude.contains("out.uv = decoded_v.uv;"));
        let wgsl = build_material_shader_with_prelude(&prelude, regular::MATERIAL_STANDARD_WGSL);
        parse_and_validate(&wgsl, "packed lod paint uv");
        assert!(wgsl.contains("@location(15) paint_uv"));
        assert!(wgsl.contains("out.paint_uv = v.paint_uv"));
    }

    #[test]
    fn packed_lod_depth_rigid_wgsl_patch_applies_and_validates() {
        let wgsl = build_packed_lod_depth_rigid_wgsl();
        assert_ne!(wgsl, regular::DEPTH_PREPASS_RIGID_WGSL);
        assert!(wgsl.contains(
            "@group(0) @binding(6) var<storage, read> packed_lod_params: array<PackedLodParam>;"
        ));
        assert!(wgsl.contains("packed_lod_params[inst.packed_lod_param_id]"));
        assert!(wgsl.contains("@location(14) @interpolate(flat) packed_lod_param_id: u32"));
        assert!(wgsl.contains("struct VertexInput { @location(0) pos: vec4<f32>,"));
        parse_and_validate(&wgsl, "packed lod depth prepass");
    }

    #[test]
    fn custom_material_paint_uv_reads_in_all_paths() {
        // MultiMesh used to lack the paint_uv attribute/varying, so a custom
        // material touching in.paint_uv paniced with `invalid field accessor`.
        let material = r#"
fn shade_material(in: FragmentInput) -> vec4<f32> {
    return vec4<f32>(in.paint_uv, in.uv.x, 1.0);
}
"#;
        for (prelude, label) in [
            (regular::prelude_rigid_wgsl(), "rigid"),
            (regular::prelude_skinned_wgsl(), "skinned"),
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Raw,
            );
            parse_and_validate(&wgsl, &format!("custom material paint_uv ({label})"));
        }
        let multi = build_custom_multimesh_material_shader(
            material,
            perro_render_bridge::CustomMaterialLighting3D::Raw,
        );
        assert!(multi.contains("@location(15) paint_uv"));
        assert!(multi.contains("@location(14) paint_uv"));
        parse_and_validate(&multi, "custom multimesh material paint_uv");
    }

    #[test]
    fn unlit_material_samples_base_color_texture() {
        assert!(
            regular::MATERIAL_UNLIT_WGSL
                .contains("textureSample(material_base_color_tex, material_sampler, in.uv)")
        );
        assert!(regular::MATERIAL_UNLIT_WGSL.contains("color * base_sample"));
        assert!(regular::MATERIAL_UNLIT_WGSL.contains("perro_unlit("));
    }

    #[test]
    fn standard_material_uses_gltf_texture_channels_and_tangent_frame() {
        let wgsl = regular::MATERIAL_STANDARD_WGSL;
        assert!(wgsl.contains("roughness * mr.g"));
        assert!(wgsl.contains("metallic * mr.b"));
        assert!(wgsl.contains("textureSample(custom_image_tex_2, material_sampler, in.uv).r"));
        assert!(wgsl.contains("lit_emissive *= textureSample(custom_image_tex_3"));

        let prelude = regular::prelude_rigid_wgsl();
        assert!(prelude.contains("fn perro_fallback_tangent"));
        assert!(prelude.contains("var handedness = 1.0"));
        assert!(prelude.contains("cross(tangent_raw, bitangent_raw)"));
        assert!(prelude.contains("sampled.xy * scale"));
    }

    #[test]
    fn multimesh_standard_material_keeps_texture_parity() {
        let wgsl = regular::multimesh_wgsl();
        assert!(wgsl.contains("@location(12) uv: vec2<f32>"));
        assert!(wgsl.contains("roughness *= metallic_roughness.g"));
        assert!(wgsl.contains("metallic *= metallic_roughness.b"));
        assert!(wgsl.contains("fn perro_apply_multimesh_normal_map"));
        assert!(wgsl.contains("let sampled_ao = textureSample(custom_image_tex_2"));
        assert!(wgsl.contains("lit_emissive *= textureSample(custom_image_tex_3"));
        assert!(wgsl.contains("return shade_standard_multimesh(in)"));
        parse_and_validate(
            &sanitize_reserved_meta_identifier(wgsl),
            "multimesh standard texture parity",
        );
    }

    #[test]
    fn custom_material_standard_lighting_wrapper_wgsl_parses() {
        let material = "fn shade_material(in: FragmentInput) -> vec4<f32> { return vec4<f32>(in.normal_ws * 0.5 + vec3<f32>(0.5), 1.0); }";
        for prelude in [
            regular::prelude_rigid_wgsl(),
            regular::prelude_skinned_wgsl(),
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Standard,
            );
            assert!(wgsl.contains("perro_standard(in, base"));
            naga::front::wgsl::parse_str(&wgsl).expect("custom lit wrapper material wgsl parses");
        }
    }

    #[test]
    fn custom_material_frame_globals_validate() {
        // Locks the custom-shader frame-globals API: time, delta, frame index,
        // phase, and resolution must stay available in every prelude.
        let material = r#"
fn shade_vertex(out_in: VertexOutput) -> VertexOutput {
    var out = out_in;
    out.world_pos.y += sin(perro_time() * 2.0 + out.world_pos.x) * 0.1;
    return out;
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    let pulse = 0.5 + 0.5 * sin(perro_time_phase() * 6.28318);
    let px = in.frag_pos.xy * perro_inv_resolution();
    let speed = perro_delta_time() + perro_frame_index() * 0.0;
    return vec4<f32>(px * pulse, speed, 1.0);
}
"#;
        for prelude in [
            regular::prelude_rigid_wgsl(),
            regular::prelude_skinned_wgsl(),
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Raw,
            );
            parse_and_validate(&wgsl, "custom material frame globals");
        }
    }

    #[test]
    fn custom_material_stylized_helpers_validate() {
        let material = r#"
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let pixel_uv = perro_pixel_uv(in.uv, vec2<f32>(32.0));
    var color = custom_image_sample(in, 0u, pixel_uv).rgb;
    let lod = perro_distance_lod(in.world_pos, 5.0, 50.0, 4u);
    let grain = perro_paper_grain(in.uv, 128.0 / f32(lod + 1u), 0.08);
    let ink = perro_crosshatch(in.uv, 0.65, 24.0, 0.785398, 0.08);
    color = mix(color + vec3<f32>(grain), vec3<f32>(0.05), ink);
    color = perro_bayer_dither(color, in.frag_pos.xy, 0.08);
    color = perro_posterize(color, 5.0);
    color = perro_palette_snap(in, color, 1u, 16u);
    return perro_hand_drawn(in, vec4<f32>(color, 1.0), 4.0, 24.0, 0.08, vec3<f32>(0.0));
}
"#;
        for prelude in [
            regular::prelude_rigid_wgsl(),
            regular::prelude_skinned_wgsl(),
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Standard,
            );
            assert!(!wgsl.contains("let base = shade_material(in);\n    return perro_standard"));
            parse_and_validate(&wgsl, "custom material stylized helpers");
        }
        let multimesh_wgsl = build_custom_multimesh_material_shader(
            material,
            perro_render_bridge::CustomMaterialLighting3D::Standard,
        );
        assert!(
            !multimesh_wgsl.contains("let base = shade_material(in);\n    return perro_standard")
        );
        parse_and_validate(&multimesh_wgsl, "custom multimesh stylized helpers");
    }

    #[test]
    fn toon_material_uses_shared_lighting_helper_and_base_texture() {
        let wgsl = regular::MATERIAL_TOON_WGSL;
        assert!(wgsl.contains("perro_toon("));
        assert!(wgsl.contains("textureSample(material_base_color_tex"));
    }

    #[test]
    fn custom_material_raw_wrapper_wgsl_parses() {
        let material = "fn shade_material(in: FragmentInput) -> vec4<f32> { return vec4<f32>(in.normal_ws * 0.5 + vec3<f32>(0.5), 1.0); }";
        for prelude in [
            regular::prelude_rigid_wgsl(),
            regular::prelude_skinned_wgsl(),
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Raw,
            );
            assert!(!wgsl.contains("let base = shade_material(in);\n    return perro_standard"));
            naga::front::wgsl::parse_str(&wgsl).expect("custom raw material wgsl parses");
        }
    }

    #[test]
    fn custom_material_lit_helper_wgsl_parses() {
        let material = r#"
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = unpack_rgba8(in.packed_emissive).xyz;
    let pbr = decode_standard_pbr_params(in.packed_pbr_params_0, in.packed_pbr_params_1);
    return perro_lit_standard(in, color, pbr.x, pbr.y, pbr.z, emissive);
}
"#;
        for prelude in [
            regular::prelude_rigid_wgsl(),
            regular::prelude_skinned_wgsl(),
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Standard,
            );
            assert!(!wgsl.contains("let base = shade_material(in);\n    return perro_standard"));
            naga::front::wgsl::parse_str(&wgsl).expect("custom lit material wgsl parses");
        }
    }

    #[test]
    fn custom_material_shade_vertex_wgsl_validates() {
        let material = r#"
fn shade_vertex(out: VertexOutput) -> VertexOutput {
    let wobble = custom_v_param(out, 0u).x;
    var next = out;
    next.world_pos.y = next.world_pos.y + wobble;
    next.clip_pos.y = next.clip_pos.y + wobble;
    return next;
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    return vec4<f32>(color.rgb, perro_material_alpha(in, color.a));
}
"#;
        for (prelude_name, prelude) in [
            ("default", regular::prelude_wgsl()),
            ("rigid", regular::prelude_rigid_wgsl()),
            ("skinned", regular::prelude_skinned_wgsl()),
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Raw,
            );
            assert!(wgsl.contains("return shade_vertex(perro_vs_main_base"));
            parse_and_validate(
                &wgsl,
                &format!("custom shade_vertex material wgsl validates ({prelude_name})"),
            );
        }
    }

    #[test]
    fn custom_multimesh_material_wgsl_validates() {
        let material = r#"
fn shade_vertex(out: VertexOutput) -> VertexOutput {
    var next = out;
    next.world_pos.y = next.world_pos.y + custom_v_param(out, 0u).x;
    next.clip_pos.y = next.clip_pos.y + custom_v_param(out, 0u).x;
    return next;
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    let tint = custom_f_param(in, 0u);
    return vec4<f32>(tint.rgb + in.normal_ws * 0.05 + in.uv.xyx * 0.0, tint.a);
}
"#;
        let wgsl = build_custom_multimesh_material_shader(
            material,
            perro_render_bridge::CustomMaterialLighting3D::Raw,
        );
        assert!(wgsl.contains("return shade_vertex(perro_multimesh_vs_main_base"));
        assert!(wgsl.contains("return shade_material(in);"));
        parse_and_validate(&wgsl, "custom multimesh material wgsl validates");
    }

    #[test]
    fn custom_multimesh_and_single_mesh_shader_hooks_validate_same_material() {
        let material = r#"
fn shade_vertex(out: VertexOutput) -> VertexOutput {
    var next = out;
    let bend = custom_v_param(out, 0u).x;
    next.world_pos = next.world_pos + out.normal_ws * bend;
    next.clip_pos.x = next.clip_pos.x + bend * 0.001;
    return next;
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    let tint = custom_f_param(in, 1u);
    return vec4<f32>(tint.rgb + in.normal_ws * 0.1, tint.a);
}
"#;
        let single = build_custom_material_shader_with_prelude(
            regular::prelude_rigid_wgsl(),
            material,
            perro_render_bridge::CustomMaterialLighting3D::Raw,
        );
        let multi = build_custom_multimesh_material_shader(
            material,
            perro_render_bridge::CustomMaterialLighting3D::Raw,
        );

        assert!(single.contains("return shade_vertex(perro_vs_main_base"));
        assert!(multi.contains("return shade_vertex(perro_multimesh_vs_main_base"));
        assert!(single.contains("return shade_material(in);"));
        assert!(multi.contains("return shade_material(in);"));
        assert!(single.contains("fn custom_f_param"));
        assert!(multi.contains("fn custom_f_param"));
        assert!(single.contains("fn custom_v_param"));
        assert!(multi.contains("fn custom_v_param"));
        parse_and_validate(&single, "single mesh custom hooks validate");
        parse_and_validate(&multi, "multimesh custom hooks validate");
    }

    #[test]
    fn custom_material_shader_interface_has_no_meshlet_inputs() {
        let material = r#"
fn shade_vertex(out: VertexOutput) -> VertexOutput {
    var next = out;
    next.world_pos.x = next.world_pos.x + custom_v_param(out, 0u).x;
    return next;
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    return vec4<f32>(custom_f_param(in, 0u).xyz + in.uv.xyx, 1.0);
}
"#;
        let vertex_entry = "fn vs_main(v: VertexInput, inst: InstanceInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput";
        let fragment_entry = "fn fs_main(in: FragmentInput) -> @location(0) vec4<f32>";
        for prelude in [
            regular::prelude_wgsl(),
            regular::prelude_rigid_wgsl(),
            regular::prelude_skinned_wgsl(),
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Raw,
            );
            assert!(wgsl.contains(vertex_entry));
            assert!(wgsl.contains(fragment_entry));
            assert!(wgsl.contains("return shade_vertex(perro_vs_main_base"));
            assert!(wgsl.contains("return shade_material(in);"));
            assert!(!wgsl.contains("@location(3) meshlet"));
            assert!(!wgsl.contains("meshlet_index"));
            parse_and_validate(&wgsl, "custom shader interface stays meshlet-free");
        }
    }

    #[test]
    fn custom_material_shader_reads_same_vertex_payload_for_split_draws() {
        let material = r#"
fn shade_vertex(out: VertexOutput) -> VertexOutput {
    var next = out;
    next.uv = out.uv + vec2<f32>(0.125, 0.25);
    next.normal_ws = normalize(out.normal_ws);
    return next;
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    return vec4<f32>(in.uv, in.normal_ws.z, 1.0);
}
"#;
        let vertex_entry = "fn vs_main(v: VertexInput, inst: InstanceInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput";
        for prelude in [
            regular::prelude_wgsl(),
            regular::prelude_rigid_wgsl(),
            regular::prelude_skinned_wgsl(),
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Raw,
            );
            assert!(wgsl.contains(vertex_entry));
            assert!(wgsl.contains("@location(8) uv: vec2<f32>"));
            assert!(wgsl.contains("return shade_vertex(perro_vs_main_base"));
            assert!(!wgsl.contains("meshlet_index"));
            parse_and_validate(&wgsl, "custom shader split draw payload validates");
        }
    }

    #[test]
    fn gpu_shader_readback_matches_full_and_split_mesh_draws() {
        pollster::block_on(async {
            let Some((device, queue)) = test_device().await else {
                eprintln!("skip gpu readback test: no wgpu adapter");
                return;
            };

            let full_range = 0..6;
            let full = render_uv_readback(&device, &queue, std::slice::from_ref(&full_range)).await;
            let split = render_uv_readback(&device, &queue, &[0..3, 3..6]).await;
            assert_eq!(full, split);
        });
    }

    async fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .ok()?;
        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("perro_test_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
            })
            .await
            .ok()
    }

    async fn render_uv_readback(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draw_ranges: &[std::ops::Range<u32>],
    ) -> Vec<u8> {
        const WIDTH: u32 = 4;
        const HEIGHT: u32 = 4;
        const BYTES_PER_PIXEL: u32 = 4;
        const READBACK_BYTES_PER_ROW: u32 = 256;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_test_uv_readback_shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) normal_ws: vec3<f32>,
};

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = vec4<f32>(v.pos.xy, 0.0, 1.0);
    out.uv = v.uv;
    out.normal_ws = normalize(v.normal);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.uv, in.normal_ws.z * 0.5 + 0.5, 1.0);
}
"#
                .into(),
            ),
        });
        let vertices = [
            TestVertex {
                pos: [-1.0, -1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
            },
            TestVertex {
                pos: [1.0, -1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 1.0],
            },
            TestVertex {
                pos: [1.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
            },
            TestVertex {
                pos: [-1.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
            },
        ];
        let indices = [0u16, 1, 2, 0, 2, 3];
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_test_uv_vertices"),
            size: std::mem::size_of_val(&vertices) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_test_uv_indices"),
            size: std::mem::size_of_val(&indices) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&indices));

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_test_uv_target"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_test_uv_readback"),
            size: (READBACK_BYTES_PER_ROW * HEIGHT) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_test_uv_pipeline_layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let pipeline = crate::pipeline_cache::create_render_pipeline(
            device,
            wgpu::RenderPipelineDescriptor {
                label: Some("perro_test_uv_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<TestVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 12,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 24,
                                shader_location: 2,
                            },
                        ],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            },
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_test_uv_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_test_uv_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            for range in draw_ranges {
                pass.draw_indexed(range.clone(), 0, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(READBACK_BYTES_PER_ROW),
                    rows_per_image: Some(HEIGHT),
                },
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv()
            .expect("readback callback")
            .expect("map readback buffer");
        let mapped = slice.get_mapped_range().expect("readback map range");
        let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * BYTES_PER_PIXEL) as usize);
        for row in 0..HEIGHT as usize {
            let start = row * READBACK_BYTES_PER_ROW as usize;
            let end = start + (WIDTH * BYTES_PER_PIXEL) as usize;
            pixels.extend_from_slice(&mapped[start..end]);
        }
        drop(mapped);
        readback.unmap();
        pixels
    }

    #[test]
    fn mesh_blend_mask_wgsl_validates() {
        for (src, label) in [
            (
                build_mesh_blend_mask_wgsl(regular::DEPTH_PREPASS_RIGID_WGSL),
                "mask rigid",
            ),
            (
                build_mesh_blend_mask_wgsl(&build_packed_lod_depth_rigid_wgsl()),
                "mask rigid packed lod",
            ),
            (
                build_mesh_blend_mask_wgsl(regular::DEPTH_PREPASS_SKINNED_WGSL),
                "mask skinned",
            ),
        ] {
            parse_and_validate(&src, label);
        }
    }

    #[test]
    fn mesh_blend_screen_wgsl_validates() {
        parse_and_validate(regular::MESH_BLEND_SCREEN_WGSL, "mesh blend screen");
    }

    // The last-cascade distance fade inside `perro_ray_shadow_factor`, exactly
    // as it survives whitespace minification. Extracted (not re-typed) by the
    // tests below so the GPU evaluates the shipped expression, and so a drifted
    // or deleted fade fails loudly instead of silently restoring the hard cut.
    const RAY_SHADOW_RANGE_FADE_WGSL: &str = "let fade_range = max(shadow.ray_splits.w, 1.0e-4); let range_fade = smoothstep(fade_range * 0.88, fade_range, view_dist); let faded = mix(visibility, 1.0, range_fade);";

    #[test]
    fn ray_shadow_range_fade_survives_every_prelude_variant() {
        // One definition in the shared prelude, so every lit variant that calls
        // perro_shadow_factor must carry it -- including the rigid/skinned
        // rewrites and the packed-LOD patch chain, which all rewrite the
        // prelude by text.
        let packed_lod = build_packed_lod_rigid_prelude();
        for (prelude, label) in [
            (regular::prelude_wgsl(), "prelude"),
            (regular::prelude_rigid_wgsl(), "prelude rigid"),
            (regular::prelude_skinned_wgsl(), "prelude skinned"),
            (packed_lod.as_str(), "prelude rigid packed lod"),
        ] {
            assert!(
                prelude.contains(RAY_SHADOW_RANGE_FADE_WGSL),
                "{label}: ray shadow range fade missing (hard cut at the range limit is back)"
            );
            // The fade must feed the return, not sit dead next to it.
            assert!(
                prelude.contains("return mix(1.0, faded, strength);"),
                "{label}: range fade computed but not returned"
            );
            let wgsl = build_material_shader_with_prelude(prelude, regular::MATERIAL_STANDARD_WGSL);
            parse_and_validate(&wgsl, &format!("{label} range fade"));
        }
    }

    /// Runs the shipped fade statements on the GPU over a sweep of view
    /// distances. Splices the text straight out of the minified prelude (only
    /// the uniform read is rebound to a local), so this measures the real
    /// expression rather than a copy of it.
    #[test]
    fn ray_shadow_range_fade_ramps_across_the_last_cascade_band() {
        const RANGE: f32 = 220.0;
        let fade_block = RAY_SHADOW_RANGE_FADE_WGSL.replace("shadow.ray_splits.w", "shadow_range");
        assert_ne!(
            fade_block, RAY_SHADOW_RANGE_FADE_WGSL,
            "fade block no longer reads shadow.ray_splits.w"
        );
        let wgsl = format!(
            "@group(0) @binding(0) var<storage, read> view_dists: array<f32>;\n\
             @group(0) @binding(1) var<storage, read_write> results: array<f32>;\n\
             @compute @workgroup_size(1)\n\
             fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {{\n\
             let view_dist = view_dists[gid.x];\n\
             let visibility = 0.0;\n\
             let shadow_range = f32({RANGE:?});\n\
             {fade_block}\n\
             results[gid.x] = faded;\n\
             }}\n"
        );
        parse_and_validate(&wgsl, "ray shadow range fade kernel");

        // 0.0 .. RANGE inclusive, so the last sample sits exactly on the limit
        // the early-out hands over at.
        const SAMPLES: usize = 65;
        let dists: Vec<f32> = (0..SAMPLES)
            .map(|i| RANGE * i as f32 / (SAMPLES - 1) as f32)
            .collect();

        let Some(values) = pollster::block_on(run_f32_kernel(&wgsl, &dists)) else {
            eprintln!("skip ray shadow range fade gpu test: no wgpu adapter");
            return;
        };

        let fade_start = RANGE * 0.88;
        for (dist, value) in dists.iter().zip(values.iter()) {
            assert!(
                (0.0..=1.0).contains(value),
                "fade left the visibility range at {dist}: {value}"
            );
            if *dist < fade_start {
                // Inside the stable band the fade must be inert: a fully
                // shadowed sample stays fully shadowed.
                assert_eq!(*value, 0.0, "fade leaked into the stable band at {dist}");
            }
        }
        // At the limit the fade must land on 1.0 -- the value the early-out
        // returns for anything past it. Any gap here is the visible line.
        let at_limit = *values.last().expect("sample at the range limit");
        assert!(
            (at_limit - 1.0).abs() <= 1.0e-5,
            "fade does not meet the early-out at the range limit: {at_limit}"
        );
        // And it must actually ramp: monotonic, with real intermediate values
        // rather than a two-state step.
        let band: Vec<f32> = dists
            .iter()
            .zip(values.iter())
            .filter(|(d, _)| **d >= fade_start)
            .map(|(_, v)| *v)
            .collect();
        assert!(band.len() >= 4, "too few samples inside the fade band");
        for pair in band.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "fade is not monotonic: {:?} -> {:?}",
                pair[0],
                pair[1]
            );
        }
        let intermediate = band.iter().filter(|v| **v > 0.01 && **v < 0.99).count();
        assert!(
            intermediate >= 2,
            "fade is a hard cut, not a ramp: {band:?}"
        );
        eprintln!("ray shadow range fade over the last 12%: {band:?}");
    }

    async fn run_f32_kernel(wgsl: &str, inputs: &[f32]) -> Option<Vec<f32>> {
        let (device, queue) = test_device().await?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_test_f32_kernel"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
        let bytes = std::mem::size_of_val(inputs) as u64;
        let input = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_test_kernel_input"),
            size: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_test_kernel_output"),
            size: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_test_kernel_readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&input, 0, bytemuck::cast_slice(inputs));
        let pipeline = crate::pipeline_cache::create_compute_pipeline(
            &device,
            wgpu::ComputePipelineDescriptor {
                label: Some("perro_test_f32_kernel_pipeline"),
                layout: None,
                module: &module,
                entry_point: Some("cs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            },
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_test_f32_kernel_bind_group"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_test_f32_kernel_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(inputs.len() as u32, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, bytes);
        queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv()
            .expect("kernel readback callback")
            .expect("map kernel readback buffer");
        let mapped = slice.get_mapped_range().expect("kernel readback range");
        let values: Vec<f32> = bytemuck::cast_slice(&mapped).to_vec();
        drop(mapped);
        readback.unmap();
        Some(values)
    }

    #[test]
    fn multimesh_wgsl_parses() {
        let wgsl = sanitize_reserved_meta_identifier(regular::multimesh_wgsl());
        naga::front::wgsl::parse_str(&wgsl).expect("multimesh wgsl parses");
    }

    #[test]
    fn multimesh_mask_salt_patch_applies_and_validates() {
        let wgsl = build_multimesh_shader_wgsl();
        // Struct field + vs_main write + fs_mask read; a drifted anchor would
        // silently drop the per-instance seam salt.
        assert_eq!(wgsl.matches("mask_salt").count(), 3);
        assert!(wgsl.contains("out.mask_salt = visible_indices[instance_index] % 7u;"));
        assert!(wgsl.contains("select(0u, in.mask_salt, mesh_blend_mask_id.y != 0u)"));
        parse_and_validate(&wgsl, "multimesh mask salt");
    }

    #[test]
    fn multimesh_cull_wgsl_validates() {
        parse_and_validate(culling::MULTIMESH_CULL_WGSL, "multimesh cull");
    }

    // The cull shader keeps its own copy of MultiMeshDrawParam; a stride or
    // offset drift against the render shader's copy (and MultiMeshDrawParamGpu
    // on the CPU) makes every draw_id > 0 read garbage bounding spheres, which
    // shows up as instances popping out by camera angle.
    #[test]
    fn multimesh_cull_draw_param_layout_matches_render_shader() {
        fn draw_param_layout(wgsl: &str, label: &str) -> (Vec<(String, u32)>, u32) {
            let module =
                naga::front::wgsl::parse_str(wgsl).unwrap_or_else(|err| panic!("{label}: {err}"));
            for (_, ty) in module.types.iter() {
                if ty.name.as_deref() == Some("MultiMeshDrawParam")
                    && let naga::TypeInner::Struct { members, span } = &ty.inner
                {
                    let fields = members
                        .iter()
                        .map(|m| (m.name.clone().unwrap_or_default(), m.offset))
                        .collect();
                    return (fields, *span);
                }
            }
            panic!("{label}: MultiMeshDrawParam struct not found");
        }
        let render_wgsl = sanitize_reserved_meta_identifier(regular::multimesh_wgsl());
        let (render_fields, render_span) = draw_param_layout(&render_wgsl, "multimesh render");
        let (cull_fields, cull_span) =
            draw_param_layout(culling::MULTIMESH_CULL_WGSL, "multimesh cull");
        // CPU MultiMeshDrawParamGpu (three_d/gpu.rs) is 96 bytes.
        assert_eq!(render_span, 96, "render draw param span");
        assert_eq!(cull_span, 96, "cull draw param span");
        for (render, cull) in render_fields.iter().zip(cull_fields.iter()) {
            assert_eq!(
                render.1, cull.1,
                "offset drift: render {render:?} vs cull {cull:?}"
            );
        }
        // scale_bits is the only packed field the cull actually reads; pin its
        // offset explicitly so a rename in either copy cannot hide a shift.
        assert_eq!(
            cull_fields
                .iter()
                .find(|(name, _)| name == "scale_bits")
                .map(|(_, offset)| *offset),
            Some(72),
            "cull scale_bits offset"
        );
    }

    /// `Shadow3D` (group 2, binding 0) is sized on the CPU by
    /// `MAX_SHADOW_{RAY_CASCADES,SPOT_LIGHTS,POINT_LIGHTS}` and in WGSL by hand-
    /// written array lengths. Nothing tied the two together, so editing a Rust
    /// light-count constant silently shrank `ShadowUniform` while the shader kept
    /// its own lengths -- and the only symptom was a `create_render_pipeline`
    /// validation panic on `perro_mesh_pipeline_rigid`:
    ///
    /// ```text
    /// Shader global ResourceBinding { group: 2, binding: 0 } is not available
    /// Buffer structure size 2304 ... greater than the given `min_binding_size`, which is 1104
    /// ```
    ///
    /// 1104 is the layout for `MAX_SHADOW_POINT_LIGHTS = 1`. Pin the WGSL span so
    /// the next edit to either side fails here instead of at pipeline creation.
    #[test]
    fn shadow_uniform_layout_matches_shader_struct() {
        let wgsl = sanitize_reserved_meta_identifier(regular::prelude_wgsl());
        let module = naga::front::wgsl::parse_str(&wgsl).expect("prelude parses");
        let span = module
            .types
            .iter()
            .find_map(|(_, ty)| match (&ty.name, &ty.inner) {
                (Some(name), naga::TypeInner::Struct { span, .. }) if name == "Shadow3D" => {
                    Some(*span)
                }
                _ => None,
            })
            .expect("Shadow3D struct in prelude");
        assert_eq!(
            span as usize,
            crate::three_d::gpu::shadow_uniform_size(),
            "Shadow3D (wgsl) and ShadowUniform (cpu) disagree; a light-count              constant moved on one side only"
        );
    }

    // BlendShapeDelta is copied into every shader that reads morph targets and
    // is byte-mirrored by BlendShapeDeltaGpu (three_d/gpu.rs, 16 bytes). A
    // stride drift in any copy makes every target past the first read the
    // previous target's deltas, so a face rig melts instead of animating.
    #[test]
    fn blend_shape_delta_layout_matches_cpu_stride() {
        fn delta_layout(wgsl: &str, label: &str) -> (Vec<(String, u32)>, u32) {
            let module =
                naga::front::wgsl::parse_str(wgsl).unwrap_or_else(|err| panic!("{label}: {err}"));
            for (_, ty) in module.types.iter() {
                if ty.name.as_deref() == Some("BlendShapeDelta")
                    && let naga::TypeInner::Struct { members, span } = &ty.inner
                {
                    let fields = members
                        .iter()
                        .map(|m| (m.name.clone().unwrap_or_default(), m.offset))
                        .collect();
                    return (fields, *span);
                }
            }
            panic!("{label}: BlendShapeDelta struct not found");
        }
        let material = regular::MATERIAL_STANDARD_WGSL;
        let sources = [
            (
                "prelude",
                build_material_shader_with_prelude(regular::prelude_wgsl(), material),
            ),
            (
                "prelude rigid",
                build_material_shader_with_prelude(regular::prelude_rigid_wgsl(), material),
            ),
            (
                "prelude skinned",
                build_material_shader_with_prelude(regular::prelude_skinned_wgsl(), material),
            ),
            (
                "prelude rigid packed lod",
                build_material_shader_with_prelude(&build_packed_lod_rigid_prelude(), material),
            ),
            (
                "multimesh",
                sanitize_reserved_meta_identifier(regular::multimesh_wgsl()),
            ),
            ("depth prepass", regular::DEPTH_PREPASS_WGSL.to_string()),
            (
                "depth prepass rigid",
                regular::DEPTH_PREPASS_RIGID_WGSL.to_string(),
            ),
            (
                "depth prepass rigid packed lod",
                build_packed_lod_depth_rigid_wgsl(),
            ),
            (
                "depth prepass skinned",
                regular::DEPTH_PREPASS_SKINNED_WGSL.to_string(),
            ),
        ];
        for (label, wgsl) in &sources {
            let (fields, span) = delta_layout(wgsl, label);
            assert_eq!(span, 16, "{label}: blend delta span");
            assert_eq!(
                fields
                    .iter()
                    .find(|(name, _)| name == "packed_normal_delta")
                    .map(|(_, offset)| *offset),
                Some(12),
                "{label}: packed normal offset"
            );
        }
    }

    #[test]
    fn hiz_downsample_wgsl_validates() {
        parse_and_validate(culling::HIZ_DEPTH_COPY_WGSL, "hiz depth copy");
        parse_and_validate(culling::HIZ_DOWNSAMPLE_WGSL, "hiz downsample");
        parse_and_validate(culling::HIZ_DOWNSAMPLE_SPD_WGSL, "hiz downsample spd");
        parse_and_validate(culling::HIZ_OCCLUSION_CULL_WGSL, "hiz occlusion cull");
    }

    #[test]
    fn sky_wgsl_parses() {
        let wgsl = build_sky_shader();
        naga::front::wgsl::parse_str(&wgsl).expect("sky wgsl parses");
    }

    #[test]
    fn custom_sky_wgsl_parses() {
        let custom = r#"
fn sky_shader(in: SkyFragment) -> vec4<f32> {
    return vec4<f32>(in.color.rgb + custom_param(in, 0u).xxx, in.color.a);
}
"#
        .to_string();
        let params = vec![perro_structs::CustomPostParam::unnamed(
            perro_structs::CustomPostParamValue::F32(0.1),
        )];
        let wgsl = build_sky_shader_with_passes(&[(custom, params.as_slice())]);
        naga::front::wgsl::parse_str(&wgsl).expect("custom sky wgsl parses");
    }
}
