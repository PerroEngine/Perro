mod regular {
    use std::sync::LazyLock;

    // Fns shared verbatim by the rigid/skinned prelude and multimesh paths
    // live once in shared_3d.wgsl and get concatenated ahead of each base
    // file here (WGSL module-scope declarations are order-independent).
    const SHARED_3D_WGSL: &str = perro_macros::include_str_stripped!("shaders/shared_3d.wgsl");
    const PRELUDE_BASE_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/prelude_3d.wgsl");
    static PRELUDE_WGSL_FULL: LazyLock<String> =
        LazyLock::new(|| format!("{SHARED_3D_WGSL}\n{PRELUDE_BASE_WGSL}"));
    const MULTIMESH_BASE_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/multimesh.wgsl");
    static MULTIMESH_WGSL_FULL: LazyLock<String> =
        LazyLock::new(|| format!("{SHARED_3D_WGSL}\n{MULTIMESH_BASE_WGSL}"));
    static PRELUDE_RIGID_WGSL: LazyLock<String> =
        LazyLock::new(|| super::build_rigid_prelude(prelude_wgsl()));
    static PRELUDE_SKINNED_WGSL: LazyLock<String> =
        LazyLock::new(|| super::build_skinned_prelude(prelude_wgsl()));

    #[inline]
    pub fn prelude_wgsl() -> &'static str {
        PRELUDE_WGSL_FULL.as_str()
    }

    #[inline]
    pub fn multimesh_wgsl() -> &'static str {
        MULTIMESH_WGSL_FULL.as_str()
    }
    pub const MATERIAL_STANDARD_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/material_standard.wgsl");
    pub const MATERIAL_UNLIT_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/material_unlit.wgsl");
    pub const MATERIAL_TOON_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/material_toon.wgsl");
    pub const DEPTH_PREPASS_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/depth_prepass.wgsl");
    pub const DEPTH_PREPASS_RIGID_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/depth_prepass_rigid.wgsl");
    pub const DEPTH_PREPASS_SKINNED_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/depth_prepass_skinned.wgsl");
    pub const MESH_BLEND_SCREEN_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/mesh_blend_screen.wgsl");
    pub const SKY3D_WGSL: &str = perro_macros::include_str_stripped!("shaders/sky3d.wgsl");

    #[inline]
    pub fn prelude_rigid_wgsl() -> &'static str {
        PRELUDE_RIGID_WGSL.as_str()
    }

    #[inline]
    pub fn prelude_skinned_wgsl() -> &'static str {
        PRELUDE_SKINNED_WGSL.as_str()
    }
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
}

#[inline]
pub fn prelude_rigid_wgsl() -> &'static str {
    regular::prelude_rigid_wgsl()
}

#[inline]
pub fn prelude_skinned_wgsl() -> &'static str {
    regular::prelude_skinned_wgsl()
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

#[inline]
pub fn build_material_shader(material_wgsl: &str) -> String {
    build_material_shader_with_prelude(regular::prelude_wgsl(), material_wgsl)
}

fn build_rigid_prelude(prelude: &str) -> String {
    prelude
        .replace(
            "@group(0) @binding(1) var<storage, read> skeletons: array<SkeletonBone>; struct SkeletonBone { row_0: vec4<f32>, row_1: vec4<f32>, row_2: vec4<f32>, } fn perro_blend_skin_rows(base: u32, joints: vec4<u32>, weights: vec4<f32>) -> mat3x4<f32> { let b0 = skeletons[base + joints.x]; let b1 = skeletons[base + joints.y]; let b2 = skeletons[base + joints.z]; let b3 = skeletons[base + joints.w]; return mat3x4<f32>( b0.row_0 * weights.x + b1.row_0 * weights.y + b2.row_0 * weights.z + b3.row_0 * weights.w, b0.row_1 * weights.x + b1.row_1 * weights.y + b2.row_1 * weights.z + b3.row_1 * weights.w, b0.row_2 * weights.x + b1.row_2 * weights.y + b2.row_2 * weights.z + b3.row_2 * weights.w, ); } ",
            "",
        )
        .replace("@group(0) @binding(2)", "@group(0) @binding(1)")
        .replace("@group(0) @binding(3)", "@group(0) @binding(2)")
        .replace("@group(0) @binding(4)", "@group(0) @binding(3)")
        .replace("@group(0) @binding(5)", "@group(0) @binding(4)")
        .replace("@group(0) @binding(6)", "@group(0) @binding(5)")
        .replace(
            "@location(2) @interpolate(flat) joints: vec4<u32>, @location(3) weights: vec4<f32>, ",
            "",
        )
        .replace(
            "@location(13) @interpolate(flat) skeleton_params: vec4<u32>,",
            "@location(13) @interpolate(flat) custom_params: vec2<u32>,",
        )
        .replace(
            "return VertexInput(out_pos, vec4<f32>(normalize(out_normal), 0.0), v.joints, v.weights, v.uv, v.paint_uv);",
            "return VertexInput(out_pos, vec4<f32>(normalize(out_normal), 0.0), v.uv, v.paint_uv);",
        )
        .replace(
            "var pos = blended.pos; var normal = blended.normal.xyz; if inst.skeleton_params.y > 0u { let rows = perro_blend_skin_rows(inst.skeleton_params.x, blended.joints, blended.weights); let p_skin = vec4<f32>(pos, 1.0); let skinned_pos = vec3<f32>(dot(rows[0], p_skin), dot(rows[1], p_skin), dot(rows[2], p_skin)); normal = vec3<f32>(dot(rows[0].xyz, normal), dot(rows[1].xyz, normal), dot(rows[2].xyz, normal)); pos = skinned_pos; } let p = vec4<f32>(pos, 1.0);",
            "let p = vec4<f32>(blended.pos, 1.0);",
        )
        .replace(
            "normal, );",
            "blended.normal.xyz, );",
        )
        .replace(
            "out.custom_range = vec2<u32>(inst.skeleton_params.z, inst.skeleton_params.w); out.uv = blended.uv; out.paint_uv = blended.paint_uv;",
            "out.custom_range = inst.custom_params; out.uv = v.uv; out.paint_uv = v.paint_uv;",
        )
}

fn build_skinned_prelude(prelude: &str) -> String {
    prelude.replace(
        "var pos = blended.pos; var normal = blended.normal.xyz; if inst.skeleton_params.y > 0u { let rows = perro_blend_skin_rows(inst.skeleton_params.x, blended.joints, blended.weights); let p_skin = vec4<f32>(pos, 1.0); let skinned_pos = vec3<f32>(dot(rows[0], p_skin), dot(rows[1], p_skin), dot(rows[2], p_skin)); normal = vec3<f32>(dot(rows[0].xyz, normal), dot(rows[1].xyz, normal), dot(rows[2].xyz, normal)); pos = skinned_pos; }",
        "let rows = perro_blend_skin_rows(inst.skeleton_params.x, blended.joints, blended.weights); let p_skin = vec4<f32>(blended.pos, 1.0); let pos = vec3<f32>(dot(rows[0], p_skin), dot(rows[1], p_skin), dot(rows[2], p_skin)); let normal = vec3<f32>( dot(rows[0].xyz, blended.normal.xyz), dot(rows[1].xyz, blended.normal.xyz), dot(rows[2].xyz, blended.normal.xyz), );",
    )
}

#[inline]
fn sanitize_reserved_meta_identifier(wgsl: &str) -> String {
    wgsl.replace(
        "let meta = custom_params_meta",
        "let packed_meta = custom_params_meta",
    )
    .replace(
        "let meta = blend_shape_instances",
        "let blend_meta = blend_shape_instances",
    )
    .replace("let kind = meta & 0x3u;", "let kind = packed_meta & 0x3u;")
    .replace(
        "let value_offset = meta >> 2u;",
        "let value_offset = packed_meta >> 2u;",
    )
}

fn build_packed_lod_rigid_prelude() -> String {
    regular::prelude_rigid_wgsl()
        .replace(
            "@group(0) @binding(5)\nvar<storage, read> blend_shape_instances: array<BlendShapeInstance>;",
            "@group(0) @binding(5)\nvar<storage, read> blend_shape_instances: array<BlendShapeInstance>;\n@group(0) @binding(6)\nvar<storage, read> packed_lod_params: array<PackedLodParam>;",
        )
        .replace(
            "struct VertexInput {\n    @location(0) pos: vec3<f32>,",
            "struct VertexInput {\n    @location(0) pos: vec4<f32>,",
        )
        .replace(
            "    @location(13) @interpolate(flat) custom_params: vec2<u32>,\n};",
            "    @location(13) @interpolate(flat) custom_params: vec2<u32>,\n    @location(14) @interpolate(flat) packed_lod_param_id: u32,\n};",
        )
        .replace(
            "struct BlendShapeDelta {\n    position_delta: vec4<f32>,\n    normal_delta: vec4<f32>,\n};",
            "struct PackedLodParam {\n    pos_min: vec4<f32>,\n    pos_extent: vec4<f32>,\n    uv_min_extent: vec4<f32>,\n};\n\nstruct BlendShapeDelta {\n    position_delta: vec4<f32>,\n    normal_delta: vec4<f32>,\n};",
        )
        .replace("    var out_pos = v.pos;", "    var out_pos = v.pos.xyz;")
        .replace(
            "    return VertexInput(out_pos, vec4<f32>(normalize(out_normal), 0.0), v.uv, v.paint_uv);",
            "    return VertexInput(vec4<f32>(out_pos, 0.0), vec4<f32>(normalize(out_normal), 0.0), v.uv, v.paint_uv);",
        )
        .replace(
            "    let blended = perro_apply_blend_shapes(v, vertex_index, instance_index);",
            "    let packed_lod = packed_lod_params[inst.packed_lod_param_id];\n    var decoded_v = v;\n    decoded_v.pos = vec4<f32>(packed_lod.pos_min.xyz + v.pos.xyz * packed_lod.pos_extent.xyz, 0.0);\n    decoded_v.uv = packed_lod.uv_min_extent.xy + v.uv * packed_lod.uv_min_extent.zw;\n    let blended = perro_apply_blend_shapes(decoded_v, vertex_index, instance_index);",
        )
        .replace(
            "    let p = vec4<f32>(blended.pos, 1.0);",
            "    let p = vec4<f32>(blended.pos.xyz, 1.0);",
        )
}

fn build_packed_lod_depth_rigid_wgsl() -> String {
    regular::DEPTH_PREPASS_RIGID_WGSL
        .replace(
            "@group(0) @binding(5)\nvar<storage, read> blend_shape_instances: array<BlendShapeInstance>;",
            "@group(0) @binding(5)\nvar<storage, read> blend_shape_instances: array<BlendShapeInstance>;\n@group(0) @binding(6)\nvar<storage, read> packed_lod_params: array<PackedLodParam>;",
        )
        .replace(
            "struct VertexInput {\n    @location(0) pos: vec3<f32>,\n};",
            "struct VertexInput {\n    @location(0) pos: vec4<f32>,\n};",
        )
        .replace(
            "struct InstanceInput {\n    @location(4) model_row_0: vec4<f32>,\n    @location(5) model_row_1: vec4<f32>,\n    @location(6) model_row_2: vec4<f32>,\n    @location(7) @interpolate(flat) packed_color: u32,\n    @location(11) @interpolate(flat) packed_material_params: u32,\n};",
            "struct InstanceInput {\n    @location(4) model_row_0: vec4<f32>,\n    @location(5) model_row_1: vec4<f32>,\n    @location(6) model_row_2: vec4<f32>,\n    @location(7) @interpolate(flat) packed_color: u32,\n    @location(11) @interpolate(flat) packed_material_params: u32,\n    @location(14) @interpolate(flat) packed_lod_param_id: u32,\n};",
        )
        .replace(
            "struct BlendShapeDelta {\n    position_delta: vec4<f32>,\n    normal_delta: vec4<f32>,\n}",
            "struct PackedLodParam {\n    pos_min: vec4<f32>,\n    pos_extent: vec4<f32>,\n    uv_min_extent: vec4<f32>,\n}\n\nstruct BlendShapeDelta {\n    position_delta: vec4<f32>,\n    normal_delta: vec4<f32>,\n}",
        )
        .replace("        return v.pos;", "        return v.pos.xyz;")
        .replace("    var pos = v.pos;", "    var pos = v.pos.xyz;")
        .replace(
            "    let pos = apply_blend_shapes(v, vertex_index, instance_index);",
            "    let packed_lod = packed_lod_params[inst.packed_lod_param_id];\n    var decoded_v = v;\n    decoded_v.pos = vec4<f32>(packed_lod.pos_min.xyz + v.pos.xyz * packed_lod.pos_extent.xyz, 0.0);\n    let pos = apply_blend_shapes(decoded_v, vertex_index, instance_index);",
        )
}

#[inline]
pub fn build_material_shader_with_prelude(prelude_wgsl: &str, material_wgsl: &str) -> String {
    build_material_shader_with_prelude_inner(prelude_wgsl, material_wgsl, false)
}

#[inline]
pub fn build_custom_material_shader_with_prelude(
    prelude_wgsl: &str,
    material_wgsl: &str,
    lighting: perro_render_bridge::CustomMaterialLighting3D,
) -> String {
    let uses_lit_helper = material_wgsl.contains("perro_lit_standard(");
    let apply_standard_lighting =
        lighting == perro_render_bridge::CustomMaterialLighting3D::Standard && !uses_lit_helper;
    build_material_shader_with_prelude_inner(prelude_wgsl, material_wgsl, apply_standard_lighting)
}

#[inline]
pub fn build_custom_multimesh_material_shader(
    material_wgsl: &str,
    lighting: perro_render_bridge::CustomMaterialLighting3D,
) -> String {
    let base = sanitize_reserved_meta_identifier(regular::multimesh_wgsl());
    let split_at = base
        .find("@vertex\nfn vs_main")
        .or_else(|| base.find("@vertex\r\nfn vs_main"))
        .or_else(|| base.find("@vertex fn vs_main"))
        .unwrap_or(base.len());
    let prelude = &base[..split_at];
    let uses_lit_helper = material_wgsl.contains("perro_lit_standard(");
    let apply_standard_lighting =
        lighting == perro_render_bridge::CustomMaterialLighting3D::Standard && !uses_lit_helper;
    let has_custom_vertex = material_wgsl.contains("shade_vertex(");
    let mut out = String::new();
    out.push_str(prelude);
    out.push('\n');
    out.push_str(material_wgsl);
    if has_custom_vertex {
        out.push_str(
            "\n@vertex\nfn vs_main(v: VertexInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput {\n    let inst = perro_fetch_instance(instance_index);\n    return shade_vertex(perro_multimesh_vs_main_base(v, inst, vertex_index));\n}\n",
        );
    } else {
        out.push_str(
            "\n@vertex\nfn vs_main(v: VertexInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput {\n    let inst = perro_fetch_instance(instance_index);\n    return perro_multimesh_vs_main_base(v, inst, vertex_index);\n}\n",
        );
    }
    if apply_standard_lighting {
        out.push_str(
            "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    let base = shade_material(in);\n    return perro_lit_standard(in, base, 0.5, 0.0, 1.0, vec3<f32>(0.0));\n}\n",
        );
    } else {
        out.push_str(
            "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    return shade_material(in);\n}\n",
        );
    }
    out
}

#[inline]
fn build_material_shader_with_prelude_inner(
    prelude_wgsl: &str,
    material_wgsl: &str,
    apply_custom_standard_lighting: bool,
) -> String {
    let has_custom_vertex = material_wgsl.contains("shade_vertex(");
    let mut out = String::new();
    let sanitized_prelude = sanitize_reserved_meta_identifier(prelude_wgsl);
    out.push_str(&sanitized_prelude);
    out.push('\n');
    out.push_str(material_wgsl);
    if has_custom_vertex {
        out.push_str(
            "\n@vertex\nfn vs_main(v: VertexInput, inst: InstanceInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput {\n    return shade_vertex(perro_vs_main_base(v, inst, vertex_index, instance_index));\n}\n",
        );
    } else {
        out.push_str(
            "\n@vertex\nfn vs_main(v: VertexInput, inst: InstanceInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput {\n    return perro_vs_main_base(v, inst, vertex_index, instance_index);\n}\n",
        );
    }
    if apply_custom_standard_lighting {
        out.push_str(
            "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    let base = shade_material(in);\n    return perro_lit_standard(in, base, 0.5, 0.0, 1.0, vec3<f32>(0.0));\n}\n",
        );
    } else {
        out.push_str(
            "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    return shade_material(in);\n}\n",
        );
    }
    out
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

#[inline]
pub fn create_multimesh_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_multimesh"),
        source: wgpu::ShaderSource::Wgsl(
            sanitize_reserved_meta_identifier(regular::multimesh_wgsl()).into(),
        ),
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

#[inline]
pub fn build_sky_shader() -> String {
    regular::SKY3D_WGSL.replace(
        "/*__PERRO_SKY_CUSTOM_STACK__*/",
        "fn apply_custom_sky_stack(base: SkyFragment) -> vec4<f32> { return base.color; }",
    )
}

#[inline]
pub fn build_sky_shader_with_passes(
    passes: &[(String, &[perro_structs::CustomPostParam])],
) -> String {
    let mut stack = String::new();
    for (idx, (source, params)) in passes.iter().enumerate() {
        let fn_name = format!("sky_shader_{idx}");
        let renamed = source.replacen("fn sky_shader", &format!("fn {fn_name}"), 1);
        stack.push('\n');
        stack.push_str(&renamed);
        stack.push('\n');
        stack.push_str(&format!(
            "fn apply_sky_shader_pass_{idx}(base: SkyFragment) -> vec4<f32> {{\n"
        ));
        stack.push_str("    let frag = SkyFragment(\n");
        stack.push_str("        base.ray,\n");
        stack.push_str("        base.uv,\n");
        stack.push_str("        base.time_of_day,\n");
        stack.push_str("        base.time_seconds,\n");
        stack.push_str("        base.day_weight,\n");
        stack.push_str("        base.evening_weight,\n");
        stack.push_str("        base.night_weight,\n");
        stack.push_str("        base.horizon_weight,\n");
        stack.push_str("        base.color,\n");
        stack.push_str(&encoded_sky_param_values(params));
        stack.push_str("    );\n");
        stack.push_str(&format!("    return {fn_name}(frag);\n"));
        stack.push_str("}\n");
        stack.push_str(&format!(
            "fn sky_custom_pass_{idx}(base: SkyFragment) -> vec4<f32> {{ return apply_sky_shader_pass_{idx}(base); }}\n"
        ));
    }
    if !passes.is_empty() {
        stack.push_str("\nfn apply_custom_sky_stack(base: SkyFragment) -> vec4<f32> {\n");
        stack.push_str("    var cur = base;\n");
        for idx in 0..passes.len() {
            stack.push_str(&format!("    cur.color = sky_custom_pass_{idx}(cur);\n"));
        }
        stack.push_str("    return cur.color;\n");
        stack.push_str("}\n");
    }
    regular::SKY3D_WGSL.replace("/*__PERRO_SKY_CUSTOM_STACK__*/", &stack)
}

fn encoded_sky_param_values(params: &[perro_structs::CustomPostParam]) -> String {
    let mut out = String::new();
    for i in 0..16 {
        let v = params
            .get(i)
            .map(|param| encode_custom_param_value(&param.value))
            .unwrap_or([0.0; 4]);
        out.push_str(&format!(
            "        vec4<f32>({x}, {y}, {z}, {w}),\n",
            x = wgsl_f32(v[0]),
            y = wgsl_f32(v[1]),
            z = wgsl_f32(v[2]),
            w = wgsl_f32(v[3])
        ));
    }
    out
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

fn wgsl_f32(v: f32) -> String {
    if v.is_finite() {
        format!("{v:?}")
    } else {
        "0.0".to_string()
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
    fn packed_lod_material_wgsl_keeps_paint_uv() {
        let prelude = build_packed_lod_rigid_prelude();
        let wgsl = build_material_shader_with_prelude(&prelude, regular::MATERIAL_STANDARD_WGSL);
        parse_and_validate(&wgsl, "packed lod paint uv");
        assert!(wgsl.contains("@location(15) paint_uv"));
        assert!(wgsl.contains("out.paint_uv = v.paint_uv"));
    }

    #[test]
    fn unlit_material_samples_base_color_texture() {
        assert!(
            regular::MATERIAL_UNLIT_WGSL
                .contains("textureSample(material_base_color_tex, material_sampler, in.uv)")
        );
        assert!(regular::MATERIAL_UNLIT_WGSL.contains("color.rgb * base_sample.rgb + emissive"));
        assert!(regular::MATERIAL_UNLIT_WGSL.contains("color.a * base_sample.a"));
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
            assert!(wgsl.contains("perro_lit_standard(in, base"));
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
            assert!(!wgsl.contains("perro_lit_standard(in, base"));
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
            assert!(!wgsl.contains("perro_lit_standard(in, base"));
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
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
        });
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

    #[test]
    fn multimesh_wgsl_parses() {
        let wgsl = sanitize_reserved_meta_identifier(regular::multimesh_wgsl());
        naga::front::wgsl::parse_str(&wgsl).expect("multimesh wgsl parses");
    }

    #[test]
    fn multimesh_cull_wgsl_validates() {
        parse_and_validate(culling::MULTIMESH_CULL_WGSL, "multimesh cull");
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
