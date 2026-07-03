use super::*;

impl GpuPointParticles3D {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Self {
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_particles3d_camera_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<CameraUniform>() as u64)
                            .expect("camera uniform size must be non-zero"),
                    ),
                },
                count: None,
            }],
        });
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_camera_buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_particles3d_camera_bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let shader = create_point_particles_shader_module(device);
        let cpu_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_particles3d_pipeline_layout"),
            bind_group_layouts: &[Some(&camera_bgl)],
            immediate_size: 0,
        });
        let cpu_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_particles3d_pipeline"),
            layout: Some(&cpu_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<PointParticleGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Unorm8x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float16x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float16x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::RED
                        | wgpu::ColorWrites::GREEN
                        | wgpu::ColorWrites::BLUE,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::PointList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::scene_depth_format(sample_count),
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });
        let cpu_billboard_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("perro_particles3d_billboard_pipeline"),
                layout: Some(&cpu_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_billboard"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<PointParticleGpu>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: 12,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Unorm8x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float16x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 20,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float16x4,
                            },
                        ],
                    }],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::RED
                            | wgpu::ColorWrites::GREEN
                            | wgpu::ColorWrites::BLUE,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::scene_depth_format(sample_count),
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: sample_count,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });
        let hybrid_emitters_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_particles3d_hybrid_emitters_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: Some(
                                std::num::NonZeroU64::new(
                                    std::mem::size_of::<GpuEmitterParams>() as u64
                                )
                                .expect("gpu emitter params size must be non-zero"),
                            ),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let hybrid_shader = create_point_particles_gpu_shader_module(device);
        let hybrid_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_particles3d_hybrid_pipeline_layout"),
            bind_group_layouts: &[Some(&camera_bgl), Some(&hybrid_emitters_bgl)],
            immediate_size: 0,
        });
        let hybrid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_particles3d_hybrid_pipeline"),
            layout: Some(&hybrid_layout),
            vertex: wgpu::VertexState {
                module: &hybrid_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &hybrid_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::RED
                        | wgpu::ColorWrites::GREEN
                        | wgpu::ColorWrites::BLUE,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::PointList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::scene_depth_format(sample_count),
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });
        let hybrid_billboard_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("perro_particles3d_hybrid_billboard_pipeline"),
                layout: Some(&hybrid_layout),
                vertex: wgpu::VertexState {
                    module: &hybrid_shader,
                    entry_point: Some("vs_billboard"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &hybrid_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::RED
                            | wgpu::ColorWrites::GREEN
                            | wgpu::ColorWrites::BLUE,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::scene_depth_format(sample_count),
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: sample_count,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });

        let compute_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_particles3d_compute_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(
                                std::mem::size_of::<GpuEmitterParams>() as u64
                            )
                            .expect("gpu emitter params size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let compute_render_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_particles3d_compute_render_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let compute_shader = create_point_particles_compute_shader_module(device);
        let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_particles3d_compute_layout"),
            bind_group_layouts: &[Some(&camera_bgl), Some(&compute_bgl)],
            immediate_size: 0,
        });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("perro_particles3d_compute_pipeline"),
            layout: Some(&compute_layout),
            module: &compute_shader,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let compute_render_shader = create_point_particles_compute_render_shader_module(device);
        let compute_render_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_particles3d_compute_render_layout"),
                bind_group_layouts: &[Some(&camera_bgl), Some(&compute_render_bgl)],
                immediate_size: 0,
            });
        let compute_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("perro_particles3d_compute_render_pipeline"),
                layout: Some(&compute_render_layout),
                vertex: wgpu::VertexState {
                    module: &compute_render_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &compute_render_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::RED
                            | wgpu::ColorWrites::GREEN
                            | wgpu::ColorWrites::BLUE,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::PointList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::scene_depth_format(sample_count),
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: sample_count,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });
        let compute_render_billboard_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("perro_particles3d_compute_render_billboard_pipeline"),
                layout: Some(&compute_render_layout),
                vertex: wgpu::VertexState {
                    module: &compute_render_shader,
                    entry_point: Some("vs_billboard"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &compute_render_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::RED
                            | wgpu::ColorWrites::GREEN
                            | wgpu::ColorWrites::BLUE,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::scene_depth_format(sample_count),
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: sample_count,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });
        let particle_capacity = 1024usize;
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_points"),
            size: (particle_capacity * std::mem::size_of::<PointParticleGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let billboard_particle_capacity = 1024usize;
        let billboard_particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_billboards"),
            size: (billboard_particle_capacity * std::mem::size_of::<PointParticleGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hybrid_emitter_capacity = 64usize;
        let hybrid_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_hybrid_emitters"),
            size: (hybrid_emitter_capacity * std::mem::size_of::<GpuEmitterParticle>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hybrid_particle_emitter_capacity = 1024usize;
        let hybrid_particle_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_hybrid_particle_emitters"),
            size: (hybrid_particle_emitter_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hybrid_particle_spawn_origin_capacity = 1024usize;
        let hybrid_particle_spawn_origin_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_hybrid_particle_spawn_origins"),
            size: (hybrid_particle_spawn_origin_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hybrid_particle_spawn_rotation_capacity = 1024usize;
        let hybrid_particle_spawn_rotation_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_hybrid_particle_spawn_rotations"),
            size: (hybrid_particle_spawn_rotation_capacity * std::mem::size_of::<[f32; 4]>())
                as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hybrid_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_hybrid_params"),
            size: std::mem::size_of::<GpuEmitterParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hybrid_params_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_particles3d_hybrid_emitters_bg"),
            layout: &hybrid_emitters_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: hybrid_emitter_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: hybrid_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: hybrid_particle_emitter_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: hybrid_particle_spawn_origin_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: hybrid_particle_spawn_rotation_buffer.as_entire_binding(),
                },
            ],
        });
        let compute_emitter_capacity = 64usize;
        let compute_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_compute_emitters"),
            size: (compute_emitter_capacity * std::mem::size_of::<GpuEmitterParticle>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_particle_emitter_capacity = 1024usize;
        let compute_particle_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_compute_particle_emitters"),
            size: (compute_particle_emitter_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_particle_spawn_origin_capacity = 1024usize;
        let compute_particle_spawn_origin_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_compute_particle_spawn_origins"),
            size: (compute_particle_spawn_origin_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_particle_spawn_rotation_capacity = 1024usize;
        let compute_particle_spawn_rotation_buffer =
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_particle_spawn_rotations"),
                size: (compute_particle_spawn_rotation_capacity * std::mem::size_of::<[f32; 4]>())
                    as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        let compute_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_compute_params"),
            size: std::mem::size_of::<GpuEmitterParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_particle_capacity = 1024usize;
        let compute_particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_compute_particles"),
            size: (compute_particle_capacity * std::mem::size_of::<GpuComputedParticle>()) as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let compute_expr_op_capacity = 1024usize;
        let compute_expr_op_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_compute_expr_ops"),
            size: (compute_expr_op_capacity * std::mem::size_of::<GpuExprOp>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_custom_param_capacity = 1024usize;
        let compute_custom_param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_compute_custom_params"),
            size: (compute_custom_param_capacity * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_particles3d_compute_bg"),
            layout: &compute_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: compute_emitter_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: compute_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: compute_particle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: compute_expr_op_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: compute_custom_param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: compute_particle_emitter_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: compute_particle_spawn_origin_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: compute_particle_spawn_rotation_buffer.as_entire_binding(),
                },
            ],
        });
        let compute_render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_particles3d_compute_render_bg"),
            layout: &compute_render_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 8,
                resource: compute_particle_buffer.as_entire_binding(),
            }],
        });
        Self {
            cpu_pipeline,
            cpu_billboard_pipeline,
            hybrid_pipeline,
            hybrid_billboard_pipeline,
            compute_pipeline,
            compute_render_pipeline,
            compute_render_billboard_pipeline,
            camera_buffer,
            camera_bg,
            hybrid_emitters_bgl,
            hybrid_params_buffer,
            hybrid_params_bg,
            compute_bgl,
            compute_bg,
            compute_render_bgl,
            compute_render_bg,
            particle_buffer,
            particle_capacity,
            billboard_particle_buffer,
            billboard_particle_capacity,
            staged: Vec::new(),
            staged_billboards: Vec::new(),
            hybrid_emitters: Vec::new(),
            hybrid_emitter_buffer,
            hybrid_emitter_capacity,
            hybrid_particle_emitter_map: Vec::new(),
            hybrid_particle_emitter_buffer,
            hybrid_particle_emitter_capacity,
            hybrid_particle_spawn_origins: Vec::new(),
            hybrid_particle_spawn_origin_buffer,
            hybrid_particle_spawn_origin_capacity,
            hybrid_particle_spawn_rotations: Vec::new(),
            hybrid_particle_spawn_rotation_buffer,
            hybrid_particle_spawn_rotation_capacity,
            hybrid_particle_count: 0,
            hybrid_has_point: false,
            hybrid_has_billboard: false,
            hybrid_point_ranges: Vec::new(),
            hybrid_billboard_ranges: Vec::new(),
            compute_emitters: Vec::new(),
            compute_emitter_buffer,
            compute_emitter_capacity,
            compute_particle_emitter_map: Vec::new(),
            compute_particle_emitter_buffer,
            compute_particle_emitter_capacity,
            compute_particle_spawn_origins: Vec::new(),
            compute_particle_spawn_origin_buffer,
            compute_particle_spawn_origin_capacity,
            compute_particle_spawn_rotations: Vec::new(),
            compute_particle_spawn_rotation_buffer,
            compute_particle_spawn_rotation_capacity,
            compute_params_buffer,
            compute_particle_buffer,
            compute_particle_capacity,
            compute_particle_count: 0,
            compute_has_point: false,
            compute_has_billboard: false,
            compute_point_ranges: Vec::new(),
            compute_billboard_ranges: Vec::new(),
            compute_expr_ops: Vec::new(),
            compute_expr_op_buffer,
            compute_expr_op_capacity,
            compute_custom_params: Vec::new(),
            compute_custom_param_buffer,
            compute_custom_param_capacity,
            compiled_exprs: Vec::new(),
            compiled_expr_lookup: AHashMap::new(),
            eval_stack: Vec::new(),
            emitter_order: Vec::new(),
            hybrid_spawn_rings: AHashMap::new(),
            hybrid_spawn_origin_dirty_slots: Vec::new(),
            hybrid_spawn_rotation_dirty_slots: Vec::new(),
            compute_spawn_rings: AHashMap::new(),
            compute_spawn_origin_dirty_slots: Vec::new(),
            compute_spawn_rotation_dirty_slots: Vec::new(),
            spawn_origin_cache: AHashMap::new(),
            spawn_origin_generation: 0,
            hybrid_map_fingerprint: 0,
            hybrid_map_uploaded_fingerprint: 0,
            hybrid_map_uploaded_count: 0,
            compute_map_fingerprint: 0,
            compute_map_uploaded_fingerprint: 0,
            compute_map_uploaded_count: 0,
        }
    }
}
