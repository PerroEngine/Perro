use crate::StaticPipelineError;

#[derive(Clone, Debug)]
pub(crate) struct ShaderBakeJob {
    pub material_uri: String,
    pub texture_uri: String,
    pub shader_uri: String,
    pub shader_source: String,
    pub material_source: String,
    pub resolution: [u32; 2],
    pub params: Vec<[f32; 4]>,
    pub image_sources: Vec<String>,
}

impl ShaderBakeJob {
    pub(crate) fn fingerprint(&self) -> u64 {
        let mut source = format!(
            "perro_shader_bake_v1\n{}\n{}x{}\n{}\n{}",
            self.shader_uri,
            self.resolution[0],
            self.resolution[1],
            self.shader_source,
            self.material_source
        );
        for param in &self.params {
            source.push_str(&format!("\n{:?}", param.map(f32::to_bits)));
        }
        for image in &self.image_sources {
            source.push_str("\nimg=");
            source.push_str(image);
        }
        perro_ids::string_to_u64(&source)
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
pub(crate) fn bake_shader_texture(job: &ShaderBakeJob) -> Result<Vec<u8>, StaticPipelineError> {
    if !job.image_sources.is_empty() {
        return Err(StaticPipelineError::ShaderBake(format!(
            "material `{}` bake images not supported yet; use pure WGSL + params",
            job.material_uri
        )));
    }
    if !job.shader_source.contains("bake_texture(") {
        return Err(StaticPipelineError::ShaderBake(format!(
            "shader `{}` needs `fn bake_texture(in: BakeInput) -> vec4<f32>`",
            job.shader_uri
        )));
    }
    if job.shader_source.contains("shade_vertex(") {
        return Err(StaticPipelineError::ShaderBake(format!(
            "material `{}` cannot bake shader vertex output",
            job.material_uri
        )));
    }
    pollster::block_on(bake_shader_texture_async(job))
}

#[cfg(not(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "linux", target_os = "macos")
)))]
pub(crate) fn bake_shader_texture(_job: &ShaderBakeJob) -> Result<Vec<u8>, StaticPipelineError> {
    Err(StaticPipelineError::ShaderBake(
        "shader texture baking requires Windows, Linux, or macOS build host".to_string(),
    ))
}

#[cfg(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
async fn bake_shader_texture_async(job: &ShaderBakeJob) -> Result<Vec<u8>, StaticPipelineError> {
    use wgpu::util::DeviceExt as _;

    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .map_err(|err| {
            StaticPipelineError::ShaderBake(format!("no GPU adapter for shader bake: {err}"))
        })?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("perro_shader_bake_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::default(),
        })
        .await
        .map_err(|err| StaticPipelineError::ShaderBake(format!("shader bake device: {err}")))?;

    let mut uniform = [[0.0f32; 4]; 17];
    let [width, height] = job.resolution;
    uniform[0] = [
        width as f32,
        height as f32,
        1.0 / width as f32,
        1.0 / height as f32,
    ];
    for (dst, src) in uniform[1..].iter_mut().zip(job.params.iter().take(16)) {
        *dst = *src;
    }
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("perro_shader_bake_uniform"),
        contents: bytemuck::cast_slice(&uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let empty_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_shader_bake_empty_layout"),
        entries: &[],
    });
    let bake_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_shader_bake_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_shader_bake_bind_group"),
        layout: &bake_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 1,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("perro_shader_bake_pipeline_layout"),
        bind_group_layouts: &[
            Some(&empty_layout),
            Some(&empty_layout),
            Some(&empty_layout),
            Some(&bake_layout),
        ],
        immediate_size: 0,
    });

    let wgsl = compose_bake_wgsl(&job.shader_source);
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_shader_bake_shader"),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_shader_bake_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("perro_bake_vs"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("perro_bake_fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_shader_bake_target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let padded_row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_shader_bake_readback"),
        size: u64::from(padded_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perro_shader_bake_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_shader_bake_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
        pass.set_bind_group(3, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    if let Some(err) = scope.pop().await {
        return Err(StaticPipelineError::ShaderBake(format!(
            "shader `{}` bake validation: {err}",
            job.shader_uri
        )));
    }

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv()
        .map_err(|err| StaticPipelineError::ShaderBake(format!("bake readback: {err}")))?
        .map_err(|err| StaticPipelineError::ShaderBake(format!("bake map: {err}")))?;
    let mapped = slice
        .get_mapped_range()
        .map_err(|err| StaticPipelineError::ShaderBake(format!("bake mapped range: {err}")))?;
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for row in mapped
        .chunks_exact(padded_row as usize)
        .take(height as usize)
    {
        rgba.extend_from_slice(&row[..width as usize * 4]);
    }
    drop(mapped);
    staging.unmap();
    Ok(rgba)
}

#[cfg(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
fn compose_bake_wgsl(user: &str) -> String {
    format!(
        "{}\n{}\n{}\n{}",
        perro_wgsl::compose::prelude_rigid_wgsl(),
        r#"
struct PerroBakeUniform {
    resolution: vec4<f32>,
    params: array<vec4<f32>, 16>,
};
@group(3) @binding(1) var<uniform> perro_bake_uniform: PerroBakeUniform;

"#,
        user,
        r#"
struct PerroBakeVertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn perro_bake_vs(@builtin(vertex_index) index: u32) -> PerroBakeVertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: PerroBakeVertexOutput;
    out.clip_pos = vec4<f32>(positions[index], 0.0, 1.0);
    out.uv = positions[index] * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    return out;
}

@fragment
fn perro_bake_fs(in: PerroBakeVertexOutput) -> @location(0) vec4<f32> {
    let resolution = perro_bake_uniform.resolution.xy;
    return bake_texture(BakeInput(
        in.uv,
        in.uv * resolution,
        resolution,
        perro_bake_uniform.params,
    ));
}
"#
    )
}

#[cfg(all(
    test,
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
mod tests {
    use super::{ShaderBakeJob, bake_shader_texture};

    #[test]
    #[ignore = "requires GPU adapter"]
    fn bakes_wgsl_to_rgba_pixels() {
        let pixels = bake_shader_texture(&ShaderBakeJob {
            material_uri: "res://materials/background.pmat".to_string(),
            texture_uri: "res://baked/background.png".to_string(),
            shader_uri: "res://shaders/background.wgsl".to_string(),
            shader_source: r#"
fn bake_texture(in: BakeInput) -> vec4<f32> {
    return vec4<f32>(in.uv.x, in.uv.y, bake_param(in, 0u).x, 1.0);
}
"#
            .to_string(),
            material_source: "release_bake = true".to_string(),
            resolution: [2, 2],
            params: vec![[0.25, 0.0, 0.0, 0.0]],
            image_sources: Vec::new(),
        })
        .expect("shader bakes");

        assert_eq!(pixels.len(), 16);
        assert_eq!(pixels[3], 255);
        assert!(pixels[2] > 100 && pixels[2] < 160);
    }
}
