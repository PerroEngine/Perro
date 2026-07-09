use super::*;

pub(super) fn sky_shader_pipeline_key(sky: &perro_render_bridge::Sky3DState) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    if sky.shaders.is_empty() {
        return None;
    }
    // In-memory cache key: feed the parts straight into the hasher instead of
    // building a String per frame. Only per-run determinism matters here.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for shader in sky.shaders.iter() {
        // str's Hash writes a delimiter, so path bytes stay self-framed.
        shader.path.as_ref().hash(&mut hasher);
        for param in shader.params.iter() {
            hash_const_param_value(&param.value, &mut hasher);
        }
    }
    Some(hasher.finish())
}

// Discriminant byte + fixed-width raw bits frame each value unambiguously; f32
// bits are hashed (not the float) so the key stays deterministic for NaN/-0.
fn hash_const_param_value<H: std::hash::Hasher>(
    value: &perro_structs::ConstParamValue,
    hasher: &mut H,
) {
    use perro_structs::ConstParamValue;
    match value {
        ConstParamValue::F32(v) => {
            hasher.write_u8(0);
            hasher.write_u32(v.to_bits());
        }
        ConstParamValue::I32(v) => {
            hasher.write_u8(1);
            hasher.write_i32(*v);
        }
        ConstParamValue::Bool(v) => {
            hasher.write_u8(2);
            hasher.write_u8(u8::from(*v));
        }
        ConstParamValue::Vec2(v) => {
            hasher.write_u8(3);
            for c in v {
                hasher.write_u32(c.to_bits());
            }
        }
        ConstParamValue::Vec3(v) => {
            hasher.write_u8(4);
            for c in v {
                hasher.write_u32(c.to_bits());
            }
        }
        ConstParamValue::Vec4(v) => {
            hasher.write_u8(5);
            for c in v {
                hasher.write_u32(c.to_bits());
            }
        }
    }
}

impl Gpu3D {
    pub(super) fn ensure_sky_pipeline(
        &mut self,
        device: &wgpu::Device,
        sky: &perro_render_bridge::Sky3DState,
        static_shader_lookup: Option<StaticShaderLookup>,
    ) {
        let Some(key) = sky_shader_pipeline_key(sky) else {
            self.active_sky_pipeline_key = None;
            return;
        };
        if self.custom_sky_pipelines.contains_key(&key) {
            self.active_sky_pipeline_key = Some(key);
            return;
        }
        let Some(wgsl) = build_sky_custom_source(sky, static_shader_lookup) else {
            self.active_sky_pipeline_key = None;
            return;
        };
        let shader = create_sky_shader_module_from_source(device, wgsl);
        let pipeline = create_sky_pipeline(
            device,
            &self.sky_pipeline_layout,
            &shader,
            self.color_format,
            self.sample_count,
        );
        self.custom_sky_pipelines.insert(key, pipeline);
        self.active_sky_pipeline_key = Some(key);
    }
}

fn build_sky_custom_source(
    sky: &perro_render_bridge::Sky3DState,
    static_shader_lookup: Option<StaticShaderLookup>,
) -> Option<String> {
    let mut passes = Vec::with_capacity(sky.shaders.len());
    for shader in sky.shaders.iter() {
        let source = load_shader_source(shader.path.as_ref(), static_shader_lookup)?;
        passes.push((source, shader.params.as_ref()));
    }
    Some(super::super::shaders::build_sky_shader_with_passes(&passes))
}

fn load_shader_source(
    shader_path: &str,
    static_shader_lookup: Option<StaticShaderLookup>,
) -> Option<String> {
    if let Some(lookup) = static_shader_lookup {
        let shader_hash = perro_ids::parse_hashed_source_uri(shader_path)
            .unwrap_or_else(|| perro_ids::string_to_u64(shader_path));
        let src = lookup(shader_hash);
        if !src.is_empty() {
            return Some(src.to_string());
        }
    }
    let bytes = load_asset(shader_path).ok()?;
    let src = std::str::from_utf8(&bytes).ok()?;
    Some(src.to_string())
}

// view_proj / inv_view_proj are computed once by the caller and shared with the
// scene uniform and frustum extraction; inv_view_proj is the raw inverse and the
// is_finite fallback stays here.
pub(super) fn build_sky_uniform(
    camera: &Camera3DState,
    lighting: &Lighting3DState,
    inv_view_proj: Mat4,
) -> Option<SkyUniform> {
    let sky = lighting.sky.as_ref()?;
    let inv = if inv_view_proj.is_finite() {
        inv_view_proj.to_cols_array_2d()
    } else {
        Mat4::IDENTITY.to_cols_array_2d()
    };
    let t_day = day_weight_from_time(sky.time.time_of_day);
    let day_colors = gradient_triplet(sky.day_colors.as_ref());
    let evening_colors = gradient_triplet(sky.evening_colors.as_ref());
    let night_colors = gradient_triplet(sky.night_colors.as_ref());
    let horizon_colors = gradient_triplet(sky.horizon_colors.as_ref());
    let evening_t = evening_weight_from_time(sky.time.time_of_day);
    let night_t = (1.0 - t_day).clamp(0.0, 1.0);
    Some(SkyUniform {
        inv_view_proj: inv,
        camera_pos: [
            camera.position[0],
            camera.position[1],
            camera.position[2],
            0.0,
        ],
        day_colors,
        evening_colors,
        night_colors,
        horizon_colors,
        params0: [
            sky.time.time_of_day.rem_euclid(1.0),
            t_day,
            evening_t,
            night_t,
        ],
        params1: [lighting.sky_time_seconds.max(0.0), 0.0, 0.0, 0.0],
    })
}

pub(super) fn gradient_triplet(colors: &[[f32; 3]]) -> [[f32; 4]; 3] {
    if colors.is_empty() {
        return [[0.0, 0.0, 0.0, 1.0]; 3];
    }
    if colors.len() == 1 {
        return [
            [colors[0][0], colors[0][1], colors[0][2], 1.0],
            [colors[0][0], colors[0][1], colors[0][2], 1.0],
            [colors[0][0], colors[0][1], colors[0][2], 1.0],
        ];
    }
    let first = colors[0];
    let middle = sample_gradient(colors, 0.5);
    let last = colors[colors.len() - 1];
    [
        [first[0], first[1], first[2], 1.0],
        [middle[0], middle[1], middle[2], 1.0],
        [last[0], last[1], last[2], 1.0],
    ]
}

pub(super) fn day_weight_from_time(time_of_day: f32) -> f32 {
    let t = time_of_day.rem_euclid(1.0);
    let a = (t * std::f32::consts::TAU) - std::f32::consts::FRAC_PI_2;
    ((a.sin() + 1.0) * 0.5).clamp(0.0, 1.0)
}

pub(super) fn evening_weight_from_time(time_of_day: f32) -> f32 {
    let t = time_of_day.rem_euclid(1.0);
    let dist = ((t - 0.75 + 0.5).rem_euclid(1.0) - 0.5).abs();
    (1.0 - (dist / 0.23)).clamp(0.0, 1.0)
}

pub(super) fn sample_gradient(colors: &[[f32; 3]], t: f32) -> [f32; 3] {
    if colors.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    if colors.len() == 1 {
        return colors[0];
    }
    let n = colors.len() - 1;
    let f = t.clamp(0.0, 1.0) * n as f32;
    let i = f.floor() as usize;
    let j = (i + 1).min(n);
    let u = (f - i as f32).clamp(0.0, 1.0);
    lerp3(colors[i], colors[j], u)
}

pub(super) fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
