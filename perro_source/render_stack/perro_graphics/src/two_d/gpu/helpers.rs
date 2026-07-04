use super::*;

/// world-space aabb 4 staged sprite; camera-free -> compute 1x / revision.
/// NaN out -> always cull.
#[inline]
pub(super) fn sprite_world_bounds(sprite: &Sprite2DCommand, size: [f32; 2]) -> [f32; 4] {
    let hx = 0.5 * size[0].max(1.0);
    let hy = 0.5 * size[1].max(1.0);
    let corners = [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)];

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for (lx, ly) in corners {
        let wx = sprite.model[0][0] * lx + sprite.model[1][0] * ly + sprite.model[2][0];
        let wy = sprite.model[0][1] * lx + sprite.model[1][1] * ly + sprite.model[2][1];
        if !(wx.is_finite() && wy.is_finite()) {
            return [f32::NAN; 4];
        }
        min_x = min_x.min(wx);
        max_x = max_x.max(wx);
        min_y = min_y.min(wy);
        max_y = max_y.max(wy);
    }

    [min_x, min_y, max_x, max_y]
}

/// conservative aabb-vs-screen test in ndc space
#[inline]
pub(super) fn sprite_bounds_intersect_screen(bounds: &[f32; 4], camera: &Camera2DUniform) -> bool {
    if !bounds.iter().all(|v| v.is_finite()) {
        return false;
    }
    let corners = [
        (bounds[0], bounds[1]),
        (bounds[2], bounds[1]),
        (bounds[2], bounds[3]),
        (bounds[0], bounds[3]),
    ];

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for (wx, wy) in corners {
        let vx = camera.view[0][0] * wx + camera.view[1][0] * wy + camera.view[3][0];
        let vy = camera.view[0][1] * wx + camera.view[1][1] * wy + camera.view[3][1];
        let ndc_x = vx * camera.ndc_scale[0];
        let ndc_y = vy * camera.ndc_scale[1];
        if !(ndc_x.is_finite() && ndc_y.is_finite()) {
            return false;
        }
        min_x = min_x.min(ndc_x);
        max_x = max_x.max(ndc_x);
        min_y = min_y.min(ndc_y);
        max_y = max_y.max(ndc_y);
    }

    !(max_x < -1.0 || min_x > 1.0 || max_y < -1.0 || min_y > 1.0)
}

pub(super) fn light_2d_gpu(light: Light2DState) -> Option<Light2DGpu> {
    let (position, range, z_index, color, intensity, direction, inner_cos, outer_cos, kind) =
        match light {
            Light2DState::Ambient(light) => (
                [0.0, 0.0],
                1.0,
                i32::MAX,
                light.color,
                light.intensity,
                [0.0, -1.0],
                1.0,
                -1.0,
                0,
            ),
            Light2DState::Ray(light) => (
                [0.0, 0.0],
                1.0,
                light.z_index,
                light.color,
                light.intensity,
                normalize2(light.direction).unwrap_or([0.0, -1.0]),
                1.0,
                -1.0,
                1,
            ),
            Light2DState::Point(light) => (
                light.position,
                light.range,
                light.z_index,
                light.color,
                light.intensity,
                [0.0, -1.0],
                1.0,
                -1.0,
                2,
            ),
            Light2DState::Spot(light) => (
                light.position,
                light.range,
                light.z_index,
                light.color,
                light.intensity,
                normalize2(light.direction).unwrap_or([0.0, -1.0]),
                light.inner_angle_radians.cos(),
                light.outer_angle_radians.cos(),
                3,
            ),
        };
    if !(range.is_finite()
        && range > 0.0
        && intensity.is_finite()
        && intensity > 0.0
        && position.iter().all(|v| v.is_finite())
        && color.iter().all(|v| v.is_finite())
        && direction.iter().all(|v| v.is_finite())
        && inner_cos.is_finite()
        && outer_cos.is_finite())
    {
        return None;
    }
    Some(Light2DGpu {
        position,
        range,
        z_index,
        color,
        intensity,
        direction,
        inner_cos,
        outer_cos,
        kind,
        pad: [0; 3],
    })
}

pub(super) fn normalize2(v: [f32; 2]) -> Option<[f32; 2]> {
    let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
    (len.is_finite() && len > 0.0).then_some([v[0] / len, v[1] / len])
}

pub(super) fn create_rect_pipeline(
    device: &wgpu::Device,
    camera_bgl: &wgpu::BindGroupLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("perro_rect_pipeline_layout"),
        bind_group_layouts: &[Some(camera_bgl)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_rect_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<QuadVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x2,
                    }],
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectInstanceGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Unorm8x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Sint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 28,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Float32,
                        },
                    ],
                }),
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::RED
                    | wgpu::ColorWrites::GREEN
                    | wgpu::ColorWrites::BLUE,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: sample_count.max(1),
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

pub(super) fn create_sprite_pipeline(
    device: &wgpu::Device,
    camera_bgl: &wgpu::BindGroupLayout,
    texture_bgl: &wgpu::BindGroupLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("perro_sprite_pipeline_layout"),
        bind_group_layouts: &[Some(camera_bgl), Some(texture_bgl)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_sprite_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SpriteVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SpriteInstanceGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 40,
                            shader_location: 7,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 8,
                            format: wgpu::VertexFormat::Sint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 52,
                            shader_location: 9,
                            format: wgpu::VertexFormat::Unorm8x4,
                        },
                    ],
                }),
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::RED
                    | wgpu::ColorWrites::GREEN
                    | wgpu::ColorWrites::BLUE,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: sample_count.max(1),
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

pub(super) fn create_point_light_pipeline(
    device: &wgpu::Device,
    camera_bgl: &wgpu::BindGroupLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("perro_point_light_2d_pipeline_layout"),
        bind_group_layouts: &[Some(camera_bgl)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_point_light_2d_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<QuadVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x2,
                    }],
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Light2DGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Sint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 28,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 40,
                            shader_location: 7,
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 44,
                            shader_location: 8,
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 9,
                            format: wgpu::VertexFormat::Uint32,
                        },
                    ],
                }),
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::RED
                    | wgpu::ColorWrites::GREEN
                    | wgpu::ColorWrites::BLUE,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: sample_count.max(1),
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

#[inline]
pub(super) fn color_to_unorm8(color: [f32; 4]) -> [u8; 4] {
    perro_structs::UnitVector4::new(color).to_u8()
}
