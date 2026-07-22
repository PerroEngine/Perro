use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

const SSAO_WGSL: &str = perro_macros::include_str_stripped!("src/three_d/shaders/ssao.wgsl");
const SSAO_BLUR_WGSL: &str =
    perro_macros::include_str_stripped!("src/three_d/shaders/ssao_bilateral_blur.wgsl");

pub(super) const SSAO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SsaoTargetSettings {
    target_divisor: u32,
    blur: bool,
}

impl SsaoTargetSettings {
    const fn for_quality(quality: crate::SsaoQuality) -> Option<Self> {
        match quality {
            crate::SsaoQuality::Off => None,
            crate::SsaoQuality::Low => Some(Self {
                target_divisor: 2,
                blur: false,
            }),
            crate::SsaoQuality::Medium | crate::SsaoQuality::High => Some(Self {
                target_divisor: 2,
                blur: true,
            }),
            crate::SsaoQuality::Ultra => Some(Self {
                target_divisor: 1,
                blur: true,
            }),
        }
    }
}

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
    sample_bind_group_layout: wgpu::BindGroupLayout,
    blur_bind_group_layout: Option<wgpu::BindGroupLayout>,
    sample_bind_group: wgpu::BindGroup,
    blur_bind_group: Option<wgpu::BindGroup>,
    sample_pipeline: wgpu::RenderPipeline,
    blur_pipeline: Option<wgpu::RenderPipeline>,
    settings: SsaoTargetSettings,
}

impl SsaoPass {
    pub(super) fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        depth_view: &wgpu::TextureView,
        quality: crate::SsaoQuality,
    ) -> Self {
        let settings = SsaoTargetSettings::for_quality(quality)
            .expect("SSAO pass must not be built for off quality");
        let sample_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_ssao_sample_bgl"),
                entries: &[
                    texture_entry(0, wgpu::TextureSampleType::Depth),
                    uniform_entry(1),
                ],
            });
        let blur_bind_group_layout = settings.blur.then(|| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_ssao_blur_bgl"),
                entries: &[
                    texture_entry(0, wgpu::TextureSampleType::Float { filterable: false }),
                    texture_entry(1, wgpu::TextureSampleType::Depth),
                    uniform_entry(2),
                ],
            })
        });
        let sample_pipeline =
            create_pipeline(device, "perro_ssao", SSAO_WGSL, &sample_bind_group_layout);
        let blur_pipeline = blur_bind_group_layout
            .as_ref()
            .map(|layout| create_pipeline(device, "perro_ssao_blur", SSAO_BLUR_WGSL, layout));
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_ssao_uniform"),
            contents: bytemuck::bytes_of(&SsaoUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let (raw_texture, raw_view) = create_target(
            device,
            width,
            height,
            settings.target_divisor,
            "perro_ssao_raw",
        );
        let final_target = settings.blur.then(|| {
            create_target(
                device,
                width,
                height,
                settings.target_divisor,
                "perro_ssao_final",
            )
        });
        let (final_texture, final_view) = match final_target {
            Some((texture, view)) => (Some(texture), Some(view)),
            None => (None, None),
        };
        let sample_bind_group =
            sample_bind_group(device, &sample_bind_group_layout, depth_view, &uniform);
        let blur_bind_group = blur_bind_group_layout
            .as_ref()
            .map(|layout| blur_bind_group(device, layout, &raw_view, depth_view, &uniform));
        Self {
            _raw_texture: raw_texture,
            raw_view,
            _final_texture: final_texture,
            final_view,
            uniform,
            sample_bind_group_layout,
            blur_bind_group_layout,
            sample_bind_group,
            blur_bind_group,
            sample_pipeline,
            blur_pipeline,
            settings,
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
        let settings = SsaoTargetSettings::for_quality(quality)
            .expect("SSAO pass must not resize for off quality");
        debug_assert_eq!(self.settings, settings);
        (self._raw_texture, self.raw_view) = create_target(
            device,
            width,
            height,
            self.settings.target_divisor,
            "perro_ssao_raw",
        );
        let final_target = self.settings.blur.then(|| {
            create_target(
                device,
                width,
                height,
                self.settings.target_divisor,
                "perro_ssao_final",
            )
        });
        (self._final_texture, self.final_view) = match final_target {
            Some((texture, view)) => (Some(texture), Some(view)),
            None => (None, None),
        };
        self.sample_bind_group = sample_bind_group(
            device,
            &self.sample_bind_group_layout,
            depth_view,
            &self.uniform,
        );
        self.blur_bind_group = self.blur_bind_group_layout.as_ref().map(|layout| {
            blur_bind_group(device, layout, &self.raw_view, depth_view, &self.uniform)
        });
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
        self.settings.target_divisor
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
    let (width, height) = target_extent(width, height, divisor);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
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

fn target_extent(width: u32, height: u32, divisor: u32) -> (u32, u32) {
    let divisor = divisor.max(1);
    (
        width.max(1).div_ceil(divisor),
        height.max(1).div_ceil(divisor),
    )
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
    fn quality_targets_scale_cost_and_off_skips_pass() {
        assert!(SsaoTargetSettings::for_quality(crate::SsaoQuality::Off).is_none());
        let low = SsaoTargetSettings::for_quality(crate::SsaoQuality::Low)
            .expect("required value must be present");
        let medium = SsaoTargetSettings::for_quality(crate::SsaoQuality::Medium)
            .expect("required value must be present");
        let high = SsaoTargetSettings::for_quality(crate::SsaoQuality::High)
            .expect("required value must be present");
        let ultra = SsaoTargetSettings::for_quality(crate::SsaoQuality::Ultra)
            .expect("required value must be present");
        assert_eq!(
            low,
            SsaoTargetSettings {
                target_divisor: 2,
                blur: false
            }
        );
        assert_eq!(
            medium,
            SsaoTargetSettings {
                target_divisor: 2,
                blur: true
            }
        );
        assert_eq!(high, medium);
        assert_eq!(
            ultra,
            SsaoTargetSettings {
                target_divisor: 1,
                blur: true
            }
        );
    }

    #[test]
    fn target_extent_handles_odd_and_zero_resize() {
        assert_eq!(target_extent(1920, 1080, 2), (960, 540));
        assert_eq!(target_extent(1919, 1079, 2), (960, 540));
        assert_eq!(target_extent(0, 0, 2), (1, 1));
        assert_eq!(target_extent(0, 0, 0), (1, 1));
    }

    #[test]
    fn ssao_surface_scope_stays_ambient_only() {
        let standard = include_str!("../shaders/prelude_3d.wgsl");
        let multimesh = include_str!("../shaders/multimesh.wgsl");
        let water = include_str!("../../water_shaders/water_3d_render.wgsl");
        assert!(standard.matches("screen_space_ambient_occlusion(").count() >= 2);
        assert!(multimesh.matches("multimesh_ssao(").count() >= 3);
        assert!(!water.contains("ssao"));
    }

    #[test]
    fn ssao_wgsl_parse_and_validate() {
        for (label, source) in [("ssao", SSAO_WGSL), ("ssao blur", SSAO_BLUR_WGSL)] {
            let module =
                naga::front::wgsl::parse_str(source).unwrap_or_else(|err| panic!("{label}: {err}"));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap_or_else(|err| panic!("{label}: {err}"));
        }
    }
}
