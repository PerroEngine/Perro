use glam::Mat4;
use std::cmp::Ordering;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    ops::Range,
};
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, Device, Queue, RenderPass,
    RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, TextureFormat,
    VERTEX_STRIDE_ALIGNMENT, util::DeviceExt,
};

use crate::{Frustum, MaterialManager, MeshManager, Transform3D};

use rayon::prelude::*;
use rustc_hash::FxHashMap;

pub const MAX_LIGHTS: usize = 16;
pub const MAX_MATERIALS: usize = 64;

// Vertex with position + normal for lighting
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

// Light uniform
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

// Material uniform
#[repr(C, align(16))]
#[derive(PartialEq, Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub _pad0: [f32; 2],
    pub emissive: [f32; 4],
}

// Camera uniform
#[repr(C, align(16))]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Camera3DUniform {
    pub view: [[f32; 4]; 4],
    pub projection: [[f32; 4]; 4],
}

// Instance struct for batched rendering
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable, PartialEq)]
pub struct MeshInstance {
    pub model_matrix: [[f32; 4]; 4],
    pub material_id: u32,
    pub _padding: [u32; 3],
}

pub struct MeshSlot {
    pub instance: MeshInstance,
    pub mesh_path: String,
    pub material_path: String, // Store material path for cache validation
    pub instance_visible: bool,
}

pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: Option<wgpu::Buffer>,
    pub index_count: u32,
    pub vertex_count: u32,
    pub bounds_center: glam::Vec3,
    pub bounds_radius: f32,
}

pub struct Renderer3D {
    pipeline: RenderPipeline,

    // Lighting
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    light_slots: Vec<Option<LightUniform>>,
    light_uuid_to_slot: HashMap<uuid::Uuid, usize>,
    free_light_slots: Vec<usize>,
    lights_need_rebuild: bool,

    // Materials
    material_buffer: wgpu::Buffer,
    material_bind_group: wgpu::BindGroup,
    material_slots: Vec<Option<MaterialUniform>>,
    material_uuid_to_slot: HashMap<uuid::Uuid, usize>,
    free_material_slots: Vec<usize>,
    materials_need_rebuild: bool,

    // Instancing - Updated to use MeshSlot
    mesh_instance_slots: Vec<Option<MeshSlot>>,
    mesh_uuid_to_slot: HashMap<uuid::Uuid, usize>,
    free_mesh_slots: Vec<usize>,

    // Batching info - Updated for better material batching
    mesh_material_groups: Vec<(String, u32, Vec<MeshInstance>)>, // (mesh_path, material_id, instances)
    mesh_instance_buffer: wgpu::Buffer,

    last_frustum_matrix: glam::Mat4,

    // Dirty state
    pub instances_need_rebuild: bool,
    pub visibility_dirty: bool,
}

impl Renderer3D {
    pub fn new(device: &Device, camera_bgl: &BindGroupLayout, format: TextureFormat) -> Self {
        println!("🟧 Renderer3D initialized with multi-light and material support");

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("3D Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/3D/basic3d.wgsl"))),
        });

        // ===== LIGHT SETUP =====
        let light_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light Buffer"),
            size: (MAX_LIGHTS * std::mem::size_of::<LightUniform>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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

        // ===== MATERIAL SETUP =====
        let material_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Material Buffer"),
            size: (MAX_MATERIALS * std::mem::size_of::<MaterialUniform>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Material BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(
                            (MAX_MATERIALS * std::mem::size_of::<MaterialUniform>()) as u64,
                        ),
                    },
                    count: None,
                }],
            });

        let material_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Material BG"),
            layout: &material_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &material_buffer,
                    offset: 0,
                    size: BufferSize::new(
                        (MAX_MATERIALS * std::mem::size_of::<MaterialUniform>()) as u64,
                    ),
                }),
            }],
        });

        // ===== PIPELINE SETUP =====
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("3D Pipeline Layout"),
            bind_group_layouts: &[
                camera_bgl,
                &light_bind_group_layout,
                &material_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

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
            size: 4096 * std::mem::size_of::<MeshInstance>() as u64, // Increased buffer size
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            light_buffer,
            light_bind_group,
            light_slots: vec![None; MAX_LIGHTS],
            light_uuid_to_slot: HashMap::new(),
            free_light_slots: (0..MAX_LIGHTS).collect(),
            lights_need_rebuild: false,

            material_buffer,
            material_bind_group,
            material_slots: vec![None; MAX_MATERIALS],
            material_uuid_to_slot: HashMap::new(),
            free_material_slots: (0..MAX_MATERIALS).rev().collect(),
            materials_need_rebuild: false,

            mesh_instance_slots: Vec::new(),
            mesh_uuid_to_slot: HashMap::new(),
            free_mesh_slots: Vec::new(),
            mesh_material_groups: Vec::new(),
            mesh_instance_buffer,
            instances_need_rebuild: false,
            visibility_dirty: false,

            last_frustum_matrix: Mat4::IDENTITY,
        }
    }

    // ===== LIGHT MANAGEMENT =====
    pub fn queue_light(&mut self, id: uuid::Uuid, light_uniform: LightUniform) {
        let slot = if let Some(&slot_idx) = self.light_uuid_to_slot.get(&id) {
            if let Some(existing_light) = &mut self.light_slots[slot_idx] {
                if *existing_light != light_uniform {
                    *existing_light = light_uniform;
                    self.lights_need_rebuild = true;
                }
            }
            slot_idx
        } else {
            if let Some(free_slot_idx) = self.free_light_slots.pop() {
                self.light_slots[free_slot_idx] = Some(light_uniform);
                self.light_uuid_to_slot.insert(id, free_slot_idx);
                self.lights_need_rebuild = true;
                println!(
                    "💡 Queued new light UUID: {:?}, slot: {}",
                    id, free_slot_idx
                );
                free_slot_idx
            } else {
                println!(
                    "⚠️ Max lights reached ({}). Skipping new light: {:?}",
                    MAX_LIGHTS, id
                );
                return;
            }
        };
    }

    pub fn upload_lights_to_gpu(&mut self, queue: &Queue) {
        if self.lights_need_rebuild {
            let mut gpu_lights_array = [LightUniform::default(); MAX_LIGHTS];
            let active_lights: Vec<LightUniform> =
                self.light_slots.iter().filter_map(|l| *l).collect();

            for (i, light) in active_lights.iter().enumerate() {
                if i < MAX_LIGHTS {
                    gpu_lights_array[i] = *light;
                }
            }

            queue.write_buffer(
                &self.light_buffer,
                0,
                bytemuck::cast_slice(&gpu_lights_array),
            );
            self.lights_need_rebuild = false;
        }
    }

    pub fn stop_rendering_light(&mut self, uuid: uuid::Uuid) {
        if let Some(&slot_idx) = self.light_uuid_to_slot.get(&uuid) {
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

    // ===== MATERIAL MANAGEMENT =====
    pub fn queue_material(&mut self, id: uuid::Uuid, material: MaterialUniform) -> u32 {
        let slot = if let Some(&slot_idx) = self.material_uuid_to_slot.get(&id) {
            if let Some(existing_mat) = &mut self.material_slots[slot_idx] {
                if *existing_mat != material {
                    *existing_mat = material;
                    self.materials_need_rebuild = true;
                }
            }
            slot_idx
        } else {
            if let Some(free_slot_idx) = self.free_material_slots.pop() {
                self.material_slots[free_slot_idx] = Some(material);
                self.material_uuid_to_slot.insert(id, free_slot_idx);
                self.materials_need_rebuild = true;
                println!(
                    "🟦 Queued new material UUID: {:?}, slot: {},",
                    id, free_slot_idx
                );
                free_slot_idx
            } else {
                println!(
                    "⚠️ Max materials reached ({}). Returning slot 0",
                    MAX_MATERIALS
                );
                return 0;
            }
        };
        slot as u32
    }

    pub fn upload_materials_to_gpu(&mut self, queue: &Queue) {
        if self.materials_need_rebuild {
            let mut gpu_materials_array = [MaterialUniform::default(); MAX_MATERIALS];
            let active_materials: Vec<MaterialUniform> =
                self.material_slots.iter().filter_map(|m| *m).collect();

            for (i, material) in active_materials.iter().enumerate() {
                if i < MAX_MATERIALS {
                    gpu_materials_array[i] = *material;
                }
            }

            queue.write_buffer(
                &self.material_buffer,
                0,
                bytemuck::cast_slice(&gpu_materials_array),
            );
            println!("✅ Materials uploaded. Active: {}", active_materials.len());
            self.materials_need_rebuild = false;
        }
    }

    pub fn stop_rendering_material(&mut self, uuid: uuid::Uuid) {
        if let Some(&slot_idx) = self.material_uuid_to_slot.get(&uuid) {
            self.material_slots[slot_idx] = None;
            self.free_material_slots.push(slot_idx);
            self.material_uuid_to_slot.remove(&uuid);
            self.materials_need_rebuild = true;
        }
    }

    // ===== MESH MANAGEMENT =====
    pub fn queue_mesh(
        &mut self,
        uuid: uuid::Uuid,
        mesh_path: &str,
        transform: Transform3D,
        material_path: Option<&str>, // Accept Option<&str>
        mesh_manager: &mut MeshManager,
        material_manager: &mut MaterialManager,
        device: &Device,
        queue: &Queue,
    ) {
        // Ensure mesh is loaded
        mesh_manager.get_or_load_mesh(mesh_path, device, queue);

        // Use default material if none provided
        let material_path = material_path.unwrap_or("__default__");

        // Resolve material to slot ID
        let material_id = material_manager.get_or_upload_material(material_path, self);

        let new_instance = MeshInstance {
            model_matrix: transform.to_mat4().to_cols_array_2d(),
            material_id,
            _padding: [0; 3],
        };

        let new_slot = MeshSlot {
            instance: new_instance,
            mesh_path: mesh_path.to_owned(),
            material_path: material_path.to_owned(),
            instance_visible: true,
        };

        if let Some(&slot) = self.mesh_uuid_to_slot.get(&uuid) {
            if let Some(existing) = &mut self.mesh_instance_slots[slot] {
                // Check if anything changed
                if existing.instance != new_instance
                    || existing.mesh_path != mesh_path
                    || existing.material_path != material_path
                {
                    *existing = new_slot;
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
            self.mesh_instance_slots[slot] = Some(new_slot);
            self.mesh_uuid_to_slot.insert(uuid, slot);
            self.instances_need_rebuild = true;
        }
    }

    pub fn stop_rendering_mesh(&mut self, uuid: uuid::Uuid) {
        if let Some(&slot_idx) = self.mesh_uuid_to_slot.get(&uuid) {
            self.mesh_instance_slots[slot_idx] = None;
            self.free_mesh_slots.push(slot_idx);
            self.mesh_uuid_to_slot.remove(&uuid);
            self.instances_need_rebuild = true;
            println!(
                "🟫 Stopped rendering mesh UUID: {:?}, slot: {}",
                uuid, slot_idx
            );
        }
    }

    pub fn rebuild_mesh_instances(
        &mut self,
        device: &Device,
        queue: &Queue,
        mesh_manager: &MeshManager,
        camera_view: &glam::Mat4,
        camera_projection: &glam::Mat4,
    ) {
        type MeshGroupKey = (String, u32);
        type MeshGroupMap = FxHashMap<MeshGroupKey, Vec<MeshInstance>>;

        let need_recull = self.instances_need_rebuild; // Objects moved

        let groups: MeshGroupMap = if need_recull {
            // Objects moved - need full re-cull
            let vp = *camera_projection * *camera_view;
            let frustum = Frustum::from_matrix(&vp);

            self.mesh_instance_slots
                .par_iter_mut()
                .filter_map(|slot| {
                    let slot_data = slot.as_mut()?;

                    // Re-cull because objects moved
                    let visible = if let Some(mesh) = mesh_manager.meshes.get(&slot_data.mesh_path)
                    {
                        let model =
                            glam::Mat4::from_cols_array_2d(&slot_data.instance.model_matrix);
                        let center_ws = model.transform_point3(mesh.bounds_center);
                        let scale = model.col(0).truncate().length();
                        let radius_ws = mesh.bounds_radius * scale;
                        frustum.contains_sphere(center_ws, radius_ws)
                    } else {
                        true
                    };

                    slot_data.instance_visible = visible;

                    if visible {
                        let key = (slot_data.mesh_path.clone(), slot_data.instance.material_id);
                        Some((key, slot_data.instance))
                    } else {
                        None
                    }
                })
                .fold(
                    || MeshGroupMap::default(), // ← Explicit type
                    |mut local: MeshGroupMap, (key, inst)| {
                        // ← Explicit type
                        local.entry(key).or_default().push(inst);
                        local
                    },
                )
                .reduce(
                    || MeshGroupMap::default(), // ← Explicit type
                    |mut a: MeshGroupMap, b: MeshGroupMap| {
                        // ← Explicit types
                        for (k, v) in b {
                            a.entry(k).or_default().extend(v);
                        }
                        a
                    },
                )
        } else {
            // Only visibility changed - use cached flags
            self.mesh_instance_slots
                .par_iter()
                .filter_map(|slot| slot.as_ref())
                .filter(|slot_data| slot_data.instance_visible)
                .map(|slot_data| {
                    let key = (slot_data.mesh_path.clone(), slot_data.instance.material_id);
                    (key, slot_data.instance)
                })
                .fold(
                    || MeshGroupMap::default(), // ← Explicit type
                    |mut local: MeshGroupMap, (key, inst)| {
                        // ← Explicit type
                        local.entry(key).or_default().push(inst);
                        local
                    },
                )
                .reduce(
                    || MeshGroupMap::default(), // ← Explicit type
                    |mut a: MeshGroupMap, b: MeshGroupMap| {
                        // ← Explicit types
                        for (k, v) in b {
                            a.entry(k).or_default().extend(v);
                        }
                        a
                    },
                )
        };

        // ---- 3️⃣  Frustum culling stats ----
        let total_instances = self
            .mesh_instance_slots
            .par_iter()
            .filter(|s| s.is_some())
            .count();

        let visible_instances = groups.values().map(|v| v.len()).sum::<usize>();

        // println!(
        //     "🧭 Frustum culling: {}/{} visible (culled {} meshes)",
        //     visible_instances,
        //     total_instances,
        //     total_instances.saturating_sub(visible_instances)
        // );

        // ---- 4️⃣  Deterministic sorting of groups (mesh + material) ----
        let mut sorted_groups: Vec<_> = groups.into_iter().collect();
        sorted_groups.sort_by(|a, b| {
            let (mesh_a, mat_a) = &a.0;
            let (mesh_b, mat_b) = &b.0;
            match mesh_a.cmp(mesh_b) {
                Ordering::Equal => mat_a.cmp(mat_b),
                ord => ord,
            }
        });

        // ---- 5️⃣  Instance sorting inside each group ----
        // Extract camera position
        let camera_pos = camera_view.inverse().transform_point3(glam::Vec3::ZERO);

        // Sort visible instances front-to-back or back-to-front based on material transparency
        for ((_, material_id), instances) in &mut sorted_groups {
            let is_transparent = self
                .material_slots
                .get(*material_id as usize)
                .and_then(|mat_opt| mat_opt.as_ref())
                .map(|mat| mat.base_color[3] < 1.0)
                .unwrap_or(false);

            instances.sort_by(|a, b| {
                let a_pos = glam::Mat4::from_cols_array_2d(&a.model_matrix)
                    .transform_point3(glam::Vec3::ZERO);
                let b_pos = glam::Mat4::from_cols_array_2d(&b.model_matrix)
                    .transform_point3(glam::Vec3::ZERO);

                let da = (a_pos - camera_pos).length_squared();
                let db = (b_pos - camera_pos).length_squared();

                let cmp = da.partial_cmp(&db).unwrap_or(Ordering::Equal);

                if is_transparent {
                    cmp.reverse() // back-to-front for transparency
                } else {
                    cmp // front-to-back for opaque
                }
            });
        }

        // ---- 6️⃣  Save as final render batches ----
        self.mesh_material_groups = sorted_groups
            .into_iter()
            .map(|((mesh_path, material_id), instances)| (mesh_path, material_id, instances))
            .collect();

        // ---- 7️⃣  Upload instance buffer ----
        let all_instances: Vec<MeshInstance> = self
            .mesh_material_groups
            .iter()
            .flat_map(|(_, _, instances)| instances.clone())
            .collect();

        if all_instances.is_empty() {
            self.instances_need_rebuild = false;
            return;
        }

        let required_size = (all_instances.len() * std::mem::size_of::<MeshInstance>()) as u64;

        if required_size > self.mesh_instance_buffer.size() {
            self.mesh_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Mesh Instances (Resized)"),
                size: required_size * 2, // growth margin
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(
            &self.mesh_instance_buffer,
            0,
            bytemuck::cast_slice(&all_instances),
        );

        self.instances_need_rebuild = false;
        self.visibility_dirty = false;

        // println!(
        //     "✅ Instance buffer updated with {} visible instances across {} batches",
        //     all_instances.len(),
        //     self.mesh_material_groups.len()
        // );
    }

    pub fn update_culling_from_camera(&mut self, mesh_manager: &MeshManager, vp: glam::Mat4) {
        let frustum = Frustum::from_matrix(&vp);

        let mut any_change = false;
        for slot in &mut self.mesh_instance_slots {
            if let Some(slot_data) = slot {
                if let Some(mesh) = mesh_manager.meshes.get(&slot_data.mesh_path) {
                    let model = glam::Mat4::from_cols_array_2d(&slot_data.instance.model_matrix);
                    let center_ws = model.transform_point3(mesh.bounds_center);
                    let scale = model.col(0).truncate().length();
                    let radius_ws = mesh.bounds_radius * scale;

                    let visible = frustum.contains_sphere(center_ws, radius_ws);
                    if visible != slot_data.instance_visible {
                        slot_data.instance_visible = visible;
                        any_change = true;
                    }
                }
            }
        }

        if any_change {
            self.visibility_dirty = true;
        }
    }

    pub fn maybe_update_culling(
        &mut self,
        mesh_manager: &MeshManager,
        camera_view: &glam::Mat4,
        camera_projection: &glam::Mat4,
        queue: &wgpu::Queue,
    ) {
        let vp = *camera_projection * *camera_view;
        // Only recull if frustum moved significantly
        if (vp - self.last_frustum_matrix).abs_diff_eq(glam::Mat4::ZERO, 0.001) {
            return;
        }

        self.last_frustum_matrix = vp;
        self.update_culling_from_camera(mesh_manager, vp);
    }

    pub fn render(
        &mut self,
        rpass: &mut RenderPass<'_>,
        mesh_manager: &MeshManager,
        camera_bind_group: &wgpu::BindGroup,
        camera_view: &glam::Mat4,
        camera_projection: &glam::Mat4,
        device: &Device,
        queue: &Queue,
    ) {
        // -------------------------------------------------------------------------
        // STEP 1: Rebuild instance buffer if needed (includes frustum culling)
        // -------------------------------------------------------------------------
        if self.instances_need_rebuild || self.visibility_dirty {
            self.rebuild_mesh_instances(
                device,
                queue,
                mesh_manager,
                camera_view,
                camera_projection,
            );
        }

        // If all instances were culled or none queued, skip render
        if self.mesh_material_groups.is_empty() {
            return;
        }

        // -------------------------------------------------------------------------
        // STEP 2: Begin rendering visible mesh instances
        // -------------------------------------------------------------------------
        let mut instance_offset = 0;

        for (mesh_path, material_id, instances) in &self.mesh_material_groups {
            if let Some(mesh) = mesh_manager.meshes.get(mesh_path) {
                // Configure pipeline and all bindings
                rpass.set_pipeline(&self.pipeline);
                rpass.set_bind_group(0, camera_bind_group, &[]);
                rpass.set_bind_group(1, &self.light_bind_group, &[]);
                rpass.set_bind_group(2, &self.material_bind_group, &[]);
                rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));

                // Compute vertex instance range for indirect drawing offset
                let instance_size = std::mem::size_of::<MeshInstance>() as u64;
                let start_offset = (instance_offset * std::mem::size_of::<MeshInstance>()) as u64;
                let end_offset = ((instance_offset + instances.len())
                    * std::mem::size_of::<MeshInstance>()) as u64;

                rpass.set_vertex_buffer(
                    1,
                    self.mesh_instance_buffer.slice(start_offset..end_offset),
                );

                // Draw by index or vertex
                if let Some(index_buf) = &mesh.index_buffer {
                    rpass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..mesh.index_count, 0, 0..instances.len() as u32);
                } else {
                    rpass.draw(0..mesh.vertex_count, 0..instances.len() as u32);
                }

                instance_offset += instances.len();
            }
        }
    }

    pub fn compute_bounds(vertices: &[Vertex3D]) -> (glam::Vec3, f32) {
        let mut min = glam::Vec3::splat(f32::INFINITY);
        let mut max = glam::Vec3::splat(f32::NEG_INFINITY);

        for v in vertices {
            let p = glam::Vec3::from_array(v.position);
            min = min.min(p);
            max = max.max(p);
        }

        let center = (min + max) * 0.5;
        let extent = 0.5 * (max - min);
        let radius = extent.length(); // → half‑diagonal length

        (center, radius)
    }

    pub fn create_cube_mesh(device: &Device) -> Mesh {
        use bytemuck::cast_slice;

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

        let (bounds_center, bounds_radius) = Self::compute_bounds(&vertices);

        Mesh {
            vertex_buffer: vb,
            index_buffer: Some(ib),
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
            bounds_center,
            bounds_radius,
        }
    }

    pub fn create_plane_mesh(device: &Device) -> Mesh {
        use bytemuck::cast_slice;

        let vertices: Vec<Vertex3D> = vec![
            Vertex3D {
                position: [-0.5, 0.0, -0.5],
                normal: [0.0, 1.0, 0.0],
            },
            Vertex3D {
                position: [0.5, 0.0, -0.5],
                normal: [0.0, 1.0, 0.0],
            },
            Vertex3D {
                position: [0.5, 0.0, 0.5],
                normal: [0.0, 1.0, 0.0],
            },
            Vertex3D {
                position: [-0.5, 0.0, 0.5],
                normal: [0.0, 1.0, 0.0],
            },
        ];

        let indices: &[u32] = &[0, 1, 2, 2, 3, 0];

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Plane VB"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Plane IB"),
            contents: cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let (bounds_center, bounds_radius) = Self::compute_bounds(&vertices);

        Mesh {
            vertex_buffer: vb,
            index_buffer: Some(ib),
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
            bounds_center,
            bounds_radius,
        }
    }

    pub fn create_sphere_mesh(device: &Device) -> Mesh {
        use bytemuck::cast_slice;
        use std::f32::consts::PI;

        let segments: u32 = 50;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Generate vertices
        for lat in 0..=segments {
            let theta = lat as f32 * PI / segments as f32;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            for lon in 0..=segments {
                let phi = lon as f32 * 2.0 * PI / segments as f32;
                let sin_phi = phi.sin();
                let cos_phi = phi.cos();

                let x = cos_phi * sin_theta;
                let y = cos_theta;
                let z = sin_phi * sin_theta;

                vertices.push(Vertex3D {
                    position: [x * 0.5, y * 0.5, z * 0.5],
                    normal: [x, y, z],
                });
            }
        }

        // Generate indices
        for lat in 0..segments {
            for lon in 0..segments {
                let first = lat * (segments + 1) + lon;
                let second = first + segments + 1;

                indices.push(first);
                indices.push(second);
                indices.push(first + 1);

                indices.push(second);
                indices.push(second + 1);
                indices.push(first + 1);
            }
        }

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere VB"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere IB"),
            contents: cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let (bounds_center, bounds_radius) = Self::compute_bounds(&vertices);

        Mesh {
            vertex_buffer: vb,
            index_buffer: Some(ib),
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
            bounds_center,
            bounds_radius,
        }
    }

    pub fn create_cylinder_mesh(device: &Device) -> Mesh {
        use bytemuck::cast_slice;
        use std::f32::consts::PI;

        let segments = 50;
        let height = 1.0;
        let radius = 0.5;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Top and bottom center points
        let top_center = Vertex3D {
            position: [0.0, height / 2.0, 0.0],
            normal: [0.0, 1.0, 0.0],
        };
        let bottom_center = Vertex3D {
            position: [0.0, -height / 2.0, 0.0],
            normal: [0.0, -1.0, 0.0],
        };

        // Side vertices
        for i in 0..=segments {
            let theta = (i as f32 / segments as f32) * 2.0 * PI;
            let x = radius * theta.cos();
            let z = radius * theta.sin();
            let normal = [x / radius, 0.0, z / radius];

            vertices.push(Vertex3D {
                position: [x, height / 2.0, z],
                normal,
            });
            vertices.push(Vertex3D {
                position: [x, -height / 2.0, z],
                normal,
            });
        }

        // Top and bottom circles
        let top_start = vertices.len() as u32;
        vertices.push(top_center);
        for i in 0..=segments {
            let theta = (i as f32 / segments as f32) * 2.0 * PI;
            let x = radius * theta.cos();
            let z = radius * theta.sin();
            vertices.push(Vertex3D {
                position: [x, height / 2.0, z],
                normal: [0.0, 1.0, 0.0],
            });
        }

        let bottom_start = vertices.len() as u32;
        vertices.push(bottom_center);
        for i in 0..=segments {
            let theta = (i as f32 / segments as f32) * 2.0 * PI;
            let x = radius * theta.cos();
            let z = radius * theta.sin();
            vertices.push(Vertex3D {
                position: [x, -height / 2.0, z],
                normal: [0.0, -1.0, 0.0],
            });
        }

        // Side indices
        for i in 0..segments {
            let top1 = i * 2;
            let bottom1 = top1 + 1;
            let top2 = ((i + 1) * 2) % ((segments + 1) * 2);
            let bottom2 = top2 + 1;

            indices.extend_from_slice(&[
                top1 as u32,
                bottom1 as u32,
                top2 as u32,
                bottom1 as u32,
                bottom2 as u32,
                top2 as u32,
            ]);
        }

        // Top cap
        for i in 1..=segments {
            indices.extend_from_slice(&[top_start, top_start + i as u32, top_start + i as u32 + 1]);
        }

        // Bottom cap
        for i in 1..=segments {
            indices.extend_from_slice(&[
                bottom_start,
                bottom_start + i as u32 + 1,
                bottom_start + i as u32,
            ]);
        }

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cylinder VB"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cylinder IB"),
            contents: cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let (bounds_center, bounds_radius) = Self::compute_bounds(&vertices);

        Mesh {
            vertex_buffer: vb,
            index_buffer: Some(ib),
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
            bounds_center,
            bounds_radius,
        }
    }

    pub fn create_capsule_mesh(device: &Device) -> Mesh {
        use bytemuck::cast_slice;
        use std::f32::consts::PI;

        let segments = 32;
        let radius = 0.5;
        let half_height = 0.5;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Hemisphere and cylinder parts
        for i in 0..=segments {
            let v = i as f32 / segments as f32;
            let theta = v * PI;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            for j in 0..=segments {
                let u = j as f32 / segments as f32;
                let phi = u * 2.0 * PI;
                let sin_phi = phi.sin();
                let cos_phi = phi.cos();

                let x = cos_phi * sin_theta;
                let y = cos_theta;
                let z = sin_phi * sin_theta;

                let mut y_pos = y * radius;

                // Offset vertically for capsule shape
                if y > 0.0 {
                    y_pos += half_height;
                } else {
                    y_pos -= half_height;
                }

                vertices.push(Vertex3D {
                    position: [x * radius, y_pos, z * radius],
                    normal: [x, y, z],
                });
            }
        }

        for i in 0..segments {
            for j in 0..segments {
                let first = i * (segments + 1) + j;
                let second = first + segments + 1;
                indices.extend_from_slice(&[
                    first,
                    second,
                    first + 1,
                    second,
                    second + 1,
                    first + 1,
                ]);
            }
        }

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Capsule VB"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Capsule IB"),
            contents: cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let (bounds_center, bounds_radius) = Self::compute_bounds(&vertices);

        Mesh {
            vertex_buffer: vb,
            index_buffer: Some(ib),
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
            bounds_center,
            bounds_radius,
        }
    }

    pub fn create_cone_mesh(device: &Device) -> Mesh {
        use bytemuck::cast_slice;
        use std::f32::consts::PI;

        let segments = 50;
        let height = 1.0;
        let radius = 0.5;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Tip and base center
        vertices.push(Vertex3D {
            position: [0.0, height / 2.0, 0.0],
            normal: [0.0, 1.0, 0.0],
        });
        vertices.push(Vertex3D {
            position: [0.0, -height / 2.0, 0.0],
            normal: [0.0, -1.0, 0.0],
        });

        let base_center_index = 1;

        // Base rim vertices
        for i in 0..=segments {
            let theta = (i as f32 / segments as f32) * 2.0 * PI;
            let x = radius * theta.cos();
            let z = radius * theta.sin();
            vertices.push(Vertex3D {
                position: [x, -height / 2.0, z],
                normal: [x, radius, z],
            });
        }

        // Side indices
        for i in 2..(2 + segments) {
            indices.extend_from_slice(&[0, i, i + 1]);
        }

        // Base
        for i in 2..(2 + segments) {
            indices.extend_from_slice(&[base_center_index, i + 1, i]);
        }

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cone VB"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cone IB"),
            contents: cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let (bounds_center, bounds_radius) = Self::compute_bounds(&vertices);

        Mesh {
            vertex_buffer: vb,
            index_buffer: Some(ib),
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
            bounds_center,
            bounds_radius,
        }
    }

    pub fn create_square_pyramid_mesh(device: &Device) -> Mesh {
        use bytemuck::cast_slice;

        let vertices = vec![
            Vertex3D {
                position: [0.0, 0.5, 0.0],
                normal: [0.0, 1.0, 0.0],
            }, // Top
            Vertex3D {
                position: [-0.5, -0.5, -0.5],
                normal: [-1.0, -1.0, -1.0],
            },
            Vertex3D {
                position: [0.5, -0.5, -0.5],
                normal: [1.0, -1.0, -1.0],
            },
            Vertex3D {
                position: [0.5, -0.5, 0.5],
                normal: [1.0, -1.0, 1.0],
            },
            Vertex3D {
                position: [-0.5, -0.5, 0.5],
                normal: [-1.0, -1.0, 1.0],
            },
        ];

        let indices: &[u32] = &[
            0, 1, 2, // Front
            0, 2, 3, // Right
            0, 3, 4, // Back
            0, 4, 1, // Left
            1, 4, 3, 3, 2, 1, // Base
        ];

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Square Pyramid VB"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Square Pyramid IB"),
            contents: cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let (bounds_center, bounds_radius) = Self::compute_bounds(&vertices);

        Mesh {
            vertex_buffer: vb,
            index_buffer: Some(ib),
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
            bounds_center,
            bounds_radius,
        }
    }

    pub fn create_triangular_pyramid_mesh(device: &Device) -> Mesh {
        use bytemuck::cast_slice;

        let vertices = vec![
            Vertex3D {
                position: [0.0, 0.5, 0.0],
                normal: [0.0, 1.0, 0.0],
            }, // Top
            Vertex3D {
                position: [-0.5, -0.5, 0.288],
                normal: [-1.0, -1.0, 0.5],
            },
            Vertex3D {
                position: [0.5, -0.5, 0.288],
                normal: [1.0, -1.0, 0.5],
            },
            Vertex3D {
                position: [0.0, -0.5, -0.577],
                normal: [0.0, -1.0, -1.0],
            },
        ];

        let indices: &[u32] = &[
            0, 1, 2, // Front face
            0, 2, 3, // Right
            0, 3, 1, // Left
            1, 3, 2, // Bottom
        ];

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Triangular Pyramid VB"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Triangular Pyramid IB"),
            contents: cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let (bounds_center, bounds_radius) = Self::compute_bounds(&vertices);

        Mesh {
            vertex_buffer: vb,
            index_buffer: Some(ib),
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
            bounds_center,
            bounds_radius,
        }
    }

    // Utility methods
    pub fn get_light_count(&self) -> usize {
        self.light_slots.iter().filter(|l| l.is_some()).count()
    }

    pub fn get_material_count(&self) -> usize {
        self.material_slots.iter().filter(|m| m.is_some()).count()
    }

    pub fn get_mesh_instance_count(&self) -> usize {
        self.mesh_instance_slots
            .iter()
            .filter(|m| m.is_some())
            .count()
    }

    pub fn get_batch_count(&self) -> usize {
        self.mesh_material_groups.len()
    }

    pub fn print_stats(&self) {
        println!("🟧 Renderer3D Stats:");
        println!("   - Lights: {}/{}", self.get_light_count(), MAX_LIGHTS);
        println!(
            "   - Materials: {}/{}",
            self.get_material_count(),
            MAX_MATERIALS
        );
        println!("   - Mesh Instances: {}", self.get_mesh_instance_count());
        println!("   - Render Batches: {}", self.get_batch_count());
        println!(
            "   - Needs Rebuild: lights={}, materials={}, instances={}",
            self.lights_need_rebuild, self.materials_need_rebuild, self.instances_need_rebuild
        );
    }
}
