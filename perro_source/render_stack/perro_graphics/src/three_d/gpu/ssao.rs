use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

const SSAO_WGSL: &str = perro_macros::include_str_stripped!("src/three_d/shaders/ssao.wgsl");
const SSAO_BLUR_WGSL: &str =
    perro_macros::include_str_stripped!("src/three_d/shaders/ssao_bilateral_blur.wgsl");

pub(super) const SSAO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct SsaoUniform {
    pub inv_view_proj: [[f32; 4]; 4],
    pub full_size: [f32; 2],
    pub radius_px: f32,
    pub strength: f32,
    pub depth_sigma: f32,
    pub sample_count: u32,
    pub target_divisor: u32,
    pub _pad: f32,
}

pub(super) struct SsaoPass {
    _raw_texture: wgpu::Texture,
    raw_view: wgpu::TextureView,
    _final_texture: Option<wgpu::Texture>,
    final_view: Option<wgpu::TextureView>,
    uniform: wgpu::Buffer,
    sample_bind_group: wgpu::BindGroup,
    blur_bind_group: Option<wgpu::BindGroup>,
    sample_pipeline: wgpu::RenderPipeline,
    blur_pipeline: Option<wgpu::RenderPipeline>,
    target_divisor: u32,
}

impl SsaoPass {
    pub(super) fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        depth_view: &wgpu::TextureView,
        quality: crate::SsaoQuality,
    ) -> Self {
        let (target_divisor, blur_enabled) = match quality {
            crate::SsaoQuality::Low => (2, false),
            crate::SsaoQuality::Medium | crate::SsaoQuality::High => (2, true),
            crate::SsaoQuality::Ultra => (1, true),
            crate::SsaoQuality::Off => (2, false),
        };
        let sample_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_ssao_sample_bgl"),
            entries: &[
                texture_entry(0, wgpu::TextureSampleType::Depth),
                uniform_entry(1),
            ],
        });
        let blur_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_ssao_blur_bgl"),
            entries: &[
                texture_entry(0, wgpu::TextureSampleType::Float { filterable: false }),
                texture_entry(1, wgpu::TextureSampleType::Depth),
                uniform_entry(2),
            ],
        });
        let sample_pipeline = create_pipeline(device, "perro_ssao", SSAO_WGSL, &sample_bgl);
        let blur_pipeline = blur_enabled
            .then(|| create_pipeline(device, "perro_ssao_blur", SSAO_BLUR_WGSL, &blur_bgl));
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_ssao_uniform"),
            contents: bytemuck::bytes_of(&SsaoUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let (raw_texture, raw_view) =
            create_target(device, width, height, target_divisor, "perro_ssao_raw");
        let final_target = blur_enabled
            .then(|| create_target(device, width, height, target_divisor, "perro_ssao_final"));
        let (final_texture, final_view) = match final_target {
            Some((texture, view)) => (Some(texture), Some(view)),
            None => (None, None),
        };
        let sample_bind_group = sample_bind_group(device, &sample_bgl, depth_view, &uniform);
        let blur_bind_group = blur_enabled
            .then(|| blur_bind_group(device, &blur_bgl, &raw_view, depth_view, &uniform));
        Self {
            _raw_texture: raw_texture,
            raw_view,
            _final_texture: final_texture,
            final_view,
            uniform,
            sample_bind_group,
            blur_bind_group,
            sample_pipeline,
            blur_pipeline,
            target_divisor,
        }
    }

    pub(super) fn resize(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        depth_view: &wgpu::TextureView,
        quality: crate::SsaoQuality,
    ) {
        let rebuilt = Self::new(device, width, height, depth_view, quality);
        *self = rebuilt;
    }

    pub(super) fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        uniform: SsaoUniform,
    ) {
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        draw_fullscreen(
            encoder,
            "perro_ssao_sample",
            &self.raw_view,
            &self.sample_pipeline,
            &self.sample_bind_group,
        );
        if let (Some(final_view), Some(pipeline), Some(bind_group)) = (
            self.final_view.as_ref(),
            self.blur_pipeline.as_ref(),
            self.blur_bind_group.as_ref(),
        ) {
            draw_fullscreen(encoder, "perro_ssao_blur", final_view, pipeline, bind_group);
        }
    }

    pub(super) fn view(&self) -> &wgpu::TextureView {
        self.final_view.as_ref().unwrap_or(&self.raw_view)
    }

    pub(super) const fn target_divisor(&self) -> u32 {
        self.target_divisor
    }
}

fn texture_entry(binding: u32, sample_type: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn create_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    divisor: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1).div_ceil(divisor.max(1)),
            height: height.max(1).div_ceil(divisor.max(1)),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SSAO_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn sample_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    depth: &wgpu::TextureView,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_ssao_sample_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(depth),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

fn blur_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    raw: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_ssao_blur_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(raw),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(depth),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

fn create_pipeline(
    device: &wgpu::Device,
    label: &str,
    source: &str,
    bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: SSAO_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn draw_fullscreen(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.draw(0..3, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssao_wgsl_parse_and_validate() {
        for (label, source) in [("ssao", SSAO_WGSL), ("ssao blur", SSAO_BLUR_WGSL)] {
            let module =
                naga::front::wgsl::parse_str(source).unwrap_or_else(|err| panic!("{label}: {err}"));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|err| panic!("{label}: {err}"));
        }
    }
}
