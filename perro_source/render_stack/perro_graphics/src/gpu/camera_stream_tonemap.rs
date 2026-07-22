use std::borrow::Cow;

pub(super) struct CameraStreamTonemapSettings {
    pub(super) hdr_status: perro_structs::HdrStatus,
    pub(super) exposure: f32,
}

const CAMERA_STREAM_TONEMAP_WGSL: &str = r#"
@group(0) @binding(0) var input_tex: texture_2d<f32>;
struct ToneMapConfig { value: vec4<f32> };
@group(0) @binding(1) var<uniform> config: ToneMapConfig;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return out;
}

fn aces_filmic(x: f32) -> f32 {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.pos.xy);
    let sample = textureLoad(input_tex, pixel, 0);
    let alpha = clamp(sample.a, 0.0, 1.0);
    if alpha <= 0.000001 {
        return vec4<f32>(0.0);
    }
    // Stream targets use premultiplied alpha. Map peak luminance once and
    // scale all channels together so bright saturated UI models keep hue.
    let straight = max(sample.rgb / alpha, vec3<f32>(0.0)) * exp2(config.value.x);
    let peak = max(straight.r, max(straight.g, straight.b));
    if peak <= 0.000001 {
        return vec4<f32>(0.0, 0.0, 0.0, alpha);
    }
    let headroom = select(1.0, max(config.value.z, 1.0), config.value.y > 0.5);
    let mapped_peak = aces_filmic(peak / headroom) * headroom;
    let mapped = straight * (mapped_peak / peak);
    return vec4<f32>(mapped * alpha, alpha);
}
"#;

pub(super) struct CameraStreamTonemap {
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    config_buffer: wgpu::Buffer,
}

impl CameraStreamTonemap {
    pub(super) fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_camera_stream_tonemap_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let config_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_camera_stream_tonemap_config"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_camera_stream_tonemap_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_camera_stream_tonemap_shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(CAMERA_STREAM_TONEMAP_WGSL)),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_camera_stream_tonemap_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
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
        Self {
            bind_group_layout,
            pipeline,
            config_buffer,
        }
    }

    pub(super) fn apply(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        input_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        settings: CameraStreamTonemapSettings,
    ) {
        let CameraStreamTonemapSettings {
            hdr_status,
            exposure,
        } = settings;
        let config = [
            if exposure.is_finite() { exposure } else { 0.0 },
            if hdr_status.active { 1.0 } else { 0.0 },
            if hdr_status.headroom.is_finite() {
                hdr_status.headroom.max(1.0)
            } else {
                1.0
            },
            0.0,
        ];
        queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&config));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_camera_stream_tonemap_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.config_buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_camera_stream_tonemap_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_validates() {
        let module = naga::front::wgsl::parse_str(CAMERA_STREAM_TONEMAP_WGSL)
            .expect("required value must be present");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("required value must be present");
    }

    #[test]
    fn tone_map_scales_channels_together_to_keep_hue() {
        assert!(CAMERA_STREAM_TONEMAP_WGSL.contains("mapped_peak / peak"));
        assert!(!CAMERA_STREAM_TONEMAP_WGSL.contains("aces_filmic(straight)"));
    }

    #[test]
    fn hdr_path_keeps_display_headroom() {
        assert!(CAMERA_STREAM_TONEMAP_WGSL.contains("aces_filmic(peak / headroom) * headroom"));
    }
}
