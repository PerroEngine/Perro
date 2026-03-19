use super::shaders::{
    create_point_particles_compute_shader_module, create_point_particles_gpu_shader_module,
    create_point_particles_shader_module,
};
use ahash::AHashMap;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use perro_ids::NodeID;
use perro_particle_math::{Op, ParticleEvalInput, Program, compile_expression, eval_ops_particle};
use perro_render_bridge::{
    Camera3DState, CameraProjectionState, ParticlePath3D, ParticleRenderMode3D,
    ParticleSimulationMode3D, PointParticles3DState,
};

const PARTICLE_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    inv_view_size: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PointParticleGpu {
    world_pos: [f32; 3],
    size_alpha: [f32; 2],
    color: [f32; 4],
    emissive: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuEmitterParticle {
    model_0: [f32; 4],
    model_1: [f32; 4],
    model_2: [f32; 4],
    model_3: [f32; 4],
    gravity_path: [f32; 4], // xyz gravity, w path kind
    color_start: [f32; 4],
    color_end: [f32; 4],
    emissive_point: [f32; 4],   // xyz emissive, w size
    life_speed: [f32; 4],       // life_min, life_max, speed_min, speed_max
    size_spread_rate: [f32; 4], // size_min, size_max, spread_radians, emission_rate
    time_path: [f32; 4],        // simulation_time, path_a, path_b, simulation_delta
    counts_seed: [u32; 4],      // start, count, max_alive_budget, seed
    flags: [u32; 4],            // looping, prewarm, spin_bits, spawn_origin_base
    custom_ops_xy: [u32; 4],    // x_off, x_len, y_off, y_len
    custom_ops_zp: [u32; 4],    // z_off, z_len, params_off, params_len
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuEmitterParams {
    emitter_count: u32,
    particle_count: u32,
    _pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuComputedParticle {
    world_pos: [f32; 4],
    color: [f32; 4],
    emissive: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuExprOp {
    words: [u32; 4], // opcode, arg_bits, reserved, reserved
}

#[derive(Clone, Copy)]
struct InstanceRange {
    start: u32,
    count: u32,
    path_kind: u32,
}

#[derive(Clone, Copy)]
struct SpawnOriginEntry {
    origin: [f32; 3],
    rotation: [f32; 4],
    last_seen_generation: u64,
}

struct SpawnRingState {
    base: u32,
    capacity: u32,
    slot_spawn_keys: Vec<u32>,
}

pub struct PreparePointParticles3D<'a> {
    pub camera: Camera3DState,
    pub emitters: &'a [(NodeID, PointParticles3DState)],
    pub width: u32,
    pub height: u32,
}

pub struct GpuPointParticles3D {
    cpu_pipeline: wgpu::RenderPipeline,
    cpu_billboard_pipeline: wgpu::RenderPipeline,
    hybrid_pipeline: wgpu::RenderPipeline,
    hybrid_billboard_pipeline: wgpu::RenderPipeline,
    compute_pipeline: wgpu::ComputePipeline,
    compute_render_pipeline: wgpu::RenderPipeline,
    compute_render_billboard_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bg: wgpu::BindGroup,
    hybrid_emitters_bgl: wgpu::BindGroupLayout,
    hybrid_params_buffer: wgpu::Buffer,
    hybrid_params_bg: wgpu::BindGroup,
    compute_bgl: wgpu::BindGroupLayout,
    compute_bg: wgpu::BindGroup,
    compute_render_bgl: wgpu::BindGroupLayout,
    compute_render_bg: wgpu::BindGroup,
    particle_buffer: wgpu::Buffer,
    particle_capacity: usize,
    billboard_particle_buffer: wgpu::Buffer,
    billboard_particle_capacity: usize,
    staged: Vec<PointParticleGpu>,
    staged_billboards: Vec<PointParticleGpu>,
    hybrid_emitters: Vec<GpuEmitterParticle>,
    hybrid_emitter_buffer: wgpu::Buffer,
    hybrid_emitter_capacity: usize,
    hybrid_particle_emitter_map: Vec<u32>,
    hybrid_particle_emitter_buffer: wgpu::Buffer,
    hybrid_particle_emitter_capacity: usize,
    hybrid_particle_spawn_origins: Vec<[f32; 4]>,
    hybrid_particle_spawn_origin_buffer: wgpu::Buffer,
    hybrid_particle_spawn_origin_capacity: usize,
    hybrid_particle_spawn_rotations: Vec<[f32; 4]>,
    hybrid_particle_spawn_rotation_buffer: wgpu::Buffer,
    hybrid_particle_spawn_rotation_capacity: usize,
    hybrid_particle_count: u32,
    hybrid_has_point: bool,
    hybrid_has_billboard: bool,
    hybrid_point_ranges: Vec<InstanceRange>,
    hybrid_billboard_ranges: Vec<InstanceRange>,
    compute_emitters: Vec<GpuEmitterParticle>,
    compute_emitter_buffer: wgpu::Buffer,
    compute_emitter_capacity: usize,
    compute_particle_emitter_map: Vec<u32>,
    compute_particle_emitter_buffer: wgpu::Buffer,
    compute_particle_emitter_capacity: usize,
    compute_particle_spawn_origins: Vec<[f32; 4]>,
    compute_particle_spawn_origin_buffer: wgpu::Buffer,
    compute_particle_spawn_origin_capacity: usize,
    compute_particle_spawn_rotations: Vec<[f32; 4]>,
    compute_particle_spawn_rotation_buffer: wgpu::Buffer,
    compute_particle_spawn_rotation_capacity: usize,
    compute_params_buffer: wgpu::Buffer,
    compute_particle_buffer: wgpu::Buffer,
    compute_particle_capacity: usize,
    compute_particle_count: u32,
    compute_has_point: bool,
    compute_has_billboard: bool,
    compute_point_ranges: Vec<InstanceRange>,
    compute_billboard_ranges: Vec<InstanceRange>,
    compute_expr_ops: Vec<GpuExprOp>,
    compute_expr_op_buffer: wgpu::Buffer,
    compute_expr_op_capacity: usize,
    compute_custom_params: Vec<f32>,
    compute_custom_param_buffer: wgpu::Buffer,
    compute_custom_param_capacity: usize,
    compiled_exprs: Vec<Program>,
    compiled_expr_lookup: AHashMap<String, usize>,
    eval_stack: Vec<f32>,
    hybrid_spawn_rings: AHashMap<NodeID, SpawnRingState>,
    hybrid_spawn_origin_dirty_slots: Vec<u32>,
    hybrid_spawn_rotation_dirty_slots: Vec<u32>,
    compute_spawn_rings: AHashMap<NodeID, SpawnRingState>,
    compute_spawn_origin_dirty_slots: Vec<u32>,
    compute_spawn_rotation_dirty_slots: Vec<u32>,
    spawn_origin_cache: AHashMap<NodeID, AHashMap<u32, SpawnOriginEntry>>,
    spawn_origin_generation: u64,
    hybrid_map_fingerprint: u64,
    hybrid_map_uploaded_fingerprint: u64,
    hybrid_map_uploaded_count: usize,
    compute_map_fingerprint: u64,
    compute_map_uploaded_fingerprint: u64,
    compute_map_uploaded_count: usize,
}

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
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 36,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x3,
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
                    write_mask: wgpu::ColorWrites::ALL,
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
                format: PARTICLE_DEPTH_FORMAT,
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
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 20,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 36,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32x3,
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
                        write_mask: wgpu::ColorWrites::ALL,
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
                    format: PARTICLE_DEPTH_FORMAT,
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
                    write_mask: wgpu::ColorWrites::ALL,
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
                format: PARTICLE_DEPTH_FORMAT,
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
                        write_mask: wgpu::ColorWrites::ALL,
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
                    format: PARTICLE_DEPTH_FORMAT,
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
                    binding: 0,
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
                    module: &compute_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &compute_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
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
                    format: PARTICLE_DEPTH_FORMAT,
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
                    module: &compute_shader,
                    entry_point: Some("vs_billboard"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &compute_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
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
                    format: PARTICLE_DEPTH_FORMAT,
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
                binding: 0,
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

    pub fn set_sample_count(
        &mut self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) {
        *self = Self::new(device, color_format, sample_count);
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: PreparePointParticles3D<'_>,
    ) {
        self.staged.clear();
        self.staged_billboards.clear();
        self.hybrid_emitters.clear();
        self.hybrid_particle_emitter_map.clear();
        self.hybrid_spawn_origin_dirty_slots.clear();
        self.hybrid_spawn_rotation_dirty_slots.clear();
        self.hybrid_particle_count = 0;
        self.hybrid_has_point = false;
        self.hybrid_has_billboard = false;
        self.hybrid_point_ranges.clear();
        self.hybrid_billboard_ranges.clear();
        self.compute_emitters.clear();
        self.compute_particle_emitter_map.clear();
        self.compute_spawn_origin_dirty_slots.clear();
        self.compute_spawn_rotation_dirty_slots.clear();
        self.compute_particle_count = 0;
        self.compute_has_point = false;
        self.compute_has_billboard = false;
        self.compute_point_ranges.clear();
        self.compute_billboard_ranges.clear();
        self.compute_expr_ops.clear();
        self.compute_custom_params.clear();
        self.hybrid_map_fingerprint = 0xcbf2_9ce4_8422_2325;
        self.compute_map_fingerprint = 0xcbf2_9ce4_8422_2325;
        self.spawn_origin_generation = self.spawn_origin_generation.wrapping_add(1);
        if self.spawn_origin_generation == 0 {
            self.spawn_origin_generation = 1;
        }
        let mut emitter_order = (0..frame.emitters.len()).collect::<Vec<_>>();
        emitter_order.sort_unstable_by_key(|&i| frame.emitters[i].0.as_u64());
        for idx in emitter_order {
            let (node, emitter) = &frame.emitters[idx];
            match emitter.sim_mode {
                ParticleSimulationMode3D::Cpu => self.push_emitter_particles(*node, emitter),
                ParticleSimulationMode3D::GpuVertex => {
                    if !self.push_hybrid_emitter_particles(*node, emitter) {
                        self.push_emitter_particles(*node, emitter);
                    }
                }
                ParticleSimulationMode3D::GpuCompute => {
                    if !self.push_compute_emitter_particles(*node, emitter) {
                        self.push_emitter_particles(*node, emitter);
                    }
                }
            }
        }
        let generation = self.spawn_origin_generation;
        self.spawn_origin_cache.retain(|_, per_particle| {
            per_particle.retain(|_, entry| entry.last_seen_generation == generation);
            !per_particle.is_empty()
        });
        if self.staged.is_empty()
            && self.staged_billboards.is_empty()
            && self.hybrid_emitters.is_empty()
            && self.compute_emitters.is_empty()
        {
            return;
        }
        if !self.staged.is_empty() {
            self.ensure_particle_capacity(device, self.staged.len());
            queue.write_buffer(&self.particle_buffer, 0, bytemuck::cast_slice(&self.staged));
        }
        if !self.staged_billboards.is_empty() {
            self.ensure_billboard_particle_capacity(device, self.staged_billboards.len());
            queue.write_buffer(
                &self.billboard_particle_buffer,
                0,
                bytemuck::cast_slice(&self.staged_billboards),
            );
        }
        if !self.hybrid_emitters.is_empty() {
            let hybrid_spawn_origin_recreated = self.ensure_hybrid_emitter_capacity(
                device,
                self.hybrid_emitters.len(),
                self.hybrid_particle_count as usize,
                self.hybrid_particle_spawn_origins.len(),
            );
            queue.write_buffer(
                &self.hybrid_emitter_buffer,
                0,
                bytemuck::cast_slice(&self.hybrid_emitters),
            );
            let hybrid_map_count = self.hybrid_particle_emitter_map.len();
            let hybrid_map_dirty = hybrid_spawn_origin_recreated
                || self.hybrid_map_uploaded_count != hybrid_map_count
                || self.hybrid_map_uploaded_fingerprint != self.hybrid_map_fingerprint;
            if hybrid_map_dirty {
                queue.write_buffer(
                    &self.hybrid_particle_emitter_buffer,
                    0,
                    bytemuck::cast_slice(&self.hybrid_particle_emitter_map),
                );
                self.hybrid_map_uploaded_count = hybrid_map_count;
                self.hybrid_map_uploaded_fingerprint = self.hybrid_map_fingerprint;
            }
            if hybrid_spawn_origin_recreated {
                queue.write_buffer(
                    &self.hybrid_particle_spawn_origin_buffer,
                    0,
                    bytemuck::cast_slice(&self.hybrid_particle_spawn_origins),
                );
                queue.write_buffer(
                    &self.hybrid_particle_spawn_rotation_buffer,
                    0,
                    bytemuck::cast_slice(&self.hybrid_particle_spawn_rotations),
                );
            } else if !self.hybrid_spawn_origin_dirty_slots.is_empty() {
                write_spawn_origin_dirty_ranges(
                    queue,
                    &self.hybrid_particle_spawn_origin_buffer,
                    &self.hybrid_particle_spawn_origins,
                    &mut self.hybrid_spawn_origin_dirty_slots,
                );
                write_spawn_origin_dirty_ranges(
                    queue,
                    &self.hybrid_particle_spawn_rotation_buffer,
                    &self.hybrid_particle_spawn_rotations,
                    &mut self.hybrid_spawn_rotation_dirty_slots,
                );
            }
            let params = GpuEmitterParams {
                emitter_count: self.hybrid_emitters.len() as u32,
                particle_count: self.hybrid_particle_count,
                _pad: [0; 2],
            };
            queue.write_buffer(&self.hybrid_params_buffer, 0, bytemuck::bytes_of(&params));
        }
        if !self.compute_emitters.is_empty() {
            let compute_spawn_origin_recreated = self.ensure_compute_capacity(
                device,
                self.compute_emitters.len(),
                self.compute_particle_count as usize,
                self.compute_particle_spawn_origins.len(),
                self.compute_expr_ops.len(),
                self.compute_custom_params.len(),
            );
            queue.write_buffer(
                &self.compute_emitter_buffer,
                0,
                bytemuck::cast_slice(&self.compute_emitters),
            );
            let compute_map_count = self.compute_particle_emitter_map.len();
            let compute_map_dirty = compute_spawn_origin_recreated
                || self.compute_map_uploaded_count != compute_map_count
                || self.compute_map_uploaded_fingerprint != self.compute_map_fingerprint;
            if compute_map_dirty {
                queue.write_buffer(
                    &self.compute_particle_emitter_buffer,
                    0,
                    bytemuck::cast_slice(&self.compute_particle_emitter_map),
                );
                self.compute_map_uploaded_count = compute_map_count;
                self.compute_map_uploaded_fingerprint = self.compute_map_fingerprint;
            }
            if compute_spawn_origin_recreated {
                queue.write_buffer(
                    &self.compute_particle_spawn_origin_buffer,
                    0,
                    bytemuck::cast_slice(&self.compute_particle_spawn_origins),
                );
                queue.write_buffer(
                    &self.compute_particle_spawn_rotation_buffer,
                    0,
                    bytemuck::cast_slice(&self.compute_particle_spawn_rotations),
                );
            } else if !self.compute_spawn_origin_dirty_slots.is_empty() {
                write_spawn_origin_dirty_ranges(
                    queue,
                    &self.compute_particle_spawn_origin_buffer,
                    &self.compute_particle_spawn_origins,
                    &mut self.compute_spawn_origin_dirty_slots,
                );
                write_spawn_origin_dirty_ranges(
                    queue,
                    &self.compute_particle_spawn_rotation_buffer,
                    &self.compute_particle_spawn_rotations,
                    &mut self.compute_spawn_rotation_dirty_slots,
                );
            }
            let params = GpuEmitterParams {
                emitter_count: self.compute_emitters.len() as u32,
                particle_count: self.compute_particle_count,
                _pad: [0; 2],
            };
            queue.write_buffer(&self.compute_params_buffer, 0, bytemuck::bytes_of(&params));
            if !self.compute_expr_ops.is_empty() {
                queue.write_buffer(
                    &self.compute_expr_op_buffer,
                    0,
                    bytemuck::cast_slice(&self.compute_expr_ops),
                );
            }
            if !self.compute_custom_params.is_empty() {
                queue.write_buffer(
                    &self.compute_custom_param_buffer,
                    0,
                    bytemuck::cast_slice(&self.compute_custom_params),
                );
            }
        }

        let uniform = CameraUniform {
            view_proj: compute_view_proj(&frame.camera, frame.width, frame.height)
                .to_cols_array_2d(),
            inv_view_size: [
                1.0 / (frame.width.max(1) as f32),
                1.0 / (frame.height.max(1) as f32),
            ],
            _pad: [0.0, 0.0],
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn render_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        if self.staged.is_empty()
            && self.staged_billboards.is_empty()
            && self.hybrid_particle_count == 0
            && self.compute_particle_count == 0
        {
            return;
        }
        if self.compute_particle_count > 0 {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_particles3d_compute_pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.camera_bg, &[]);
            compute_pass.set_bind_group(1, &self.compute_bg, &[]);
            let groups = self.compute_particle_count.div_ceil(64);
            compute_pass.dispatch_workgroups(groups, 1, 1);
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_particles3d_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if !self.staged.is_empty() {
            pass.set_pipeline(&self.cpu_pipeline);
            pass.set_bind_group(0, &self.camera_bg, &[]);
            pass.set_vertex_buffer(0, self.particle_buffer.slice(..));
            pass.draw(0..self.staged.len() as u32, 0..1);
        }
        if !self.staged_billboards.is_empty() {
            pass.set_pipeline(&self.cpu_billboard_pipeline);
            pass.set_bind_group(0, &self.camera_bg, &[]);
            pass.set_vertex_buffer(0, self.billboard_particle_buffer.slice(..));
            pass.draw(0..4, 0..self.staged_billboards.len() as u32);
        }
        if self.hybrid_particle_count > 0 {
            if self.hybrid_has_point {
                pass.set_pipeline(&self.hybrid_pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_bind_group(1, &self.hybrid_params_bg, &[]);
                for range in &self.hybrid_point_ranges {
                    pass.draw(0..1, range.start..(range.start + range.count));
                }
            }
            if self.hybrid_has_billboard {
                pass.set_pipeline(&self.hybrid_billboard_pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_bind_group(1, &self.hybrid_params_bg, &[]);
                for range in &self.hybrid_billboard_ranges {
                    pass.draw(0..4, range.start..(range.start + range.count));
                }
            }
        }
        if self.compute_particle_count > 0 {
            if self.compute_has_point {
                pass.set_pipeline(&self.compute_render_pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_bind_group(1, &self.compute_render_bg, &[]);
                for range in &self.compute_point_ranges {
                    pass.draw(0..1, range.start..(range.start + range.count));
                }
            }
            if self.compute_has_billboard {
                pass.set_pipeline(&self.compute_render_billboard_pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_bind_group(1, &self.compute_render_bg, &[]);
                for range in &self.compute_billboard_ranges {
                    pass.draw(0..4, range.start..(range.start + range.count));
                }
            }
        }
    }

    fn ensure_particle_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.particle_capacity {
            return;
        }
        let mut new_capacity = self.particle_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_points"),
            size: (new_capacity * std::mem::size_of::<PointParticleGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.particle_capacity = new_capacity;
    }

    fn ensure_billboard_particle_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.billboard_particle_capacity {
            return;
        }
        let mut new_capacity = self.billboard_particle_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.billboard_particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_billboards"),
            size: (new_capacity * std::mem::size_of::<PointParticleGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.billboard_particle_capacity = new_capacity;
    }

    fn ensure_hybrid_emitter_capacity(
        &mut self,
        device: &wgpu::Device,
        needed_emitters: usize,
        needed_particles: usize,
        needed_spawn_slots: usize,
    ) -> bool {
        let mut emitter_recreated = false;
        if needed_emitters > self.hybrid_emitter_capacity {
            let mut new_capacity = self.hybrid_emitter_capacity.max(1);
            while new_capacity < needed_emitters {
                new_capacity *= 2;
            }
            self.hybrid_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_hybrid_emitters"),
                size: (new_capacity * std::mem::size_of::<GpuEmitterParticle>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.hybrid_emitter_capacity = new_capacity;
            emitter_recreated = true;
        }
        let mut map_recreated = false;
        if needed_particles > self.hybrid_particle_emitter_capacity {
            let mut new_capacity = self.hybrid_particle_emitter_capacity.max(1);
            while new_capacity < needed_particles {
                new_capacity *= 2;
            }
            self.hybrid_particle_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_hybrid_particle_emitters"),
                size: (new_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.hybrid_particle_emitter_capacity = new_capacity;
            map_recreated = true;
        }
        let mut spawn_origin_recreated = false;
        if needed_spawn_slots > self.hybrid_particle_spawn_origin_capacity {
            let mut new_capacity = self.hybrid_particle_spawn_origin_capacity.max(1);
            while new_capacity < needed_spawn_slots {
                new_capacity *= 2;
            }
            self.hybrid_particle_spawn_origin_buffer =
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_particles3d_hybrid_particle_spawn_origins"),
                    size: (new_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            self.hybrid_particle_spawn_origin_capacity = new_capacity;
            spawn_origin_recreated = true;
        }
        let mut spawn_rotation_recreated = false;
        if needed_spawn_slots > self.hybrid_particle_spawn_rotation_capacity {
            let mut new_capacity = self.hybrid_particle_spawn_rotation_capacity.max(1);
            while new_capacity < needed_spawn_slots {
                new_capacity *= 2;
            }
            self.hybrid_particle_spawn_rotation_buffer =
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_particles3d_hybrid_particle_spawn_rotations"),
                    size: (new_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            self.hybrid_particle_spawn_rotation_capacity = new_capacity;
            spawn_rotation_recreated = true;
        }
        if emitter_recreated || map_recreated || spawn_origin_recreated || spawn_rotation_recreated
        {
            self.hybrid_params_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_particles3d_hybrid_emitters_bg"),
                layout: &self.hybrid_emitters_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.hybrid_emitter_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.hybrid_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.hybrid_particle_emitter_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.hybrid_particle_spawn_origin_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self
                            .hybrid_particle_spawn_rotation_buffer
                            .as_entire_binding(),
                    },
                ],
            });
        }
        spawn_origin_recreated || spawn_rotation_recreated
    }

    fn ensure_compute_capacity(
        &mut self,
        device: &wgpu::Device,
        needed_emitters: usize,
        needed_particles: usize,
        needed_spawn_slots: usize,
        needed_expr_ops: usize,
        needed_custom_params: usize,
    ) -> bool {
        let mut emitter_recreated = false;
        if needed_emitters > self.compute_emitter_capacity {
            let mut new_capacity = self.compute_emitter_capacity.max(1);
            while new_capacity < needed_emitters {
                new_capacity *= 2;
            }
            self.compute_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_emitters"),
                size: (new_capacity * std::mem::size_of::<GpuEmitterParticle>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.compute_emitter_capacity = new_capacity;
            emitter_recreated = true;
        }

        let mut particle_recreated = false;
        if needed_particles > self.compute_particle_capacity {
            let mut new_capacity = self.compute_particle_capacity.max(1);
            while new_capacity < needed_particles {
                new_capacity *= 2;
            }
            self.compute_particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_particles"),
                size: (new_capacity * std::mem::size_of::<GpuComputedParticle>()) as u64,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
            self.compute_particle_capacity = new_capacity;
            particle_recreated = true;
        }

        let mut expr_recreated = false;
        if needed_expr_ops > self.compute_expr_op_capacity {
            let mut new_capacity = self.compute_expr_op_capacity.max(1);
            while new_capacity < needed_expr_ops {
                new_capacity *= 2;
            }
            self.compute_expr_op_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_expr_ops"),
                size: (new_capacity * std::mem::size_of::<GpuExprOp>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.compute_expr_op_capacity = new_capacity;
            expr_recreated = true;
        }

        let mut params_recreated = false;
        if needed_custom_params > self.compute_custom_param_capacity {
            let mut new_capacity = self.compute_custom_param_capacity.max(1);
            while new_capacity < needed_custom_params {
                new_capacity *= 2;
            }
            self.compute_custom_param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_custom_params"),
                size: (new_capacity * std::mem::size_of::<f32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.compute_custom_param_capacity = new_capacity;
            params_recreated = true;
        }
        let mut map_recreated = false;
        if needed_particles > self.compute_particle_emitter_capacity {
            let mut new_capacity = self.compute_particle_emitter_capacity.max(1);
            while new_capacity < needed_particles {
                new_capacity *= 2;
            }
            self.compute_particle_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_particle_emitters"),
                size: (new_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.compute_particle_emitter_capacity = new_capacity;
            map_recreated = true;
        }
        let mut spawn_origin_recreated = false;
        if needed_spawn_slots > self.compute_particle_spawn_origin_capacity {
            let mut new_capacity = self.compute_particle_spawn_origin_capacity.max(1);
            while new_capacity < needed_spawn_slots {
                new_capacity *= 2;
            }
            self.compute_particle_spawn_origin_buffer =
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_particles3d_compute_particle_spawn_origins"),
                    size: (new_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            self.compute_particle_spawn_origin_capacity = new_capacity;
            spawn_origin_recreated = true;
        }
        let mut spawn_rotation_recreated = false;
        if needed_spawn_slots > self.compute_particle_spawn_rotation_capacity {
            let mut new_capacity = self.compute_particle_spawn_rotation_capacity.max(1);
            while new_capacity < needed_spawn_slots {
                new_capacity *= 2;
            }
            self.compute_particle_spawn_rotation_buffer =
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_particles3d_compute_particle_spawn_rotations"),
                    size: (new_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            self.compute_particle_spawn_rotation_capacity = new_capacity;
            spawn_rotation_recreated = true;
        }

        if emitter_recreated
            || particle_recreated
            || expr_recreated
            || params_recreated
            || map_recreated
            || spawn_origin_recreated
            || spawn_rotation_recreated
        {
            self.compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_particles3d_compute_bg"),
                layout: &self.compute_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.compute_emitter_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.compute_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.compute_particle_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.compute_expr_op_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.compute_custom_param_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: self.compute_particle_emitter_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: self
                            .compute_particle_spawn_origin_buffer
                            .as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: self
                            .compute_particle_spawn_rotation_buffer
                            .as_entire_binding(),
                    },
                ],
            });
            self.compute_render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_particles3d_compute_render_bg"),
                layout: &self.compute_render_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.compute_particle_buffer.as_entire_binding(),
                }],
            });
        }
        spawn_origin_recreated || spawn_rotation_recreated
    }

    fn resolve_spawn_state(
        &mut self,
        node: NodeID,
        particle_key: u32,
        current_origin: [f32; 3],
        current_rotation: [f32; 4],
    ) -> ([f32; 3], [f32; 4]) {
        let per_particle = self.spawn_origin_cache.entry(node).or_default();
        let generation = self.spawn_origin_generation;
        let entry = per_particle
            .entry(particle_key)
            .or_insert(SpawnOriginEntry {
                origin: current_origin,
                rotation: current_rotation,
                last_seen_generation: generation,
            });
        entry.last_seen_generation = generation;
        (entry.origin, entry.rotation)
    }

    fn push_emitter_particles(&mut self, node: NodeID, emitter: &PointParticles3DState) {
        if !emitter.active || emitter.emission_rate <= 0.0 {
            return;
        }
        let model = Mat4::from_cols_array_2d(&emitter.model);
        let current_origin = model.transform_point3(Vec3::ZERO);
        let (_, rot_raw, _) = model.to_scale_rotation_translation();
        let spawn_rot = if rot_raw.is_finite() && rot_raw.length_squared() > 1.0e-6 {
            rot_raw.normalize()
        } else {
            Quat::IDENTITY
        };
        let time = emitter.simulation_time.max(0.0);
        let sim_delta = emitter.simulation_delta.max(0.0);
        let life_min = emitter.lifetime_min.max(0.001);
        let life_max = emitter.lifetime_max.max(life_min);
        let max_alive_budget = emitter.alive_budget.max(1);
        if max_alive_budget == 0 {
            return;
        }
        let emit_count = emitter_emission_count(emitter, max_alive_budget);
        if emit_count == 0 {
            return;
        }
        let speed_min = emitter.speed_min.max(0.0);
        let speed_max = emitter.speed_max.max(speed_min);
        let size_min = emitter.size_min.max(0.01);
        let size_max = emitter.size_max.max(size_min);
        enum CustomEval<'a> {
            ProgramIds(usize, usize, usize),
            Ops(&'a [Op], &'a [Op], &'a [Op]),
        }

        let compiled_custom = if let (Some(x_ops), Some(y_ops), Some(z_ops)) = (
            emitter.profile.expr_x_ops.as_ref(),
            emitter.profile.expr_y_ops.as_ref(),
            emitter.profile.expr_z_ops.as_ref(),
        ) {
            Some(CustomEval::Ops(
                x_ops.as_ref(),
                y_ops.as_ref(),
                z_ops.as_ref(),
            ))
        } else {
            match &emitter.profile.path {
                ParticlePath3D::Custom {
                    expr_x,
                    expr_y,
                    expr_z,
                } => {
                    match (
                        self.get_or_compile_expr(expr_x),
                        self.get_or_compile_expr(expr_y),
                        self.get_or_compile_expr(expr_z),
                    ) {
                        (Some(x), Some(y), Some(z)) => Some(CustomEval::ProgramIds(x, y, z)),
                        _ => None,
                    }
                }
                ParticlePath3D::CustomCompiled {
                    expr_x_ops,
                    expr_y_ops,
                    expr_z_ops,
                } => Some(CustomEval::Ops(
                    expr_x_ops.as_ref(),
                    expr_y_ops.as_ref(),
                    expr_z_ops.as_ref(),
                )),
                _ => None,
            }
        };
        let billboard_mode = emitter.render_mode == ParticleRenderMode3D::Billboard;
        if billboard_mode {
            self.staged_billboards.reserve(emit_count as usize);
        } else {
            self.staged.reserve(emit_count as usize);
        }
        let prewarm_time = if emitter.looping && emitter.prewarm {
            time + life_max
        } else {
            time
        };
        let emission_rate = emitter.emission_rate.max(1.0e-6);
        let mut total_spawned = (prewarm_time * emission_rate).floor() as u32;
        if emitter.looping && emitter.prewarm {
            total_spawned = total_spawned.max(emit_count.saturating_sub(1));
        }

        for i in 0..emit_count {
            let spawn_index = if emitter.looping {
                let back = emit_count.saturating_sub(1).saturating_sub(i);
                total_spawned.saturating_sub(back)
            } else {
                i
            };
            let particle_key = spawn_index;
            let h0 = hash01(emitter.seed ^ particle_key);
            let h1 = hash01(emitter.seed.wrapping_add(0x9E37_79B9) ^ particle_key.wrapping_mul(3));
            let h2 = hash01(emitter.seed.wrapping_add(0x7F4A_7C15) ^ particle_key.wrapping_mul(7));
            let h3 = hash01(emitter.seed.wrapping_add(0x94D0_49BB) ^ particle_key.wrapping_mul(11));
            let life = life_min + (life_max - life_min) * h0;
            let spawn_t = (spawn_index as f32) / emission_rate;
            let local_t = prewarm_time - spawn_t;
            if !(0.0..=life).contains(&local_t) {
                continue;
            }
            let prev_local_t = (local_t - sim_delta).max(0.0);
            let age = (local_t / life).clamp(0.0, 1.0);
            let prev_age = (prev_local_t / life).clamp(0.0, 1.0);
            let speed = speed_min + (speed_max - speed_min) * h1;
            let spread = emitter.spread_radians * (h2 * 2.0 - 1.0);
            let (yaw_sin, yaw_cos) = (h0 * std::f32::consts::TAU).sin_cos();
            let (spread_sin, spread_cos) = spread.sin_cos();
            let dir_y = spread_cos - yaw_cos * spread_sin;
            let dir_z = spread_sin + yaw_cos * spread_cos;
            let dir = Vec3::new(yaw_sin, dir_y, dir_z).normalize_or_zero();
            let vel = dir * speed;
            let lifetime = life;
            let ring_u = ((particle_key as f32) * 0.618_033_95 + h3 * 0.123_456_7).fract();
            let index01 = if emit_count > 1 {
                (i as f32) / ((emit_count - 1) as f32)
            } else {
                0.0
            };
            let seed_value = particle_key as f32;
            let dir_arr = [dir.x, dir.y, dir.z];
            let vel_arr = [vel.x, vel.y, vel.z];
            let (spawn_origin, spawn_rotation) = self.resolve_spawn_state(
                node,
                particle_key,
                [current_origin.x, current_origin.y, current_origin.z],
                [spawn_rot.x, spawn_rot.y, spawn_rot.z, spawn_rot.w],
            );
            let origin = Vec3::from_array(spawn_origin);
            let spawn_rotation = Quat::from_xyzw(
                spawn_rotation[0],
                spawn_rotation[1],
                spawn_rotation[2],
                spawn_rotation[3],
            );
            let spawn_rotation =
                if spawn_rotation.is_finite() && spawn_rotation.length_squared() > 1.0e-6 {
                    spawn_rotation.normalize()
                } else {
                    Quat::IDENTITY
                };
            let emitter_pos = spawn_origin;
            let mut pos = origin;
            let mut prev_pos = origin;
            match &emitter.profile.path {
                ParticlePath3D::None => {}
                ParticlePath3D::Ballistic => {
                    pos += dir * speed * local_t;
                    prev_pos += dir * speed * prev_local_t;
                }
                ParticlePath3D::Spiral {
                    angular_velocity,
                    radius,
                } => {
                    let theta = local_t * *angular_velocity + h0 * std::f32::consts::TAU;
                    pos += Vec3::new(theta.cos() * *radius, 0.0, theta.sin() * *radius);
                    let prev_theta = prev_local_t * *angular_velocity + h0 * std::f32::consts::TAU;
                    prev_pos +=
                        Vec3::new(prev_theta.cos() * *radius, 0.0, prev_theta.sin() * *radius);
                }
                ParticlePath3D::OrbitY {
                    angular_velocity,
                    radius,
                } => {
                    let theta = local_t * *angular_velocity + h1 * std::f32::consts::TAU;
                    pos = origin
                        + Vec3::new(
                            theta.cos() * *radius,
                            pos.y - origin.y,
                            theta.sin() * *radius,
                        );
                    let prev_theta = prev_local_t * *angular_velocity + h1 * std::f32::consts::TAU;
                    prev_pos = origin
                        + Vec3::new(
                            prev_theta.cos() * *radius,
                            prev_pos.y - origin.y,
                            prev_theta.sin() * *radius,
                        );
                }
                ParticlePath3D::NoiseDrift {
                    amplitude,
                    frequency,
                } => {
                    let n = (local_t * *frequency + h2 * 37.0).sin();
                    let m = (local_t * *frequency * 1.37 + h1 * 17.0).cos();
                    pos += Vec3::new(n, m, n * m) * *amplitude;
                    let prev_n = (prev_local_t * *frequency + h2 * 37.0).sin();
                    let prev_m = (prev_local_t * *frequency * 1.37 + h1 * 17.0).cos();
                    prev_pos += Vec3::new(prev_n, prev_m, prev_n * prev_m) * *amplitude;
                }
                ParticlePath3D::FlatDisk { radius } => {
                    let seq = ((i as f32) + 0.5) / (emit_count.max(1) as f32);
                    let theta = (i as f32) * 2.399_963_1 + h3 * 0.35;
                    let radial = seq.sqrt();
                    let r = *radius * radial * age;
                    pos += Vec3::new(theta.cos() * r, 0.0, theta.sin() * r);
                    let prev_r = *radius * radial * prev_age;
                    prev_pos += Vec3::new(theta.cos() * prev_r, 0.0, theta.sin() * prev_r);
                }
                ParticlePath3D::Custom { .. } | ParticlePath3D::CustomCompiled { .. } => {}
            }
            let force = Vec3::from_array(emitter.gravity);
            pos += 0.5 * force * local_t * local_t;
            prev_pos += 0.5 * force * prev_local_t * prev_local_t;
            let prev_pos_arr = [prev_pos.x, prev_pos.y, prev_pos.z];
            if let Some(custom_eval) = &compiled_custom {
                let eval_input = ParticleEvalInput {
                    t: age,
                    life: local_t,
                    lifetime,
                    spawn_time: spawn_t,
                    emitter_time: time,
                    speed,
                    particle_id: particle_key as f32,
                    dir: dir_arr,
                    vel: vel_arr,
                    rand: [h0, h1, h2],
                    seed: seed_value,
                    ring_u,
                    index01,
                    emitter_pos,
                    prev_pos: prev_pos_arr,
                    params: &emitter.params,
                };
                let (dx, dy, dz) = match *custom_eval {
                    CustomEval::ProgramIds(x_id, y_id, z_id) => (
                        self.eval_compiled_expr(x_id, &eval_input).unwrap_or(0.0),
                        self.eval_compiled_expr(y_id, &eval_input).unwrap_or(0.0),
                        self.eval_compiled_expr(z_id, &eval_input).unwrap_or(0.0),
                    ),
                    CustomEval::Ops(x_ops, y_ops, z_ops) => (
                        eval_ops_particle(x_ops, &eval_input, &mut self.eval_stack).unwrap_or(0.0),
                        eval_ops_particle(y_ops, &eval_input, &mut self.eval_stack).unwrap_or(0.0),
                        eval_ops_particle(z_ops, &eval_input, &mut self.eval_stack).unwrap_or(0.0),
                    ),
                };
                pos += Vec3::new(dx, dy, dz);
            }
            if emitter.profile.spin_angular_velocity.abs() > 1.0e-6 {
                let rel = pos - origin;
                let theta = emitter.profile.spin_angular_velocity * local_t;
                let (s, c) = theta.sin_cos();
                let spun = Vec3::new(rel.x * c - rel.z * s, rel.y, rel.x * s + rel.z * c);
                pos = origin + spun;
            }
            pos = origin + spawn_rotation * (pos - origin);
            let size = emitter.size * (size_min + (size_max - size_min) * h2);
            let color = lerp4(emitter.color_start, emitter.color_end, age);
            let particle = PointParticleGpu {
                world_pos: pos.to_array(),
                size_alpha: [size, color[3]],
                color,
                emissive: emitter.emissive,
            };
            if billboard_mode {
                self.staged_billboards.push(particle);
            } else {
                self.staged.push(particle);
            }
        }
    }

    fn push_hybrid_emitter_particles(
        &mut self,
        node: NodeID,
        emitter: &PointParticles3DState,
    ) -> bool {
        if !emitter.active || emitter.emission_rate <= 0.0 {
            return true;
        }
        if emitter.profile.expr_x_ops.is_some()
            || emitter.profile.expr_y_ops.is_some()
            || emitter.profile.expr_z_ops.is_some()
        {
            return false;
        }
        let Some((path_kind, path_a, path_b)) = gpu_path_params(&emitter.profile.path) else {
            return false;
        };

        let life_min = emitter.lifetime_min.max(0.001);
        let life_max = emitter.lifetime_max.max(life_min);
        let max_alive_budget = emitter.alive_budget.max(1);
        let mut emit_count = emitter_emission_count(emitter, max_alive_budget);
        if emit_count == 0 {
            return true;
        }
        if self.hybrid_particle_count > u32::MAX - emit_count {
            emit_count = u32::MAX - self.hybrid_particle_count;
        }
        if emit_count == 0 {
            return true;
        }
        let model = Mat4::from_cols_array_2d(&emitter.model);
        let current_origin = model.transform_point3(Vec3::ZERO);
        let (_, rot_raw, _) = model.to_scale_rotation_translation();
        let spawn_rot = if rot_raw.is_finite() && rot_raw.length_squared() > 1.0e-6 {
            rot_raw.normalize()
        } else {
            Quat::IDENTITY
        };
        let spawn_rot_arr = [spawn_rot.x, spawn_rot.y, spawn_rot.z, spawn_rot.w];
        let time = emitter.simulation_time.max(0.0);
        let prewarm_time = if emitter.looping && emitter.prewarm {
            time + life_max
        } else {
            time
        };
        let emission_rate = emitter.emission_rate.max(1.0e-6);
        let mut total_spawned = (prewarm_time * emission_rate).floor() as u32;
        if emitter.looping && emitter.prewarm {
            total_spawned = total_spawned.max(emit_count.saturating_sub(1));
        }
        let particle_start = self.hybrid_particle_count;
        let emitter_index = self.hybrid_emitters.len() as u32;
        append_emitter_map_entries(
            &mut self.hybrid_particle_emitter_map,
            emitter_index,
            emit_count,
            &mut self.hybrid_map_fingerprint,
        );
        let spawn_origin_capacity = max_alive_budget.max(1);
        let mut spawn_origin_updates = Vec::<(u32, [f32; 3], [f32; 4])>::new();
        let spawn_origin_base = {
            let entry = self.hybrid_spawn_rings.entry(node).or_insert_with(|| {
                let base = self.hybrid_particle_spawn_origins.len() as u32;
                self.hybrid_particle_spawn_origins
                    .resize((base + spawn_origin_capacity) as usize, [0.0; 4]);
                self.hybrid_particle_spawn_rotations.resize(
                    (base + spawn_origin_capacity) as usize,
                    [0.0, 0.0, 0.0, 1.0],
                );
                SpawnRingState {
                    base,
                    capacity: spawn_origin_capacity,
                    slot_spawn_keys: vec![u32::MAX; spawn_origin_capacity as usize],
                }
            });
            if entry.capacity != spawn_origin_capacity {
                let base = self.hybrid_particle_spawn_origins.len() as u32;
                self.hybrid_particle_spawn_origins
                    .resize((base + spawn_origin_capacity) as usize, [0.0; 4]);
                self.hybrid_particle_spawn_rotations.resize(
                    (base + spawn_origin_capacity) as usize,
                    [0.0, 0.0, 0.0, 1.0],
                );
                entry.base = base;
                entry.capacity = spawn_origin_capacity;
                entry.slot_spawn_keys = vec![u32::MAX; spawn_origin_capacity as usize];
            }
            for i in 0..emit_count {
                let spawn_index = if emitter.looping {
                    let back = emit_count.saturating_sub(1).saturating_sub(i);
                    total_spawned.saturating_sub(back)
                } else {
                    i
                };
                let slot = spawn_index % entry.capacity;
                let slot_idx = slot as usize;
                if entry.slot_spawn_keys[slot_idx] != spawn_index {
                    entry.slot_spawn_keys[slot_idx] = spawn_index;
                    spawn_origin_updates.push((
                        entry.base + slot,
                        [current_origin.x, current_origin.y, current_origin.z],
                        spawn_rot_arr,
                    ));
                }
            }
            entry.base
        };
        for (slot, origin, rotation) in spawn_origin_updates {
            self.hybrid_particle_spawn_origins[slot as usize] =
                [origin[0], origin[1], origin[2], 0.0];
            self.hybrid_particle_spawn_rotations[slot as usize] = rotation;
            self.hybrid_spawn_origin_dirty_slots.push(slot);
            self.hybrid_spawn_rotation_dirty_slots.push(slot);
        }
        self.hybrid_emitters.push(GpuEmitterParticle {
            model_0: emitter.model[0],
            model_1: emitter.model[1],
            model_2: emitter.model[2],
            model_3: emitter.model[3],
            gravity_path: [
                emitter.gravity[0],
                emitter.gravity[1],
                emitter.gravity[2],
                path_kind as f32,
            ],
            color_start: emitter.color_start,
            color_end: emitter.color_end,
            emissive_point: [
                emitter.emissive[0],
                emitter.emissive[1],
                emitter.emissive[2],
                emitter.size,
            ],
            life_speed: [
                life_min,
                life_max,
                emitter.speed_min.max(0.0),
                emitter.speed_max.max(emitter.speed_min.max(0.0)),
            ],
            size_spread_rate: [
                emitter.size_min.max(0.01),
                emitter.size_max.max(emitter.size_min.max(0.01)),
                emitter.spread_radians.clamp(0.0, std::f32::consts::PI),
                emitter.emission_rate.max(0.0),
            ],
            time_path: [
                emitter.simulation_time.max(0.0),
                path_a,
                path_b,
                emitter.simulation_delta.max(0.0),
            ],
            counts_seed: [
                particle_start,
                emit_count,
                max_alive_budget.max(1),
                emitter.seed,
            ],
            flags: [
                u32::from(emitter.looping),
                u32::from(emitter.prewarm),
                emitter.profile.spin_angular_velocity.to_bits(),
                spawn_origin_base,
            ],
            custom_ops_xy: [0; 4],
            custom_ops_zp: [0; 4],
        });
        if emitter.render_mode == ParticleRenderMode3D::Billboard {
            self.hybrid_has_billboard = true;
            push_instance_range(
                &mut self.hybrid_billboard_ranges,
                particle_start,
                emit_count,
                path_kind,
            );
        } else {
            self.hybrid_has_point = true;
            push_instance_range(
                &mut self.hybrid_point_ranges,
                particle_start,
                emit_count,
                path_kind,
            );
        }
        self.hybrid_particle_count += emit_count;
        true
    }

    fn push_compute_emitter_particles(
        &mut self,
        node: NodeID,
        emitter: &PointParticles3DState,
    ) -> bool {
        if !emitter.active || emitter.emission_rate <= 0.0 {
            return true;
        }
        let (path_kind, path_a, path_b) = match &emitter.profile.path {
            ParticlePath3D::None => (0u32, 0.0, 0.0),
            ParticlePath3D::Ballistic => (1u32, 0.0, 0.0),
            ParticlePath3D::Spiral {
                angular_velocity,
                radius,
            } => (2u32, *angular_velocity, *radius),
            ParticlePath3D::OrbitY {
                angular_velocity,
                radius,
            } => (3u32, *angular_velocity, *radius),
            ParticlePath3D::NoiseDrift {
                amplitude,
                frequency,
            } => (4u32, *amplitude, *frequency),
            ParticlePath3D::FlatDisk { radius } => (5u32, 0.0, *radius),
            ParticlePath3D::CustomCompiled {
                expr_x_ops,
                expr_y_ops,
                expr_z_ops,
            } => {
                let _ = self.append_compute_custom_data(
                    expr_x_ops.as_ref(),
                    expr_y_ops.as_ref(),
                    expr_z_ops.as_ref(),
                    &emitter.params,
                );
                (0u32, 0.0, 0.0)
            }
            ParticlePath3D::Custom {
                expr_x,
                expr_y,
                expr_z,
            } => {
                let expr_x_prog = match compile_expression(expr_x) {
                    Ok(program) => program,
                    Err(_) => return false,
                };
                let expr_y_prog = match compile_expression(expr_y) {
                    Ok(program) => program,
                    Err(_) => return false,
                };
                let expr_z_prog = match compile_expression(expr_z) {
                    Ok(program) => program,
                    Err(_) => return false,
                };
                let _ = self.append_compute_custom_data(
                    expr_x_prog.ops(),
                    expr_y_prog.ops(),
                    expr_z_prog.ops(),
                    &emitter.params,
                );
                (0u32, 0.0, 0.0)
            }
        };
        let mut custom_ops_xy = [0u32; 4];
        let mut custom_ops_zp = [0u32; 4];
        if let (Some(x_ops), Some(y_ops), Some(z_ops)) = (
            emitter.profile.expr_x_ops.as_ref(),
            emitter.profile.expr_y_ops.as_ref(),
            emitter.profile.expr_z_ops.as_ref(),
        ) {
            let (ops_xy, ops_zp) = self.append_compute_custom_data(
                x_ops.as_ref(),
                y_ops.as_ref(),
                z_ops.as_ref(),
                &emitter.params,
            );
            custom_ops_xy = ops_xy;
            custom_ops_zp = ops_zp;
        } else {
            match &emitter.profile.path {
                ParticlePath3D::CustomCompiled {
                    expr_x_ops,
                    expr_y_ops,
                    expr_z_ops,
                } => {
                    let (ops_xy, ops_zp) = self.append_compute_custom_data(
                        expr_x_ops.as_ref(),
                        expr_y_ops.as_ref(),
                        expr_z_ops.as_ref(),
                        &emitter.params,
                    );
                    custom_ops_xy = ops_xy;
                    custom_ops_zp = ops_zp;
                }
                ParticlePath3D::Custom {
                    expr_x,
                    expr_y,
                    expr_z,
                } => {
                    let expr_x_prog = match compile_expression(expr_x) {
                        Ok(program) => program,
                        Err(_) => return false,
                    };
                    let expr_y_prog = match compile_expression(expr_y) {
                        Ok(program) => program,
                        Err(_) => return false,
                    };
                    let expr_z_prog = match compile_expression(expr_z) {
                        Ok(program) => program,
                        Err(_) => return false,
                    };
                    let (ops_xy, ops_zp) = self.append_compute_custom_data(
                        expr_x_prog.ops(),
                        expr_y_prog.ops(),
                        expr_z_prog.ops(),
                        &emitter.params,
                    );
                    custom_ops_xy = ops_xy;
                    custom_ops_zp = ops_zp;
                }
                _ => {}
            }
        }

        let life_min = emitter.lifetime_min.max(0.001);
        let life_max = emitter.lifetime_max.max(life_min);
        let max_alive_budget = emitter.alive_budget.max(1);
        let mut emit_count = emitter_emission_count(emitter, max_alive_budget);
        if emit_count == 0 {
            return true;
        }
        if self.compute_particle_count > u32::MAX - emit_count {
            emit_count = u32::MAX - self.compute_particle_count;
        }
        if emit_count == 0 {
            return true;
        }
        let model = Mat4::from_cols_array_2d(&emitter.model);
        let current_origin = model.transform_point3(Vec3::ZERO);
        let (_, rot_raw, _) = model.to_scale_rotation_translation();
        let spawn_rot = if rot_raw.is_finite() && rot_raw.length_squared() > 1.0e-6 {
            rot_raw.normalize()
        } else {
            Quat::IDENTITY
        };
        let spawn_rot_arr = [spawn_rot.x, spawn_rot.y, spawn_rot.z, spawn_rot.w];
        let time = emitter.simulation_time.max(0.0);
        let prewarm_time = if emitter.looping && emitter.prewarm {
            time + life_max
        } else {
            time
        };
        let emission_rate = emitter.emission_rate.max(1.0e-6);
        let mut total_spawned = (prewarm_time * emission_rate).floor() as u32;
        if emitter.looping && emitter.prewarm {
            total_spawned = total_spawned.max(emit_count.saturating_sub(1));
        }
        let particle_start = self.compute_particle_count;
        let emitter_index = self.compute_emitters.len() as u32;
        append_emitter_map_entries(
            &mut self.compute_particle_emitter_map,
            emitter_index,
            emit_count,
            &mut self.compute_map_fingerprint,
        );
        let spawn_origin_capacity = max_alive_budget.max(1);
        let mut spawn_origin_updates = Vec::<(u32, [f32; 3], [f32; 4])>::new();
        let spawn_origin_base = {
            let entry = self.compute_spawn_rings.entry(node).or_insert_with(|| {
                let base = self.compute_particle_spawn_origins.len() as u32;
                self.compute_particle_spawn_origins
                    .resize((base + spawn_origin_capacity) as usize, [0.0; 4]);
                self.compute_particle_spawn_rotations.resize(
                    (base + spawn_origin_capacity) as usize,
                    [0.0, 0.0, 0.0, 1.0],
                );
                SpawnRingState {
                    base,
                    capacity: spawn_origin_capacity,
                    slot_spawn_keys: vec![u32::MAX; spawn_origin_capacity as usize],
                }
            });
            if entry.capacity != spawn_origin_capacity {
                let base = self.compute_particle_spawn_origins.len() as u32;
                self.compute_particle_spawn_origins
                    .resize((base + spawn_origin_capacity) as usize, [0.0; 4]);
                self.compute_particle_spawn_rotations.resize(
                    (base + spawn_origin_capacity) as usize,
                    [0.0, 0.0, 0.0, 1.0],
                );
                entry.base = base;
                entry.capacity = spawn_origin_capacity;
                entry.slot_spawn_keys = vec![u32::MAX; spawn_origin_capacity as usize];
            }
            for i in 0..emit_count {
                let spawn_index = if emitter.looping {
                    let back = emit_count.saturating_sub(1).saturating_sub(i);
                    total_spawned.saturating_sub(back)
                } else {
                    i
                };
                let slot = spawn_index % entry.capacity;
                let slot_idx = slot as usize;
                if entry.slot_spawn_keys[slot_idx] != spawn_index {
                    entry.slot_spawn_keys[slot_idx] = spawn_index;
                    spawn_origin_updates.push((
                        entry.base + slot,
                        [current_origin.x, current_origin.y, current_origin.z],
                        spawn_rot_arr,
                    ));
                }
            }
            entry.base
        };
        for (slot, origin, rotation) in spawn_origin_updates {
            self.compute_particle_spawn_origins[slot as usize] =
                [origin[0], origin[1], origin[2], 0.0];
            self.compute_particle_spawn_rotations[slot as usize] = rotation;
            self.compute_spawn_origin_dirty_slots.push(slot);
            self.compute_spawn_rotation_dirty_slots.push(slot);
        }
        self.compute_emitters.push(GpuEmitterParticle {
            model_0: emitter.model[0],
            model_1: emitter.model[1],
            model_2: emitter.model[2],
            model_3: emitter.model[3],
            gravity_path: [
                emitter.gravity[0],
                emitter.gravity[1],
                emitter.gravity[2],
                path_kind as f32,
            ],
            color_start: emitter.color_start,
            color_end: emitter.color_end,
            emissive_point: [
                emitter.emissive[0],
                emitter.emissive[1],
                emitter.emissive[2],
                emitter.size,
            ],
            life_speed: [
                life_min,
                life_max,
                emitter.speed_min.max(0.0),
                emitter.speed_max.max(emitter.speed_min.max(0.0)),
            ],
            size_spread_rate: [
                emitter.size_min.max(0.01),
                emitter.size_max.max(emitter.size_min.max(0.01)),
                emitter.spread_radians.clamp(0.0, std::f32::consts::PI),
                emitter.emission_rate.max(0.0),
            ],
            time_path: [
                emitter.simulation_time.max(0.0),
                path_a,
                path_b,
                emitter.simulation_delta.max(0.0),
            ],
            counts_seed: [
                particle_start,
                emit_count,
                max_alive_budget.max(1),
                emitter.seed,
            ],
            flags: [
                u32::from(emitter.looping),
                u32::from(emitter.prewarm),
                emitter.profile.spin_angular_velocity.to_bits(),
                spawn_origin_base,
            ],
            custom_ops_xy,
            custom_ops_zp,
        });
        if emitter.render_mode == ParticleRenderMode3D::Billboard {
            self.compute_has_billboard = true;
            push_instance_range(
                &mut self.compute_billboard_ranges,
                particle_start,
                emit_count,
                path_kind,
            );
        } else {
            self.compute_has_point = true;
            push_instance_range(
                &mut self.compute_point_ranges,
                particle_start,
                emit_count,
                path_kind,
            );
        }
        self.compute_particle_count += emit_count;
        true
    }

    fn append_compute_custom_data(
        &mut self,
        expr_x_ops: &[Op],
        expr_y_ops: &[Op],
        expr_z_ops: &[Op],
        params: &[f32],
    ) -> ([u32; 4], [u32; 4]) {
        let (x_off, x_len) = append_gpu_ops(&mut self.compute_expr_ops, expr_x_ops);
        let (y_off, y_len) = append_gpu_ops(&mut self.compute_expr_ops, expr_y_ops);
        let (z_off, z_len) = append_gpu_ops(&mut self.compute_expr_ops, expr_z_ops);
        let params_off = self.compute_custom_params.len() as u32;
        self.compute_custom_params.extend_from_slice(params);
        let params_len = params.len() as u32;
        (
            [x_off, x_len, y_off, y_len],
            [z_off, z_len, params_off, params_len],
        )
    }

    fn get_or_compile_expr(&mut self, expr: &str) -> Option<usize> {
        if let Some(id) = self.compiled_expr_lookup.get(expr).copied() {
            return Some(id);
        }
        let compiled = compile_expression(expr).ok()?;
        let id = self.compiled_exprs.len();
        self.compiled_exprs.push(compiled);
        self.compiled_expr_lookup.insert(expr.to_string(), id);
        Some(id)
    }

    fn eval_compiled_expr(&mut self, id: usize, input: &ParticleEvalInput<'_>) -> Option<f32> {
        let compiled = self.compiled_exprs.get(id)?;
        compiled.eval_particle(input, &mut self.eval_stack)
    }
}

fn emitter_emission_count(emitter: &PointParticles3DState, max_alive_budget: u32) -> u32 {
    if max_alive_budget == 0 {
        return 0;
    }
    let time = emitter.simulation_time.max(0.0);
    let spawned = (time * emitter.emission_rate.max(0.0)) as u32;
    if emitter.looping && emitter.prewarm {
        max_alive_budget
    } else {
        spawned.min(max_alive_budget)
    }
}

fn gpu_path_params(path: &ParticlePath3D) -> Option<(u32, f32, f32)> {
    match path {
        ParticlePath3D::None => Some((0, 0.0, 0.0)),
        ParticlePath3D::Ballistic => Some((1, 0.0, 0.0)),
        ParticlePath3D::Spiral {
            angular_velocity,
            radius,
        } => Some((2, *angular_velocity, *radius)),
        ParticlePath3D::OrbitY {
            angular_velocity,
            radius,
        } => Some((3, *angular_velocity, *radius)),
        ParticlePath3D::NoiseDrift {
            amplitude,
            frequency,
        } => Some((4, *amplitude, *frequency)),
        ParticlePath3D::FlatDisk { radius } => Some((5, 0.0, *radius)),
        ParticlePath3D::Custom { .. } => None,
        ParticlePath3D::CustomCompiled { .. } => None,
    }
}

fn append_gpu_ops(dst: &mut Vec<GpuExprOp>, ops: &[Op]) -> (u32, u32) {
    let offset = dst.len() as u32;
    for op in ops {
        dst.push(encode_gpu_op(op));
    }
    (offset, ops.len() as u32)
}

fn encode_gpu_op(op: &Op) -> GpuExprOp {
    let (opcode, arg) = match op {
        Op::Const(v) => (0u32, v.to_bits()),
        Op::T => (1u32, 0u32),
        Op::Life => (2u32, 0u32),
        Op::Id => (3u32, 0u32),
        Op::Rand => (4u32, 0u32),
        Op::Rand2 => (5u32, 0u32),
        Op::Rand3 => (6u32, 0u32),
        Op::Param => (7u32, 0u32),
        Op::Add => (8u32, 0u32),
        Op::Sub => (9u32, 0u32),
        Op::Mul => (10u32, 0u32),
        Op::Div => (11u32, 0u32),
        Op::Pow => (12u32, 0u32),
        Op::Neg => (13u32, 0u32),
        Op::Sin => (14u32, 0u32),
        Op::Cos => (15u32, 0u32),
        Op::Tan => (16u32, 0u32),
        Op::Abs => (17u32, 0u32),
        Op::Sqrt => (18u32, 0u32),
        Op::Min => (19u32, 0u32),
        Op::Max => (20u32, 0u32),
        Op::Clamp => (21u32, 0u32),
        Op::Speed => (22u32, 0u32),
        Op::Lifetime => (23u32, 0u32),
        Op::AgeLeft => (24u32, 0u32),
        Op::Age01 => (25u32, 0u32),
        Op::SpawnTime => (26u32, 0u32),
        Op::EmitterTime => (27u32, 0u32),
        Op::DirX => (28u32, 0u32),
        Op::DirY => (29u32, 0u32),
        Op::DirZ => (30u32, 0u32),
        Op::VelX => (31u32, 0u32),
        Op::VelY => (32u32, 0u32),
        Op::VelZ => (33u32, 0u32),
        Op::Seed => (34u32, 0u32),
        Op::RingU => (35u32, 0u32),
        Op::Index01 => (36u32, 0u32),
        Op::EmitterX => (37u32, 0u32),
        Op::EmitterY => (38u32, 0u32),
        Op::EmitterZ => (39u32, 0u32),
        Op::Hash => (43u32, 0u32),
    };
    GpuExprOp {
        words: [opcode, arg, 0, 0],
    }
}

fn compute_view_proj(camera: &Camera3DState, width: u32, height: u32) -> Mat4 {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let aspect = w / h;
    let proj = projection_matrix(camera.projection, aspect);
    let pos = Vec3::from_array(camera.position);
    let rot_raw = Quat::from_xyzw(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
        camera.rotation[3],
    );
    let rot = if rot_raw.is_finite() && rot_raw.length_squared() > 1.0e-6 {
        rot_raw.normalize()
    } else {
        Quat::IDENTITY
    };
    let world = Mat4::from_rotation_translation(rot, pos);
    proj * world.inverse()
}

fn push_instance_range(ranges: &mut Vec<InstanceRange>, start: u32, count: u32, path_kind: u32) {
    if count == 0 {
        return;
    }
    if let Some(last) = ranges.last_mut() {
        let last_end = last.start.saturating_add(last.count);
        if last_end == start && last.path_kind == path_kind {
            last.count = last.count.saturating_add(count);
            return;
        }
    }
    ranges.push(InstanceRange {
        start,
        count,
        path_kind,
    });
}

fn append_emitter_map_entries(
    map: &mut Vec<u32>,
    emitter_index: u32,
    count: u32,
    fingerprint: &mut u64,
) {
    if count == 0 {
        return;
    }
    let old_len = map.len();
    map.resize(old_len + count as usize, emitter_index);
    for _ in 0..count {
        hash_u32(fingerprint, emitter_index);
    }
}

fn write_spawn_origin_dirty_ranges(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    all_origins: &[[f32; 4]],
    dirty_slots: &mut Vec<u32>,
) {
    dirty_slots.sort_unstable();
    dirty_slots.dedup();
    let mut i = 0usize;
    while i < dirty_slots.len() {
        let start = dirty_slots[i];
        let mut end = start;
        i += 1;
        while i < dirty_slots.len() {
            let slot = dirty_slots[i];
            if slot == end.saturating_add(1) {
                end = slot;
                i += 1;
            } else {
                break;
            }
        }
        let start_idx = start as usize;
        let end_idx = end as usize + 1;
        let byte_offset = (start_idx * std::mem::size_of::<[f32; 4]>()) as u64;
        queue.write_buffer(
            buffer,
            byte_offset,
            bytemuck::cast_slice(&all_origins[start_idx..end_idx]),
        );
    }
    dirty_slots.clear();
}

#[inline]
fn hash_u32(fingerprint: &mut u64, value: u32) {
    *fingerprint ^= value as u64;
    *fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01B3);
}

fn projection_matrix(projection: CameraProjectionState, aspect: f32) -> Mat4 {
    match projection {
        CameraProjectionState::Perspective {
            fov_y_degrees,
            near,
            far,
        } => {
            let fov_y_radians = fov_y_degrees
                .to_radians()
                .clamp(10.0f32.to_radians(), 120.0f32.to_radians());
            Mat4::perspective_rh_gl(
                fov_y_radians,
                aspect.max(1.0e-6),
                near.max(1.0e-3),
                far.max(near + 1.0e-3),
            )
        }
        CameraProjectionState::Orthographic { size, near, far } => {
            let half_h = (size.abs() * 0.5).max(1.0e-3);
            let half_w = half_h * aspect.max(1.0e-6);
            Mat4::orthographic_rh(
                -half_w,
                half_w,
                -half_h,
                half_h,
                near.max(1.0e-3),
                far.max(near + 1.0e-3),
            )
        }
        CameraProjectionState::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => Mat4::frustum_rh_gl(
            left,
            right,
            bottom,
            top,
            near.max(1.0e-3),
            far.max(near + 1.0e-3),
        ),
    }
}

#[inline]
fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

#[inline]
fn hash01(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    x = (x >> ((x >> 28) + 4)) ^ x;
    x = x.wrapping_mul(277_803_737);
    x = (x >> 22) ^ x;
    (x as f32) / (u32::MAX as f32)
}
