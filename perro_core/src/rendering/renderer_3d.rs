use bincode::de;
use glam::Mat4;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ops::Range,
};
use wgpu::{
    BindGroupLayout, BufferDescriptor, BufferUsages, Device, RenderPass, RenderPipeline,
    RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, TextureFormat, util::DeviceExt,
};

use crate::{MeshManager, Transform3D};

#[repr(C, align(16))]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Camera3DUniform {
    pub view: [[f32; 4]; 4],
    pub projection: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable, PartialEq)]
pub struct MeshInstance {
    pub model_matrix: [[f32; 4]; 4],
    pub material_id: u32,
    pub _padding: [u32; 3],
}

pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: Option<wgpu::Buffer>,
    pub index_count: u32,
    pub vertex_count: u32,
}

pub struct Renderer3D {
    pipeline: RenderPipeline,

    // Slot-based instance management
    mesh_instance_slots: Vec<Option<(MeshInstance, String)>>,
    mesh_uuid_to_slot: HashMap<uuid::Uuid, usize>,
    free_mesh_slots: Vec<usize>,

    // Batching and rendering
    mesh_groups: Vec<(String, Vec<MeshInstance>)>,
    group_offsets: Vec<(usize, usize)>,
    buffer_ranges: Vec<Range<u64>>,
    mesh_instance_buffer: wgpu::Buffer,

    // Dirty tracking
    dirty_slots: HashSet<usize>,
    dirty_count: usize,
    instances_need_rebuild: bool,
}

impl Renderer3D {
    pub fn new(device: &Device, camera_bgl: &BindGroupLayout, format: TextureFormat) -> Self {
        println!("🟧 Renderer3D initialized");

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Simple 3D Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/basic3d.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("3D Pipeline Layout"),
            bind_group_layouts: &[camera_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Basic 3D Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Vertex buffer (positions)
                    wgpu::VertexBufferLayout {
                        array_stride: (3 * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        }],
                    },
                    // Instance buffer
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            // Model matrix (4x4 = 4 attributes)
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 32,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 48,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            // Material ID
                            wgpu::VertexAttribute {
                                offset: 64,
                                shader_location: 5,
                                format: wgpu::VertexFormat::Uint32,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create instance buffer
        let mesh_instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Mesh Instance Buffer"),
            size: 1024 * std::mem::size_of::<MeshInstance>() as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            mesh_instance_slots: Vec::new(),
            mesh_uuid_to_slot: HashMap::new(),
            free_mesh_slots: Vec::new(),
            mesh_groups: Vec::new(),
            group_offsets: Vec::new(),
            buffer_ranges: Vec::new(),
            mesh_instance_buffer,
            dirty_slots: HashSet::new(),
            dirty_count: 0,
            instances_need_rebuild: false,
        }
    }

    fn create_mesh_instance(&self, transform: Transform3D, material_id: u32) -> MeshInstance {
        MeshInstance {
            model_matrix: transform.to_mat4().to_cols_array_2d(),
            material_id,
            _padding: [0; 3],
        }
    }

    fn mark_mesh_slot_dirty(&mut self, slot: usize) {
        self.dirty_slots.insert(slot);
    }

    pub fn queue_mesh(
        &mut self,
        uuid: uuid::Uuid,
        mesh_path: &str,
        transform: Transform3D,
        material_id: u32,
        mesh_manager: &mut MeshManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        // Load or fetch cached mesh info
        mesh_manager.get_or_load_mesh(mesh_path, device, queue);

        let new_instance = self.create_mesh_instance(transform, material_id);
        let mesh_path = mesh_path.to_string();

        if let Some(&slot) = self.mesh_uuid_to_slot.get(&uuid) {
            if let Some(ref mut existing) = self.mesh_instance_slots[slot] {
                if existing.0 != new_instance || existing.1 != mesh_path {
                    existing.0 = new_instance;
                    existing.1 = mesh_path;
                    self.mark_mesh_slot_dirty(slot);
                    self.dirty_count += 1;
                    self.instances_need_rebuild = true;
                }
            }
        } else {
            let slot = if let Some(free_slot) = self.free_mesh_slots.pop() {
                free_slot
            } else {
                let new_slot = self.mesh_instance_slots.len();
                self.mesh_instance_slots.push(None);
                new_slot
            };

            self.mesh_instance_slots[slot] = Some((new_instance, mesh_path));
            self.mesh_uuid_to_slot.insert(uuid, slot);
            self.mark_mesh_slot_dirty(slot);
            self.dirty_count += 1;
            self.instances_need_rebuild = true;
        }
    }

    pub fn render(
        &mut self,
        rpass: &mut wgpu::RenderPass<'_>,
        mesh_manager: &MeshManager,
        camera_bind_group: &wgpu::BindGroup,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        if self.instances_need_rebuild {
            self.rebuild_mesh_instances(device, queue);
        }

        self.render_meshes(
            &self.mesh_groups,
            &self.group_offsets,
            &self.buffer_ranges,
            rpass,
            mesh_manager,
            camera_bind_group,
            &self.mesh_instance_buffer,
        );
    }

    fn rebuild_mesh_instances(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        // Group instances by mesh
        let mut groups: HashMap<String, Vec<MeshInstance>> = HashMap::new();

        for slot in &self.mesh_instance_slots {
            if let Some((instance, mesh_path)) = slot {
                groups
                    .entry(mesh_path.clone())
                    .or_insert_with(Vec::new)
                    .push(*instance);
            }
        }

        self.mesh_groups = groups.into_iter().collect();
        self.group_offsets.clear();
        self.buffer_ranges.clear();

        let mut current_offset = 0;
        for (_, instances) in &self.mesh_groups {
            let count = instances.len();
            self.group_offsets.push((current_offset, count));

            let start_byte = current_offset * std::mem::size_of::<MeshInstance>();
            let end_byte = (current_offset + count) * std::mem::size_of::<MeshInstance>();
            self.buffer_ranges
                .push((start_byte as u64)..(end_byte as u64));

            current_offset += count;
        }

        // Write all instances to buffer
        let mut all_instances = Vec::new();
        for (_, instances) in &self.mesh_groups {
            all_instances.extend_from_slice(instances);
        }

        if !all_instances.is_empty() {
            let required_size = (all_instances.len() * std::mem::size_of::<MeshInstance>()) as u64;

            if required_size > self.mesh_instance_buffer.size() {
                println!(
                    "⚠️ Resizing instance buffer from {} → {} bytes",
                    self.mesh_instance_buffer.size(),
                    required_size.next_power_of_two()
                );

                self.mesh_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Mesh Instance Buffer (Resized)"),
                    size: required_size.next_power_of_two(),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }

            // Write instances to GPU
            queue.write_buffer(
                &self.mesh_instance_buffer,
                0,
                bytemuck::cast_slice(&all_instances),
            );
        }

        self.instances_need_rebuild = false;
        self.dirty_slots.clear();
        self.dirty_count = 0;
    }

    fn render_meshes(
        &self,
        mesh_groups: &[(String, Vec<MeshInstance>)],
        group_offsets: &[(usize, usize)],
        buffer_ranges: &[Range<u64>],
        rpass: &mut RenderPass<'_>,
        mesh_manager: &MeshManager,
        camera_bind_group: &wgpu::BindGroup,
        instance_buffer: &wgpu::Buffer,
    ) {
        for (i, (mesh_path, _)) in mesh_groups.iter().enumerate() {
            let (_, count) = group_offsets[i];

            if count > 0 {
                if let Some(mesh) = mesh_manager.meshes.get(mesh_path) {
                    rpass.set_pipeline(&self.pipeline);
                    rpass.set_bind_group(0, camera_bind_group, &[]);
                    rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    rpass.set_vertex_buffer(1, instance_buffer.slice(buffer_ranges[i].clone()));

                    if let Some(index_buf) = &mesh.index_buffer {
                        rpass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
                        rpass.draw_indexed(0..mesh.index_count, 0, 0..count as u32);
                    } else {
                        rpass.draw(0..mesh.vertex_count, 0..count as u32);
                    }
                }
            }
        }
    }

    pub fn stop_rendering(&mut self, uuid: uuid::Uuid) {
        if let Some(&slot) = self.mesh_uuid_to_slot.get(&uuid) {
            self.mesh_instance_slots[slot] = None;
            self.mesh_uuid_to_slot.remove(&uuid);
            self.free_mesh_slots.push(slot);
            self.instances_need_rebuild = true;
        }
    }

    pub fn create_cube_mesh(device: &wgpu::Device) -> Mesh {
        let vertices: &[f32] = &[
            // Front face
            -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, 0.5, 0.5, 0.5, -0.5, 0.5, 0.5, // Back face
            -0.5, -0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, -0.5,
        ];

        let indices: &[u32] = &[
            // Front
            0, 1, 2, 2, 3, 0, // Right
            1, 5, 6, 6, 2, 1, // Back
            5, 4, 7, 7, 6, 5, // Left
            4, 0, 3, 3, 7, 4, // Top
            3, 2, 6, 6, 7, 3, // Bottom
            4, 5, 1, 1, 0, 4,
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cube Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cube Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Mesh {
            vertex_buffer,
            index_buffer: Some(index_buffer),
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32 / 3, // 3 floats per vertex
        }
    }
}
