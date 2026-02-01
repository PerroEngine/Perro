use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::{
    ops::Range,
    time::{Duration, Instant},
};
use wgpu::{
    BindGroupLayout, BlendState, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites,
    Device, FragmentState, PipelineLayoutDescriptor, Queue, RenderPass, RenderPipeline,
    RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, TextureFormat, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
};

use crate::{
    ids::TextureID,
    rendering::TextureManager,
    structs2d::{Transform2D, Vector2},
    ui_elements::ui_container::CornerRadius,
    vertex::Vertex,
};

const MAX_INSTANCES: usize = 100000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderLayer {
    World2D,
    UI,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable, PartialEq)]
pub struct RectInstance {
    pub transform_0: [f32; 3],
    pub transform_1: [f32; 3],
    pub transform_2: [f32; 3],

    pub color: [f32; 4],
    pub size: [f32; 2],
    pub pivot: [f32; 2],

    pub corner_radius_xy: [f32; 4],
    pub corner_radius_zw: [f32; 4],

    pub border_thickness: f32,
    pub is_border: u32,
    pub z_index: i32,
}

impl Default for RectInstance {
    fn default() -> Self {
        Self {
            transform_0: [0.0; 3],
            transform_1: [0.0; 3],
            transform_2: [0.0; 3],
            color: [0.0; 4],
            size: [0.0; 2],
            pivot: [0.0; 2],
            corner_radius_xy: [0.0; 4],
            corner_radius_zw: [0.0; 4],
            border_thickness: 0.0,
            is_border: 0,
            z_index: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextureInstance {
    pub transform_0: [f32; 3],
    pub transform_1: [f32; 3],
    pub transform_2: [f32; 3],

    pub pivot: [f32; 2],
    pub z_index: i32,
    pub _pad: f32,
}

impl Default for TextureInstance {
    fn default() -> Self {
        Self {
            transform_0: [0.0; 3],
            transform_1: [0.0; 3],
            transform_2: [0.0; 3],

            pivot: [0.0; 2],
            z_index: 0,
            _pad: 0.0,
        }
    }
}
/// Platform sprite output: 0 = linear (Mac/Linux sRGB swapchain), 1 = sRGB in shader (Windows).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SpriteOutputConfig {
    output_srgb: u32,
    _pad: [u32; 3],
}

pub struct PrimitiveRenderer {
    rect_instance_buffer: wgpu::Buffer,
    texture_instance_buffer: wgpu::Buffer,

    rect_instanced_pipeline: RenderPipeline,
    texture_instanced_pipeline: RenderPipeline,

    texture_bind_group_layout: BindGroupLayout,
    sprite_output_config_bind_group: wgpu::BindGroup,

    // Optimized rect storage
    rect_instance_slots: Vec<Option<(RenderLayer, RectInstance, u64)>>, // Added timestamp for sorting
    rect_id_to_slot: FxHashMap<u64, usize>,
    free_rect_slots: SmallVec<[usize; 16]>,
    rect_dirty_ranges: SmallVec<[Range<usize>; 8]>,

    // Optimized texture storage (by TextureID; path only at load time)
    texture_instance_slots: Vec<Option<(RenderLayer, TextureInstance, TextureID, Vector2, u64)>>,
    texture_id_to_slot: FxHashMap<u64, usize>,
    free_texture_slots: SmallVec<[usize; 16]>,
    texture_dirty_ranges: SmallVec<[Range<usize>; 8]>,

    // Rendered instances (built from slots when needed)
    world_rect_instances: Vec<RectInstance>,
    ui_rect_instances: Vec<RectInstance>,
    world_texture_groups: Vec<(TextureID, Vec<TextureInstance>)>,
    ui_texture_groups: Vec<(TextureID, Vec<TextureInstance>)>,

    world_texture_group_offsets: Vec<(usize, usize)>,
    ui_texture_group_offsets: Vec<(usize, usize)>,
    world_texture_buffer_ranges: Vec<Range<u64>>,
    ui_texture_buffer_ranges: Vec<Range<u64>>,

    temp_all_texture_instances: Vec<TextureInstance>,

    // Batching optimization fields
    last_rebuild_time: Instant,
    dirty_count: usize,
    max_rebuild_interval: Duration,
    dirty_threshold: usize,

    instances_need_rebuild: bool,
    /// Set when add/remove or z_index changes; forces full rebuild. When false, we can partial-update only dirty slots.
    structure_changed: bool,

    /// Slot index -> (layer, index in world_rect_instances or ui_rect_instances). Built on full rebuild; used for partial uploads.
    rect_slot_to_buffer: Vec<Option<(RenderLayer, usize)>>,
    /// Slot index -> flat index in texture instance buffer. Built on full rebuild; used for partial uploads.
    texture_slot_to_flat_index: Vec<Option<usize>>,

    // OPTIMIZED: 2D viewport culling - cache camera info
    camera_position: Vector2,
    camera_rotation: f32,
    camera_zoom: f32,
    viewport_enabled: bool,
    /// Virtual resolution (from project [graphics]) for viewport culling
    virtual_width: f32,
    virtual_height: f32,
}

impl PrimitiveRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        camera_bgl: &BindGroupLayout,
        format: TextureFormat,
        virtual_width: f32,
        virtual_height: f32,
        sample_count: u32,
    ) -> Self {
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture BGL"),
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
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                ],
            });

        let sprite_output_config_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Sprite Output Config BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                }],
            });

        let output_srgb = if cfg!(target_os = "windows") { 1u32 } else { 0u32 };
        let output_config = SpriteOutputConfig {
            output_srgb,
            _pad: [0, 0, 0],
        };
        let output_config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sprite Output Config Buffer"),
            size: 16,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &output_config_buffer,
            0,
            bytemuck::bytes_of(&output_config),
        );
        let sprite_output_config_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Sprite Output Config Bind Group"),
                layout: &sprite_output_config_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: output_config_buffer.as_entire_binding(),
                }],
            });

        let rect_instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Rect Instance Buffer"),
            size: (std::mem::size_of::<RectInstance>() * MAX_INSTANCES) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let texture_instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Texture Instance Buffer"),
            size: (std::mem::size_of::<TextureInstance>() * MAX_INSTANCES) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let rect_instanced_pipeline =
            Self::create_rect_pipeline(device, camera_bgl, format, sample_count);
        let texture_instanced_pipeline = Self::create_texture_pipeline(
            device,
            &texture_bind_group_layout,
            camera_bgl,
            &sprite_output_config_bgl,
            format,
            sample_count,
        );

        Self {
            rect_instance_buffer,
            texture_instance_buffer,
            rect_instanced_pipeline,
            texture_instanced_pipeline,
            texture_bind_group_layout,
            sprite_output_config_bind_group,

            // Optimized storage
            rect_instance_slots: Vec::with_capacity(MAX_INSTANCES),
            rect_id_to_slot: FxHashMap::default(),
            free_rect_slots: SmallVec::new(),
            rect_dirty_ranges: SmallVec::new(),

            texture_instance_slots: Vec::with_capacity(MAX_INSTANCES),
            texture_id_to_slot: FxHashMap::default(),
            free_texture_slots: SmallVec::new(),
            texture_dirty_ranges: SmallVec::new(),

            world_rect_instances: Vec::new(),
            ui_rect_instances: Vec::new(),
            world_texture_groups: Vec::new(),
            ui_texture_groups: Vec::new(),
            world_texture_group_offsets: Vec::new(),
            ui_texture_group_offsets: Vec::new(),
            world_texture_buffer_ranges: Vec::new(),
            ui_texture_buffer_ranges: Vec::new(),
            temp_all_texture_instances: Vec::new(),

            // Batching optimization
            last_rebuild_time: Instant::now(),
            dirty_count: 0,
            max_rebuild_interval: Duration::from_millis(16), // ~60 FPS max
            dirty_threshold: 100, // Rebuild when 100+ elements are dirty (reduced rebuild frequency)

            instances_need_rebuild: false,
            structure_changed: false,
            rect_slot_to_buffer: Vec::new(),
            texture_slot_to_flat_index: Vec::new(),

            // OPTIMIZED: Initialize camera culling info
            camera_position: Vector2::ZERO,
            camera_rotation: 0.0,
            camera_zoom: 1.0,
            viewport_enabled: false,
            virtual_width,
            virtual_height,
        }
    }

    /// OPTIMIZED: Update camera info for viewport culling (only for World2D layer)
    pub fn update_camera_2d(&mut self, position: Vector2, rotation: f32, zoom: f32) {
        self.camera_position = position;
        self.camera_rotation = rotation;
        self.camera_zoom = zoom;
        self.viewport_enabled = true;
    }

    /// OPTIMIZED: Check if a sprite AABB is fully outside the viewport
    fn is_sprite_offscreen(&self, transform: &Transform2D, size: &Vector2) -> bool {
        if !self.viewport_enabled {
            return false; // No culling if camera not set
        }

        // Calculate viewport bounds in world space (axis-aligned, ignoring camera rotation for simplicity)
        let viewport_half_width = (self.virtual_width / self.camera_zoom) * 0.5;
        let viewport_half_height = (self.virtual_height / self.camera_zoom) * 0.5;

        let viewport_min_x = self.camera_position.x - viewport_half_width;
        let viewport_max_x = self.camera_position.x + viewport_half_width;
        let viewport_min_y = self.camera_position.y - viewport_half_height;
        let viewport_max_y = self.camera_position.y + viewport_half_height;

        // Calculate sprite AABB in world space
        // For rotated sprites, compute the axis-aligned bounding box of the rotated rectangle
        let scaled_size = Vector2::new(size.x * transform.scale.x, size.y * transform.scale.y);

        // Calculate the four corners of the sprite in local space (centered at origin)
        let half_w = scaled_size.x * 0.5;
        let half_h = scaled_size.y * 0.5;
        let corners_local = [
            Vector2::new(-half_w, -half_h),
            Vector2::new(half_w, -half_h),
            Vector2::new(half_w, half_h),
            Vector2::new(-half_w, half_h),
        ];

        // Rotate and translate corners to world space (Transform2D.rotation is in degrees)
        let r = transform.rotation.to_radians();
        let cos_r = r.cos();
        let sin_r = r.sin();
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for corner in &corners_local {
            // Rotate
            let rotated = Vector2::new(
                corner.x * cos_r - corner.y * sin_r,
                corner.x * sin_r + corner.y * cos_r,
            );
            // Translate
            let world = rotated + transform.position;

            min_x = min_x.min(world.x);
            max_x = max_x.max(world.x);
            min_y = min_y.min(world.y);
            max_y = max_y.max(world.y);
        }

        // Check if sprite AABB is fully outside viewport
        max_x < viewport_min_x
            || min_x > viewport_max_x
            || max_y < viewport_min_y
            || min_y > viewport_max_y
    }

    /// Extract Transform2D from TextureInstance matrix
    #[allow(dead_code)]
    fn texture_instance_to_transform(instance: &TextureInstance) -> Transform2D {
        // Reconstruct Mat3 (column-major)
        let m = glam::Mat3::from_cols(
            glam::Vec3::from(instance.transform_0),
            glam::Vec3::from(instance.transform_1),
            glam::Vec3::from(instance.transform_2),
        );

        // Translation
        let position = Vector2::new(m.z_axis.x, m.z_axis.y);

        // Scale (length of basis vectors)
        let scale_x = m.x_axis.truncate().length();
        let scale_y = m.y_axis.truncate().length();
        let scale = Vector2::new(scale_x, scale_y);

        // Rotation (from normalized x axis, atan2 returns radians; Transform2D stores degrees)
        let rotation_rad = (m.x_axis.y / scale_x).atan2(m.x_axis.x / scale_x);

        Transform2D {
            position,
            scale,
            rotation: rotation_rad.to_degrees(),
        }
    }

    /// Calculate axis-aligned bounding box (AABB) for an object with given transform and size
    #[allow(dead_code)]
    fn calculate_aabb(transform: &Transform2D, size: &Vector2) -> (f32, f32, f32, f32) {
        // Calculate scaled size
        let scaled_size = Vector2::new(size.x * transform.scale.x, size.y * transform.scale.y);
        let half_w = scaled_size.x * 0.5;
        let half_h = scaled_size.y * 0.5;

        // For simplicity, use axis-aligned bounding box (ignoring rotation for now)
        // This is a conservative approximation - if rotation is significant, the AABB will be larger
        let min_x = transform.position.x - half_w;
        let max_x = transform.position.x + half_w;
        let min_y = transform.position.y - half_h;
        let max_y = transform.position.y + half_h;

        (min_x, min_y, max_x, max_y)
    }

    /// Check if AABB a completely contains AABB b
    #[allow(dead_code)]
    fn aabb_contains(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
        // a contains b if a's bounds completely enclose b's bounds
        a.0 <= b.0 && a.1 <= b.1 && a.2 >= b.2 && a.3 >= b.3
    }

    /// Check if a visual object is occluded by any texture instances with higher z_index
    /// Returns (is_occluded, occluder_info) where occluder_info is for debug printing
    #[allow(dead_code)]
    fn is_visual_occluded_by_textures(
        &self,
        visual_transform: &Transform2D,
        visual_size: &Vector2,
        visual_z_index: i32,
        visual_type: &str,
    ) -> (bool, Option<String>) {
        let visual_aabb = Self::calculate_aabb(visual_transform, visual_size);

        // Check against all texture instances with higher z_index
        for slot in &self.texture_instance_slots {
            if let Some((layer, texture_instance, texture_id, _texture_size, _timestamp)) = slot {
                if *layer == RenderLayer::World2D && texture_instance.z_index > visual_z_index {
                    let texture_transform = Self::texture_instance_to_transform(texture_instance);
                    let texture_size = texture_transform.scale;
                    let texture_aabb = Self::calculate_aabb(&texture_transform, &texture_size);
                    if Self::aabb_contains(texture_aabb, visual_aabb) {
                        let occluder_info = format!(
                            "{} (z={}) occluded by sprite {:?} (z={})",
                            visual_type, visual_z_index, texture_id, texture_instance.z_index
                        );
                        return (true, Some(occluder_info));
                    }
                }
            }
        }

        (false, None)
    }

    pub fn queue_rect(
        &mut self,
        id: u64,
        layer: RenderLayer,
        transform: Transform2D,
        size: Vector2,
        pivot: Vector2,
        color: crate::structs::Color,
        corner_radius: Option<CornerRadius>,
        border_thickness: f32,
        is_border: bool,
        z_index: i32,
        created_timestamp: u64,
    ) {
        // VALIDATION: Skip zero-size, negative, NaN, or infinite sizes to prevent buffer issues
        if size.x <= 0.0 || size.y <= 0.0 || !size.x.is_finite() || !size.y.is_finite() {
            // Remove from slots if it exists (element became invalid)
            if let Some(&slot) = self.rect_id_to_slot.get(&id) {
                if let Some(_existing) = &mut self.rect_instance_slots[slot] {
                    self.rect_instance_slots[slot] = None;
                    self.free_rect_slots.push(slot);
                    self.rect_id_to_slot.remove(&id);
                    self.mark_rect_slot_dirty(slot);
                    self.dirty_count += 1;
                    self.instances_need_rebuild = true;
                    self.structure_changed = true;
                }
            }
            return; // Don't queue invalid sizes
        }

        // VALIDATION: Check for invalid transform values
        if !transform.position.x.is_finite()
            || !transform.position.y.is_finite()
            || !transform.scale.x.is_finite()
            || !transform.scale.y.is_finite()
            || !transform.rotation.is_finite()
        {
            // Remove from slots if it exists (element has invalid transform)
            if let Some(&slot) = self.rect_id_to_slot.get(&id) {
                if let Some(_existing) = &mut self.rect_instance_slots[slot] {
                    self.rect_instance_slots[slot] = None;
                    self.free_rect_slots.push(slot);
                    self.rect_id_to_slot.remove(&id);
                    self.mark_rect_slot_dirty(slot);
                    self.dirty_count += 1;
                    self.instances_need_rebuild = true;
                    self.structure_changed = true;
                }
            }
            return; // Don't queue invalid transforms
        }

        // OPTIMIZED: Viewport culling - skip offscreen sprites (only for World2D)
        if layer == RenderLayer::World2D && self.is_sprite_offscreen(&transform, &size) {
            // Remove from slots if it exists (sprite moved offscreen)
            if let Some(&slot) = self.rect_id_to_slot.get(&id) {
                if let Some(_existing) = &mut self.rect_instance_slots[slot] {
                    // Mark as removed by setting to None
                    self.rect_instance_slots[slot] = None;
                    self.free_rect_slots.push(slot);
                    self.rect_id_to_slot.remove(&id);
                    self.mark_rect_slot_dirty(slot);
                    self.dirty_count += 1;
                    self.instances_need_rebuild = true;
                    self.structure_changed = true;
                }
            }
            return; // Don't queue offscreen sprites
        }

        // DISABLED: Occlusion culling - O(n²) performance bottleneck
        // The GPU can handle overdraw efficiently, and modern GPUs are fast at fragment shading
        // if layer == RenderLayer::World2D {
        //     let (is_occluded, _occluder_info) = self.is_visual_occluded_by_textures(&transform, &size, z_index, "shape");
        //     if is_occluded {
        //         // Remove from slots if it exists (shape is occluded)
        //         if let Some(&slot) = self.rect_id_to_slot.get(&id) {
        //             if let Some(_existing) = &mut self.rect_instance_slots[slot] {
        //                 self.rect_instance_slots[slot] = None;
        //                 self.free_rect_slots.push(slot);
        //                 self.rect_id_to_slot.remove(&id);
        //                 self.mark_rect_slot_dirty(slot);
        //                 self.dirty_count += 1;
        //                 self.instances_need_rebuild = true;
        //             }
        //         }
        //         return; // Don't queue occluded shapes
        //     }
        // }

        let new_instance = self.create_rect_instance(
            transform,
            size,
            pivot,
            color,
            corner_radius,
            border_thickness,
            is_border,
            z_index,
            created_timestamp,
        );

        // Check if this rect already exists
        if let Some(&slot) = self.rect_id_to_slot.get(&id) {
            // Update existing slot if changed
            if let Some(ref mut existing) = self.rect_instance_slots[slot] {
                if existing.0 != layer || existing.1 != new_instance {
                    let order_changed =
                        existing.0 != layer || existing.1.z_index != new_instance.z_index;
                    if order_changed {
                        self.structure_changed = true;
                    }
                    existing.0 = layer;
                    existing.1 = new_instance;
                    // Keep existing timestamp
                    self.mark_rect_slot_dirty(slot);
                    self.dirty_count += 1;
                    self.instances_need_rebuild = true;
                }
            }
        } else {
            // Allocate new slot
            self.structure_changed = true;
            let slot = if let Some(free_slot) = self.free_rect_slots.pop() {
                free_slot
            } else {
                // Check if we're about to exceed MAX_INSTANCES
                if self.rect_instance_slots.len() >= MAX_INSTANCES {
                    eprintln!(
                        "⚠️ WARNING: Rect instance buffer full ({} instances). Skipping panel with UUID {}",
                        MAX_INSTANCES, id
                    );
                    return; // Don't queue if buffer is full
                }
                let new_slot = self.rect_instance_slots.len();
                self.rect_instance_slots.push(None);
                new_slot
            };

            self.rect_instance_slots[slot] = Some((layer, new_instance, created_timestamp));
            self.rect_id_to_slot.insert(id, slot);
            self.mark_rect_slot_dirty(slot);
            self.dirty_count += 1;
            self.instances_need_rebuild = true;
        }
    }

    /// Remove a rect instance from the render cache
    /// Call this when an element becomes invisible to clear it from GPU buffers
    pub fn remove_rect(&mut self, id: u64) {
        if let Some(&slot) = self.rect_id_to_slot.get(&id) {
            if let Some(_existing) = &mut self.rect_instance_slots[slot] {
                self.rect_instance_slots[slot] = None;
                self.free_rect_slots.push(slot);
                self.rect_id_to_slot.remove(&id);
                self.mark_rect_slot_dirty(slot);
                self.dirty_count += 1;
                self.instances_need_rebuild = true;
            }
        }
    }

    /// Remove a text instance from the render cache
    /// Call this when an element becomes invisible to clear it from GPU buffers
    pub fn remove_text(&mut self, _id: u64) {
        // Text rendering now handled by egui
    }

    pub fn queue_texture(
        &mut self,
        id: u64,
        layer: RenderLayer,
        texture_id: TextureID,
        transform: Transform2D,
        pivot: Vector2,
        z_index: i32,
        created_timestamp: u64,
        texture_manager: &mut crate::rendering::TextureManager,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) {
        let tex_size = texture_manager
            .get_texture_size_by_id(&texture_id)
            .unwrap_or_else(|| Vector2::new(1.0, 1.0));

        // Create a *new* version for rendering
        let adjusted_transform = Transform2D {
            position: transform.position,
            scale: Vector2::new(
                transform.scale.x * tex_size.x,
                transform.scale.y * tex_size.y,
            ),
            rotation: transform.rotation,
        };

        // OPTIMIZED: Viewport culling - skip offscreen sprites (only for World2D).
        // We clear the draw slot so we don't render this frame, but we do NOT unregister the
        // texture user: the node still "owns" the texture and may come back on screen.
        if layer == RenderLayer::World2D && self.is_sprite_offscreen(&adjusted_transform, &tex_size)
        {
            if let Some(&slot) = self.texture_id_to_slot.get(&id) {
                if self.texture_instance_slots[slot].is_some() {
                    self.texture_instance_slots[slot] = None;
                    self.free_texture_slots.push(slot);
                    self.texture_id_to_slot.remove(&id);
                    self.mark_texture_slot_dirty(slot);
                    self.dirty_count += 1;
                    self.instances_need_rebuild = true;
                    self.structure_changed = true;
                }
            }
            return;
        }

        // DISABLED: Occlusion culling - O(n²) performance bottleneck (9M comparisons for 3000 sprites)
        // The GPU can handle overdraw efficiently, and modern GPUs are fast at fragment shading
        // If occlusion culling is needed in the future, implement spatial indexing (quadtree/spatial hash)
        // if layer == RenderLayer::World2D {
        //     let (is_occluded, _occluder_info) = self.is_visual_occluded_by_textures(&transform, &tex_size, z_index, "sprite");
        //     if is_occluded {
        //         // Remove from slots if it exists (sprite is occluded)
        //         if let Some(&slot) = self.texture_id_to_slot.get(&id) {
        //             if let Some(_existing) = &mut self.texture_instance_slots[slot] {
        //                 self.texture_instance_slots[slot] = None;
        //                 self.free_texture_slots.push(slot);
        //                 self.texture_id_to_slot.remove(&id);
        //                 self.mark_texture_slot_dirty(slot);
        //                 self.dirty_count += 1;
        //                 self.instances_need_rebuild = true;
        //             }
        //         }
        //         return; // Don't queue occluded sprites
        //     }
        // }

        if let Some(&slot) = self.texture_id_to_slot.get(&id) {
            if let Some(existing) = &self.texture_instance_slots[slot] {
                if existing.0 == layer && existing.2 == texture_id {
                    // Create new instance to compare (but we need tex_size first, which we already have)
                    let test_instance = self.create_texture_instance(
                        Transform2D {
                            position: transform.position,
                            scale: Vector2::new(
                                transform.scale.x * tex_size.x,
                                transform.scale.y * tex_size.y,
                            ),
                            rotation: transform.rotation,
                        },
                        pivot,
                        z_index,
                        created_timestamp,
                    );

                    // Compare instance data directly (much faster than matrix reconstruction)
                    if existing.1.transform_0 == test_instance.transform_0
                        && existing.1.transform_1 == test_instance.transform_1
                        && existing.1.transform_2 == test_instance.transform_2
                        && existing.1.pivot == test_instance.pivot
                        && existing.1.z_index == test_instance.z_index
                    {
                        // Nothing changed, skip all the expensive work
                        return;
                    }
                }
            }
        }

        let new_instance =
            self.create_texture_instance(adjusted_transform, pivot, z_index, created_timestamp);

        if let Some(&slot) = self.texture_id_to_slot.get(&id) {
            if let Some(ref mut existing) = self.texture_instance_slots[slot] {
                let instance_changed = existing.1.transform_0 != new_instance.transform_0
                    || existing.1.transform_1 != new_instance.transform_1
                    || existing.1.transform_2 != new_instance.transform_2
                    || existing.1.pivot != new_instance.pivot
                    || existing.1.z_index != new_instance.z_index;
                if existing.0 != layer || instance_changed || existing.2 != texture_id {
                    if existing.2 != texture_id {
                        texture_manager.remove_texture_user(existing.2, id);
                    }
                    let order_or_group_changed = existing.0 != layer
                        || existing.1.z_index != new_instance.z_index
                        || existing.2 != texture_id;
                    if order_or_group_changed {
                        self.structure_changed = true;
                    }
                    existing.0 = layer;
                    existing.1 = new_instance;
                    existing.2 = texture_id;
                    existing.3 = tex_size;
                    texture_manager.add_texture_user(texture_id, id);
                    self.mark_texture_slot_dirty(slot);
                    self.dirty_count += 1;
                    self.instances_need_rebuild = true;
                }
            }
        } else {
            self.structure_changed = true;
            texture_manager.add_texture_user(texture_id, id);
            let slot = if let Some(free_slot) = self.free_texture_slots.pop() {
                free_slot
            } else {
                let new_slot = self.texture_instance_slots.len();
                self.texture_instance_slots.push(None);
                new_slot
            };
            self.texture_instance_slots[slot] =
                Some((layer, new_instance, texture_id, tex_size, created_timestamp));
            self.texture_id_to_slot.insert(id, slot);
            self.mark_texture_slot_dirty(slot);
            self.dirty_count += 1;
            self.instances_need_rebuild = true;
        }
    }

    pub fn queue_text(
        &mut self,
        _id: u64,
        _layer: RenderLayer,
        _text: &str,
        _font_size: f32,
        _transform: Transform2D,
        _pivot: Vector2,
        _color: crate::structs::Color,
        _z_index: i32,
        _created_timestamp: u64,
        _device: &Device,
        _queue: &Queue,
    ) {
        // Text rendering now handled by egui
    }

    pub fn queue_text_aligned(
        &mut self,
        _id: u64,
        _layer: RenderLayer,
        _text: &str,
        _font_size: f32,
        _transform: Transform2D,
        _pivot: Vector2,
        _color: crate::structs::Color,
        _z_index: i32,
        _created_timestamp: u64,
        _align_h: crate::ui_elements::ui_text::TextAlignment,
        _align_v: crate::ui_elements::ui_text::TextAlignment,
        _device: &Device,
        _queue: &Queue,
    ) {
        // Text rendering now handled by egui
    }

    pub fn queue_text_aligned_with_font(
        &mut self,
        _id: u64,
        _layer: RenderLayer,
        _text: &str,
        _font_size: f32,
        _transform: Transform2D,
        _pivot: Vector2,
        _color: crate::structs::Color,
        _z_index: i32,
        _created_timestamp: u64,
        _align_h: crate::ui_elements::ui_text::TextAlignment,
        _align_v: crate::ui_elements::ui_text::TextAlignment,
        _font_spec: Option<&str>,
        _device: &Device,
        _queue: &Queue,
    ) {
        // Text rendering now handled by egui
    }

    // OPTIMIZED: Proper range tracking with merging
    fn mark_rect_slot_dirty(&mut self, slot: usize) {
        let new_range = slot..(slot + 1);

        // Try to merge with existing ranges
        let mut merged = false;
        for existing_range in &mut self.rect_dirty_ranges {
            if new_range.start <= existing_range.end && new_range.end >= existing_range.start {
                // Overlapping ranges - merge them
                existing_range.start = existing_range.start.min(new_range.start);
                existing_range.end = existing_range.end.max(new_range.end);
                merged = true;
                break;
            }
        }

        if !merged {
            self.rect_dirty_ranges.push(new_range);
        }

        // Keep ranges list small by periodically consolidating
        if self.rect_dirty_ranges.len() > 16 {
            self.consolidate_dirty_ranges();
        }
    }

    fn mark_texture_slot_dirty(&mut self, slot: usize) {
        let new_range = slot..(slot + 1);

        // Try to merge with existing ranges
        let mut merged = false;
        for existing_range in &mut self.texture_dirty_ranges {
            if new_range.start <= existing_range.end && new_range.end >= existing_range.start {
                existing_range.start = existing_range.start.min(new_range.start);
                existing_range.end = existing_range.end.max(new_range.end);
                merged = true;
                break;
            }
        }

        if !merged {
            self.texture_dirty_ranges.push(new_range);
        }

        if self.texture_dirty_ranges.len() > 16 {
            self.consolidate_texture_dirty_ranges();
        }
    }

    // OPTIMIZED: Consolidate overlapping ranges
    fn consolidate_dirty_ranges(&mut self) {
        if self.rect_dirty_ranges.len() <= 1 {
            return;
        }

        self.rect_dirty_ranges.sort_by_key(|r| r.start);
        // Consolidate in-place to avoid allocations
        // Collect ranges first to avoid borrow conflicts, then write back
        let ranges: SmallVec<[Range<usize>; 8]> = self.rect_dirty_ranges.iter().cloned().collect();
        let mut consolidated = SmallVec::<[Range<usize>; 8]>::new();

        if let Some(mut current) = ranges.first().cloned() {
            for range in ranges.iter().skip(1) {
                if range.start <= current.end {
                    current.end = current.end.max(range.end);
                } else {
                    consolidated.push(current);
                    current = range.clone();
                }
            }
            consolidated.push(current);
        }

        // Write back consolidated ranges
        self.rect_dirty_ranges.clear();
        self.rect_dirty_ranges.extend(consolidated);
    }

    fn consolidate_texture_dirty_ranges(&mut self) {
        if self.texture_dirty_ranges.len() <= 1 {
            return;
        }

        self.texture_dirty_ranges.sort_by_key(|r| r.start);
        // Consolidate in-place to avoid allocations
        // Collect ranges first to avoid borrow conflicts, then write back
        let ranges: SmallVec<[Range<usize>; 8]> =
            self.texture_dirty_ranges.iter().cloned().collect();
        let mut consolidated = SmallVec::<[Range<usize>; 8]>::new();

        if let Some(mut current) = ranges.first().cloned() {
            for range in ranges.iter().skip(1) {
                if range.start <= current.end {
                    current.end = current.end.max(range.end);
                } else {
                    consolidated.push(current);
                    current = range.clone();
                }
            }
            consolidated.push(current);
        }

        // Write back consolidated ranges
        self.texture_dirty_ranges.clear();
        self.texture_dirty_ranges.extend(consolidated);
    }

    #[allow(dead_code)]
    pub fn initialize_font_atlas(
        &mut self,
        _device: &Device,
        _queue: &Queue,
        _font_atlas: crate::font::FontAtlas,
    ) {
        // DEPRECATED: This method is kept for compatibility but does nothing
        // Native text rendering uses glyph_atlas instead, initialized on-demand
    }

    /// Stops rendering the given node (rect + texture). Unregisters texture user for eviction.
    pub fn stop_rendering(&mut self, id: u64, texture_manager: &mut TextureManager) {
        // Remove from rect slots
        if let Some(slot) = self.rect_id_to_slot.remove(&id) {
            self.rect_instance_slots[slot] = None;
            self.free_rect_slots.push(slot);
            self.mark_rect_slot_dirty(slot);
            self.dirty_count += 1;
            self.instances_need_rebuild = true;
            self.structure_changed = true;
        }

        // Remove from texture slots and unregister texture user for eviction
        if let Some(slot) = self.texture_id_to_slot.remove(&id) {
            if let Some(ref existing) = self.texture_instance_slots[slot] {
                texture_manager.remove_texture_user(existing.2, id);
            }
            self.texture_instance_slots[slot] = None;
            self.free_texture_slots.push(slot);
            self.mark_texture_slot_dirty(slot);
            self.dirty_count += 1;
            self.structure_changed = true;
            self.instances_need_rebuild = true;
        }

        // Text rendering now handled by egui
    }

    // OPTIMIZED: Smart batching with time and dirty count thresholds
    pub fn render_layer(
        &mut self,
        layer: RenderLayer,
        rpass: &mut RenderPass<'_>,
        texture_manager: &mut TextureManager,
        device: &Device,
        queue: &Queue,
        camera_bind_group: &wgpu::BindGroup,
        vertex_buffer: &wgpu::Buffer,
    ) {
        let now = Instant::now();
        let time_since_rebuild = now.duration_since(self.last_rebuild_time);

        // When only transforms changed (no add/remove/z_index), partial-update dirty rect and/or
        // texture slots to avoid full buffer re-upload every frame.
        if self.instances_need_rebuild {
            let rect_mapping_ok = self.rect_slot_to_buffer.len() >= self.rect_instance_slots.len();
            let texture_mapping_ok =
                self.texture_slot_to_flat_index.len() >= self.texture_instance_slots.len();
            let rect_dirty = !self.rect_dirty_ranges.is_empty();
            let texture_dirty = !self.texture_dirty_ranges.is_empty();
            let can_partial = !self.structure_changed
                && ((rect_dirty && rect_mapping_ok) || (texture_dirty && texture_mapping_ok));

            if can_partial {
                let mut updated_count = 0usize;
                if rect_dirty && rect_mapping_ok {
                    updated_count += self
                        .rect_dirty_ranges
                        .iter()
                        .map(|r| r.len())
                        .sum::<usize>();
                    self.partial_update_rects_to_gpu(queue);
                }
                if texture_dirty && texture_mapping_ok {
                    updated_count += self
                        .texture_dirty_ranges
                        .iter()
                        .map(|r| r.len())
                        .sum::<usize>();
                    self.partial_update_textures_to_gpu(queue);
                }
                self.instances_need_rebuild = false;
                self.dirty_count = self.dirty_count.saturating_sub(updated_count);
            } else if self.structure_changed
                || self.dirty_count >= self.dirty_threshold
                || time_since_rebuild >= self.max_rebuild_interval
            {
                self.rebuild_instances(queue);
                self.instances_need_rebuild = false;
                self.dirty_count = 0;
                self.last_rebuild_time = now;
            }
        }

        // Text rendering now handled by egui

        match layer {
            RenderLayer::World2D => {
                self.render_rects(
                    &self.world_rect_instances,
                    rpass,
                    camera_bind_group,
                    vertex_buffer,
                    0, // World instances start at offset 0
                );
                self.render_textures(
                    &self.world_texture_groups,
                    &self.world_texture_group_offsets,
                    &self.world_texture_buffer_ranges,
                    rpass,
                    texture_manager,
                    device,
                    queue,
                    camera_bind_group,
                    vertex_buffer,
                );
                // Text rendering now handled by egui
            }

            RenderLayer::UI => {
                let ui_instance_offset = self.world_rect_instances.len() as u32;
                self.render_rects(
                    &self.ui_rect_instances,
                    rpass,
                    camera_bind_group,
                    vertex_buffer,
                    ui_instance_offset, // UI instances start after world instances
                );
                self.render_textures(
                    &self.ui_texture_groups,
                    &self.ui_texture_group_offsets,
                    &self.ui_texture_buffer_ranges,
                    rpass,
                    texture_manager,
                    device,
                    queue,
                    camera_bind_group,
                    vertex_buffer,
                );
                // Text rendering now handled by egui
            }
        }
    }

    fn create_rect_instance(
        &self,
        transform: Transform2D,
        size: Vector2,
        pivot: Vector2,
        color: crate::structs::Color,
        corner_radius: Option<CornerRadius>,
        border_thickness: f32,
        is_border: bool,
        z_index: i32,
        _created_timestamp: u64,
    ) -> RectInstance {
        fn srgb_to_linear(c: f32) -> f32 {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }

        let color_lin = [
            srgb_to_linear(color.r as f32 / 255.0),
            srgb_to_linear(color.g as f32 / 255.0),
            srgb_to_linear(color.b as f32 / 255.0),
            color.a as f32 / 255.0,
        ];

        let cr = corner_radius.unwrap_or_default();
        let sx = transform.scale.x.abs();
        let sy = transform.scale.y.abs();
        let scaled_size_x = size.x * sx;
        let scaled_size_y = size.y * sy;
        let max_radius = (scaled_size_x.min(scaled_size_y)) * 0.5;

        let corner_radius_xy = [
            cr.top_left * max_radius,
            cr.top_left * max_radius,
            cr.top_right * max_radius,
            cr.top_right * max_radius,
        ];
        let corner_radius_zw = [
            cr.bottom_right * max_radius,
            cr.bottom_right * max_radius,
            cr.bottom_left * max_radius,
            cr.bottom_left * max_radius,
        ];

        let mat = transform.to_mat3().to_cols_array();

        RectInstance {
            transform_0: [mat[0], mat[1], mat[2]],
            transform_1: [mat[3], mat[4], mat[5]],
            transform_2: [mat[6], mat[7], mat[8]],

            color: color_lin,
            size: [scaled_size_x, scaled_size_y],
            pivot: [pivot.x, pivot.y],
            corner_radius_xy,
            corner_radius_zw,
            border_thickness,
            is_border: is_border as u32,
            z_index,
        }
    }

    fn create_texture_instance(
        &self,
        transform: Transform2D,
        pivot: Vector2,
        z_index: i32,
        _created_timestamp: u64,
    ) -> TextureInstance {
        let mat = transform.to_mat3().to_cols_array();

        TextureInstance {
            transform_0: [mat[0], mat[1], mat[2]],
            transform_1: [mat[3], mat[4], mat[5]],
            transform_2: [mat[6], mat[7], mat[8]],

            pivot: [pivot.x, pivot.y],
            z_index,
            _pad: 0.0,
        }
    }

    // OPTIMIZED: Full rebuild with slot->buffer mapping for later partial uploads
    fn rebuild_instances(&mut self, queue: &Queue) {
        // Rebuild from slots and build slot->buffer index mapping
        self.world_rect_instances.clear();
        self.ui_rect_instances.clear();

        let cap = self.rect_instance_slots.len();
        let mut world_with_ts: Vec<(usize, RectInstance, u64)> = Vec::with_capacity(cap);
        let mut ui_with_ts: Vec<(usize, RectInstance, u64)> = Vec::with_capacity(cap);

        for (slot_idx, slot) in self.rect_instance_slots.iter().enumerate() {
            if let Some((layer, instance, timestamp)) = slot {
                match layer {
                    RenderLayer::World2D => world_with_ts.push((slot_idx, *instance, *timestamp)),
                    RenderLayer::UI => ui_with_ts.push((slot_idx, *instance, *timestamp)),
                }
            }
        }

        if world_with_ts.len() > 1 {
            world_with_ts.sort_by(|a, b| a.1.z_index.cmp(&b.1.z_index).then_with(|| a.2.cmp(&b.2)));
        }
        if ui_with_ts.len() > 1 {
            ui_with_ts.sort_by(|a, b| a.1.z_index.cmp(&b.1.z_index).then_with(|| a.2.cmp(&b.2)));
        }

        self.rect_slot_to_buffer
            .resize(self.rect_instance_slots.len(), None);
        for (slot_idx, inst, _) in &world_with_ts {
            let idx = self.world_rect_instances.len();
            self.world_rect_instances.push(*inst);
            self.rect_slot_to_buffer[*slot_idx] = Some((RenderLayer::World2D, idx));
        }
        for (slot_idx, inst, _) in &ui_with_ts {
            let idx = self.ui_rect_instances.len();
            self.ui_rect_instances.push(*inst);
            self.rect_slot_to_buffer[*slot_idx] = Some((RenderLayer::UI, idx));
        }

        self.rebuild_texture_groups_by_layer();
        self.upload_instances_to_gpu(queue);

        self.rect_dirty_ranges.clear();
        self.texture_dirty_ranges.clear();
        self.structure_changed = false;
    }

    // Text rendering now handled by egui - rebuild_text_instances removed

    fn rebuild_texture_groups_by_layer(&mut self) {
        self.world_texture_groups.clear();
        self.ui_texture_groups.clear();
        self.world_texture_group_offsets.clear();
        self.ui_texture_group_offsets.clear();
        self.world_texture_buffer_ranges.clear();
        self.ui_texture_buffer_ranges.clear();

        let cap = self.texture_instance_slots.len();
        let mut world_with_slot: Vec<(usize, TextureInstance)> = Vec::with_capacity(cap);
        let mut ui_with_slot: Vec<(usize, TextureInstance)> = Vec::with_capacity(cap);
        let mut world_texture_id: Option<TextureID> = None;
        let mut ui_texture_id: Option<TextureID> = None;
        let mut world_single_texture = true;
        let mut ui_single_texture = true;

        for (slot_idx, slot) in self.texture_instance_slots.iter().enumerate() {
            if let Some((layer, instance, texture_id, _tex_size, _timestamp)) = slot {
                match layer {
                    RenderLayer::World2D => {
                        if world_texture_id.is_none() {
                            world_texture_id = Some(*texture_id);
                        } else if world_texture_id != Some(*texture_id) {
                            world_single_texture = false;
                        }
                        world_with_slot.push((slot_idx, *instance));
                    }
                    RenderLayer::UI => {
                        if ui_texture_id.is_none() {
                            ui_texture_id = Some(*texture_id);
                        } else if ui_texture_id != Some(*texture_id) {
                            ui_single_texture = false;
                        }
                        ui_with_slot.push((slot_idx, *instance));
                    }
                }
            }
        }

        self.texture_slot_to_flat_index
            .resize(self.texture_instance_slots.len(), None);

        if world_single_texture && ui_single_texture {
            if !world_with_slot.is_empty() {
                if let Some(tid) = world_texture_id {
                    if world_with_slot.len() > 1 {
                        world_with_slot.sort_by(|a, b| a.1.z_index.cmp(&b.1.z_index));
                    }
                    let mut world_instances = Vec::with_capacity(world_with_slot.len());
                    world_instances.extend(world_with_slot.iter().map(|(_, i)| *i));
                    for (flat_i, (slot_idx, _)) in world_with_slot.iter().enumerate() {
                        self.texture_slot_to_flat_index[*slot_idx] = Some(flat_i);
                    }
                    self.world_texture_groups.push((tid, world_instances));
                    self.world_texture_group_offsets
                        .push((0, self.world_texture_groups[0].1.len()));
                    const INSTANCE_SIZE: usize = std::mem::size_of::<TextureInstance>();
                    let size_bytes = self.world_texture_groups[0].1.len() * INSTANCE_SIZE;
                    self.world_texture_buffer_ranges
                        .push(0..(size_bytes as u64));
                }
            }

            if !ui_with_slot.is_empty() {
                if let Some(tid) = ui_texture_id {
                    if ui_with_slot.len() > 1 {
                        ui_with_slot.sort_by(|a, b| a.1.z_index.cmp(&b.1.z_index));
                    }
                    let world_offset = if !self.world_texture_groups.is_empty() {
                        self.world_texture_groups[0].1.len()
                    } else {
                        0
                    };
                    let mut ui_instances = Vec::with_capacity(ui_with_slot.len());
                    ui_instances.extend(ui_with_slot.iter().map(|(_, i)| *i));
                    for (flat_i, (slot_idx, _)) in ui_with_slot.iter().enumerate() {
                        self.texture_slot_to_flat_index[*slot_idx] = Some(world_offset + flat_i);
                    }
                    self.ui_texture_groups.push((tid, ui_instances));
                    self.ui_texture_group_offsets
                        .push((world_offset, self.ui_texture_groups[0].1.len()));
                    const INSTANCE_SIZE: usize = std::mem::size_of::<TextureInstance>();
                    let start_byte = world_offset * INSTANCE_SIZE;
                    let size_bytes = self.ui_texture_groups[0].1.len() * INSTANCE_SIZE;
                    self.ui_texture_buffer_ranges
                        .push((start_byte as u64)..((start_byte + size_bytes) as u64));
                }
            }
        } else {
            self.world_texture_groups.clear();
            self.ui_texture_groups.clear();
            self.world_texture_group_offsets.clear();
            self.ui_texture_group_offsets.clear();
            self.world_texture_buffer_ranges.clear();
            self.ui_texture_buffer_ranges.clear();

            self.texture_slot_to_flat_index
                .resize(self.texture_instance_slots.len(), None);

            let mut world_texture_map: FxHashMap<TextureID, Vec<(usize, TextureInstance)>> =
                FxHashMap::default();
            let mut ui_texture_map: FxHashMap<TextureID, Vec<(usize, TextureInstance)>> =
                FxHashMap::default();

            for (slot_idx, slot) in self.texture_instance_slots.iter().enumerate() {
                if let Some((layer, instance, texture_id, _tex_size, _timestamp)) = slot {
                    match layer {
                        RenderLayer::World2D => {
                            world_texture_map
                                .entry(*texture_id)
                                .or_insert_with(Vec::new)
                                .push((slot_idx, *instance));
                        }
                        RenderLayer::UI => {
                            ui_texture_map
                                .entry(*texture_id)
                                .or_insert_with(Vec::new)
                                .push((slot_idx, *instance));
                        }
                    }
                }
            }

            let map_cap = world_texture_map.len().max(1);
            let mut temp_sorted_with_slots: Vec<(TextureID, Vec<(usize, TextureInstance)>)> =
                Vec::with_capacity(map_cap);
            let mut world_slot_order = Vec::with_capacity(self.texture_instance_slots.len());
            Self::build_texture_groups_with_slots(
                world_texture_map,
                &mut self.world_texture_groups,
                &mut self.world_texture_group_offsets,
                &mut self.world_texture_buffer_ranges,
                0,
                &mut temp_sorted_with_slots,
                &mut world_slot_order,
            );

            let world_total_instances: usize = self
                .world_texture_groups
                .iter()
                .map(|(_, instances)| instances.len())
                .sum();

            let mut ui_slot_order = Vec::with_capacity(self.texture_instance_slots.len());
            Self::build_texture_groups_with_slots(
                ui_texture_map,
                &mut self.ui_texture_groups,
                &mut self.ui_texture_group_offsets,
                &mut self.ui_texture_buffer_ranges,
                world_total_instances,
                &mut temp_sorted_with_slots,
                &mut ui_slot_order,
            );

            for (i, &slot_idx) in world_slot_order.iter().enumerate() {
                if slot_idx < self.texture_slot_to_flat_index.len() {
                    self.texture_slot_to_flat_index[slot_idx] = Some(i);
                }
            }
            for (i, &slot_idx) in ui_slot_order.iter().enumerate() {
                if slot_idx < self.texture_slot_to_flat_index.len() {
                    self.texture_slot_to_flat_index[slot_idx] = Some(world_total_instances + i);
                }
            }
        }
    }

    /// Partial upload: only write dirty rect slots (transform-only changes). Used when !structure_changed.
    fn partial_update_rects_to_gpu(&mut self, queue: &Queue) {
        const RECT_INSTANCE_SIZE: usize = std::mem::size_of::<RectInstance>();
        let world_len = self.world_rect_instances.len();

        let ranges = std::mem::take(&mut self.rect_dirty_ranges);
        for range in &ranges {
            for slot in range.clone() {
                if slot >= self.rect_slot_to_buffer.len() {
                    continue;
                }
                let Some((layer, idx)) = self.rect_slot_to_buffer[slot] else {
                    continue;
                };
                let Some((_, instance, _)) = &self.rect_instance_slots[slot] else {
                    continue;
                };
                let byte_offset = match layer {
                    RenderLayer::World2D => (idx * RECT_INSTANCE_SIZE) as u64,
                    RenderLayer::UI => ((world_len + idx) * RECT_INSTANCE_SIZE) as u64,
                };
                match layer {
                    RenderLayer::World2D => self.world_rect_instances[idx] = *instance,
                    RenderLayer::UI => self.ui_rect_instances[idx] = *instance,
                }
                queue.write_buffer(
                    &self.rect_instance_buffer,
                    byte_offset,
                    bytemuck::bytes_of(instance),
                );
            }
        }
    }

    /// Partial upload: only write dirty texture slots (transform-only changes). Used when !structure_changed.
    fn partial_update_textures_to_gpu(&mut self, queue: &Queue) {
        const TEXTURE_INSTANCE_SIZE: usize = std::mem::size_of::<TextureInstance>();

        let ranges = std::mem::take(&mut self.texture_dirty_ranges);
        for range in &ranges {
            for slot in range.clone() {
                if slot >= self.texture_slot_to_flat_index.len() {
                    continue;
                }
                let Some(flat_idx) = self.texture_slot_to_flat_index[slot] else {
                    continue;
                };
                let Some((_, instance, _, _, _)) = &self.texture_instance_slots[slot] else {
                    continue;
                };
                if flat_idx < self.temp_all_texture_instances.len() {
                    self.temp_all_texture_instances[flat_idx] = *instance;
                }
                let byte_offset = (flat_idx * TEXTURE_INSTANCE_SIZE) as u64;
                queue.write_buffer(
                    &self.texture_instance_buffer,
                    byte_offset,
                    bytemuck::bytes_of(instance),
                );
            }
        }
    }

    fn upload_instances_to_gpu(&mut self, queue: &Queue) {
        // Upload rect instances
        // Always write world first at offset 0, then UI at byte offset after world
        // This ensures UI instances are always at instance index = world_rect_instances.len()
        // regardless of total count, making the offset calculation consistent
        if !self.world_rect_instances.is_empty() {
            queue.write_buffer(
                &self.rect_instance_buffer,
                0,
                bytemuck::cast_slice(&self.world_rect_instances),
            );
        }
        // Write UI instances after world instances
        if !self.ui_rect_instances.is_empty() {
            let ui_byte_offset =
                (self.world_rect_instances.len() * std::mem::size_of::<RectInstance>()) as u64;
            queue.write_buffer(
                &self.rect_instance_buffer,
                ui_byte_offset,
                bytemuck::cast_slice(&self.ui_rect_instances),
            );
        }

        // Upload texture instances
        let total: usize = self.world_texture_groups.iter().map(|(_, i)| i.len()).sum::<usize>()
            + self.ui_texture_groups.iter().map(|(_, i)| i.len()).sum::<usize>();
        self.temp_all_texture_instances.clear();
        self.temp_all_texture_instances.reserve(total);
        for (_, instances) in &self.world_texture_groups {
            self.temp_all_texture_instances.extend(instances);
        }
        for (_, instances) in &self.ui_texture_groups {
            self.temp_all_texture_instances.extend(instances);
        }

        if !self.temp_all_texture_instances.is_empty() {
            // Clamp to MAX_INSTANCES to prevent buffer overflow
            let instances_to_write = self.temp_all_texture_instances.len().min(MAX_INSTANCES);
            if instances_to_write < self.temp_all_texture_instances.len() {
                eprintln!(
                    "Warning: {} texture instances queued, but buffer only supports {}. Truncating.",
                    self.temp_all_texture_instances.len(),
                    MAX_INSTANCES
                );
            }
            queue.write_buffer(
                &self.texture_instance_buffer,
                0,
                bytemuck::cast_slice(&self.temp_all_texture_instances[..instances_to_write]),
            );
        }
    }

    /// Build texture groups and preserve slot indices for texture_slot_to_flat_index.
    fn build_texture_groups_with_slots(
        mut texture_map: FxHashMap<TextureID, Vec<(usize, TextureInstance)>>,
        groups: &mut Vec<(TextureID, Vec<TextureInstance>)>,
        offsets: &mut Vec<(usize, usize)>,
        ranges: &mut Vec<Range<u64>>,
        buffer_offset: usize,
        temp_sorted_groups: &mut Vec<(TextureID, Vec<(usize, TextureInstance)>)>,
        slot_order: &mut Vec<usize>,
    ) {
        temp_sorted_groups.clear();
        temp_sorted_groups.extend(texture_map.drain());
        if temp_sorted_groups.len() > 1 {
            temp_sorted_groups.sort_by(|a, b| {
                let z_a = a.1.first().map(|(_, c)| c.z_index).unwrap_or(0);
                let z_b = b.1.first().map(|(_, c)| c.z_index).unwrap_or(0);
                z_a.cmp(&z_b)
            });
        }

        let mut current_offset = buffer_offset;
        const INSTANCE_SIZE: usize = std::mem::size_of::<TextureInstance>();

        for (texture_id, mut instances_with_slot) in temp_sorted_groups.drain(..) {
            if instances_with_slot.len() > 1 {
                instances_with_slot.sort_by(|a, b| a.1.z_index.cmp(&b.1.z_index));
            }
            let n = instances_with_slot.len();
            let mut slot_indices = Vec::with_capacity(n);
            let mut instances = Vec::with_capacity(n);
            for (s, i) in instances_with_slot {
                slot_indices.push(s);
                instances.push(i);
            }
            let count = instances.len();
            let start_byte = current_offset * INSTANCE_SIZE;
            let size_bytes = count * INSTANCE_SIZE;
            let range = (start_byte as u64)..((start_byte + size_bytes) as u64);
            groups.push((texture_id, instances));
            offsets.push((current_offset, count));
            ranges.push(range);
            slot_order.extend(slot_indices);
            current_offset += count;
        }
    }

    fn render_rects(
        &self,
        instances: &[RectInstance],
        rpass: &mut RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
        vertex_buffer: &wgpu::Buffer,
        instance_offset: u32,
    ) {
        if !instances.is_empty() {
            rpass.set_pipeline(&self.rect_instanced_pipeline);
            rpass.set_bind_group(0, camera_bind_group, &[]);
            rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.rect_instance_buffer.slice(..));
            rpass.draw(
                0..6,
                instance_offset..(instance_offset + instances.len() as u32),
            );
        }
    }

    fn render_textures(
        &self,
        texture_groups: &[(TextureID, Vec<TextureInstance>)],
        group_offsets: &[(usize, usize)],
        buffer_ranges: &[Range<u64>],
        rpass: &mut RenderPass<'_>,
        texture_manager: &mut TextureManager,
        device: &Device,
        queue: &Queue,
        camera_bind_group: &wgpu::BindGroup,
        vertex_buffer: &wgpu::Buffer,
    ) {
        for (i, (texture_id, _)) in texture_groups.iter().enumerate() {
            let (_, count) = group_offsets[i];
            if count > 0 {
                if let Some(tex_bg) = texture_manager.get_or_create_bind_group_by_id(
                    *texture_id,
                    device,
                    queue,
                    &self.texture_bind_group_layout,
                ) {
                    rpass.set_pipeline(&self.texture_instanced_pipeline);
                    rpass.set_bind_group(0, tex_bg, &[]);
                    rpass.set_bind_group(1, camera_bind_group, &[]);
                    rpass.set_bind_group(2, &self.sprite_output_config_bind_group, &[]);
                    rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    rpass.set_vertex_buffer(
                        1,
                        self.texture_instance_buffer.slice(buffer_ranges[i].clone()),
                    );
                    rpass.draw(0..6, 0..count as u32);
                }
            }
        }
    }

    // Text rendering now handled by egui - render_text removed

    // [Pipeline creation methods remain the same - keeping them for completeness but truncating for space]
    fn create_rect_pipeline(
        device: &Device,
        camera_bgl: &BindGroupLayout,
        format: TextureFormat,
        sample_count: u32,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Rect Instanced Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/rect_instanced.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Rect Instanced Pipeline Layout"),
            bind_group_layouts: &[camera_bgl],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Rect Instanced Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as _,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &[
                            VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: VertexFormat::Float32x2,
                            },
                            VertexAttribute {
                                offset: std::mem::size_of::<[f32; 2]>() as _,
                                shader_location: 1,
                                format: VertexFormat::Float32x2,
                            },
                        ],
                    },
                    VertexBufferLayout {
                        array_stride: std::mem::size_of::<RectInstance>() as _,
                        step_mode: VertexStepMode::Instance,
                        attributes: &[
                            // Mat3
                            VertexAttribute {
                                offset: 0,
                                shader_location: 2,
                                format: VertexFormat::Float32x3,
                            },
                            VertexAttribute {
                                offset: 12,
                                shader_location: 3,
                                format: VertexFormat::Float32x3,
                            },
                            VertexAttribute {
                                offset: 24,
                                shader_location: 4,
                                format: VertexFormat::Float32x3,
                            },
                            // color
                            VertexAttribute {
                                offset: 36,
                                shader_location: 5,
                                format: VertexFormat::Float32x4,
                            },
                            // size, pivot
                            VertexAttribute {
                                offset: 52,
                                shader_location: 6,
                                format: VertexFormat::Float32x2,
                            },
                            VertexAttribute {
                                offset: 60,
                                shader_location: 7,
                                format: VertexFormat::Float32x2,
                            },
                            // corner radii
                            VertexAttribute {
                                offset: 68,
                                shader_location: 8,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 84,
                                shader_location: 9,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 100,
                                shader_location: 10,
                                format: VertexFormat::Float32,
                            },
                            VertexAttribute {
                                offset: 104,
                                shader_location: 11,
                                format: VertexFormat::Uint32,
                            },
                            VertexAttribute {
                                offset: 108,
                                shader_location: 12,
                                format: VertexFormat::Sint32,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    fn create_texture_pipeline(
        device: &Device,
        texture_bgl: &BindGroupLayout,
        camera_bgl: &BindGroupLayout,
        output_config_bgl: &BindGroupLayout,
        format: TextureFormat,
        sample_count: u32,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Sprite Instanced Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "shaders/sprite_instanced.wgsl"
            ))),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Sprite Instanced Pipeline Layout"),
            bind_group_layouts: &[texture_bgl, camera_bgl, output_config_bgl],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Sprite Instanced Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as _,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &[
                            VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: VertexFormat::Float32x2,
                            },
                            VertexAttribute {
                                offset: std::mem::size_of::<[f32; 2]>() as _,
                                shader_location: 1,
                                format: VertexFormat::Float32x2,
                            },
                        ],
                    },
                    VertexBufferLayout {
                        array_stride: std::mem::size_of::<TextureInstance>() as _,
                        step_mode: VertexStepMode::Instance,
                        attributes: &[
                            VertexAttribute {
                                offset: 0,
                                shader_location: 2,
                                format: VertexFormat::Float32x3,
                            },
                            VertexAttribute {
                                offset: 12,
                                shader_location: 3,
                                format: VertexFormat::Float32x3,
                            },
                            VertexAttribute {
                                offset: 24,
                                shader_location: 4,
                                format: VertexFormat::Float32x3,
                            },
                            VertexAttribute {
                                offset: 36,
                                shader_location: 5,
                                format: VertexFormat::Float32x2,
                            },
                            VertexAttribute {
                                offset: 44,
                                shader_location: 6,
                                format: VertexFormat::Sint32,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    // Text rendering now handled by egui - create_font_pipeline removed
}
