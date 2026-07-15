// Screen-space mesh blend: an id-mask pass over blend participants plus a
// fullscreen seam pass that cross-samples scene color across mask boundaries.
// Sources tagged screen_blending render fully opaque; all softening happens
// here, so nothing ghosts through geometry.

use super::*;

pub(super) const MESH_BLEND_MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Uint;
// Dynamic-offset stride for the per-batch mask id uniform.
const MASK_ID_STRIDE: u64 = 256;
const MESH_BLEND_ID_PARAM_COUNT: usize = 256;
// Ids 1..=127 are sources, 128..=255 receivers (mirrored in the seam shader).
const RECEIVER_ID_BASE: u32 = 128;

pub(super) fn create_mesh_blend_mask_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_mesh_blend_mask"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: MESH_BLEND_MASK_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

pub(super) fn create_mesh_blend_mask_id_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_mesh_blend_mask_id_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: Some(
                    std::num::NonZeroU64::new(16).expect("mask id uniform size"),
                ),
            },
            count: None,
        }],
    })
}

pub(super) fn create_mesh_blend_mask_id_buffer(
    device: &wgpu::Device,
    entries: u64,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_mesh_blend_mask_id_buffer"),
        size: entries.max(1) * MASK_ID_STRIDE,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(super) fn create_mesh_blend_mask_id_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_mesh_blend_mask_id_bg"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: Some(std::num::NonZeroU64::new(16).expect("mask id uniform size")),
            }),
        }],
    })
}

pub(super) fn create_mesh_blend_seam_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_mesh_blend_seam_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Scene3DUniform>() as u64)
                            .expect("scene uniform size"),
                    ),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

pub(super) fn create_mesh_blend_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_mesh_blend_id_params"),
        size: (MESH_BLEND_ID_PARAM_COUNT * 16) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(super) fn create_mesh_blend_seam_pipeline(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = create_mesh_blend_screen_shader_module(device);
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("perro_mesh_blend_seam_layout"),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_mesh_blend_seam_pipeline"),
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
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn mask_depth_stencil() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_PREPASS_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn mask_color_target() -> Option<wgpu::ColorTargetState> {
    Some(wgpu::ColorTargetState {
        format: MESH_BLEND_MASK_FORMAT,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    })
}

pub(super) fn create_mesh_blend_mask_pipeline_rigid(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_mesh_blend_mask_pipeline_rigid"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RigidMeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x3,
                    }],
                }),
                Some(rigid_path::rigid_instance_transform_layout()),
                Some(rigid_path::rigid_meta_layout()),
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_mask"),
            targets: &[mask_color_target()],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(mask_depth_stencil()),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub(super) fn create_mesh_blend_mask_pipeline_rigid_packed_lod(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_mesh_blend_mask_pipeline_rigid_packed_lod"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                Some(rigid_path::rigid_packed_lod_vertex_layout()),
                Some(rigid_path::rigid_instance_transform_layout()),
                Some(rigid_path::rigid_meta_layout()),
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_mask"),
            targets: &[mask_color_target()],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(mask_depth_stencil()),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub(super) fn create_mesh_blend_mask_pipeline_skinned(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_mesh_blend_mask_pipeline_skinned"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SkinnedMeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 28,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Uint16x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 36,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Unorm8x4,
                        },
                    ],
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TransformInstanceGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SkinnedInstanceMetaGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 7,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 8,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 11,
                            format: wgpu::VertexFormat::Uint32x4,
                        },
                    ],
                }),
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_mask"),
            targets: &[mask_color_target()],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(mask_depth_stencil()),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

// Quantized packed params -> floats the seam shader expects; keep in sync
// with pack_mesh_blend_params / decode_mesh_blend_params.
fn unpack_mesh_blend_params(packed: u32) -> [f32; 4] {
    let lane = |shift: u32| ((packed >> shift) & 0xff) as f32 / 255.0;
    [
        lane(0) * 16.0,
        lane(8) * 16.0,
        lane(16),
        lane(24) * 64.0 * 0.05,
    ]
}

impl Gpu3D {
    pub fn set_screen_blend_supported(&mut self, supported: bool) {
        self.screen_blend_supported = supported;
    }

    pub fn screen_blend_active(&self) -> bool {
        self.mesh_blend_screen_active
    }

    // Assigns blend ids per participating batch and uploads the id lookup
    // table + per-batch mask uniforms. Runs after rebuild_batch_views.
    pub(super) fn prepare_mesh_blend_screen(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.mesh_blend_mask_batch_entries.clear();
        let any_screen = self
            .draw_batches
            .iter()
            .any(|batch| batch.mesh_blend_screen)
            || self
                .multimesh_batches
                .iter()
                .any(|batch| batch.mesh_blend_screen);
        self.mesh_blend_screen_active = any_screen;
        if !any_screen {
            return;
        }
        // Receivers carry no params of their own; give them the widest
        // source's tuning so both sides of a seam agree.
        let mut receiver_params = [0.0f32; 4];
        for batch in &self.draw_batches {
            if !batch.mesh_blend_screen {
                continue;
            }
            let params = unpack_mesh_blend_params(batch.mesh_blend_params);
            if params[0] > receiver_params[0] {
                receiver_params = params;
            }
        }
        for batch in &self.multimesh_batches {
            if !batch.mesh_blend_screen {
                continue;
            }
            let params = unpack_mesh_blend_params(batch.mesh_blend_params);
            if params[0] > receiver_params[0] {
                receiver_params = params;
            }
        }
        let mut id_params = [[0.0f32; 4]; MESH_BLEND_ID_PARAM_COUNT];
        let mut next_source_id: u32 = 1;
        let mut next_receiver_id: u32 = RECEIVER_ID_BASE;
        for (index, batch) in self.draw_batches.iter().enumerate() {
            if batch.mesh_blend_screen {
                let id = next_source_id;
                next_source_id = if next_source_id + 1 >= RECEIVER_ID_BASE {
                    1
                } else {
                    next_source_id + 1
                };
                id_params[id as usize] = unpack_mesh_blend_params(batch.mesh_blend_params);
                self.mesh_blend_mask_batch_entries
                    .push(MeshBlendMaskEntry::Draw {
                        batch_index: index,
                        id,
                    });
            } else if batch.mesh_blend_depth
                && !batch.mesh_blend
                && !batch.draw_on_top
                && batch.alpha_mode != 2
            {
                let id = next_receiver_id;
                next_receiver_id = if next_receiver_id == 255 {
                    RECEIVER_ID_BASE
                } else {
                    next_receiver_id + 1
                };
                id_params[id as usize] = receiver_params;
                self.mesh_blend_mask_batch_entries
                    .push(MeshBlendMaskEntry::Draw {
                        batch_index: index,
                        id,
                    });
            }
        }
        for (index, batch) in self.multimesh_batches.iter().enumerate() {
            if batch.mesh_blend_screen {
                let id = next_source_id;
                next_source_id = if next_source_id + 1 >= RECEIVER_ID_BASE {
                    1
                } else {
                    next_source_id + 1
                };
                id_params[id as usize] = unpack_mesh_blend_params(batch.mesh_blend_params);
                self.mesh_blend_mask_batch_entries
                    .push(MeshBlendMaskEntry::MultiMesh {
                        batch_index: index,
                        id,
                    });
            } else if batch.mesh_blend_depth && !batch.mesh_blend {
                let id = next_receiver_id;
                next_receiver_id = if next_receiver_id == 255 {
                    RECEIVER_ID_BASE
                } else {
                    next_receiver_id + 1
                };
                id_params[id as usize] = receiver_params;
                self.mesh_blend_mask_batch_entries
                    .push(MeshBlendMaskEntry::MultiMesh {
                        batch_index: index,
                        id,
                    });
            }
        }
        queue.write_buffer(
            &self.mesh_blend_params_buffer,
            0,
            bytemuck::cast_slice(&id_params),
        );
        let entries = self.mesh_blend_mask_batch_entries.len() as u64;
        if entries > self.mesh_blend_mask_id_capacity {
            let mut capacity = self.mesh_blend_mask_id_capacity.max(16);
            while capacity < entries {
                capacity *= 2;
            }
            self.mesh_blend_mask_id_buffer = create_mesh_blend_mask_id_buffer(device, capacity);
            self.mesh_blend_mask_id_bind_group = create_mesh_blend_mask_id_bind_group(
                device,
                &self.mesh_blend_mask_id_bgl,
                &self.mesh_blend_mask_id_buffer,
            );
            self.mesh_blend_mask_id_capacity = capacity;
        }
        let mut staged = vec![0u8; (entries * MASK_ID_STRIDE) as usize];
        for (slot, entry) in self.mesh_blend_mask_batch_entries.iter().enumerate() {
            let id = match *entry {
                MeshBlendMaskEntry::Draw { id, .. } | MeshBlendMaskEntry::MultiMesh { id, .. } => {
                    id
                }
            };
            let offset = slot * MASK_ID_STRIDE as usize;
            staged[offset..offset + 4].copy_from_slice(&id.to_le_bytes());
        }
        queue.write_buffer(&self.mesh_blend_mask_id_buffer, 0, &staged);
    }

    // Renders participant blend ids into the mask, depth-tested against the
    // depth prepass so only visible surfaces tag pixels.
    pub(super) fn encode_mesh_blend_mask_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frustum_cull_active: bool,
    ) {
        if !self.mesh_blend_screen_active || self.mesh_blend_mask_batch_entries.is_empty() {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_mesh_blend_mask_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.mesh_blend_mask_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_prepass_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let mut current_state: Option<(RenderPath3D, bool, bool)> = None;
        for (slot, entry) in self.mesh_blend_mask_batch_entries.iter().enumerate() {
            let MeshBlendMaskEntry::Draw { batch_index, .. } = *entry else {
                continue;
            };
            let batch = &self.draw_batches[batch_index];
            let state = (batch.path, batch.double_sided, batch.packed_lod);
            if current_state != Some(state) {
                let (camera_bg, vertex_buf, pipeline) = if batch.path == RenderPath3D::Rigid {
                    let pipeline = if batch.double_sided {
                        if batch.packed_lod {
                            &self.pipeline_mask_rigid_packed_lod_double_sided
                        } else {
                            &self.pipeline_mask_rigid_double_sided
                        }
                    } else {
                        if batch.packed_lod {
                            &self.pipeline_mask_rigid_packed_lod_culled
                        } else {
                            &self.pipeline_mask_rigid_culled
                        }
                    };
                    let vertex_buf = if batch.packed_lod {
                        &self.packed_lod_vertex_buffer
                    } else {
                        &self.rigid_vertex_buffer
                    };
                    (&self.rigid_camera_bind_group, vertex_buf, pipeline)
                } else {
                    let pipeline = if batch.double_sided {
                        &self.pipeline_mask_skinned_double_sided
                    } else {
                        &self.pipeline_mask_skinned_culled
                    };
                    (&self.camera_bind_group, &self.vertex_buffer, pipeline)
                };
                pass.set_bind_group(0, camera_bg, &[]);
                if batch.packed_lod {
                    pass.set_index_buffer(
                        self.packed_lod_index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                } else {
                    pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                }
                pass.set_vertex_buffer(0, vertex_buf.slice(..));
                pass.set_vertex_buffer(1, self.instance_transform_buffer.slice(..));
                if batch.path == RenderPath3D::Skinned {
                    pass.set_vertex_buffer(2, self.skinned_instance_meta_buffer.slice(..));
                } else {
                    pass.set_vertex_buffer(2, self.rigid_instance_meta_buffer.slice(..));
                }
                pass.set_pipeline(pipeline);
                current_state = Some(state);
            }
            pass.set_bind_group(
                1,
                &self.mesh_blend_mask_id_bind_group,
                &[(slot as u32) * MASK_ID_STRIDE as u32],
            );
            if frustum_cull_active {
                let offset = (batch_index * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                pass.draw_indexed_indirect(&self.indirect_buffer, offset);
            } else {
                let start = batch.mesh.index_start;
                let end = start + batch.mesh.index_count;
                let instances = batch.instance_start..batch.instance_start + batch.instance_count;
                pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
            }
        }
        let mut current_multimesh_double_sided: Option<bool> = None;
        for (slot, entry) in self.mesh_blend_mask_batch_entries.iter().enumerate() {
            let MeshBlendMaskEntry::MultiMesh { batch_index, .. } = *entry else {
                continue;
            };
            let batch = &self.multimesh_batches[batch_index];
            if current_multimesh_double_sided.is_none() {
                pass.set_bind_group(0, &self.multimesh_bind_group, &[]);
                if let Some(fallback) = self.fallback_material_texture_bind_group() {
                    pass.set_bind_group(1, fallback, &[]);
                }
                pass.set_bind_group(3, &self.ibl_bind_group, &[]);
                pass.set_vertex_buffer(0, self.rigid_vertex_buffer.slice(..));
                pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            }
            if current_multimesh_double_sided != Some(batch.double_sided) {
                let pipeline = if batch.double_sided {
                    &self.pipeline_multimesh_mask_double_sided
                } else {
                    &self.pipeline_multimesh_mask_culled
                };
                pass.set_pipeline(pipeline);
                current_multimesh_double_sided = Some(batch.double_sided);
            }
            pass.set_bind_group(
                2,
                &self.mesh_blend_mask_id_bind_group,
                &[(slot as u32) * MASK_ID_STRIDE as u32],
            );
            if self.multimesh_cull_active {
                let offset = (batch_index * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                pass.draw_indexed_indirect(&self.multimesh_indirect_buffer, offset);
            } else {
                let start = batch.mesh.index_start;
                let end = start + batch.mesh.index_count;
                let instances = batch.instance_start..batch.instance_start + batch.instance_count;
                pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
            }
        }
    }

    // Fullscreen seam pass: copies the scene color aside, then rewrites it
    // with cross-blended colors along visible mask boundaries. `scene_texture`
    // must be the single-sample texture behind `scene_view`.
    pub fn mesh_blend_screen_pass(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        scene_texture: &wgpu::Texture,
        scene_view: &wgpu::TextureView,
    ) {
        if !self.mesh_blend_screen_active {
            return;
        }
        let (width, height) = self.depth_size;
        if scene_texture.width() != width || scene_texture.height() != height {
            return;
        }
        let needs_copy_target = match &self.mesh_blend_scene_copy {
            Some((texture, _)) => {
                texture.width() != width
                    || texture.height() != height
                    || texture.format() != scene_texture.format()
            }
            None => true,
        };
        if needs_copy_target {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_mesh_blend_scene_copy"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: scene_texture.format(),
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.mesh_blend_scene_copy = Some((texture, view));
            self.mesh_blend_seam_bind_group = None;
        }
        let Some((copy_texture, copy_view)) = self.mesh_blend_scene_copy.as_ref() else {
            return;
        };
        encoder.copy_texture_to_texture(
            scene_texture.as_image_copy(),
            copy_texture.as_image_copy(),
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        if self.mesh_blend_seam_bind_group.is_none() {
            self.mesh_blend_seam_bind_group =
                Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("perro_mesh_blend_seam_bg"),
                    layout: &self.mesh_blend_seam_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.camera_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(copy_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(
                                &self.mesh_blend_mask_view,
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(&self.depth_prepass_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: self.mesh_blend_params_buffer.as_entire_binding(),
                        },
                    ],
                }));
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_mesh_blend_seam_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: scene_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.mesh_blend_seam_pipeline);
        let Some(seam_bind_group) = self.mesh_blend_seam_bind_group.as_ref() else {
            return;
        };
        pass.set_bind_group(0, seam_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
