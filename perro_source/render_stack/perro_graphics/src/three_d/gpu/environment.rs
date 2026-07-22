use super::*;
use std::f32::consts::{PI, TAU};

pub(super) const ENV_IRRADIANCE_SIZE: u32 = 16;
pub(super) const ENV_SPECULAR_SIZE: u32 = 64;
pub(super) const ENV_SPECULAR_MIP_COUNT: u32 = 7;
pub(super) const ENV_BRDF_LUT_SIZE: u32 = 128;
const ENV_SAMPLE_COUNT: u32 = 64;
const BRDF_SAMPLE_COUNT: u32 = 128;

pub(super) struct EnvironmentCubeGpu {
    packed_buffer: wgpu::Buffer,
}

pub(super) struct EnvironmentGpuMaps {
    fallback: EnvironmentCubeGpu,
    active: Option<EnvironmentCubeGpu>,
    brdf: BrdfLutBake,
    source: Option<String>,
}

impl EnvironmentGpuMaps {
    fn cube(&self) -> &EnvironmentCubeGpu {
        self.active.as_ref().unwrap_or(&self.fallback)
    }

    pub(super) fn active(&self) -> bool {
        self.active.is_some()
    }
}

fn requested_environment_source(
    environment: Option<&perro_render_bridge::EnvironmentMap3DState>,
) -> Option<&str> {
    environment
        .map(|environment| environment.source.trim())
        .filter(|source| !source.is_empty())
}

fn environment_source_changed(cached: Option<&str>, requested: Option<&str>) -> bool {
    cached != requested
}

pub(super) struct CubeLevel {
    pub size: u32,
    /// Face-major RGBA16F texels: +X, -X, +Y, -Y, +Z, -Z.
    pub rgba16f: Vec<u16>,
}

pub(super) struct EnvironmentBake {
    pub irradiance: CubeLevel,
    pub specular: Vec<CubeLevel>,
}

pub(super) struct BrdfLutBake {
    pub size: u32,
    pub rg16f: Vec<u16>,
}

struct LinearEquirect {
    pixels: Vec<[f32; 3]>,
    width: u32,
    height: u32,
}

pub(super) fn load_environment_rgba(
    source: &str,
    resources: &ResourceStore,
    static_texture_lookup: Option<StaticTextureLookup>,
) -> Option<(Vec<u8>, u32, u32)> {
    if let Some(decoded) = resources.decoded_texture_data_by_source(source) {
        return Some((decoded.rgba.clone(), decoded.width, decoded.height));
    }
    if resources.has_texture_source(source) {
        return None;
    }
    if let Some(lookup) = static_texture_lookup {
        let hash = perro_ids::parse_hashed_source_uri(source)
            .unwrap_or_else(|| perro_ids::string_to_u64(source));
        let bytes = lookup(hash);
        if !bytes.is_empty() {
            return perro_graphics_assets::decode_ptex(bytes)
                .or_else(|| perro_graphics_assets::decode_image_rgba(bytes));
        }
    }
    perro_graphics_assets::load_texture_rgba(source)
}

pub(super) fn bake_environment(rgba: &[u8], width: u32, height: u32) -> Option<EnvironmentBake> {
    let source = LinearEquirect::from_rgba8(rgba, width, height)?;
    let irradiance = bake_cube_level(ENV_IRRADIANCE_SIZE, |dir| integrate_diffuse(&source, dir));
    let mut specular = Vec::with_capacity(ENV_SPECULAR_MIP_COUNT as usize);
    for mip in 0..ENV_SPECULAR_MIP_COUNT {
        let size = (ENV_SPECULAR_SIZE >> mip).max(1);
        let roughness = mip as f32 / (ENV_SPECULAR_MIP_COUNT - 1) as f32;
        specular.push(bake_cube_level(size, |dir| {
            integrate_specular(&source, dir, roughness)
        }));
    }
    Some(EnvironmentBake {
        irradiance,
        specular,
    })
}

pub(super) fn bake_brdf_lut() -> BrdfLutBake {
    let mut rg16f = Vec::with_capacity((ENV_BRDF_LUT_SIZE * ENV_BRDF_LUT_SIZE * 2) as usize);
    for y in 0..ENV_BRDF_LUT_SIZE {
        let roughness = (y as f32 + 0.5) / ENV_BRDF_LUT_SIZE as f32;
        for x in 0..ENV_BRDF_LUT_SIZE {
            let n_dot_v = (x as f32 + 0.5) / ENV_BRDF_LUT_SIZE as f32;
            let [a, b] = integrate_brdf(n_dot_v, roughness);
            rg16f.push(f32_to_f16(a));
            rg16f.push(f32_to_f16(b));
        }
    }
    BrdfLutBake {
        size: ENV_BRDF_LUT_SIZE,
        rg16f,
    }
}

pub(super) fn create_environment_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_environment_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    })
}

pub(super) fn create_environment_gpu_maps(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> EnvironmentGpuMaps {
    let brdf = bake_brdf_lut();
    let fallback = upload_environment_bake(device, queue, &black_environment_bake(), &brdf);
    EnvironmentGpuMaps {
        fallback,
        active: None,
        brdf,
        source: None,
    }
}

pub(super) fn create_environment_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    maps: &EnvironmentGpuMaps,
    mesh_blend_depth_view: &wgpu::TextureView,
    ssao_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    let cube = maps.cube();
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_environment_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: cube.packed_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(mesh_blend_depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(ssao_view),
            },
        ],
    })
}

impl Gpu3D {
    pub(super) fn ensure_environment_map(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        environment: Option<&perro_render_bridge::EnvironmentMap3DState>,
        resources: &ResourceStore,
        static_texture_lookup: Option<StaticTextureLookup>,
    ) {
        let requested = requested_environment_source(environment);
        let pending = requested.is_some_and(|source| {
            resources.has_texture_source(source)
                && resources.decoded_texture_data_by_source(source).is_none()
        });
        if !environment_source_changed(self.ibl_maps.source.as_deref(), requested) && !pending {
            return;
        }
        self.ibl_maps.source = requested.map(str::to_owned);
        self.ibl_maps.active = requested.and_then(|source| {
            let Some((rgba, width, height)) =
                load_environment_rgba(source, resources, static_texture_lookup)
            else {
                if !pending {
                    eprintln!("[perro][ibl] load fail: {source}; use procedural fallback");
                }
                return None;
            };
            let Some(bake) = bake_environment(&rgba, width, height) else {
                eprintln!("[perro][ibl] bake fail: {source}; use procedural fallback");
                return None;
            };
            Some(upload_environment_bake(
                device,
                queue,
                &bake,
                &self.ibl_maps.brdf,
            ))
        });
        self.rebuild_environment_bind_group(device);
    }

    pub(super) fn rebuild_environment_bind_group(&mut self, device: &wgpu::Device) {
        self.ibl_bind_group = create_environment_bind_group(
            device,
            &self.ibl_bgl,
            &self.ibl_maps,
            &self.mesh_blend_depth_view,
            self.ssao_pass
                .as_ref()
                .map(ssao::SsaoPass::view)
                .unwrap_or(&self.ssao_fallback_view),
        );
    }
}

fn black_environment_bake() -> EnvironmentBake {
    let black_level = |size| CubeLevel {
        size,
        rgba16f: [0, 0, 0, f32_to_f16(1.0)].repeat((size * size * 6) as usize),
    };
    EnvironmentBake {
        irradiance: black_level(ENV_IRRADIANCE_SIZE),
        specular: (0..ENV_SPECULAR_MIP_COUNT)
            .map(|mip| black_level((ENV_SPECULAR_SIZE >> mip).max(1)))
            .collect(),
    }
}

fn upload_environment_bake(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bake: &EnvironmentBake,
    brdf: &BrdfLutBake,
) -> EnvironmentCubeGpu {
    let cube_half_count = |level: &CubeLevel| level.size as usize * level.size as usize * 6 * 4;
    let brdf_half_count = brdf.size as usize * brdf.size as usize * 2;
    let half_count = cube_half_count(&bake.irradiance)
        + bake.specular.iter().map(cube_half_count).sum::<usize>()
        + brdf_half_count;
    let mut words = Vec::with_capacity(half_count);
    append_half_as_f32_bits(&mut words, &bake.irradiance.rgba16f);
    for level in &bake.specular {
        append_half_as_f32_bits(&mut words, &level.rgba16f);
    }
    append_half_as_f32_bits(&mut words, &brdf.rg16f);
    let packed_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_environment_packed_buffer"),
        size: (words.len() * std::mem::size_of::<u32>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&packed_buffer, 0, bytemuck::cast_slice(&words));
    EnvironmentCubeGpu { packed_buffer }
}

fn append_half_as_f32_bits(words: &mut Vec<u32>, values: &[u16]) {
    words.extend(values.iter().map(|value| f16_to_f32(*value).to_bits()));
}

fn f16_to_f32(value: u16) -> f32 {
    let sign = ((value & 0x8000) as u32) << 16;
    let exponent = ((value >> 10) & 0x1f) as u32;
    let mantissa = (value & 0x03ff) as u32;
    let bits = match exponent {
        0 if mantissa == 0 => sign,
        0 => {
            let mut mantissa = mantissa;
            let mut exponent = 113u32;
            while mantissa & 0x0400 == 0 {
                mantissa <<= 1;
                exponent -= 1;
            }
            sign | (exponent << 23) | ((mantissa & 0x03ff) << 13)
        }
        31 => sign | 0x7f80_0000 | (mantissa << 13),
        _ => sign | ((exponent + 112) << 23) | (mantissa << 13),
    };
    f32::from_bits(bits)
}

fn bake_cube_level(size: u32, mut sample: impl FnMut([f32; 3]) -> [f32; 3]) -> CubeLevel {
    let mut rgba16f = Vec::with_capacity((size * size * 6 * 4) as usize);
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let u = 2.0 * (x as f32 + 0.5) / size as f32 - 1.0;
                let v = 2.0 * (y as f32 + 0.5) / size as f32 - 1.0;
                let rgb = sample(cube_direction(face, u, v));
                rgba16f.extend_from_slice(&[
                    f32_to_f16(rgb[0]),
                    f32_to_f16(rgb[1]),
                    f32_to_f16(rgb[2]),
                    f32_to_f16(1.0),
                ]);
            }
        }
    }
    CubeLevel { size, rgba16f }
}

fn cube_direction(face: u32, u: f32, v: f32) -> [f32; 3] {
    let dir = match face {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        _ => [-u, -v, -1.0],
    };
    normalize(dir)
}

fn integrate_diffuse(source: &LinearEquirect, normal: [f32; 3]) -> [f32; 3] {
    let (tangent, bitangent) = tangent_basis(normal);
    let mut sum = [0.0; 3];
    for index in 0..ENV_SAMPLE_COUNT {
        let [u, v] = hammersley(index, ENV_SAMPLE_COUNT);
        let phi = TAU * u;
        let radius = v.sqrt();
        let local = [radius * phi.cos(), radius * phi.sin(), (1.0 - v).sqrt()];
        let dir = tangent_to_world(local, tangent, bitangent, normal);
        let rgb = source.sample(dir);
        sum[0] += rgb[0];
        sum[1] += rgb[1];
        sum[2] += rgb[2];
    }
    scale(sum, 1.0 / ENV_SAMPLE_COUNT as f32)
}

fn integrate_specular(source: &LinearEquirect, normal: [f32; 3], roughness: f32) -> [f32; 3] {
    if roughness <= f32::EPSILON {
        return source.sample(normal);
    }
    let (tangent, bitangent) = tangent_basis(normal);
    let mut sum = [0.0; 3];
    let mut weight = 0.0;
    for index in 0..ENV_SAMPLE_COUNT {
        let h_local = importance_sample_ggx(hammersley(index, ENV_SAMPLE_COUNT), roughness);
        let h = tangent_to_world(h_local, tangent, bitangent, normal);
        let v_dot_h = dot(normal, h).max(0.0);
        let light = normalize(sub(scale(h, 2.0 * v_dot_h), normal));
        let n_dot_l = dot(normal, light).max(0.0);
        if n_dot_l > 0.0 {
            let rgb = source.sample(light);
            sum[0] += rgb[0] * n_dot_l;
            sum[1] += rgb[1] * n_dot_l;
            sum[2] += rgb[2] * n_dot_l;
            weight += n_dot_l;
        }
    }
    scale(sum, weight.max(1.0e-5).recip())
}

fn integrate_brdf(n_dot_v: f32, roughness: f32) -> [f32; 2] {
    let view = [(1.0 - n_dot_v * n_dot_v).sqrt(), 0.0, n_dot_v];
    let mut a = 0.0;
    let mut b = 0.0;
    for index in 0..BRDF_SAMPLE_COUNT {
        let h = importance_sample_ggx(hammersley(index, BRDF_SAMPLE_COUNT), roughness);
        let v_dot_h = dot(view, h).max(0.0);
        let light = normalize(sub(scale(h, 2.0 * v_dot_h), view));
        let n_dot_l = light[2].max(0.0);
        let n_dot_h = h[2].max(0.0);
        if n_dot_l > 0.0 {
            let geometry = geometry_smith_ibl(n_dot_v, n_dot_l, roughness);
            let visible = geometry * v_dot_h / (n_dot_h * n_dot_v).max(1.0e-5);
            let fresnel = (1.0 - v_dot_h).powi(5);
            a += (1.0 - fresnel) * visible;
            b += fresnel * visible;
        }
    }
    [a / BRDF_SAMPLE_COUNT as f32, b / BRDF_SAMPLE_COUNT as f32]
}

fn importance_sample_ggx(sample: [f32; 2], roughness: f32) -> [f32; 3] {
    let alpha = roughness * roughness;
    let alpha2 = alpha * alpha;
    let phi = TAU * sample[0];
    let cos_theta = ((1.0 - sample[1]) / (1.0 + (alpha2 - 1.0) * sample[1])).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    [phi.cos() * sin_theta, phi.sin() * sin_theta, cos_theta]
}

fn geometry_smith_ibl(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let k = roughness * roughness * 0.5;
    let g_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let g_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
    g_v * g_l
}

fn hammersley(index: u32, count: u32) -> [f32; 2] {
    [index as f32 / count as f32, radical_inverse(index)]
}

fn radical_inverse(mut bits: u32) -> f32 {
    bits = bits.rotate_right(16);
    bits = ((bits & 0x5555_5555) << 1) | ((bits & 0xaaaa_aaaa) >> 1);
    bits = ((bits & 0x3333_3333) << 2) | ((bits & 0xcccc_cccc) >> 2);
    bits = ((bits & 0x0f0f_0f0f) << 4) | ((bits & 0xf0f0_f0f0) >> 4);
    bits = ((bits & 0x00ff_00ff) << 8) | ((bits & 0xff00_ff00) >> 8);
    bits as f32 * 2.328_306_4e-10
}

fn tangent_basis(normal: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let up = if normal[2].abs() < 0.999 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let tangent = normalize(cross(up, normal));
    (tangent, cross(normal, tangent))
}

fn tangent_to_world(
    local: [f32; 3],
    tangent: [f32; 3],
    bitangent: [f32; 3],
    normal: [f32; 3],
) -> [f32; 3] {
    normalize([
        tangent[0] * local[0] + bitangent[0] * local[1] + normal[0] * local[2],
        tangent[1] * local[0] + bitangent[1] * local[1] + normal[1] * local[2],
        tangent[2] * local[0] + bitangent[2] * local[1] + normal[2] * local[2],
    ])
}

impl LinearEquirect {
    fn from_rgba8(rgba: &[u8], width: u32, height: u32) -> Option<Self> {
        if width == 0
            || height == 0
            || rgba.len() != width.checked_mul(height)?.checked_mul(4)? as usize
        {
            return None;
        }
        let pixels = rgba
            .chunks_exact(4)
            .map(|pixel| {
                [
                    srgb_to_linear(pixel[0]),
                    srgb_to_linear(pixel[1]),
                    srgb_to_linear(pixel[2]),
                ]
            })
            .collect();
        Some(Self {
            pixels,
            width,
            height,
        })
    }

    fn sample(&self, direction: [f32; 3]) -> [f32; 3] {
        let u = (direction[2].atan2(direction[0]) / TAU + 0.5).rem_euclid(1.0);
        let v = direction[1].clamp(-1.0, 1.0).acos() / PI;
        let x = u * self.width as f32 - 0.5;
        let y = v * self.height as f32 - 0.5;
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let tx = x - x.floor();
        let ty = y - y.floor();
        let a = self.pixel(x0, y0);
        let b = self.pixel(x0 + 1, y0);
        let c = self.pixel(x0, y0 + 1);
        let d = self.pixel(x0 + 1, y0 + 1);
        mix3(mix3(a, b, tx), mix3(c, d, tx), ty)
    }

    fn pixel(&self, x: i32, y: i32) -> [f32; 3] {
        let x = x.rem_euclid(self.width as i32) as u32;
        let y = y.clamp(0, self.height as i32 - 1) as u32;
        self.pixels[(y * self.width + x) as usize]
    }
}

fn srgb_to_linear(value: u8) -> f32 {
    let value = value as f32 / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mantissa = bits & 0x7f_ffff;
    if exponent <= 0 {
        if exponent < -10 {
            return sign;
        }
        let mantissa = mantissa | 0x80_0000;
        let shift = 14 - exponent;
        let rounded = (mantissa + (1 << (shift - 1))) >> shift;
        return sign | rounded as u16;
    }
    if exponent >= 31 {
        return sign | 0x7c00;
    }
    let rounded = mantissa + 0x1000;
    if rounded & 0x80_0000 != 0 {
        let next_exponent = exponent + 1;
        return if next_exponent >= 31 {
            sign | 0x7c00
        } else {
            sign | ((next_exponent as u16) << 10)
        };
    }
    sign | ((exponent as u16) << 10) | ((rounded >> 13) as u16)
}

fn normalize(value: [f32; 3]) -> [f32; 3] {
    scale(value, dot(value, value).max(1.0e-12).sqrt().recip())
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mix3(a: [f32; 3], b: [f32; 3], weight: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * weight,
        a[1] + (b[1] - a[1]) * weight,
        a[2] + (b[2] - a[2]) * weight,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_environment_filters_to_same_color() {
        let rgba = [128, 64, 32, 255].repeat(8);
        let source = LinearEquirect::from_rgba8(&rgba, 4, 2).expect("valid source");
        let expected = [srgb_to_linear(128), srgb_to_linear(64), srgb_to_linear(32)];
        for filtered in [
            integrate_diffuse(&source, [0.0, 1.0, 0.0]),
            integrate_specular(&source, [1.0, 0.0, 0.0], 0.6),
        ] {
            for channel in 0..3 {
                assert!((filtered[channel] - expected[channel]).abs() < 0.0001);
            }
        }
    }

    #[test]
    fn specular_bake_has_full_roughness_mip_chain() {
        let bake = black_environment_bake();
        assert_eq!(bake.specular.len(), ENV_SPECULAR_MIP_COUNT as usize);
        assert_eq!(
            bake.specular
                .first()
                .expect("required value must be present")
                .size,
            ENV_SPECULAR_SIZE
        );
        assert_eq!(
            bake.specular
                .last()
                .expect("required value must be present")
                .size,
            1
        );
    }

    #[test]
    fn brdf_lut_is_deterministic_and_bounded() {
        for (n_dot_v, roughness) in [(0.1, 0.1), (0.5, 0.5), (0.9, 0.9)] {
            let first = integrate_brdf(n_dot_v, roughness);
            let second = integrate_brdf(n_dot_v, roughness);
            assert_eq!(first, second);
            assert!(
                first
                    .into_iter()
                    .all(|value| (0.0..=1.001).contains(&value))
            );
        }
    }

    #[test]
    fn half_float_pack_keeps_zero_one_and_hdr() {
        for value in [0.0, 1.0, 4.0] {
            assert!((f16_to_f32(f32_to_f16(value)) - value).abs() < 0.001);
        }
    }

    #[test]
    fn environment_source_change_gate_trims_and_detects_changes() {
        let mut environment = perro_render_bridge::EnvironmentMap3DState {
            source: "  res://studio.png  ".into(),
            intensity: 1.0,
            rotation_degrees: 0.0,
        };
        let requested = requested_environment_source(Some(&environment));
        assert_eq!(requested, Some("res://studio.png"));
        assert!(!environment_source_changed(
            Some("res://studio.png"),
            requested
        ));
        assert!(environment_source_changed(Some("res://old.png"), requested));

        environment.source = "   ".into();
        assert_eq!(requested_environment_source(Some(&environment)), None);
    }

    #[test]
    fn environment_load_uses_resource_data_and_missing_signals_fallback() {
        let source = "res://environment-test.png";
        let mut resources = ResourceStore::new();
        let id = resources.create_texture(source, false);
        assert!(load_environment_rgba(source, &resources, None).is_none());

        let rgba = vec![16, 32, 64, 255, 128, 96, 48, 255];
        assert!(resources.set_decoded_texture_data(
            id,
            crate::resources::DecodedTextureRgba {
                rgba: rgba.clone(),
                width: 2,
                height: 1,
            },
        ));
        assert_eq!(
            load_environment_rgba(source, &resources, None),
            Some((rgba, 2, 1))
        );
        assert!(load_environment_rgba("res://missing.png", &resources, None).is_none());
    }
}
