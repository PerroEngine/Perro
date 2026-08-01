use super::*;

#[cfg(not(target_arch = "wasm32"))]
fn frustum_cull_default(indirect_first_instance_enabled: bool) -> bool {
    indirect_first_instance_enabled
}

#[cfg(target_arch = "wasm32")]
fn frustum_cull_default(_: bool) -> bool {
    false
}

// Bind group for the indirect command-compaction compute pass:
// 0 params (uniform), 1 runs (ro), 2 culled commands (ro), 3 compacted
// commands (rw), 4 per-run survivor counts (rw).
pub(in super::super) fn indirect_compact_bgl_entries() -> [wgpu::BindGroupLayoutEntry; 5] {
    let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    [
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
        storage(1, true),
        storage(2, true),
        storage(3, false),
        storage(4, false),
    ]
}

// The compacted-command / count / run buffers are recreated together with the
// indirect buffer on every grow + shrink, so their descriptors live here.
// Contents are fully rewritten every frame (compaction pass + run upload), so
// no content-preserving copy is needed on resize.
pub(in super::super) fn create_indirect_compact_buffer(
    device: &wgpu::Device,
    capacity: usize,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_draw_indirect_compact"),
        size: (capacity.max(1) * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
        usage: wgpu::BufferUsages::INDIRECT
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

pub(in super::super) fn create_indirect_count_buffer(
    device: &wgpu::Device,
    capacity: usize,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_draw_indirect_counts"),
        size: (capacity.max(1) * std::mem::size_of::<u32>()) as u64,
        usage: wgpu::BufferUsages::INDIRECT
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

pub(in super::super) fn create_indirect_run_buffer(
    device: &wgpu::Device,
    capacity: usize,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_draw_indirect_runs"),
        size: (capacity.max(1) * std::mem::size_of::<IndirectRunGpu>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn multimesh_cull_bgl_entries() -> [wgpu::BindGroupLayoutEntry; 11] {
    let uniform = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    [
        uniform(0),        // frustum params (shared with rigid path)
        uniform(1),        // multimesh cull params
        storage(2, true),  // draw params
        storage(3, true),  // instances
        storage(4, true),  // per-instance batch id
        storage(5, true),  // per-batch cull records
        storage(6, false), // visible indices (write)
        storage(7, false), // indirect commands (write)
        storage(8, false), // per-batch atomic counters (write)
        uniform(9),        // hi-z cull params (cs_main_hiz only)
        wgpu::BindGroupLayoutEntry {
            binding: 10, // hi-z pyramid (cs_main_hiz only)
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
    ]
}

pub(in super::super) fn create_multimesh_shadow_counter_buffer(
    device: &wgpu::Device,
    capacity: usize,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_multimesh_shadow_cull_counters"),
        size: (capacity.max(1) * std::mem::size_of::<u32>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(in super::super) fn create_multimesh_shadow_indirect_buffer(
    device: &wgpu::Device,
    capacity: usize,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_multimesh_shadow_indirect"),
        size: (capacity.max(1) * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
        // COPY_SRC so the shadow-cull tests can read the per-layer survivor
        // counts back and check them against a CPU reference frustum test.
        usage: wgpu::BufferUsages::INDIRECT
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn create_multimesh_cull_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    frustum_params: &wgpu::Buffer,
    cull_params: &wgpu::Buffer,
    draws: &wgpu::Buffer,
    instances: &wgpu::Buffer,
    instance_batch: &wgpu::Buffer,
    cull_batches: &wgpu::Buffer,
    visible_indices: &wgpu::Buffer,
    indirect: &wgpu::Buffer,
    counters: &wgpu::Buffer,
    hiz_params: &wgpu::Buffer,
    hiz_sample_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_multimesh_cull_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: frustum_params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: cull_params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: draws.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: instances.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: instance_batch.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: cull_batches.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: visible_indices.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: indirect.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: counters.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: hiz_params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: wgpu::BindingResource::TextureView(hiz_sample_view),
            },
        ],
    })
}

impl Gpu3D {
    /// `mesh_arena` is the process-wide shared mesh arena: the new view mirrors
    /// its buffer handles instead of allocating (and re-uploading the builtin
    /// prefix into) a private arena set of its own.
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        config: Gpu3DConfig,
        pipelines: Arc<PipelineRegistry>,
        mesh_arena: &SharedMeshArena,
    ) -> Self {
        debug_assert_eq!(pipelines.color_format(), color_format);
        debug_assert_eq!(pipelines.sample_count(), config.sample_count.max(1));
        let Gpu3DConfig {
            sample_count,
            width,
            height,
            meshlets_enabled,
            // Decode-time only; the shared mesh arena owns this decision now.
            dev_meshlets: _,
            meshlet_debug_view,
            occlusion_culling,
            ssao,
            indirect_first_instance_enabled,
            multi_draw_indirect_enabled,
            multi_draw_indirect_count_enabled,
            texture_filter,
            shader_variant_mode,
            shadow_pcf_high,
        } = config;
        let (gpu_occlusion_enabled, cpu_occlusion_enabled) = occlusion_flags(occlusion_culling);
        // Shared mesh arena handles. Two of them (blend-shape deltas, packed-LOD
        // params) are bind group entries, so they must be in hand before the
        // camera / shadow / multimesh bind groups below are created.
        let mesh_arena_handles = mesh_arena.handles();
        let vertex_buffer = mesh_arena_handles.vertex_buffer.clone();
        let rigid_vertex_buffer = mesh_arena_handles.rigid_vertex_buffer.clone();
        let index_buffer = mesh_arena_handles.index_buffer.clone();
        let packed_lod_vertex_buffer = mesh_arena_handles.packed_lod_vertex_buffer.clone();
        let packed_lod_index_buffer = mesh_arena_handles.packed_lod_index_buffer.clone();
        let packed_lod_param_buffer = mesh_arena_handles.packed_lod_param_buffer.clone();
        let blend_shape_delta_buffer = mesh_arena_handles.blend_shape_delta_buffer.clone();
        let (shadow_map_size, shadow_spot_map_size, shadow_point_map_size) =
            default_shadow_map_sizes();
        let shadow_caster_debug_view = std::env::var_os("PERRO_DEBUG_SHADOW_CASTERS").is_some()
            || std::env::var_os("PERRO_SHADOW_DEBUG_CASTERS").is_some()
            || std::env::var_os("PERRO_SHADOW_DEBUG_CASCADES").is_some();
        let disable_meshlet_shadows = std::env::var_os("PERRO_DISABLE_MESHLET_SHADOWS").is_some();
        let bgls = pipelines.bgls();
        let camera_bgl = bgls.camera.clone();
        let water_camera_bgl = bgls.water_camera.clone();
        let rigid_camera_bgl = bgls.rigid_camera.clone();
        let multimesh_bgl = bgls.multimesh.clone();
        let material_texture_bgl = bgls.material_texture.clone();
        let ibl_bgl = bgls.ibl.clone();
        let shadow_bgl = bgls.shadow.clone();
        let sky_bgl = bgls.sky.clone();
        let mesh_blend_mask_id_bgl = bgls.mesh_blend_mask_id.clone();
        let mesh_blend_seam_bgl = bgls.mesh_blend_seam.clone();
        let ibl_maps = environment::create_environment_gpu_maps(device, queue);
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_camera3d_buffer"),
            size: std::mem::size_of::<Scene3DUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_camera_buffers: Vec<_> = (0..SHADOW_CAMERA_COUNT)
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_shadow_camera3d_buffer"),
                    size: std::mem::size_of::<Scene3DUniform>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let shadow_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_shadow3d_buffer"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // 1x1 single-layer placeholders: the real atlases are allocated lazily
        // in ensure_shadow_atlas_capacity once shadow-casting lights of a type
        // actually appear. Empty layer-view vecs keep every shadow render loop
        // clamped to zero until then.
        let (shadow_map_texture, shadow_map_view, _) =
            create_shadow_map_array_texture(device, "perro_ray_shadow_map", 1, 1);
        let shadow_layer_views = Vec::new();
        let (spot_shadow_map_texture, spot_shadow_map_view, _) =
            create_shadow_map_array_texture(device, "perro_spot_shadow_map", 1, 1);
        let spot_shadow_layer_views = Vec::new();
        let (point_shadow_map_texture, point_shadow_map_view, _) =
            create_shadow_map_array_texture(device, "perro_point_shadow_map", 1, 1);
        let point_shadow_layer_views = Vec::new();
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
            size: (skeleton_capacity * std::mem::size_of::<[[f32; 4]; 3]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let custom_params_meta_capacity = 1usize;
        let custom_params_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_custom_material_params_meta"),
            size: (custom_params_meta_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let custom_params_values_capacity = 1usize;
        let custom_params_values_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_custom_material_params_values"),
            size: (custom_params_values_capacity * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let blend_shape_weight_capacity = 1usize;
        let blend_shape_weight_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_blend_shape_weights"),
            size: std::mem::size_of::<f32>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let blend_shape_instance_meta_capacity = 1usize;
        let blend_shape_instance_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_blend_shape_instance_meta"),
            size: std::mem::size_of::<BlendShapeInstanceMetaGpu>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let decal_buffer_capacity = 8usize;
        let decal_buffer = create_decal_buffer(device, decal_buffer_capacity);
        let decal_texture_layers = decals::DECAL_INITIAL_LAYERS;
        let (decal_texture, decal_texture_view) =
            create_decal_texture_array(device, decal_texture_layers);
        let decal_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("perro_decal_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let multimesh_draw_params_capacity = 256usize;
        let multimesh_draw_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_draw_params"),
            size: (multimesh_draw_params_capacity * std::mem::size_of::<MultiMeshDrawParamGpu>())
                as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: blend_shape_delta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: blend_shape_weight_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: blend_shape_instance_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: decal_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&decal_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&decal_sampler),
                },
            ],
        });
        let water_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_water_camera3d_bg"),
            layout: &water_camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: blend_shape_delta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: blend_shape_weight_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: blend_shape_instance_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: packed_lod_param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: decal_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&decal_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&decal_sampler),
                },
            ],
        });
        let shadow_camera_bind_groups: Vec<_> = shadow_camera_buffers
            .iter()
            .map(|buffer| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("perro_shadow_camera3d_bg"),
                    layout: &camera_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
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
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: blend_shape_delta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: blend_shape_weight_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: blend_shape_instance_meta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: decal_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 8,
                            resource: wgpu::BindingResource::TextureView(&decal_texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 9,
                            resource: wgpu::BindingResource::Sampler(&decal_sampler),
                        },
                    ],
                })
            })
            .collect();
        let rigid_shadow_camera_bind_groups: Vec<_> = shadow_camera_buffers
            .iter()
            .map(|buffer| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("perro_shadow_camera3d_rigid_bg"),
                    layout: &rigid_camera_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: custom_params_meta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: custom_params_values_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: blend_shape_delta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: blend_shape_weight_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: blend_shape_instance_meta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: packed_lod_param_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: decal_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 8,
                            resource: wgpu::BindingResource::TextureView(&decal_texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 9,
                            resource: wgpu::BindingResource::Sampler(&decal_sampler),
                        },
                    ],
                })
            })
            .collect();
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&spot_shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&point_shadow_map_view),
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

        let mesh_blend_params_buffer = mesh_blend_screen::create_mesh_blend_params_buffer(device);
        let mesh_blend_mask_id_buffer =
            mesh_blend_screen::create_mesh_blend_mask_id_buffer(device, 16);
        let mesh_blend_mask_id_bind_group = mesh_blend_screen::create_mesh_blend_mask_id_bind_group(
            device,
            &mesh_blend_mask_id_bgl,
            &mesh_blend_mask_id_buffer,
        );
        // 1x1 placeholder; ensure_mesh_blend_targets upgrades it to full
        // resolution the first prepare that actually stages a mesh blend.
        let (mesh_blend_mask_texture, mesh_blend_mask_view) =
            mesh_blend_screen::create_mesh_blend_mask_texture(device, 1, 1);
        let instance_transform_capacity = 256usize;
        let instance_transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_transforms"),
            size: (instance_transform_capacity * std::mem::size_of::<TransformInstanceGpu>())
                as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let rigid_instance_meta_capacity = 256usize;
        let rigid_instance_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_rigid_meta"),
            size: (rigid_instance_meta_capacity * std::mem::size_of::<RigidInstanceMetaGpu>())
                as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let skinned_instance_meta_capacity = 256usize;
        let skinned_instance_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_skinned_meta"),
            size: (skinned_instance_meta_capacity * std::mem::size_of::<SkinnedInstanceMetaGpu>())
                as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let multimesh_instance_capacity = 256usize;
        let multimesh_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_instances"),
            size: (multimesh_instance_capacity * std::mem::size_of::<MultiMeshInstanceGpu>())
                as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let frustum_cull_enabled = frustum_cull_default(indirect_first_instance_enabled);
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
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
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
        let frustum_cull_static_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_frustum_cull_static"),
            size: (frustum_cull_items_capacity * std::mem::size_of::<FrustumCullStaticGpu>())
                as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let frustum_cull_dynamic_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_frustum_cull_dynamic"),
            size: (frustum_cull_items_capacity * std::mem::size_of::<FrustumCullDynamicGpu>())
                as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
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
        let hiz_debug_readback_buffer = HIZ_DEBUG_READBACK_ENABLED.then(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_hiz_indirect_readback"),
                size: (indirect_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
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
                    resource: frustum_cull_static_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: frustum_cull_dynamic_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: indirect_buffer.as_entire_binding(),
                },
            ],
        });

        // Indirect-count compaction: stream-compacts the culled indirect buffer
        // per state run so the main pass can draw with
        // multi_draw_indexed_indirect_count. Buffers track indirect_capacity.
        let indirect_compact_shader = create_indirect_compact_shader_module(device);
        let indirect_compact_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_indirect_compact_bgl"),
                entries: &indirect_compact_bgl_entries(),
            });
        let indirect_compact_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_indirect_compact_layout"),
                bind_group_layouts: &[Some(&indirect_compact_bgl)],
                immediate_size: 0,
            });
        let indirect_compact_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("perro_indirect_compact_pipeline"),
                layout: Some(&indirect_compact_layout),
                module: &indirect_compact_shader,
                entry_point: Some("cs_compact"),
                compilation_options: Default::default(),
                cache: None,
            });
        // One params buffer per dispatch slot: queue writes all land before the
        // first command of a submission, so two dispatches in the same frame
        // cannot share one uniform buffer.
        let indirect_compact_params_buffers: [wgpu::Buffer; INDIRECT_COMPACT_DISPATCHES] =
            std::array::from_fn(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_indirect_compact_params"),
                    size: std::mem::size_of::<IndirectCompactParamsGpu>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            });
        // Without the feature the compaction pass never runs, so the three
        // buffers stay at one slot: the bind group still needs valid bindings,
        // but a fallback adapter pays ~36 bytes instead of 108 bytes per slot.
        let indirect_compact_region_stride = if multi_draw_indirect_count_enabled {
            indirect_capacity
        } else {
            1
        };
        let indirect_compact_capacity = indirect_compact_region_stride * INDIRECT_COMPACT_REGIONS;
        let indirect_compact_buffer =
            create_indirect_compact_buffer(device, indirect_compact_capacity);
        let indirect_count_buffer = create_indirect_count_buffer(device, indirect_compact_capacity);
        let indirect_run_buffer = create_indirect_run_buffer(device, indirect_compact_capacity);
        let indirect_compact_bind_groups: [wgpu::BindGroup; INDIRECT_COMPACT_DISPATCHES] =
            std::array::from_fn(|dispatch| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("perro_indirect_compact_bg"),
                    layout: &indirect_compact_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: indirect_compact_params_buffers[dispatch].as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: indirect_run_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: indirect_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: indirect_compact_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: indirect_count_buffer.as_entire_binding(),
                        },
                    ],
                })
            });

        // Multimesh GPU cull resources (item 1).
        let multimesh_cull_shader = create_multimesh_cull_shader_module(device);
        let multimesh_cull_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_multimesh_cull_bgl"),
                entries: &multimesh_cull_bgl_entries(),
            });
        let multimesh_cull_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_multimesh_cull_layout"),
                bind_group_layouts: &[Some(&multimesh_cull_bgl)],
                immediate_size: 0,
            });
        let multimesh_cull_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("perro_multimesh_cull_pipeline"),
                layout: Some(&multimesh_cull_layout),
                module: &multimesh_cull_shader,
                entry_point: Some("cs_main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let multimesh_cull_finalize_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("perro_multimesh_cull_finalize_pipeline"),
                layout: Some(&multimesh_cull_layout),
                module: &multimesh_cull_shader,
                entry_point: Some("cs_finalize"),
                compilation_options: Default::default(),
                cache: None,
            });
        let multimesh_cull_hiz_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("perro_multimesh_cull_hiz_pipeline"),
                layout: Some(&multimesh_cull_layout),
                module: &multimesh_cull_shader,
                entry_point: Some("cs_main_hiz"),
                compilation_options: Default::default(),
                cache: None,
            });
        let multimesh_cull_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_cull_params"),
            size: std::mem::size_of::<MultiMeshCullParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let multimesh_cull_instance_capacity = 256usize;
        let multimesh_cull_batch_capacity = 64usize;
        let multimesh_instance_batch_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_instance_batch"),
            size: (multimesh_cull_instance_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let multimesh_cull_batch_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_cull_batches"),
            size: (multimesh_cull_batch_capacity * std::mem::size_of::<MultiMeshCullBatchGpu>())
                as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let multimesh_visible_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_visible_indices"),
            size: (multimesh_cull_instance_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let multimesh_shadow_identity_capacity = multimesh_cull_instance_capacity;
        let multimesh_shadow_identity_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_shadow_identity"),
            size: (multimesh_shadow_identity_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let multimesh_shadow_cull_counter_buffer =
            create_multimesh_shadow_counter_buffer(device, multimesh_cull_batch_capacity);
        let multimesh_shadow_indirect_buffer =
            create_multimesh_shadow_indirect_buffer(device, multimesh_cull_batch_capacity);
        let multimesh_shadow_cull_plane_buffers = (0..SHADOW_CAMERA_COUNT)
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_multimesh_shadow_cull_params"),
                    size: std::mem::size_of::<FrustumCullParamsGpu>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect::<Vec<_>>();
        let multimesh_cull_counter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_cull_counters"),
            size: (multimesh_cull_batch_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let multimesh_indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_indirect"),
            size: (multimesh_cull_batch_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>())
                as u64,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (depth_prepass_texture, depth_prepass_view) =
            create_depth_prepass_texture(device, width, height);
        let (depth_texture, depth_view) = create_scene_depth_target(
            device,
            width,
            height,
            sample_count,
            &depth_prepass_texture,
            &depth_prepass_view,
        );
        let ssao_fallback_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("perro_ssao_fallback"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: ssao::SSAO_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[255],
        );
        let ssao_fallback_view =
            ssao_fallback_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ssao_pass = (ssao != crate::SsaoQuality::Off)
            .then(|| ssao::SsaoPass::new(device, width, height, &depth_prepass_view, ssao));
        // 1x1 placeholder (see ensure_mesh_blend_targets); it stays bound as a
        // dummy while no mesh blend is active.
        let (mesh_blend_depth_texture, mesh_blend_depth_view) =
            create_depth_prepass_texture(device, 1, 1);
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: blend_shape_delta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: blend_shape_weight_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: blend_shape_instance_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: custom_params_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: custom_params_values_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: multimesh_visible_index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: multimesh_instance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: decal_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&decal_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::Sampler(&decal_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(
                        ssao_pass
                            .as_ref()
                            .map(ssao::SsaoPass::view)
                            .unwrap_or(&ssao_fallback_view),
                    ),
                },
            ],
        });
        let shadow_multimesh_bind_groups =
            buffers::build_shadow_multimesh_bind_groups(buffers::ShadowMultimeshBgArgs {
                device,
                multimesh_bgl: &multimesh_bgl,
                shadow_camera_buffers: &shadow_camera_buffers,
                multimesh_draw_params_buffer: &multimesh_draw_params_buffer,
                mesh_blend_depth_view: &mesh_blend_depth_view,
                blend_shape_delta_buffer: &blend_shape_delta_buffer,
                blend_shape_weight_buffer: &blend_shape_weight_buffer,
                blend_shape_instance_meta_buffer: &blend_shape_instance_meta_buffer,
                custom_params_meta_buffer: &custom_params_meta_buffer,
                custom_params_values_buffer: &custom_params_values_buffer,
                shadow_identity_buffer: &multimesh_shadow_identity_buffer,
                multimesh_instance_buffer: &multimesh_instance_buffer,
                decal_buffer: &decal_buffer,
                decal_texture_view: &decal_texture_view,
                decal_sampler: &decal_sampler,
                ssao_view: ssao_pass
                    .as_ref()
                    .map(ssao::SsaoPass::view)
                    .unwrap_or(&ssao_fallback_view),
            });
        let ssao_view = ssao_pass
            .as_ref()
            .map(ssao::SsaoPass::view)
            .unwrap_or(&ssao_fallback_view);
        let ibl_bind_group = environment::create_environment_bind_group(
            device,
            &ibl_bgl,
            &ibl_maps,
            &mesh_blend_depth_view,
            ssao_view,
        );
        // GPU occlusion off => the pyramid is never dispatched or sampled for
        // culling; keep a 1x1 stand-in bound instead of a full mip chain.
        let (hiz_width, hiz_height) = if gpu_occlusion_enabled {
            (width, height)
        } else {
            (1, 1)
        };
        let (hiz_texture, hiz_mip_views, hiz_sample_view, hiz_mip_count, hiz_size) =
            create_hiz_texture(device, hiz_width, hiz_height);

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
        // SPD downsampler needs 1 sampled src + HIZ_SPD_MIPS storage writes in a
        // single stage. Gate on the device supporting that many storage textures
        // per stage (downlevel gl caps at 4); otherwise fall back to per-mip.
        let hiz_spd_supported =
            device.limits().max_storage_textures_per_shader_stage >= HIZ_SPD_MIPS;
        let hiz_spd_bgl = {
            let mut entries = vec![wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }];
            for mip in 0..HIZ_SPD_MIPS {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding: 1 + mip,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                });
            }
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: 1 + HIZ_SPD_MIPS,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_hiz_spd_bgl"),
                entries: &entries,
            })
        };
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
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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

        // GPU occlusion off => the hi-z passes never dispatch (hiz_active
        // requires gpu_occlusion_enabled and occlusion_mode is fixed at
        // construction), so skip these 4 shader compiles at startup entirely.
        let hiz_copy_pipeline = gpu_occlusion_enabled.then(|| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
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
            })
        });
        let hiz_downsample_pipeline = gpu_occlusion_enabled.then(|| {
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
            })
        });
        let hiz_spd_pipeline = gpu_occlusion_enabled.then(|| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("perro_hiz_spd_pipeline"),
                layout: Some(
                    &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("perro_hiz_spd_layout"),
                        bind_group_layouts: &[Some(&hiz_spd_bgl)],
                        immediate_size: 0,
                    }),
                ),
                module: &create_hiz_downsample_spd_shader_module(device),
                entry_point: Some("cs_main"),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        let hiz_cull_pipeline = gpu_occlusion_enabled.then(|| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
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
            })
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
                    resource: frustum_cull_static_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: frustum_cull_dynamic_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&hiz_sample_view),
                },
            ],
        });
        let multimesh_cull_bind_group = create_multimesh_cull_bind_group(
            device,
            &multimesh_cull_bgl,
            &frustum_cull_params_buffer,
            &multimesh_cull_params_buffer,
            &multimesh_draw_params_buffer,
            &multimesh_instance_buffer,
            &multimesh_instance_batch_buffer,
            &multimesh_cull_batch_buffer,
            &multimesh_visible_index_buffer,
            &multimesh_indirect_buffer,
            &multimesh_cull_counter_buffer,
            &hiz_cull_params,
            &hiz_sample_view,
        );

        let mut gpu = Self {
            color_format,
            camera_bgl,
            water_camera_bgl,
            rigid_camera_bgl,
            multimesh_bgl,
            material_texture_bgl,
            ibl_bgl,
            shadow_bgl,
            pipelines,
            custom_sky_pipelines: AHashMap::new(),
            active_sky_pipeline_key: None,
            screen_blend_supported: true,
            mesh_blend_screen_active: false,
            mesh_blend_mask_batch_entries: Vec::new(),
            _mesh_blend_mask_texture: mesh_blend_mask_texture,
            mesh_blend_mask_view,
            mesh_blend_mask_id_bgl,
            mesh_blend_mask_id_buffer,
            mesh_blend_mask_id_bind_group,
            mesh_blend_mask_id_capacity: 16,
            mesh_blend_params_buffer,
            mesh_blend_seam_bgl,
            mesh_blend_seam_bind_group: None,
            mesh_blend_scene_copy: None,
            camera_buffer,
            camera_bind_group,
            water_camera_bind_group,
            rigid_camera_bind_group,
            shadow_camera_buffers,
            shadow_camera_bind_groups,
            rigid_shadow_camera_bind_groups,
            shadow_multimesh_bind_groups,
            multimesh_shadow_identity_buffer,
            multimesh_shadow_identity_capacity,
            multimesh_shadow_identity_uploaded_len: 0,
            multimesh_shadow_cull_plane_buffers,
            // Filled by `rebuild_multimesh_shadow_cull_bind_groups` below, once
            // every buffer this borrows is owned by `gpu`.
            multimesh_shadow_cull_bind_groups: Vec::new(),
            multimesh_shadow_indirect_buffer,
            multimesh_shadow_cull_counter_buffer,
            last_shadow_cull_planes: Vec::new(),
            multimesh_shadow_cull_active: false,
            shadow_buffer,
            shadow_bind_group,
            _shadow_map_texture: shadow_map_texture,
            shadow_map_view,
            shadow_layer_views,
            ray_shadow_layers_allocated: 0,
            _spot_shadow_map_texture: spot_shadow_map_texture,
            spot_shadow_map_view,
            spot_shadow_layer_views,
            spot_shadow_layers_allocated: 0,
            _point_shadow_map_texture: point_shadow_map_texture,
            point_shadow_map_view,
            point_shadow_layer_views,
            point_shadow_layers_allocated: 0,
            shadow_map_sampler,
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
            material_normal_fallback_texture: None,
            material_fallback_bind_group: None,
            material_textures: AHashMap::new(),
            stream_texture_slots: AHashSet::new(),
            material_texture_bind_groups: AHashMap::new(),
            ibl_maps,
            ibl_bind_group,
            custom_material_texture_slots: AHashMap::new(),
            next_custom_material_texture_slot: CUSTOM_MATERIAL_TEXTURE_SLOT_BASE,
            texture_filter,
            instance_transform_buffer,
            instance_transform_capacity,
            staged_instance_transforms: Vec::new(),
            rigid_instance_meta_buffer,
            rigid_instance_meta_capacity,
            staged_rigid_instance_meta: Vec::new(),
            skinned_instance_meta_buffer,
            skinned_instance_meta_capacity,
            staged_skinned_instance_meta: Vec::new(),
            blend_shape_delta_buffer,
            blend_shape_weight_buffer,
            blend_shape_weight_capacity,
            staged_blend_shape_weights: Vec::new(),
            blend_shape_instance_meta_buffer,
            blend_shape_instance_meta_capacity,
            staged_blend_shape_instance_meta: Vec::new(),
            packed_lod_param_buffer,
            decal_buffer,
            decal_buffer_capacity,
            decal_texture,
            decal_texture_view,
            decal_texture_layers,
            decal_sampler,
            decal_layer_by_texture: AHashMap::new(),
            decal_sources_pending: false,
            decal_count: 0,
            last_decals_revision: u64::MAX,
            multimesh_bind_group,
            multimesh_draw_params_buffer,
            multimesh_draw_params_capacity,
            staged_multimesh_draw_params: Vec::new(),
            multimesh_instance_buffer,
            multimesh_instance_capacity,
            staged_multimesh_instances: Vec::new(),
            multimesh_batches: Vec::new(),
            staged_multimesh_blend_meta: Vec::new(),
            staged_multimesh_blend_weights: Vec::new(),
            multimesh_blend_meta_base: 0,
            multimesh_blend_weight_base: 0,
            multimesh_blend_tail_revision: 0,
            uploaded_multimesh_blend_meta: None,
            uploaded_multimesh_blend_weights: None,
            staged_multimesh_custom_params_meta: Vec::new(),
            staged_multimesh_custom_params_values: Vec::new(),
            staged_multimesh_custom_params_dedupe: AHashMap::new(),
            staged_multimesh_custom_params_entry_starts: Vec::new(),
            multimesh_custom_params_meta_base: 0,
            multimesh_custom_params_values_base: 0,
            multimesh_custom_params_tail_revision: 0,
            uploaded_multimesh_custom_params_meta: None,
            uploaded_multimesh_custom_params_values: None,
            multimesh_staging_cache: Vec::new(),
            multimesh_staging_cache_valid: false,
            multimesh_staging_cache_camera: [0.0; 3],
            multimesh_staging_reuse_count: 0,
            multimesh_cull_pipeline,
            multimesh_cull_finalize_pipeline,
            multimesh_cull_hiz_pipeline,
            multimesh_cull_bgl,
            multimesh_cull_bind_group,
            multimesh_cull_params_buffer,
            multimesh_instance_batch_buffer,
            staged_multimesh_instance_batch: Vec::new(),
            multimesh_cull_batch_buffer,
            staged_multimesh_cull_batches: Vec::new(),
            multimesh_visible_index_buffer,
            staged_multimesh_visible_identity: Vec::new(),
            multimesh_cull_counter_buffer,
            multimesh_indirect_buffer,
            multimesh_indirect_staging: Vec::new(),
            multimesh_cull_instance_capacity,
            multimesh_cull_batch_capacity,
            multimesh_cull_active: false,
            last_multimesh_cull_params: None,
            shadow_pcf_high,
            shadow_map_size,
            shadow_spot_map_size,
            shadow_point_map_size,
            last_uploaded_multimesh_instances_hash: None,
            last_uploaded_multimesh_draw_params_hash: None,
            last_uploaded_multimesh_cull_batches: Vec::new(),
            last_uploaded_multimesh_indirect: Vec::new(),
            multimesh_instance_batch_uploaded_len: 0,
            last_uploaded_multimesh_identity_len: 0,
            multimesh_identity_primed: false,
            frustum_cull_enabled,
            frustum_cull_supported: frustum_cull_enabled,
            multi_draw_indirect_enabled,
            multi_draw_indirect_count_enabled,
            multi_draw_indirect_count_active: false,
            multi_draw_indirect_count_prepass_active: false,
            multi_draw_indirect_count_blend_depth_active: false,
            frustum_cull_pipeline,
            frustum_cull_bgl,
            frustum_cull_bind_group,
            frustum_cull_params_buffer,
            frustum_cull_static_buffer,
            frustum_cull_dynamic_buffer,
            frustum_cull_items_capacity,
            frustum_cull_static_staging: Vec::new(),
            frustum_cull_dynamic_staging: Vec::new(),
            indirect_buffer,
            indirect_capacity,
            indirect_staging: Vec::new(),
            last_uploaded_indirect_hash: None,
            indirect_counts_gpu_dirty: false,
            indirect_compact_pipeline,
            indirect_compact_bgl,
            indirect_compact_bind_groups,
            indirect_compact_params_buffers,
            indirect_compact_buffer,
            indirect_count_buffer,
            indirect_run_buffer,
            indirect_compact_region_stride,
            indirect_run_plan: Vec::new(),
            indirect_run_plan_prepass: 0..0,
            indirect_run_plan_blend_depth: 0..0,
            indirect_run_plan_main: 0..0,
            indirect_run_plan_groups: [0..0, 0..0, 0..0],
            last_uploaded_indirect_runs: Vec::new(),
            last_indirect_compact_params: [None; INDIRECT_COMPACT_DISPATCHES],
            indirect_planned_command_count: 0,
            indirect_planned_command_count_prepass: 0,
            indirect_planned_command_count_blend_depth: 0,
            frustum_gpu_inputs_valid: false,
            last_frustum_params: None,
            last_hiz_params: None,
            last_prepare_step_timing: Prepare3DStepTiming::default(),
            prepare_full_rebuild_count: 0,
            draw_batches: Vec::new(),
            opaque_batch_indices: Vec::new(),
            alpha_batch_indices: Vec::new(),
            mesh_blend_batch_indices: Vec::new(),
            overlay_batch_indices: Vec::new(),
            shadow_batch_indices: Vec::new(),
            depth_prepass_batch_indices: Vec::new(),
            mesh_blend_depth_batch_indices: Vec::new(),
            has_shadow_casters: false,
            mesh_blend_depth_active: false,
            surface_entries_scratch: Vec::new(),
            mesh_blend_scratch: Vec::new(),
            bleed_emitters_scratch: Vec::new(),
            bleed_occluders_scratch: Vec::new(),
            bleed_multimesh_bounds_scratch: Vec::new(),
            mesh_blend_source_receivers: Vec::new(),
            mesh_blend_receiver_indices: Vec::new(),
            mesh_blend_batch_spheres_scratch: Vec::new(),
            mesh_blend_prev_spheres: Vec::new(),
            last_draws: Vec::new(),
            last_draws_revision: u64::MAX,
            last_prepare_upload_stats: FrameUploadStats::default(),
            last_uploaded_instance_transforms_hash: None,
            last_uploaded_rigid_instance_meta_hash: None,
            last_uploaded_skinned_instance_meta_hash: None,
            last_uploaded_blend_shape_weights_hash: None,
            last_uploaded_blend_shape_meta_hash: None,
            last_uploaded_skeletons_hash: None,
            last_uploaded_frustum_cull_static_hash: None,
            last_uploaded_frustum_cull_dynamic_hash: None,
            staged_has_skinned: false,
            last_draw_instance_spans: Vec::new(),
            last_draw_instance_span_ranges: Vec::new(),
            last_draw_multimesh_param_ranges: Vec::new(),
            last_draw_skeleton_bounds: Vec::new(),
            last_draw_blend_meta_bounds: Vec::new(),
            compact_instance_owner_scratch: Vec::new(),
            compact_dst_transforms_scratch: Vec::new(),
            compact_dst_rigid_meta_scratch: Vec::new(),
            compact_dst_skinned_meta_scratch: Vec::new(),
            compact_dst_batches_scratch: Vec::new(),
            compact_spans_per_draw_scratch: Vec::new(),
            compact_src_region_dedup_scratch: AHashMap::default(),
            compact_multimesh_dst_instances_scratch: Vec::new(),
            compact_multimesh_dst_batches_scratch: Vec::new(),
            multimesh_pose_pack_cache: AHashMap::default(),
            multimesh_pose_pack_cache_seen: ahash::AHashSet::default(),
            last_scene: None,
            last_shadow_scenes: vec![None; SHADOW_CAMERA_COUNT],
            shadow_camera_frustums: vec![[Vec4::ZERO; 6]; SHADOW_CAMERA_COUNT],
            last_shadow: None,
            shadow_layer_valid: Vec::new(),
            shadow_cascade_defer_age: [0; MAX_SHADOW_RAY_CASCADES],
            shadow_cascade_defer_count: 0,
            shadow_cascade_light_dir: [[0.0; 3]; MAX_SHADOW_RAY_CASCADES],
            shadow_casters_dirty: true,
            last_shadow_caster_key: None,
            last_shadow_input_camera: None,
            last_shadow_input_lighting: None,
            last_shadow_input_key: None,
            shadow_setup_run_count: 0,
            shadow_cull_scratch: Vec::new(),
            shadow_pass_enabled: false,
            ray_shadow_enabled: false,
            spot_shadow_count: 0,
            point_shadow_count: 0,
            shadow_focus_center: Vec3::ZERO,
            shadow_focus_radius: 64.0,
            last_sky: None,
            last_sky_time_seconds: -1.0,
            sky_enabled: false,
            vertex_buffer,
            rigid_vertex_buffer,
            packed_lod_vertex_buffer,
            index_buffer,
            packed_lod_index_buffer,
            mesh_arena_buffer_generation: mesh_arena_handles.buffer_generation,
            mesh_arena_bind_generation: mesh_arena_handles.bind_generation,
            mesh_arena_layout_generation: mesh_arena_handles.layout_generation,
            shrink: Gpu3DShrink::default(),
            depth_texture,
            depth_view,
            depth_prepass_texture,
            depth_prepass_view,
            depth_prepass_view_generation: next_depth_prepass_view_generation(),
            taa_base_view_proj: Mat4::IDENTITY,
            taa_base_inv_view_proj: Mat4::IDENTITY,
            taa_jitter_applied: false,
            water_private_depth: None,
            ssao_pass,
            _ssao_fallback_texture: ssao_fallback_texture,
            ssao_fallback_view,
            ssao_quality: ssao,
            mesh_blend_depth_texture,
            mesh_blend_depth_view,
            depth_size: (width.max(1), height.max(1)),
            unified_depth_active: false,
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
            hiz_spd_supported,
            hiz_spd_pipeline,
            hiz_spd_bgl,
            hiz_spd_bind_groups: Vec::new(),
            hiz_spd_params_buffers: Vec::new(),
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
            meshlet_debug_view,
            shadow_caster_debug_view,
            disable_meshlet_shadows,
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
            transform_dirty_spans_snapshot: Vec::new(),
            dirty_cull_batch_spans_scratch: Vec::new(),
            dirty_animation_spans_scratch: Vec::new(),
            merged_animation_spans_scratch: Vec::new(),
            transform_only_kinds_scratch: Vec::new(),
            debug_point_instances_scratch: Vec::new(),
            debug_edge_instances_scratch: Vec::new(),
            camera_bind_group_generation: 1,
            multimesh_bind_group_generation: 1,
            shadow_layer_shrink: shadows::ShadowLayerShrink::default(),
            decal_layer_shrink: shadows::LayerShrinkTracker::default(),
            rendered_since_gc_tick: false,
            idle_gc_ticks: 0,
            perf_counters: RenderPerfCounters::default(),
            pass_counters: PassCounters::default(),
            shadow_timestamps_written: false,
            mesh_blend_seam_region: SeamRegion::default(),
            custom_pipelines: AHashMap::new(),
            custom_pipelines_rigid: AHashMap::new(),
            custom_pipelines_multimesh: AHashMap::new(),
            builtin_variant_pipelines: AHashMap::new(),
            pipeline_gc_tick: 0,
            pipeline_compiles: 0,
            render_paths_drawn: 0,
            base_families_warmed: false,
            shader_variant_mode,
            custom_pipeline_tokens: AHashMap::new(),
            custom_shader_sources: AHashMap::new(),
            custom_pipeline_vertex_hooks: AHashMap::new(),
            next_custom_pipeline_token: 1,
        };
        gpu.rebuild_hiz_bind_groups(device);
        gpu.rebuild_multimesh_shadow_cull_bind_groups(device);
        gpu
    }
}

#[cfg(test)]
mod tests {
    use super::frustum_cull_default;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_keeps_frustum_cull_support() {
        assert!(frustum_cull_default(true));
        assert!(!frustum_cull_default(false));
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn wasm_disables_frustum_cull_support() {
        assert!(!frustum_cull_default(true));
        assert!(!frustum_cull_default(false));
    }
}
