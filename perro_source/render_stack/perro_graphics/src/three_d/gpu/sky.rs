use super::*;

const SKY_SHADER_DEFAULT: &str = "DEFAULT";
const SKY_SHADER_VOLUMETRIC: &str = "VOLUMETRIC";
const SKY_SHADER_WISPY: &str = "WISPY";

pub(super) fn normalize_sky_shader_path(path: Option<&str>) -> Option<&str> {
    let value = path?.trim();
    if value.is_empty()
        || value.eq_ignore_ascii_case(SKY_SHADER_DEFAULT)
        || value.eq_ignore_ascii_case(SKY_SHADER_VOLUMETRIC)
        || value.eq_ignore_ascii_case(SKY_SHADER_WISPY)
    {
        None
    } else {
        Some(value)
    }
}

pub(super) fn sky_shader_pipeline_key(sky: &perro_render_bridge::Sky3DState) -> Option<String> {
    let whole = normalize_sky_shader_path(sky.sky_shader.as_deref());
    let clouds = normalize_sky_shader_path(sky.cloud_shader.as_deref());
    let sun = normalize_sky_shader_path(sky.sun_shader.as_deref());
    let moon = normalize_sky_shader_path(sky.moon_shader.as_deref());
    if whole.is_none() && clouds.is_none() && sun.is_none() && moon.is_none() {
        return None;
    }
    if let Some(whole) = whole {
        return Some(format!("sky={whole}"));
    }
    Some(format!(
        "clouds={}|sun={}|moon={}",
        clouds.unwrap_or(SKY_SHADER_DEFAULT),
        sun.unwrap_or(SKY_SHADER_DEFAULT),
        moon.unwrap_or(SKY_SHADER_DEFAULT)
    ))
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
        self.custom_sky_pipelines.insert(key.clone(), pipeline);
        self.active_sky_pipeline_key = Some(key);
    }
}

fn build_sky_custom_source(
    sky: &perro_render_bridge::Sky3DState,
    static_shader_lookup: Option<StaticShaderLookup>,
) -> Option<String> {
    if let Some(path) = normalize_sky_shader_path(sky.sky_shader.as_deref()) {
        return load_shader_source(path, static_shader_lookup);
    }
    let moon = match normalize_sky_shader_path(sky.moon_shader.as_deref()) {
        Some(path) => load_shader_source(path, static_shader_lookup)?,
        None => {
            perro_macros::include_str_stripped!("three_d/shaders/sky3d_parts/moon.wgsl").to_string()
        }
    };
    let sun = match normalize_sky_shader_path(sky.sun_shader.as_deref()) {
        Some(path) => load_shader_source(path, static_shader_lookup)?,
        None => {
            perro_macros::include_str_stripped!("three_d/shaders/sky3d_parts/sun.wgsl").to_string()
        }
    };
    let clouds = match normalize_sky_shader_path(sky.cloud_shader.as_deref()) {
        Some(path) => load_shader_source(path, static_shader_lookup)?,
        None => perro_macros::include_str_stripped!("three_d/shaders/sky3d_parts/clouds.wgsl")
            .to_string(),
    };
    Some(build_sky_shader_with_parts(&moon, &sun, &clouds))
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

pub(super) fn build_sky_uniform(
    camera: &Camera3DState,
    lighting: &Lighting3DState,
    width: u32,
    height: u32,
) -> Option<SkyUniform> {
    let sky = lighting.sky.as_ref()?;
    let view_proj = compute_view_proj_mat(camera, width, height);
    let inv_view_proj = view_proj.inverse();
    let inv = if inv_view_proj.is_finite() {
        inv_view_proj.to_cols_array_2d()
    } else {
        Mat4::IDENTITY.to_cols_array_2d()
    };
    let t_day = day_weight_from_time(sky.time.time_of_day);
    let day_colors = gradient_triplet(sky.day_colors.as_ref());
    let evening_colors = gradient_triplet(sky.evening_colors.as_ref());
    let night_colors = gradient_triplet(sky.night_colors.as_ref());
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
        params0: [
            sky.cloud_size.max(0.0),
            sky.cloud_density.clamp(0.0, 1.0),
            sky.cloud_variance.clamp(0.0, 1.0),
            sky.time.time_of_day.rem_euclid(1.0),
        ],
        params1: [
            sky.star_size.max(0.0),
            sky.star_scatter.clamp(0.0, 1.0),
            sky.star_gleam.max(0.0),
            sky.sky_angle,
        ],
        params2: [
            sky.sun_size.max(0.0),
            sky.moon_size.max(0.0),
            t_day,
            lighting.sky_cloud_time_seconds.max(0.0),
        ],
        wind: [
            sky.cloud_wind_vector[0],
            sky.cloud_wind_vector[1],
            sky.style_blend.clamp(0.0, 1.0),
            sky.cloud_mode as f32,
        ],
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

pub(super) fn horizon_visibility(y: f32) -> f32 {
    let t = ((y + 0.08) / 0.16).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn sun_moon_dirs_from_time(time_of_day: f32, sky_angle: f32) -> (Vec3, Vec3) {
    let t = time_of_day.rem_euclid(1.0);
    let theta = (t * std::f32::consts::TAU) - std::f32::consts::FRAC_PI_2 + sky_angle;
    let sun = Vec3::new(theta.cos(), theta.sin(), -0.25).normalize_or_zero();
    let moon = -sun;
    (sun, moon)
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
