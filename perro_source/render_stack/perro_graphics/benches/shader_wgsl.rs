use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use naga::valid::{Capabilities, ValidationFlags, Validator};
use perro_graphics::three_d;
use perro_graphics::two_d::shaders::{
    POINT_LIGHT_2D_WGSL, RECT_INSTANCED_WGSL, SPRITE_INSTANCED_WGSL,
};

const PRELUDE_3D_WGSL: &str = include_str!("../src/three_d/shaders/prelude_3d.wgsl");
const PRELUDE_RIGID_3D_WGSL: &str = include_str!("../src/three_d/shaders/prelude_rigid_3d.wgsl");
const PRELUDE_SKINNED_3D_WGSL: &str =
    include_str!("../src/three_d/shaders/prelude_skinned_3d.wgsl");
const MATERIAL_STANDARD_WGSL: &str = include_str!("../src/three_d/shaders/material_standard.wgsl");
const MATERIAL_UNLIT_WGSL: &str = include_str!("../src/three_d/shaders/material_unlit.wgsl");
const MATERIAL_TOON_WGSL: &str = include_str!("../src/three_d/shaders/material_toon.wgsl");
const SKY3D_ATMO_WGSL: &str = include_str!("../src/three_d/shaders/sky3d_parts/atmo.wgsl");
const SKY3D_MOON_WGSL: &str = include_str!("../src/three_d/shaders/sky3d_parts/moon.wgsl");
const SKY3D_SUN_WGSL: &str = include_str!("../src/three_d/shaders/sky3d_parts/sun.wgsl");
const SKY3D_CLOUDS_WGSL: &str = include_str!("../src/three_d/shaders/sky3d_parts/clouds.wgsl");
const FRUSTUM_CULL_WGSL: &str = include_str!("../src/three_d/shaders/frustum_cull.wgsl");
const HIZ_OCCLUSION_CULL_WGSL: &str =
    include_str!("../src/three_d/shaders/hiz_occlusion_cull.wgsl");
const POST_PRELUDE_WGSL: &str = include_str!("../src/postprocess/shaders.rs");
const POST_BUILTIN_BODY_WGSL: &str =
    include_str!("../src/postprocess/shaders/postprocess_builtin_body.wgsl");
const POST_BLUR_WGSL: &str = include_str!("../src/postprocess/shaders/effects/blur.wgsl");
const POST_BLOOM_WGSL: &str = include_str!("../src/postprocess/shaders/effects/bloom.wgsl");
const POST_CRT_WGSL: &str = include_str!("../src/postprocess/shaders/effects/crt.wgsl");
const PARTICLES_CPU_WGSL: &str = perro_graphics::three_d::particles::shaders::POINT_PARTICLES_WGSL;
const PARTICLES_GPU_WGSL: &str =
    perro_graphics::three_d::particles::shaders::POINT_PARTICLES_GPU_WGSL;
const PARTICLES_COMPUTE_WGSL: &str =
    perro_graphics::three_d::particles::shaders::POINT_PARTICLES_COMPUTE_WGSL;

fn parse_and_validate(wgsl: &str) -> usize {
    let module = naga::front::wgsl::parse_str(wgsl).expect("WGSL parse");
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::empty());
    let info = validator.validate(&module).expect("WGSL validate");
    black_box(info);
    module.functions.len()
}

fn build_sky_shader() -> String {
    let mut out = String::new();
    out.push_str(SKY3D_ATMO_WGSL);
    out.push('\n');
    out.push_str(SKY3D_MOON_WGSL);
    out.push('\n');
    out.push_str(SKY3D_SUN_WGSL);
    out.push('\n');
    out.push_str(SKY3D_CLOUDS_WGSL);
    out
}

fn post_prelude_literal() -> &'static str {
    POST_PRELUDE_WGSL
        .split("const PRELUDE_WGSL: &str = r#\"")
        .nth(1)
        .expect("post prelude start")
        .split("\"#;")
        .next()
        .expect("post prelude end")
}

fn build_post_subset_shader() -> String {
    let mut out = String::new();
    out.push_str(post_prelude_literal());
    out.push_str(POST_BLUR_WGSL);
    out.push_str(POST_BLOOM_WGSL);
    out.push_str(POST_CRT_WGSL);
    out.push_str(POST_BUILTIN_BODY_WGSL);
    out
}

fn bench_shader_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_shader_build");
    let material_cases = [
        ("standard", PRELUDE_3D_WGSL, MATERIAL_STANDARD_WGSL),
        (
            "rigid_standard",
            PRELUDE_RIGID_3D_WGSL,
            MATERIAL_STANDARD_WGSL,
        ),
        (
            "skinned_standard",
            PRELUDE_SKINNED_3D_WGSL,
            MATERIAL_STANDARD_WGSL,
        ),
        ("unlit", PRELUDE_3D_WGSL, MATERIAL_UNLIT_WGSL),
        ("toon", PRELUDE_3D_WGSL, MATERIAL_TOON_WGSL),
    ];
    for (name, prelude, material) in material_cases {
        group.bench_function(BenchmarkId::new("material", name), |b| {
            b.iter(|| {
                black_box(three_d::shaders::build_material_shader_with_prelude(
                    black_box(prelude),
                    black_box(material),
                ))
            });
        });
    }
    group.bench_function("sky", |b| b.iter(|| black_box(build_sky_shader())));
    group.bench_function("post_subset", |b| {
        b.iter(|| black_box(build_post_subset_shader()))
    });
    group.finish();
}

fn bench_shader_parse_validate(c: &mut Criterion) {
    let sky = build_sky_shader();
    let post = build_post_subset_shader();
    let material_standard = three_d::shaders::build_material_shader_with_prelude(
        PRELUDE_3D_WGSL,
        MATERIAL_STANDARD_WGSL,
    );
    let material_skinned = three_d::shaders::build_material_shader_with_prelude(
        PRELUDE_SKINNED_3D_WGSL,
        MATERIAL_STANDARD_WGSL,
    );
    let cases = [
        ("2d_sprite", SPRITE_INSTANCED_WGSL),
        ("2d_rect", RECT_INSTANCED_WGSL),
        ("2d_point_light", POINT_LIGHT_2D_WGSL),
        ("3d_material_standard", material_standard.as_str()),
        ("3d_material_skinned", material_skinned.as_str()),
        ("3d_sky", sky.as_str()),
        ("3d_frustum_cull", FRUSTUM_CULL_WGSL),
        ("3d_hiz_occlusion", HIZ_OCCLUSION_CULL_WGSL),
        ("post_subset", post.as_str()),
        ("particles_cpu", PARTICLES_CPU_WGSL),
        ("particles_gpu", PARTICLES_GPU_WGSL),
        ("particles_compute", PARTICLES_COMPUTE_WGSL),
    ];

    let mut group = c.benchmark_group("graphics_shader_parse_validate");
    for (name, wgsl) in cases {
        group.bench_function(name, |b| {
            b.iter(|| black_box(parse_and_validate(black_box(wgsl))))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_shader_build, bench_shader_parse_validate);
criterion_main!(benches);
