use glam::Mat4;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ops::Range,
};
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, Device, Queue, RenderPass,
    RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, TextureFormat,
    util::DeviceExt,
};

use crate::{MeshManager, Transform3D};

pub const MAX_LIGHTS: usize = 16; // Keep this constant for the GPU array size

// Vertex with position + normal for lighting
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

// Single light uniform (unchanged)
#[repr(C, align(16))]
#[derive(PartialEq, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct LightUniform {
    pub position: [f32; 3],
    pub _pad0: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub ambient: [f32; 3],
    pub _pad1: f32,
}

// Camera uniform (unchanged)
#[repr(C, align(16))]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Camera3DUniform {
    pub view: [[f32; 4]; 4],
    pub projection: [[f32; 4]; 4],
}

// Instance struct for batched rendering (unchanged)
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

    // Lighting (updated to be slot-based)
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    light_slots: Vec<Option<LightUniform>>, // Stores the actual light data in slots
    light_uuid_to_slot: HashMap<uuid::Uuid, usize>, // Maps light UUIDs to their slot index
    free_light_slots: Vec<usize>,           // Pool of unused slots
    lights_need_rebuild: bool,              // Flag to indicate if the light buffer needs updating

    // Instancing (mesh management, mostly unchanged)
    mesh_instance_slots: Vec<Option<(MeshInstance, String)>>,
    mesh_uuid_to_slot: HashMap<uuid::Uuid, usize>,
    free_mesh_slots: Vec<usize>,

    // Batching info (unchanged)
    mesh_groups: Vec<(String, Vec<MeshInstance>)>,
    group_offsets: Vec<(usize, usize)>,
    buffer_ranges: Vec<Range<u64>>,
    mesh_instance_buffer: wgpu::Buffer,

    // Dirty state (unchanged for meshes, but lights_need_rebuild is new)
    dirty_slots: HashSet<usize>,
    dirty_count: usize,
    instances_need_rebuild: bool,
}

impl Renderer3D {
    pub fn new(device: &Device, camera_bgl: &BindGroupLayout, format: TextureFormat) -> Self {
        println!("🟧 Renderer3D initialized with multi-light support");

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("3D Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/basic3d.wgsl"))),
        });

        // Light buffer array (MAX_LIGHTS)
        let light_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light Buffer"),
            size: (MAX_LIGHTS * std::mem::size_of::<LightUniform>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Light bind group layout
        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Light BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(
                            (MAX_LIGHTS * std::mem::size_of::<LightUniform>()) as u64,
                        ),
                    },
                    count: None,
                }],
            });

        // Bind group for lights
        let light_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Light BG"),
            layout: &light_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &light_buffer,
                    offset: 0,
                    size: BufferSize::new(
                        (MAX_LIGHTS * std::mem::size_of::<LightUniform>()) as u64,
                    ),
                }),
            }],
        });

        // Pipeline layout (camera + light)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("3D Pipeline Layout"),
            bind_group_layouts: &[camera_bgl, &light_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("3D Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Vertex
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex3D>() as u64,
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
                                format: wgpu::VertexFormat::Float32x3,
                            },
                        ],
                    },
                    // Instance
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 32,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 48,
                                shader_location: 5,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 64,
                                shader_location: 6,
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let mesh_instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Mesh Instances"),
            size: 1024 * std::mem::size_of::<MeshInstance>() as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            light_buffer,
            light_bind_group,
            // Initialize light slots to MAX_LIGHTS capacity with None
            light_slots: vec![None; MAX_LIGHTS],
            light_uuid_to_slot: HashMap::new(),
            free_light_slots: (0..MAX_LIGHTS).collect(), // All slots are initially free
            lights_need_rebuild: false, // Initial state, will be true when first light is added

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

    // No longer clears all lights, just updates a slot.
    // Call this for every light that exists in your scene (even if not changed)
    pub fn queue_light(&mut self, id: uuid::Uuid, light_uniform: LightUniform) {
        let slot = if let Some(&slot_idx) = self.light_uuid_to_slot.get(&id) {
            // Light already exists, update it if changed
            if let Some(existing_light) = &mut self.light_slots[slot_idx] {
                if *existing_light != light_uniform {
                    *existing_light = light_uniform;
                    self.lights_need_rebuild = true;
                    println!("🟨 Updated light UUID: {:?}, slot: {}", id, slot_idx);
                }
            }
            slot_idx
        } else {
            // New light, find a free slot
            if let Some(free_slot_idx) = self.free_light_slots.pop() {
                self.light_slots[free_slot_idx] = Some(light_uniform);
                self.light_uuid_to_slot.insert(id, free_slot_idx);
                self.lights_need_rebuild = true;
                println!(
                    "🟨 Queued new light UUID: {:?}, slot: {}",
                    id, free_slot_idx
                );
                free_slot_idx
            } else {
                // No free slots, log warning and skip
                println!(
                    "⚠️ Max lights reached ({}). Skipping new light: {:?}",
                    MAX_LIGHTS, id
                );
                return;
            }
        };

        // You might want to remove this for production, but good for debugging
        println!(
            "Current light queue status: {} active / {} free slots",
            self.light_uuid_to_slot.len(),
            self.free_light_slots.len()
        );
    }

    // Uploads the full `light_slots` buffer to the GPU only if changes occurred.
    pub fn upload_lights_to_gpu(&mut self, queue: &wgpu::Queue) {
        if self.lights_need_rebuild {
            // Extract the actual LightUniforms from the Option enum
            let active_lights: Vec<LightUniform> =
                self.light_slots.iter().filter_map(|l| *l).collect();
            // Fill a temporary array with actual lights, padding the rest with Default (zeroes)
            let mut gpu_lights_array = [LightUniform::default(); MAX_LIGHTS];
            for (i, light) in active_lights.iter().enumerate() {
                if i < MAX_LIGHTS {
                    // Ensure we don't exceed array bounds, even if active_lights somehow grows
                    gpu_lights_array[i] = *light;
                }
            }

            queue.write_buffer(
                &self.light_buffer,
                0,
                bytemuck::cast_slice(&gpu_lights_array),
            );
            println!(
                "✅ Lights uploaded. Active: {} First intensity: {}",
                active_lights.len(),
                gpu_lights_array[0].intensity
            );
            self.lights_need_rebuild = false; // Reset the dirty flag
        } else {
            // This is expected if no lights changed this frame
            // println!("No light rebuild needed.");
        }
    }

    pub fn stop_rendering_light(&mut self, uuid: uuid::Uuid) {
        if let Some(&slot_idx) = self.light_uuid_to_slot.get(&uuid) {
            // Clear the slot and add it back to free list
            self.light_slots[slot_idx] = None;
            self.free_light_slots.push(slot_idx);
            self.light_uuid_to_slot.remove(&uuid);
            self.lights_need_rebuild = true;
            println!(
                "🟫 Stopped rendering light UUID: {:?}, slot: {}",
                uuid, slot_idx
            );
        }
    }

    fn create_mesh_instance(&self, transform: Transform3D, material_id: u32) -> MeshInstance {
        MeshInstance {
            model_matrix: transform.to_mat4().to_cols_array_2d(),
            material_id,
            _padding: [0; 3],
        }
    }

    pub fn queue_mesh(
        &mut self,
        uuid: uuid::Uuid,
        mesh_path: &str,
        transform: Transform3D,
        material_id: u32,
        mesh_manager: &mut MeshManager,
        device: &Device,
        queue: &wgpu::Queue,
    ) {
        mesh_manager.get_or_load_mesh(mesh_path, device, queue);

        let new_instance = self.create_mesh_instance(transform, material_id);
        let mesh_path = mesh_path.to_owned();

        if let Some(&slot) = self.mesh_uuid_to_slot.get(&uuid) {
            if let Some(existing) = &mut self.mesh_instance_slots[slot] {
                if existing.0 != new_instance || existing.1 != mesh_path {
                    existing.0 = new_instance;
                    existing.1 = mesh_path;
                    self.instances_need_rebuild = true;
                }
            }
        } else {
            let slot = self
                .free_mesh_slots
                .pop()
                .unwrap_or_else(|| self.mesh_instance_slots.len());
            if slot == self.mesh_instance_slots.len() {
                self.mesh_instance_slots.push(None);
            }
            self.mesh_instance_slots[slot] = Some((new_instance, mesh_path.clone()));
            self.mesh_uuid_to_slot.insert(uuid, slot);
            self.instances_need_rebuild = true;
        }
    }

    fn rebuild_mesh_instances(&mut self, device: &Device, queue: &wgpu::Queue) {
        // Rebuild all grouped meshes
        let mut groups: HashMap<String, Vec<MeshInstance>> = HashMap::new();
        for slot in &self.mesh_instance_slots {
            if let Some((inst, path)) = slot {
                groups.entry(path.clone()).or_default().push(*inst);
            }
        }

        self.mesh_groups = groups.into_iter().collect();

        let all_instances: Vec<MeshInstance> = self
            .mesh_groups
            .iter()
            .flat_map(|(_, v)| v.clone())
            .collect();

        if all_instances.is_empty() {
            return;
        }

        queue.write_buffer(
            &self.mesh_instance_buffer,
            0,
            bytemuck::cast_slice(&all_instances),
        );

        self.instances_need_rebuild = false;
    }

    pub fn render(
        &mut self,
        rpass: &mut RenderPass<'_>,
        mesh_manager: &MeshManager,
        camera_bind_group: &wgpu::BindGroup,
        device: &Device,
        queue: &Queue,
    ) {
        if self.instances_need_rebuild {
            // Rebuild instance buffer
            self.rebuild_mesh_instances(device, queue);
        }

        for (i, (mesh_path, instances)) in self.mesh_groups.iter().enumerate() {
            if let Some(mesh) = mesh_manager.meshes.get(mesh_path) {
                rpass.set_pipeline(&self.pipeline);
                rpass.set_bind_group(0, camera_bind_group, &[]);
                rpass.set_bind_group(1, &self.light_bind_group, &[]);
                rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rpass.set_vertex_buffer(
                    1,
                    self.mesh_instance_buffer.slice(
                        (i * instances.len() * std::mem::size_of::<MeshInstance>()) as u64
                            ..((i + 1) * instances.len() * std::mem::size_of::<MeshInstance>())
                                as u64,
                    ),
                );

                if let Some(index_buf) = &mesh.index_buffer {
                    rpass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..mesh.index_count, 0, 0..instances.len() as u32);
                } else {
                    rpass.draw(0..mesh.vertex_count, 0..instances.len() as u32);
                }
            }
        }
    }

    // Helper to create a test cube mesh
    pub fn create_cube_mesh(device: &wgpu::Device) -> Mesh {
        use bytemuck::cast_slice;

        let v = |p, n| Vertex3D {
            position: p,
            normal: n,
        };

        let vertices: Vec<Vertex3D> = vec![
            // Front (+Z)
            Vertex3D {
                position: [-0.5, -0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
            },
            Vertex3D {
                position: [0.5, -0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
            },
            Vertex3D {
                position: [0.5, 0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
            },
            Vertex3D {
                position: [-0.5, 0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
            },
            // Back (−Z)
            Vertex3D {
                position: [0.5, -0.5, -0.5],
                normal: [0.0, 0.0, -1.0],
            },
            Vertex3D {
                position: [-0.5, -0.5, -0.5],
                normal: [0.0, 0.0, -1.0],
            },
            Vertex3D {
                position: [-0.5, 0.5, -0.5],
                normal: [0.0, 0.0, -1.0],
            },
            Vertex3D {
                position: [0.5, 0.5, -0.5],
                normal: [0.0, 0.0, -1.0],
            },
            // Right (+X)
            Vertex3D {
                position: [0.5, -0.5, 0.5],
                normal: [1.0, 0.0, 0.0],
            },
            Vertex3D {
                position: [0.5, -0.5, -0.5],
                normal: [1.0, 0.0, 0.0],
            },
            Vertex3D {
                position: [0.5, 0.5, -0.5],
                normal: [1.0, 0.0, 0.0],
            },
            Vertex3D {
                position: [0.5, 0.5, 0.5],
                normal: [1.0, 0.0, 0.0],
            },
            // Left (−X)
            Vertex3D {
                position: [-0.5, -0.5, -0.5],
                normal: [-1.0, 0.0, 0.0],
            },
            Vertex3D {
                position: [-0.5, -0.5, 0.5],
                normal: [-1.0, 0.0, 0.0],
            },
            Vertex3D {
                position: [-0.5, 0.5, 0.5],
                normal: [-1.0, 0.0, 0.0],
            },
            Vertex3D {
                position: [-0.5, 0.5, -0.5],
                normal: [-1.0, 0.0, 0.0],
            },
            // Top (+Y)
            Vertex3D {
                position: [-0.5, 0.5, 0.5],
                normal: [0.0, 1.0, 0.0],
            },
            Vertex3D {
                position: [0.5, 0.5, 0.5],
                normal: [0.0, 1.0, 0.0],
            },
            Vertex3D {
                position: [0.5, 0.5, -0.5],
                normal: [0.0, 1.0, 0.0],
            },
            Vertex3D {
                position: [-0.5, 0.5, -0.5],
                normal: [0.0, 1.0, 0.0],
            },
            // Bottom (−Y)
            Vertex3D {
                position: [-0.5, -0.5, -0.5],
                normal: [0.0, -1.0, 0.0],
            },
            Vertex3D {
                position: [0.5, -0.5, -0.5],
                normal: [0.0, -1.0, 0.0],
            },
            Vertex3D {
                position: [0.5, -0.5, 0.5],
                normal: [0.0, -1.0, 0.0],
            },
            Vertex3D {
                position: [-0.5, -0.5, 0.5],
                normal: [0.0, -1.0, 0.0],
            },
        ];

        let indices: &[u32] = &[
            0, 1, 2, 2, 3, 0, // Front
            4, 5, 6, 6, 7, 4, // Back
            8, 9, 10, 10, 11, 8, // Right
            12, 13, 14, 14, 15, 12, // Left
            16, 17, 18, 18, 19, 16, // Top
            20, 21, 22, 22, 23, 20, // Bottom
        ];

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cube VB"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cube IB"),
            contents: cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Mesh {
            vertex_buffer: vb,
            index_buffer: Some(ib),
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
        }
    }
}
