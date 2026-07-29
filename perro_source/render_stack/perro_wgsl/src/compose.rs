//! Shader composition shared by the renderer and the offline validators.
//!
//! Game `.wgsl` files are fragments: the engine wraps them with a prelude and
//! entry points before handing the module to wgpu. `perro doctor` and the
//! static build reuse the exact same composition so an off-GPU parse sees what
//! the runtime sees.

use std::sync::LazyLock;

/// Raw (un-minified) shader sources. Kept public for parity tests.
pub mod raw {
    pub const SHARED_3D_WGSL: &str = include_str!("shaders/shared_3d.wgsl");
    pub const PRELUDE_3D_WGSL: &str = include_str!("shaders/prelude_3d.wgsl");
    pub const STYLIZED_3D_WGSL: &str = include_str!("shaders/stylized_3d.wgsl");
    pub const MULTIMESH_WGSL: &str = include_str!("shaders/multimesh.wgsl");
    pub const STYLIZED_MULTIMESH_WGSL: &str = include_str!("shaders/stylized_multimesh.wgsl");
    pub const SKY3D_WGSL: &str = include_str!("shaders/sky3d.wgsl");
}

/// How a custom material's `shade_material` return value gets used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MaterialOutput {
    /// Return value is base color; engine wraps it in standard lighting.
    #[default]
    Surface,
    /// Return value is the final shaded color.
    Final,
}

/// Sky pass params, pre-encoded to the 16 `vec4<f32>` slots the stack reads.
pub type SkyPassParams = [[f32; 4]; 16];

static SHARED_3D: LazyLock<String> = LazyLock::new(|| optimized(raw::SHARED_3D_WGSL));
static STYLIZED_3D: LazyLock<String> = LazyLock::new(|| optimized(raw::STYLIZED_3D_WGSL));
static STYLIZED_MULTIMESH: LazyLock<String> =
    LazyLock::new(|| optimized(raw::STYLIZED_MULTIMESH_WGSL));
static SKY3D: LazyLock<String> = LazyLock::new(|| optimized(raw::SKY3D_WGSL));

static PRELUDE_WGSL_FULL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{}\n{}\n{}",
        SHARED_3D.as_str(),
        optimized(raw::PRELUDE_3D_WGSL),
        STYLIZED_3D.as_str()
    )
});

static MULTIMESH_WGSL_FULL: LazyLock<String> = LazyLock::new(|| {
    let base = format!("{}\n{}", SHARED_3D.as_str(), optimized(raw::MULTIMESH_WGSL));
    let split_at = base
        .find("@vertex\nfn vs_main")
        .or_else(|| base.find("@vertex\r\nfn vs_main"))
        .or_else(|| base.find("@vertex fn vs_main"))
        .unwrap_or(base.len());
    format!(
        "{}\n{}\n{}",
        &base[..split_at],
        STYLIZED_MULTIMESH.as_str(),
        &base[split_at..],
    )
});

static PRELUDE_RIGID_WGSL: LazyLock<String> = LazyLock::new(|| build_rigid_prelude(prelude_wgsl()));
static PRELUDE_SKINNED_WGSL: LazyLock<String> =
    LazyLock::new(|| build_skinned_prelude(prelude_wgsl()));

#[inline]
fn optimized(source: &str) -> String {
    crate::optimize_source(source)
}

#[inline]
#[must_use]
pub fn prelude_wgsl() -> &'static str {
    PRELUDE_WGSL_FULL.as_str()
}

#[inline]
#[must_use]
pub fn multimesh_wgsl() -> &'static str {
    MULTIMESH_WGSL_FULL.as_str()
}

#[inline]
#[must_use]
pub fn prelude_rigid_wgsl() -> &'static str {
    PRELUDE_RIGID_WGSL.as_str()
}

#[inline]
#[must_use]
pub fn prelude_skinned_wgsl() -> &'static str {
    PRELUDE_SKINNED_WGSL.as_str()
}

#[inline]
#[must_use]
pub fn sky3d_wgsl() -> &'static str {
    SKY3D.as_str()
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
        .replace("normal, );", "blended.normal.xyz, );")
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

/// `meta` is reserved in newer WGSL; rename the prelude locals that use it.
#[inline]
#[must_use]
pub fn sanitize_reserved_meta_identifier(wgsl: &str) -> String {
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

#[inline]
#[must_use]
pub fn build_material_shader(material_wgsl: &str) -> String {
    build_material_shader_with_prelude(prelude_wgsl(), material_wgsl)
}

#[inline]
#[must_use]
pub fn build_material_shader_with_prelude(prelude_wgsl: &str, material_wgsl: &str) -> String {
    build_material_shader_with_prelude_inner(prelude_wgsl, material_wgsl, false)
}

#[inline]
#[must_use]
pub fn build_custom_material_shader_with_prelude(
    prelude_wgsl: &str,
    material_wgsl: &str,
    output: MaterialOutput,
) -> String {
    build_material_shader_with_prelude_inner(
        prelude_wgsl,
        material_wgsl,
        applies_standard_lighting(material_wgsl, output),
    )
}

#[must_use]
pub fn build_custom_multimesh_material_shader(
    material_wgsl: &str,
    output: MaterialOutput,
) -> String {
    let base = sanitize_reserved_meta_identifier(multimesh_wgsl());
    let split_at = base
        .find("@vertex\nfn vs_main")
        .or_else(|| base.find("@vertex\r\nfn vs_main"))
        .or_else(|| base.find("@vertex fn vs_main"))
        .unwrap_or(base.len());
    let prelude = &base[..split_at];
    let apply_standard_lighting = applies_standard_lighting(material_wgsl, output);
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
            "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    let base = shade_material(in);\n    return perro_standard(in, base, 0.5, 0.0, 1.0, vec3<f32>(0.0));\n}\n",
        );
    } else {
        out.push_str(
            "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    return shade_material(in);\n}\n",
        );
    }
    out
}

#[inline]
fn applies_standard_lighting(material_wgsl: &str, output: MaterialOutput) -> bool {
    output == MaterialOutput::Surface && !material_uses_final_shade_helper(material_wgsl)
}

#[inline]
#[must_use]
pub fn material_uses_final_shade_helper(material_wgsl: &str) -> bool {
    [
        "perro_standard(",
        "perro_toon(",
        "perro_unlit(",
        "perro_hand_drawn(",
        "perro_pixel_surface(",
        "perro_lit_",
    ]
    .iter()
    .any(|name| material_wgsl.contains(name))
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
            "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    let base = shade_material(in);\n    return perro_standard(in, base, 0.5, 0.0, 1.0, vec3<f32>(0.0));\n}\n",
        );
    } else {
        out.push_str(
            "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    return shade_material(in);\n}\n",
        );
    }
    out
}

#[inline]
#[must_use]
pub fn build_sky_shader() -> String {
    sky3d_wgsl().replace(
        "/*__PERRO_SKY_CUSTOM_STACK__*/",
        "fn apply_custom_sky_stack(base: SkyFragment) -> vec4<f32> { return base.color; }",
    )
}

#[must_use]
pub fn build_sky_shader_with_passes(passes: &[(String, SkyPassParams)]) -> String {
    let mut stack = String::new();
    for (idx, (source, params)) in passes.iter().enumerate() {
        let fn_name = format!("sky_shader_{idx}");
        stack.push('\n');
        stack.push_str(&rename_sky_pass(source, &fn_name));
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
    sky3d_wgsl().replace("/*__PERRO_SKY_CUSTOM_STACK__*/", &stack)
}

#[inline]
pub(crate) fn rename_sky_pass(source: &str, fn_name: &str) -> String {
    source.replacen("fn sky_shader", &format!("fn {fn_name}"), 1)
}

fn encoded_sky_param_values(params: &SkyPassParams) -> String {
    let mut out = String::new();
    for v in params {
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

fn wgsl_f32(v: f32) -> String {
    if v.is_finite() {
        format!("{v:?}")
    } else {
        "0.0".to_string()
    }
}

/// Prelude wrapped around a custom post-process `.wgsl`. Defines the bindings,
/// the fullscreen vertex stage, and an `fs_main` that calls `post_process`.
pub const POST_PRELUDE_WGSL: &str = r#"
struct PostUniform {
    effect_type: u32,
    param_count: u32,
    projection_mode: u32,
    _pad0: u32,
    params0: vec4<f32>,
    params1: vec4<f32>,
    params2: vec4<f32>,
    params3: vec4<f32>,
    params4: vec4<f32>,
    params5: vec4<f32>,
    resolution: vec2<f32>,
    inv_resolution: vec2<f32>,
    near: f32,
    far: f32,
    time: vec2<f32>,
};

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var<uniform> post: PostUniform;
@group(0) @binding(4) var<storage, read> custom_params: array<vec4<f32>>;
@group(0) @binding(5) var lut_2d_tex: texture_2d<f32>;
@group(0) @binding(6) var lut_3d_tex: texture_3d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vid], 0.0, 1.0);
    out.uv = (out.pos.xy * vec2<f32>(0.5, -0.5)) + vec2<f32>(0.5, 0.5);
    return out;
}

fn load_depth(uv: vec2<f32>) -> f32 {
    let dims = textureDimensions(depth_tex);
    let ix = clamp(i32(uv.x * f32(dims.x)), 0, i32(dims.x) - 1);
    let iy = clamp(i32(uv.y * f32(dims.y)), 0, i32(dims.y) - 1);
    return textureLoad(depth_tex, vec2<i32>(ix, iy), 0);
}

fn linearize_depth(depth: f32) -> f32 {
    if post.projection_mode == 1u {
        return post.near + depth * (post.far - post.near);
    }
    return (post.near * post.far) / (post.far - depth * (post.far - post.near));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let color = textureSample(input_tex, input_sampler, in.uv);
    var depth = 1.0;
    if post.effect_type == 0u {
        depth = load_depth(in.uv);
    }
    let out_color = post_process(in.uv, color, depth);
    return out_color;
}

"#;

#[inline]
#[must_use]
pub fn build_post_shader(custom_wgsl: &str) -> String {
    let mut out = String::new();
    out.push_str(POST_PRELUDE_WGSL);
    out.push_str(custom_wgsl);
    out
}
