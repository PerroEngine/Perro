use super::*;

impl Gpu3D {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        config: Gpu3DConfig,
    ) -> Self {
        let Gpu3DConfig {
            sample_count,
            width,
            height,
            meshlets_enabled,
            dev_meshlets,
            meshlet_debug_view,
            occlusion_culling,
            indirect_first_instance_enabled,
        } = config;
        let (gpu_occlusion_enabled, cpu_occlusion_enabled) = occlusion_flags(occlusion_culling);
        let shader = create_mesh_shader_module_skinned(device);
        let shader_unlit = create_unlit_shader_module_skinned(device);
        let shader_toon = create_toon_shader_module_skinned(device);
        let shader_rigid = create_mesh_shader_module_rigid(device);
        let shader_rigid_unlit = create_unlit_shader_module_rigid(device);
        let shader_rigid_toon = create_toon_shader_module_rigid(device);
        let shader_multimesh = create_multimesh_shader_module(device);
        let sky_shader = create_sky_shader_module(device);
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_camera3d_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<Scene3DUniform>() as u64)
                                .expect("camera uniform size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let rigid_camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_camera3d_rigid_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<Scene3DUniform>() as u64)
                                .expect("camera uniform size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let multimesh_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_multimesh_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<Scene3DUniform>() as u64)
                                .expect("scene size"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let material_texture_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_material_texture_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_shadow3d_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<ShadowUniform>() as u64)
                                .expect("shadow uniform size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let mesh_blend_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_mesh_blend_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let sky_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_sky3d_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<SkyUniform>() as u64)
                                .expect("sky uniform size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_camera3d_buffer"),
            size: std::mem::size_of::<Scene3DUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_shadow_camera3d_buffer"),
            size: std::mem::size_of::<Scene3DUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_shadow3d_buffer"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (shadow_map_texture, shadow_map_view) =
            create_shadow_map_texture(device, SHADOW_MAP_SIZE);
        let shadow_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("perro_shadow3d_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let sky_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_sky3d_buffer"),
            size: std::mem::size_of::<SkyUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (sky_noise_texture, sky_noise_view, sky_noise_sampler) =
            create_sky_noise_texture(device, queue);
        let skeleton_capacity = 1usize;
        let skeleton_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_skeleton_palette_buffer"),
            size: (skeleton_capacity * std::mem::size_of::<[[f32; 4]; 4]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let custom_params_meta_capacity = 1usize;
        let custom_params_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_custom_material_params_meta"),
            size: (custom_params_meta_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let custom_params_values_capacity = 1usize;
        let custom_params_values_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_custom_material_params_values"),
            size: (custom_params_values_capacity * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let multimesh_draw_params_capacity = 256usize;
        let multimesh_draw_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_draw_params"),
            size: (multimesh_draw_params_capacity * std::mem::size_of::<MultiMeshDrawParamGpu>())
                as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_camera3d_bg"),
            layout: &camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: skeleton_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: custom_params_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: custom_params_values_buffer.as_entire_binding(),
                },
            ],
        });
        let rigid_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_camera3d_rigid_bg"),
            layout: &rigid_camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: custom_params_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: custom_params_values_buffer.as_entire_binding(),
                },
            ],
        });
        let shadow_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_shadow_camera3d_bg"),
            layout: &camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: skeleton_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: custom_params_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: custom_params_values_buffer.as_entire_binding(),
                },
            ],
        });
        let rigid_shadow_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_shadow_camera3d_rigid_bg"),
            layout: &rigid_camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: custom_params_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: custom_params_values_buffer.as_entire_binding(),
                },
            ],
        });
        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_shadow3d_bg"),
            layout: &shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_map_sampler),
                },
            ],
        });
        let sky_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_sky3d_bg"),
            layout: &sky_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sky_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&sky_noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sky_noise_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_mesh_pipeline_layout"),
            bind_group_layouts: &[
                Some(&camera_bgl),
                Some(&material_texture_bgl),
                Some(&shadow_bgl),
                Some(&mesh_blend_bgl),
            ],
            immediate_size: 0,
        });
        let rigid_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_mesh_pipeline_layout_rigid"),
                bind_group_layouts: &[
                    Some(&rigid_camera_bgl),
                    Some(&material_texture_bgl),
                    Some(&shadow_bgl),
                    Some(&mesh_blend_bgl),
                ],
                immediate_size: 0,
            });
        let multimesh_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_multimesh_pipeline_layout"),
                bind_group_layouts: &[Some(&multimesh_bgl)],
                immediate_size: 0,
            });
        let depth_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_depth_pipeline_layout"),
                bind_group_layouts: &[Some(&camera_bgl)],
                immediate_size: 0,
            });
        let rigid_depth_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_depth_pipeline_layout_rigid"),
                bind_group_layouts: &[Some(&rigid_camera_bgl)],
                immediate_size: 0,
            });
        let sky_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_sky3d_pipeline_layout"),
            bind_group_layouts: &[Some(&sky_bgl)],
            immediate_size: 0,
        });
        let sky_pipeline = create_sky_pipeline(
            device,
            &sky_pipeline_layout,
            &sky_shader,
            color_format,
            sample_count,
        );
        let pipeline_culled = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_double_sided = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            None,
        );
        let pipeline_blend_culled = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_blend_double_sided = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            None,
        );
        let pipeline_unlit_culled = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_unlit_double_sided = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            None,
        );
        let pipeline_unlit_blend_culled = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_unlit_blend_double_sided = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            None,
        );
        let pipeline_toon_culled = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_toon_double_sided = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            None,
        );
        let pipeline_toon_blend_culled = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_toon_blend_double_sided = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            None,
        );
        let pipeline_overlay_culled = create_pipeline_overlay_skinned(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_overlay_double_sided = create_pipeline_overlay_skinned(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            None,
        );
        let pipeline_rigid_culled = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_rigid_double_sided = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            None,
        );
        let pipeline_rigid_blend_culled = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_rigid_blend_double_sided = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            None,
        );
        let pipeline_rigid_unlit_culled = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_unlit,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_rigid_unlit_double_sided = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_unlit,
            color_format,
            sample_count,
            None,
        );
        let pipeline_rigid_unlit_blend_culled = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_unlit,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_rigid_unlit_blend_double_sided = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_unlit,
            color_format,
            sample_count,
            None,
        );
        let pipeline_rigid_toon_culled = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_toon,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_rigid_toon_double_sided = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_toon,
            color_format,
            sample_count,
            None,
        );
        let pipeline_rigid_toon_blend_culled = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_toon,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_rigid_toon_blend_double_sided = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_toon,
            color_format,
            sample_count,
            None,
        );
        let pipeline_rigid_overlay_culled = create_pipeline_overlay_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_rigid_overlay_double_sided = create_pipeline_overlay_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            None,
        );
        let pipeline_multimesh_culled = create_multimesh_pipeline(
            device,
            &multimesh_pipeline_layout,
            &shader_multimesh,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_multimesh_double_sided = create_multimesh_pipeline(
            device,
            &multimesh_pipeline_layout,
            &shader_multimesh,
            color_format,
            sample_count,
            None,
        );
        let pipeline_multimesh_blend_culled = create_multimesh_blend_pipeline(
            device,
            &multimesh_pipeline_layout,
            &shader_multimesh,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_multimesh_blend_double_sided = create_multimesh_blend_pipeline(
            device,
            &multimesh_pipeline_layout,
            &shader_multimesh,
            color_format,
            sample_count,
            None,
        );
        let depth_prepass_shader_rigid = create_depth_prepass_shader_module_rigid(device);
        let depth_prepass_shader_skinned = create_depth_prepass_shader_module_skinned(device);
        let pipeline_depth_prepass_culled = create_depth_prepass_pipeline_skinned(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader_skinned,
            Some(wgpu::Face::Back),
        );
        let pipeline_depth_prepass_double_sided = create_depth_prepass_pipeline_skinned(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader_skinned,
            None,
        );
        let pipeline_depth_prepass_rigid_culled = create_depth_prepass_pipeline_rigid(
            device,
            &rigid_depth_pipeline_layout,
            &depth_prepass_shader_rigid,
            Some(wgpu::Face::Back),
        );
        let pipeline_depth_prepass_rigid_double_sided = create_depth_prepass_pipeline_rigid(
            device,
            &rigid_depth_pipeline_layout,
            &depth_prepass_shader_rigid,
            None,
        );
        let pipeline_shadow_depth_culled = create_shadow_depth_pipeline_skinned(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader_skinned,
            Some(wgpu::Face::Back),
        );
        let pipeline_shadow_depth_double_sided = create_shadow_depth_pipeline_skinned(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader_skinned,
            None,
        );
        let pipeline_shadow_depth_rigid_culled = create_shadow_depth_pipeline_rigid(
            device,
            &rigid_depth_pipeline_layout,
            &depth_prepass_shader_rigid,
            Some(wgpu::Face::Back),
        );
        let pipeline_shadow_depth_rigid_double_sided = create_shadow_depth_pipeline_rigid(
            device,
            &rigid_depth_pipeline_layout,
            &depth_prepass_shader_rigid,
            None,
        );

        let (vertices, indices, builtin_mesh_ranges, builtin_meshlets) =
            build_builtin_mesh_buffer();
        let builtin_mesh_bounds =
            compute_builtin_mesh_bounds(&vertices, &indices, &builtin_mesh_ranges);
        let rigid_vertices: Vec<RigidMeshVertex> = vertices
            .iter()
            .map(|v| RigidMeshVertex {
                pos: v.pos,
                normal: v.normal,
                uv: v.uv,
            })
            .collect();
        let vertex_capacity = vertices.len().max(1);
        let rigid_vertex_capacity = rigid_vertices.len().max(1);
        let index_capacity = indices.len().max(1);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let rigid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_vertices_rigid"),
            contents: bytemuck::cast_slice(&rigid_vertices),
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });

        let instance_transform_capacity = 256usize;
        let instance_transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_transforms"),
            size: (instance_transform_capacity * std::mem::size_of::<TransformInstanceGpu>())
                as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instance_material_capacity = 256usize;
        let instance_material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_materials"),
            size: (instance_material_capacity * std::mem::size_of::<MaterialInstanceGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let rigid_instance_meta_capacity = 256usize;
        let rigid_instance_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_rigid_meta"),
            size: (rigid_instance_meta_capacity * std::mem::size_of::<RigidInstanceMetaGpu>())
                as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let skinned_instance_meta_capacity = 256usize;
        let skinned_instance_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_skinned_meta"),
            size: (skinned_instance_meta_capacity * std::mem::size_of::<SkinnedInstanceMetaGpu>())
                as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let multimesh_instance_capacity = 256usize;
        let multimesh_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_instances"),
            size: (multimesh_instance_capacity * std::mem::size_of::<MultiMeshInstanceGpu>())
                as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frustum_cull_enabled = indirect_first_instance_enabled;
        let frustum_shader = create_frustum_cull_shader_module(device);
        let frustum_cull_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_frustum_cull_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(
                                std::mem::size_of::<FrustumCullParamsGpu>() as u64
                            )
                            .expect("frustum cull params size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
            ],
        });
        let frustum_cull_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_frustum_cull_layout"),
            bind_group_layouts: &[Some(&frustum_cull_bgl)],
            immediate_size: 0,
        });
        let frustum_cull_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("perro_frustum_cull_pipeline"),
                layout: Some(&frustum_cull_layout),
                module: &frustum_shader,
                entry_point: Some("cs_main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let frustum_cull_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_frustum_cull_params"),
            size: std::mem::size_of::<FrustumCullParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frustum_cull_items_capacity = 256usize;
        let frustum_cull_items_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_frustum_cull_items"),
            size: (frustum_cull_items_capacity * std::mem::size_of::<FrustumCullItemGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let indirect_capacity = 256usize;
        let indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_draw_indirect"),
            size: (indirect_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let hiz_debug_readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_hiz_indirect_readback"),
            size: (indirect_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let frustum_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_frustum_cull_bg"),
            layout: &frustum_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frustum_cull_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: frustum_cull_items_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: indirect_buffer.as_entire_binding(),
                },
            ],
        });

        let (depth_texture, depth_view) = create_depth_texture(device, width, height, sample_count);
        let (depth_prepass_texture, depth_prepass_view) =
            create_depth_prepass_texture(device, width, height);
        let (mesh_blend_depth_texture, mesh_blend_depth_view) =
            create_depth_prepass_texture(device, width, height);
        let multimesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_multimesh_bg"),
            layout: &multimesh_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: multimesh_draw_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&mesh_blend_depth_view),
                },
            ],
        });
        let mesh_blend_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_mesh_blend_bg"),
            layout: &mesh_blend_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&mesh_blend_depth_view),
            }],
        });
        let (hiz_texture, hiz_mip_views, hiz_sample_view, hiz_mip_count, hiz_size) =
            create_hiz_texture(device, width, height);

        let hiz_copy_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_hiz_copy_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let hiz_downsample_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_hiz_downsample_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });
        let hiz_cull_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_hiz_cull_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let hiz_copy_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("perro_hiz_copy_pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("perro_hiz_copy_layout"),
                    bind_group_layouts: &[Some(&hiz_copy_bgl)],
                    immediate_size: 0,
                }),
            ),
            module: &create_hiz_depth_copy_shader_module(device),
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let hiz_downsample_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("perro_hiz_downsample_pipeline"),
                layout: Some(
                    &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("perro_hiz_downsample_layout"),
                        bind_group_layouts: &[Some(&hiz_downsample_bgl)],
                        immediate_size: 0,
                    }),
                ),
                module: &create_hiz_downsample_shader_module(device),
                entry_point: Some("cs_main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let hiz_cull_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("perro_hiz_cull_pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("perro_hiz_cull_layout"),
                    bind_group_layouts: &[Some(&hiz_cull_bgl)],
                    immediate_size: 0,
                }),
            ),
            module: &create_hiz_occlusion_cull_shader_module(device),
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let hiz_cull_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_hiz_cull_params"),
            size: std::mem::size_of::<HizCullParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hiz_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_hiz_cull_bg"),
            layout: &hiz_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: hiz_cull_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: frustum_cull_items_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&hiz_sample_view),
                },
            ],
        });

        let mut gpu = Self {
            color_format,
            camera_bgl,
            rigid_camera_bgl,
            multimesh_bgl,
            material_texture_bgl,
            shadow_bgl,
            sky_bgl,
            material_pipeline_layout: pipeline_layout,
            rigid_material_pipeline_layout: rigid_pipeline_layout,
            sky_pipeline,
            pipeline_rigid_culled,
            pipeline_rigid_double_sided,
            pipeline_rigid_blend_culled,
            pipeline_rigid_blend_double_sided,
            pipeline_rigid_unlit_culled,
            pipeline_rigid_unlit_double_sided,
            pipeline_rigid_unlit_blend_culled,
            pipeline_rigid_unlit_blend_double_sided,
            pipeline_rigid_toon_culled,
            pipeline_rigid_toon_double_sided,
            pipeline_rigid_toon_blend_culled,
            pipeline_rigid_toon_blend_double_sided,
            pipeline_rigid_overlay_culled,
            pipeline_rigid_overlay_double_sided,
            pipeline_culled,
            pipeline_double_sided,
            pipeline_blend_culled,
            pipeline_blend_double_sided,
            pipeline_unlit_culled,
            pipeline_unlit_double_sided,
            pipeline_unlit_blend_culled,
            pipeline_unlit_blend_double_sided,
            pipeline_toon_culled,
            pipeline_toon_double_sided,
            pipeline_toon_blend_culled,
            pipeline_toon_blend_double_sided,
            pipeline_overlay_culled,
            pipeline_overlay_double_sided,
            pipeline_depth_prepass_culled,
            pipeline_depth_prepass_double_sided,
            pipeline_depth_prepass_rigid_culled,
            pipeline_depth_prepass_rigid_double_sided,
            pipeline_shadow_depth_culled,
            pipeline_shadow_depth_double_sided,
            pipeline_shadow_depth_rigid_culled,
            pipeline_shadow_depth_rigid_double_sided,
            pipeline_multimesh_culled,
            pipeline_multimesh_double_sided,
            pipeline_multimesh_blend_culled,
            pipeline_multimesh_blend_double_sided,
            camera_buffer,
            camera_bind_group,
            rigid_camera_bind_group,
            shadow_camera_buffer,
            shadow_camera_bind_group,
            rigid_shadow_camera_bind_group,
            shadow_buffer,
            shadow_bind_group,
            _shadow_map_texture: shadow_map_texture,
            shadow_map_view,
            _shadow_map_sampler: shadow_map_sampler,
            mesh_blend_bgl,
            mesh_blend_bind_group,
            sky_buffer,
            sky_bind_group,
            _sky_noise_texture: sky_noise_texture,
            _sky_noise_view: sky_noise_view,
            _sky_noise_sampler: sky_noise_sampler,
            skeleton_buffer,
            skeleton_capacity,
            staged_skeletons: Vec::new(),
            custom_params_meta_buffer,
            custom_params_meta_capacity,
            staged_custom_params_meta: Vec::new(),
            custom_params_meta_uploaded: 0,
            custom_params_values_buffer,
            custom_params_values_capacity,
            staged_custom_params_values: Vec::new(),
            custom_params_values_uploaded: 0,
            staged_custom_params_dedupe: AHashMap::new(),
            staged_custom_params_key_scratch: Vec::new(),
            staged_custom_params_meta_scratch: Vec::new(),
            staged_custom_params_values_scratch: Vec::new(),
            material_fallback_texture: None,
            material_textures: AHashMap::new(),
            instance_transform_buffer,
            instance_transform_capacity,
            staged_instance_transforms: Vec::new(),
            instance_material_buffer,
            instance_material_capacity,
            staged_instance_materials: Vec::new(),
            rigid_instance_meta_buffer,
            rigid_instance_meta_capacity,
            staged_rigid_instance_meta: Vec::new(),
            skinned_instance_meta_buffer,
            skinned_instance_meta_capacity,
            staged_skinned_instance_meta: Vec::new(),
            multimesh_bind_group,
            multimesh_draw_params_buffer,
            multimesh_draw_params_capacity,
            staged_multimesh_draw_params: Vec::new(),
            multimesh_instance_buffer,
            multimesh_instance_capacity,
            staged_multimesh_instances: Vec::new(),
            multimesh_batches: Vec::new(),
            frustum_cull_enabled,
            frustum_cull_supported: frustum_cull_enabled,
            frustum_cull_pipeline,
            frustum_cull_bgl,
            frustum_cull_bind_group,
            frustum_cull_params_buffer,
            frustum_cull_items_buffer,
            frustum_cull_items_capacity,
            frustum_cull_staging: Vec::new(),
            indirect_buffer,
            indirect_capacity,
            indirect_staging: Vec::new(),
            frustum_gpu_inputs_valid: false,
            last_frustum_params: None,
            last_hiz_params: None,
            last_prepare_step_timing: Prepare3DStepTiming::default(),
            draw_batches: Vec::new(),
            has_shadow_casters: false,
            surface_entries_scratch: Vec::new(),
            mesh_blend_scratch: Vec::new(),
            last_draws: Vec::new(),
            last_draws_revision: u64::MAX,
            last_draw_instance_spans: Vec::new(),
            last_draw_instance_span_ranges: Vec::new(),
            last_scene: None,
            last_shadow_scene: None,
            last_shadow: None,
            shadow_pass_enabled: false,
            shadow_focus_center: Vec3::ZERO,
            shadow_focus_radius: 64.0,
            last_sky: None,
            last_sky_cloud_time_seconds: -1.0,
            sky_enabled: false,
            mesh_vertices: vertices,
            rigid_mesh_vertices: rigid_vertices,
            mesh_indices: indices,
            vertex_buffer,
            rigid_vertex_buffer,
            index_buffer,
            vertex_capacity,
            rigid_vertex_capacity,
            index_capacity,
            builtin_mesh_ranges,
            builtin_mesh_bounds,
            builtin_meshlets,
            custom_mesh_ranges: AHashMap::new(),
            depth_texture,
            depth_view,
            depth_prepass_texture,
            depth_prepass_view,
            mesh_blend_depth_texture,
            mesh_blend_depth_view,
            depth_size: (width.max(1), height.max(1)),
            gpu_occlusion_enabled,
            hiz_texture,
            hiz_mip_views,
            hiz_sample_view,
            hiz_size,
            hiz_mip_count,
            hiz_copy_pipeline,
            hiz_downsample_pipeline,
            hiz_cull_pipeline,
            hiz_copy_bgl,
            hiz_downsample_bgl,
            hiz_cull_bgl,
            hiz_copy_bind_group: None,
            hiz_downsample_bind_groups: Vec::new(),
            hiz_cull_params,
            hiz_cull_bind_group,
            hiz_debug_readback_buffer,
            pending_hiz_debug_count: 0,
            pending_hiz_debug_frustum_visible_est: 0,
            pending_hiz_debug_map_rx: None,
            debug_frustum_visible_est: 0,
            last_aspect: (width.max(1) as f32) / (height.max(1) as f32),
            last_proj_y_scale: projection_y_scale_from_projection(CameraProjectionState::default()),
            sample_count,
            occlusion_mode: occlusion_culling,
            meshlets_enabled,
            dev_meshlets,
            meshlet_debug_view,
            cpu_occlusion_enabled,
            last_total_meshlets: 0,
            last_total_drawn: 0,
            occlusion_frame: 0,
            occlusion_state: AHashMap::new(),
            occlusion_query_set: None,
            occlusion_query_capacity: 0,
            occlusion_resolve_buffer: None,
            occlusion_readback_buffer: None,
            occlusion_query_keys_this_frame: Vec::new(),
            pending_occlusion_query_keys: Vec::new(),
            pending_occlusion_query_count: 0,
            pending_occlusion_map_rx: None,
            last_occlusion_queried: 0,
            last_occlusion_visible: 0,
            last_occlusion_culled: 0,
            dirty_instance_spans_scratch: Vec::new(),
            merged_instance_spans_scratch: Vec::new(),
            dirty_cull_batch_spans_scratch: Vec::new(),
            debug_point_instances_scratch: Vec::new(),
            debug_edge_instances_scratch: Vec::new(),
            custom_pipelines: AHashMap::new(),
            custom_pipelines_rigid: AHashMap::new(),
            custom_pipeline_tokens: AHashMap::new(),
            next_custom_pipeline_token: 1,
        };
        gpu.rebuild_hiz_bind_groups(device);
        gpu
    }
}
