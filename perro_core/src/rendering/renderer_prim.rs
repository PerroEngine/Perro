use std::borrow::Cow;
use std::{
    ops::Range,
    time::{Duration, Instant},
};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use wgpu::{
    BindGroupLayout, BlendState, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites,
    Device, FragmentState, PipelineLayoutDescriptor, Queue, RenderPass, RenderPipeline,
    RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, TextureFormat, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
};

use crate::{
    font::FontAtlas,
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
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable, PartialEq)]
pub struct FontInstance {
    pub transform_0: [f32; 3],
    pub transform_1: [f32; 3],
    pub transform_2: [f32; 3],

    pub color: [f32; 4],
    pub uv_offset: [f32; 2],
    pub uv_size: [f32; 2],

    pub z_index: i32,
    pub _pad: [f32; 3],
}
pub struct PrimitiveRenderer {
    rect_instance_buffer: wgpu::Buffer,
    texture_instance_buffer: wgpu::Buffer,
    font_instance_buffer: wgpu::Buffer,

    rect_instanced_pipeline: RenderPipeline,
    texture_instanced_pipeline: RenderPipeline,
    font_instanced_pipeline: RenderPipeline,

    texture_bind_group_layout: BindGroupLayout,
    font_bind_group_layout: BindGroupLayout,

    font_atlas: Option<FontAtlas>,
    font_bind_group: Option<wgpu::BindGroup>,

    // Optimized rect storage
    rect_instance_slots: Vec<Option<(RenderLayer, RectInstance, u64)>>, // Added timestamp for sorting
    rect_uuid_to_slot: FxHashMap<uuid::Uuid, usize>,
    free_rect_slots: SmallVec<[usize; 16]>,
    rect_dirty_ranges: SmallVec<[Range<usize>; 8]>,

    // Optimized texture storage
    texture_instance_slots: Vec<Option<(RenderLayer, TextureInstance, String, Vector2, u64)>>, // Added texture size and timestamp for sorting
    texture_uuid_to_slot: FxHashMap<uuid::Uuid, usize>,
    free_texture_slots: SmallVec<[usize; 16]>,
    texture_dirty_ranges: SmallVec<[Range<usize>; 8]>,

    // Text storage (less critical to optimize since text changes more frequently)
    cached_text: FxHashMap<uuid::Uuid, (RenderLayer, Vec<FontInstance>, u64)>, // Added timestamp for sorting

    // Rendered instances (built from slots when needed)
    world_rect_instances: Vec<RectInstance>,
    ui_rect_instances: Vec<RectInstance>,
    world_texture_groups: Vec<(String, Vec<TextureInstance>)>,
    ui_texture_groups: Vec<(String, Vec<TextureInstance>)>,
    world_text_instances: Vec<FontInstance>,
    ui_text_instances: Vec<FontInstance>,

    world_texture_group_offsets: Vec<(usize, usize)>,
    ui_texture_group_offsets: Vec<(usize, usize)>,
    world_texture_buffer_ranges: Vec<Range<u64>>,
    ui_texture_buffer_ranges: Vec<Range<u64>>,

    temp_texture_map: FxHashMap<String, Vec<TextureInstance>>,
    temp_sorted_groups: Vec<(String, Vec<TextureInstance>)>,
    temp_all_texture_instances: Vec<TextureInstance>,
    temp_all_font_instances: Vec<FontInstance>,

    // Batching optimization fields
    last_rebuild_time: Instant,
    dirty_count: usize,
    max_rebuild_interval: Duration,
    dirty_threshold: usize,

    instances_need_rebuild: bool,
    text_instances_need_rebuild: bool,
    
    // OPTIMIZED: 2D viewport culling - cache camera info
    camera_position: Vector2,
    camera_rotation: f32,
    camera_zoom: f32,
    viewport_enabled: bool,
}

impl PrimitiveRenderer {
    pub fn new(device: &Device, camera_bgl: &BindGroupLayout, format: TextureFormat) -> Self {
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

        let font_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Font BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
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

        let font_instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Font Instance Buffer"),
            size: (std::mem::size_of::<FontInstance>() * MAX_INSTANCES) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let rect_instanced_pipeline = Self::create_rect_pipeline(device, camera_bgl, format);
        let texture_instanced_pipeline =
            Self::create_texture_pipeline(device, &texture_bind_group_layout, camera_bgl, format);
        let font_instanced_pipeline =
            Self::create_font_pipeline(device, &font_bind_group_layout, camera_bgl, format);

        Self {
            rect_instance_buffer,
            texture_instance_buffer,
            font_instance_buffer,
            rect_instanced_pipeline,
            texture_instanced_pipeline,
            font_instanced_pipeline,
            texture_bind_group_layout,
            font_bind_group_layout,
            font_atlas: None,
            font_bind_group: None,

            // Optimized storage
            rect_instance_slots: Vec::with_capacity(MAX_INSTANCES),
            rect_uuid_to_slot: FxHashMap::default(),
            free_rect_slots: SmallVec::new(),
            rect_dirty_ranges: SmallVec::new(),

            texture_instance_slots: Vec::with_capacity(MAX_INSTANCES),
            texture_uuid_to_slot: FxHashMap::default(),
            free_texture_slots: SmallVec::new(),
            texture_dirty_ranges: SmallVec::new(),

            cached_text: FxHashMap::default(),

            world_rect_instances: Vec::new(),
            ui_rect_instances: Vec::new(),
            world_texture_groups: Vec::new(),
            ui_texture_groups: Vec::new(),
            world_text_instances: Vec::new(),
            ui_text_instances: Vec::new(),
            world_texture_group_offsets: Vec::new(),
            ui_texture_group_offsets: Vec::new(),
            world_texture_buffer_ranges: Vec::new(),
            ui_texture_buffer_ranges: Vec::new(),
            temp_texture_map: FxHashMap::default(),
            temp_sorted_groups: Vec::new(),
            temp_all_texture_instances: Vec::new(),
            temp_all_font_instances: Vec::new(),

            // Batching optimization
            last_rebuild_time: Instant::now(),
            dirty_count: 0,
            max_rebuild_interval: Duration::from_millis(16), // ~60 FPS max
            dirty_threshold: 100,                            // Rebuild when 100+ elements are dirty (reduced rebuild frequency)

            instances_need_rebuild: false,
            text_instances_need_rebuild: false,
            
            // OPTIMIZED: Initialize camera culling info
            camera_position: Vector2::zero(),
            camera_rotation: 0.0,
            camera_zoom: 1.0,
            viewport_enabled: false,
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
        
        use crate::rendering::VIRTUAL_WIDTH;
        use crate::rendering::VIRTUAL_HEIGHT;
        
        // Calculate viewport bounds in world space (axis-aligned, ignoring camera rotation for simplicity)
        let viewport_half_width = (VIRTUAL_WIDTH / self.camera_zoom) * 0.5;
        let viewport_half_height = (VIRTUAL_HEIGHT / self.camera_zoom) * 0.5;
        
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
        
        // Rotate and translate corners to world space
        let cos_r = transform.rotation.cos();
        let sin_r = transform.rotation.sin();
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
    
        // Rotation (from normalized x axis)
        let rotation = (m.x_axis.y / scale_x).atan2(m.x_axis.x / scale_x);
    
        Transform2D {
            position,
            rotation,
            scale,
        }
    }
    
    /// Calculate axis-aligned bounding box (AABB) for an object with given transform and size
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
    fn aabb_contains(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
        // a contains b if a's bounds completely enclose b's bounds
        a.0 <= b.0 && a.1 <= b.1 && a.2 >= b.2 && a.3 >= b.3
    }
    
    /// Check if a visual object is occluded by any texture instances with higher z_index
    /// Returns (is_occluded, occluder_info) where occluder_info is for debug printing
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
            if let Some((layer, texture_instance, texture_path, _texture_size, _timestamp)) = slot {
                // Only check World2D layer textures
                if *layer == RenderLayer::World2D && texture_instance.z_index > visual_z_index {
                    // Extract transform from texture instance (scale already includes texture size)
                    let texture_transform = Self::texture_instance_to_transform(texture_instance);
                    // Use the scale from the transform as the size (it already includes texture size)
                    let texture_size = texture_transform.scale;
                    let texture_aabb = Self::calculate_aabb(&texture_transform, &texture_size);
                    
                    if Self::aabb_contains(texture_aabb, visual_aabb) {
                        let occluder_info = format!(
                            "{} (z={}) occluded by sprite '{}' (z={})",
                            visual_type, visual_z_index, texture_path, texture_instance.z_index
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
        uuid: uuid::Uuid,
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
        // OPTIMIZED: Viewport culling - skip offscreen sprites (only for World2D)
        if layer == RenderLayer::World2D && self.is_sprite_offscreen(&transform, &size) {
            // Remove from slots if it exists (sprite moved offscreen)
            if let Some(&slot) = self.rect_uuid_to_slot.get(&uuid) {
                if let Some(existing) = &mut self.rect_instance_slots[slot] {
                    // Mark as removed by setting to None
                    self.rect_instance_slots[slot] = None;
                    self.free_rect_slots.push(slot);
                    self.rect_uuid_to_slot.remove(&uuid);
                    self.mark_rect_slot_dirty(slot);
                    self.dirty_count += 1;
                    self.instances_need_rebuild = true;
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
        //         if let Some(&slot) = self.rect_uuid_to_slot.get(&uuid) {
        //             if let Some(_existing) = &mut self.rect_instance_slots[slot] {
        //                 self.rect_instance_slots[slot] = None;
        //                 self.free_rect_slots.push(slot);
        //                 self.rect_uuid_to_slot.remove(&uuid);
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
        if let Some(&slot) = self.rect_uuid_to_slot.get(&uuid) {
            // Update existing slot if changed
            if let Some(ref mut existing) = self.rect_instance_slots[slot] {
                if existing.0 != layer || existing.1 != new_instance {
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
            let slot = if let Some(free_slot) = self.free_rect_slots.pop() {
                free_slot
            } else {
                let new_slot = self.rect_instance_slots.len();
                self.rect_instance_slots.push(None);
                new_slot
            };

            self.rect_instance_slots[slot] = Some((layer, new_instance, created_timestamp));
            self.rect_uuid_to_slot.insert(uuid, slot);
            self.mark_rect_slot_dirty(slot);
            self.dirty_count += 1;
            self.instances_need_rebuild = true;
        }
    }

    pub fn queue_texture(
        &mut self,
        uuid: uuid::Uuid,
        layer: RenderLayer,
        texture_path: &str,
        transform: Transform2D,
        pivot: Vector2,
        z_index: i32,
        created_timestamp: u64,
        texture_manager: &mut crate::rendering::TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        // OPTIMIZED: Only lookup texture if we don't already have it cached
        // Check if sprite already exists and has same texture path
        let tex_size = if let Some(&slot) = self.texture_uuid_to_slot.get(&uuid) {
            if let Some(existing) = &self.texture_instance_slots[slot] {
                // If texture path matches, reuse cached texture size
                if existing.2 == texture_path {
                    existing.3 // Reuse cached texture size
                } else {
                    // Texture path changed, need to lookup new texture
                    let tex = texture_manager.get_or_load_texture_sync(texture_path, device, queue);
                    Vector2::new(tex.width as f32, tex.height as f32)
                }
            } else {
                // Slot exists but is None, lookup texture
                let tex = texture_manager.get_or_load_texture_sync(texture_path, device, queue);
                Vector2::new(tex.width as f32, tex.height as f32)
            }
        } else {
            // New sprite, lookup texture
            let tex = texture_manager.get_or_load_texture_sync(texture_path, device, queue);
            Vector2::new(tex.width as f32, tex.height as f32)
        };

        // Create a *new* version for rendering
        let adjusted_transform = Transform2D {
            position: transform.position,
            rotation: transform.rotation,
            scale: Vector2::new(
                transform.scale.x * tex_size.x,
                transform.scale.y * tex_size.y,
            ),
        };
        
        // OPTIMIZED: Viewport culling - skip offscreen sprites (only for World2D)
        if layer == RenderLayer::World2D && self.is_sprite_offscreen(&adjusted_transform, &tex_size) {
            // Remove from slots if it exists (sprite moved offscreen)
            if let Some(&slot) = self.texture_uuid_to_slot.get(&uuid) {
                if let Some(_existing) = &mut self.texture_instance_slots[slot] {
                    // Mark as removed by setting to None
                    self.texture_instance_slots[slot] = None;
                    self.free_texture_slots.push(slot);
                    self.texture_uuid_to_slot.remove(&uuid);
                    self.mark_texture_slot_dirty(slot);
                    self.dirty_count += 1;
                    self.instances_need_rebuild = true;
                }
            }
            return; // Don't queue offscreen sprites
        }
        
        // DISABLED: Occlusion culling - O(n²) performance bottleneck (9M comparisons for 3000 sprites)
        // The GPU can handle overdraw efficiently, and modern GPUs are fast at fragment shading
        // If occlusion culling is needed in the future, implement spatial indexing (quadtree/spatial hash)
        // if layer == RenderLayer::World2D {
        //     let (is_occluded, _occluder_info) = self.is_visual_occluded_by_textures(&transform, &tex_size, z_index, "sprite");
        //     if is_occluded {
        //         // Remove from slots if it exists (sprite is occluded)
        //         if let Some(&slot) = self.texture_uuid_to_slot.get(&uuid) {
        //             if let Some(_existing) = &mut self.texture_instance_slots[slot] {
        //                 self.texture_instance_slots[slot] = None;
        //                 self.free_texture_slots.push(slot);
        //                 self.texture_uuid_to_slot.remove(&uuid);
        //                 self.mark_texture_slot_dirty(slot);
        //                 self.dirty_count += 1;
        //                 self.instances_need_rebuild = true;
        //             }
        //         }
        //         return; // Don't queue occluded sprites
        //     }
        // }

        // OPTIMIZED: Early exit if sprite exists and transform matrix hasn't changed
        // Compare the instance data directly (cheaper than reconstructing transform)
        if let Some(&slot) = self.texture_uuid_to_slot.get(&uuid) {
            if let Some(existing) = &self.texture_instance_slots[slot] {
                // Quick checks: texture path and layer must match
                if existing.0 == layer && existing.2 == texture_path {
                    // Create new instance to compare (but we need tex_size first, which we already have)
                    let test_instance = self.create_texture_instance(
                        Transform2D {
                            position: transform.position,
                            rotation: transform.rotation,
                            scale: Vector2::new(transform.scale.x * tex_size.x, transform.scale.y * tex_size.y),
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

        let new_instance = self.create_texture_instance(adjusted_transform, pivot, z_index, created_timestamp);
        let texture_path = texture_path.to_string();

        // The rest stays exactly the same
        if let Some(&slot) = self.texture_uuid_to_slot.get(&uuid) {
            if let Some(ref mut existing) = self.texture_instance_slots[slot] {
                // Always update if layer, instance, or texture path changed
                // Use a more robust comparison for TextureInstance to handle floating point precision
                let instance_changed =
                existing.1.transform_0 != new_instance.transform_0
                || existing.1.transform_1 != new_instance.transform_1
                || existing.1.transform_2 != new_instance.transform_2
                || existing.1.pivot != new_instance.pivot
                || existing.1.z_index != new_instance.z_index;

                if existing.0 != layer || instance_changed || existing.2 != texture_path {
                    existing.0 = layer;
                    existing.1 = new_instance;
                    existing.2 = texture_path;
                    existing.3 = tex_size; // Update texture size
                    // Keep existing timestamp (existing.4)
                    self.mark_texture_slot_dirty(slot);
                    self.dirty_count += 1;
                    self.instances_need_rebuild = true;
                }
            }
        } else {
            let slot = if let Some(free_slot) = self.free_texture_slots.pop() {
                free_slot
            } else {
                let new_slot = self.texture_instance_slots.len();
                self.texture_instance_slots.push(None);
                new_slot
            };

            self.texture_instance_slots[slot] = Some((layer, new_instance, texture_path, tex_size, created_timestamp));
            self.texture_uuid_to_slot.insert(uuid, slot);
            self.mark_texture_slot_dirty(slot);
            self.dirty_count += 1;
            self.instances_need_rebuild = true;
        }
    }

    pub fn queue_text(
        &mut self,
        uuid: uuid::Uuid,
        layer: RenderLayer,
        text: &str,
        font_size: f32,
        transform: Transform2D,
        _pivot: Vector2,
        color: crate::structs::Color,
        z_index: i32,
        created_timestamp: u64,
    ) {
        self.queue_text_aligned(
            uuid,
            layer,
            text,
            font_size,
            transform,
            _pivot,
            color,
            z_index,
            created_timestamp,
            crate::ui_elements::ui_text::TextAlignment::Left,
            crate::ui_elements::ui_text::TextAlignment::Center,
        );
    }

    pub fn queue_text_aligned(
        &mut self,
        uuid: uuid::Uuid,
        layer: RenderLayer,
        text: &str,
        font_size: f32,
        transform: Transform2D,
        _pivot: Vector2,
        color: crate::structs::Color,
        z_index: i32,
        created_timestamp: u64,
        align_h: crate::ui_elements::ui_text::TextAlignment,
        align_v: crate::ui_elements::ui_text::TextAlignment,
    ) {
        // DISABLED: Occlusion culling for text - O(n²) performance bottleneck
        // The GPU can handle overdraw efficiently
        // if layer == RenderLayer::World2D {
        //     // Estimate text bounding box (rough approximation)
        //     // Average character width is about 0.6 * font_size, height is font_size
        //     let estimated_width = text.len() as f32 * font_size * 0.6;
        //     let estimated_height = font_size;
        //     let text_size = Vector2::new(estimated_width, estimated_height);
        //     
        //     let (is_occluded, _occluder_info) = self.is_visual_occluded_by_textures(&transform, &text_size, z_index, "text");
        //     if is_occluded {
        //         // Remove from text cache if it exists
        //         if self.cached_text.remove(&uuid).is_some() {
        //             self.text_instances_need_rebuild = true;
        //         }
        //         return; // Don't queue occluded text
        //     }
        // }
        
        if let Some(ref atlas) = self.font_atlas {
            let scale = font_size / atlas.design_size;

            // First pass: measure text width
            let mut text_width = 0.0;
            for ch in text.chars() {
                if let Some(g) = atlas.get_glyph(ch) {
                    text_width += g.metrics.advance_width as f32 * scale;
                }
            }

            // Adjust starting position based on horizontal alignment
            let mut cursor_x = transform.position.x;
            match align_h {
                crate::ui_elements::ui_text::TextAlignment::Left => {
                    // Start at the left edge (no adjustment needed)
                }
                crate::ui_elements::ui_text::TextAlignment::Center => {
                    // Center the text: move left by half the text width
                    cursor_x -= text_width * 0.5;
                }
                crate::ui_elements::ui_text::TextAlignment::Right => {
                    // Right align: move left by the full text width
                    cursor_x -= text_width;
                }
                _ => {
                    // Top/Bottom are invalid for horizontal alignment, default to center
                    cursor_x -= text_width * 0.5;
                }
            }

            // Adjust vertical position based on vertical alignment
            // For vertical alignment, we use the font's line metrics
            // Text block extends from (baseline - ascent) to (baseline + descent)
            let baseline_y = match align_v {
                crate::ui_elements::ui_text::TextAlignment::Top => {
                    // Position baseline so text top aligns with position
                    // Top of text = baseline - ascent, so: position.y = baseline - ascent * scale
                    transform.position.y + atlas.ascent * scale
                }
                crate::ui_elements::ui_text::TextAlignment::Center => {
                    // Center vertically: position is where the center of the text block should be
                    // Text block center = baseline + (descent - ascent) * scale * 0.5
                    // We want center = position.y, so: baseline = position.y - (descent - ascent) * scale * 0.5
                    // Simplifying: baseline = position.y - (descent * scale) / 2 + (ascent * scale) / 2
                    // Or: baseline = position.y + (ascent - descent) * scale * 0.5
                    transform.position.y + (atlas.ascent - atlas.descent) * scale * 0.5
                }
                crate::ui_elements::ui_text::TextAlignment::Bottom => {
                    // Position baseline so text bottom aligns with position
                    // Bottom of text = baseline + descent, so: position.y = baseline + descent * scale
                    transform.position.y - atlas.descent * scale
                }
                _ => {
                    // Left/Right are invalid for vertical alignment, default to center
                    transform.position.y + (atlas.ascent - atlas.descent) * scale * 0.5
                }
            };

            // Pre-allocate with estimated capacity (most characters will produce glyphs)
            let mut instances = Vec::with_capacity(text.chars().count());

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

            for ch in text.chars() {
                if let Some(g) = atlas.get_glyph(ch) {
                    let m = &g.metrics;

                    let gw = m.width as f32 * scale;
                    let gh = m.height as f32 * scale;

                    if gw > 0.0 && gh > 0.0 {
                        // bearing_x is xmin (typically negative or zero), offset from origin to left edge
                        let gx = cursor_x + g.bearing_x * scale;
                        
                        // In fontdue: ymin is bottom edge (negative), ymax = ymin + height (positive)
                        // Glyph bitmap in atlas starts at top-left of bounding box
                        // Glyph top edge Y = baseline_y + (ymin + height) * scale = baseline_y + ymax * scale
                        // Glyph center Y = baseline_y + (ymin + height) * scale - gh * 0.5
                        // Simplifying: center Y = baseline_y + ymin * scale + gh * 0.5
                        // Or: center Y = baseline_y + (ymin + height/2) * scale
                        let cy = baseline_y + g.bearing_y * scale + gh * 0.5;
                        
                        // bearing_x positions the left edge, center is at left + half width
                        let cx = gx + gw * 0.5;

                        let glyph_transform = Transform2D {
                            position: Vector2::new(cx, cy),
                            rotation: 0.0,
                            scale: Vector2::new(gw, gh),
                        };

                        let tfm = glyph_transform.to_mat3().to_cols_array();

                        let instance = FontInstance {
                            transform_0: [tfm[0], tfm[1], tfm[2]],
                            transform_1: [tfm[3], tfm[4], tfm[5]],
                            transform_2: [tfm[6], tfm[7], tfm[8]],
                        
                            color: color_lin,
                            uv_offset: [g.u0, g.v0],
                            uv_size: [g.u1 - g.u0, g.v1 - g.v0],
                            z_index,
                            _pad: [0.0; 3],
                        };
                        instances.push(instance);
                    }

                    cursor_x += m.advance_width as f32 * scale;
                }
            }

            // Only update if text actually changed
            let needs_update = if let Some(existing) = self.cached_text.get(&uuid) {
                existing.0 != layer || existing.1 != instances
            } else {
                true
            };

            if needs_update {
                self.cached_text.insert(uuid, (layer, instances, created_timestamp));
                self.text_instances_need_rebuild = true;
            }
        }
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
        let ranges: SmallVec<[Range<usize>; 8]> = self.texture_dirty_ranges.iter().cloned().collect();
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

    pub fn initialize_font_atlas(&mut self, device: &Device, queue: &Queue, font_atlas: FontAtlas) {
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Font Atlas"),
            size: wgpu::Extent3d {
                width: font_atlas.width,
                height: font_atlas.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &font_atlas.bitmap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(font_atlas.width),
                rows_per_image: Some(font_atlas.height),
            },
            wgpu::Extent3d {
                width: font_atlas.width,
                height: font_atlas.height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Font Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let font_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Font Bind Group"),
            layout: &self.font_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        self.font_atlas = Some(font_atlas);
        self.font_bind_group = Some(font_bind_group);
    }

    pub fn stop_rendering(&mut self, uuid: uuid::Uuid) {
        // Remove from rect slots
        if let Some(slot) = self.rect_uuid_to_slot.remove(&uuid) {
            self.rect_instance_slots[slot] = None;
            self.free_rect_slots.push(slot);
            self.mark_rect_slot_dirty(slot);
            self.dirty_count += 1;
            self.instances_need_rebuild = true;
        }

        // Remove from texture slots
        if let Some(slot) = self.texture_uuid_to_slot.remove(&uuid) {
            self.texture_instance_slots[slot] = None;
            self.free_texture_slots.push(slot);
            self.mark_texture_slot_dirty(slot);
            self.dirty_count += 1;
            self.instances_need_rebuild = true;
        }

        // Remove from text cache
        if self.cached_text.remove(&uuid).is_some() {
            self.text_instances_need_rebuild = true;
        }
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

        // Smart batching: only rebuild when threshold is met OR max interval passed
        if self.instances_need_rebuild
            && (self.dirty_count >= self.dirty_threshold
                || time_since_rebuild >= self.max_rebuild_interval)
        {
            self.rebuild_instances(queue);
            self.instances_need_rebuild = false;
            self.dirty_count = 0;
            self.last_rebuild_time = now;
        }

        if self.text_instances_need_rebuild {
            self.rebuild_text_instances(queue);
            self.text_instances_need_rebuild = false;
        }

        match layer {
            RenderLayer::World2D => {
                self.render_rects(
                    &self.world_rect_instances,
                    rpass,
                    camera_bind_group,
                    vertex_buffer,
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
                self.render_text(
                    &self.world_text_instances,
                    rpass,
                    camera_bind_group,
                    vertex_buffer,
                );
            }

            RenderLayer::UI => {
                self.render_rects(
                    &self.ui_rect_instances,
                    rpass,
                    camera_bind_group,
                    vertex_buffer,
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
                self.render_text(
                    &self.ui_text_instances,
                    rpass,
                    camera_bind_group,
                    vertex_buffer,
                );
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
        created_timestamp: u64,
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

        let mut xf_no_scale = transform.clone();
        xf_no_scale.scale = Vector2::new(1.0, 1.0);
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
        created_timestamp: u64,
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

    // OPTIMIZED: Only upload dirty ranges instead of entire buffer
    fn rebuild_instances(&mut self, queue: &Queue) {
        // Rebuild from slots
        self.world_rect_instances.clear();
        self.ui_rect_instances.clear();

        // Collect instances with their timestamps for sorting
        let mut world_with_ts: Vec<(RectInstance, u64)> = Vec::new();
        let mut ui_with_ts: Vec<(RectInstance, u64)> = Vec::new();
        
        for slot in &self.rect_instance_slots {
            if let Some((layer, instance, timestamp)) = slot {
                match layer {
                    RenderLayer::World2D => world_with_ts.push((*instance, *timestamp)),
                    RenderLayer::UI => ui_with_ts.push((*instance, *timestamp)),
                }
            }
        }

        // Sort by z_index first, then by timestamp (newer nodes render above older when z_index is same)
        if world_with_ts.len() > 1 {
            world_with_ts.sort_by(|a, b| a.0.z_index.cmp(&b.0.z_index).then_with(|| a.1.cmp(&b.1)));
        }
        if ui_with_ts.len() > 1 {
            ui_with_ts.sort_by(|a, b| a.0.z_index.cmp(&b.0.z_index).then_with(|| a.1.cmp(&b.1)));
        }
        
        // Extract just the instances after sorting
        self.world_rect_instances = world_with_ts.into_iter().map(|(inst, _)| inst).collect();
        self.ui_rect_instances = ui_with_ts.into_iter().map(|(inst, _)| inst).collect();

        self.rebuild_texture_groups_by_layer();
        self.upload_instances_to_gpu(queue);

        // Clear dirty ranges after upload
        self.rect_dirty_ranges.clear();
        self.texture_dirty_ranges.clear();
    }

    fn rebuild_text_instances(&mut self, queue: &Queue) {
        self.world_text_instances.clear();
        self.ui_text_instances.clear();

        let mut world_with_ts: Vec<(FontInstance, u64)> = Vec::new();
        let mut ui_with_ts: Vec<(FontInstance, u64)> = Vec::new();
        
        for (layer, instances, timestamp) in self.cached_text.values() {
            match layer {
                RenderLayer::World2D => {
                    world_with_ts.extend(instances.iter().map(|inst| (*inst, *timestamp)));
                }
                RenderLayer::UI => {
                    ui_with_ts.extend(instances.iter().map(|inst| (*inst, *timestamp)));
                }
            }
        }

        // Sort by z_index first, then by timestamp (newer nodes render above older when z_index is same)
        if world_with_ts.len() > 1 {
            world_with_ts.sort_by(|a, b| a.0.z_index.cmp(&b.0.z_index).then_with(|| a.1.cmp(&b.1)));
        }
        if ui_with_ts.len() > 1 {
            ui_with_ts.sort_by(|a, b| a.0.z_index.cmp(&b.0.z_index).then_with(|| a.1.cmp(&b.1)));
        }
        
        self.world_text_instances = world_with_ts.into_iter().map(|(inst, _)| inst).collect();
        self.ui_text_instances = ui_with_ts.into_iter().map(|(inst, _)| inst).collect();

        self.temp_all_font_instances.clear();
        self.temp_all_font_instances
            .extend(&self.world_text_instances);
        self.temp_all_font_instances.extend(&self.ui_text_instances);

        if !self.temp_all_font_instances.is_empty() {
            // Clamp to MAX_INSTANCES to prevent buffer overflow
            let instances_to_write = self.temp_all_font_instances.len().min(MAX_INSTANCES);
            if instances_to_write < self.temp_all_font_instances.len() {
                eprintln!("Warning: {} font instances queued, but buffer only supports {}. Truncating.", 
                    self.temp_all_font_instances.len(), MAX_INSTANCES);
            }
            queue.write_buffer(
                &self.font_instance_buffer,
                0,
                bytemuck::cast_slice(&self.temp_all_font_instances[..instances_to_write]),
            );
        }
    }

    fn rebuild_texture_groups_by_layer(&mut self) {
        self.world_texture_groups.clear();
        self.ui_texture_groups.clear();
        self.world_texture_group_offsets.clear();
        self.ui_texture_group_offsets.clear();
        self.world_texture_buffer_ranges.clear();
        self.ui_texture_buffer_ranges.clear();

        // OPTIMIZED: Fast path for single texture (common case)
        // First pass: verify all sprites use the same texture, collect instances
        let mut world_instances: Vec<TextureInstance> = Vec::new();
        let mut ui_instances: Vec<TextureInstance> = Vec::new();
        let mut world_texture_path: Option<String> = None;
        let mut ui_texture_path: Option<String> = None;
        let mut world_single_texture = true;
        let mut ui_single_texture = true;

        for slot in &self.texture_instance_slots {
            if let Some((layer, instance, texture_path, _tex_size, _timestamp)) = slot {
                match layer {
                    RenderLayer::World2D => {
                        if world_texture_path.is_none() {
                            world_texture_path = Some(texture_path.clone());
                        } else if world_texture_path.as_ref().unwrap() != texture_path {
                            // Different texture detected, can't use fast path
                            world_single_texture = false;
                        }
                        world_instances.push(*instance);
                    }
                    RenderLayer::UI => {
                        if ui_texture_path.is_none() {
                            ui_texture_path = Some(texture_path.clone());
                        } else if ui_texture_path.as_ref().unwrap() != texture_path {
                            // Different texture detected, can't use fast path
                            ui_single_texture = false;
                        }
                        ui_instances.push(*instance);
                    }
                }
            }
        }

        // Use fast path only if all sprites use the same texture
        if world_single_texture && ui_single_texture {
            // Fast path: single texture, skip hashmap grouping
            if !world_instances.is_empty() {
                if let Some(path) = world_texture_path {
                    // OPTIMIZED: Only sort if more than one instance
                    if world_instances.len() > 1 {
                        world_instances.sort_by(|a, b| a.z_index.cmp(&b.z_index));
                    }
                    self.world_texture_groups.push((path, world_instances));
                    self.world_texture_group_offsets.push((0, self.world_texture_groups[0].1.len()));
                    const INSTANCE_SIZE: usize = std::mem::size_of::<TextureInstance>();
                    let size_bytes = self.world_texture_groups[0].1.len() * INSTANCE_SIZE;
                    self.world_texture_buffer_ranges.push(0..(size_bytes as u64));
                }
            }

            if !ui_instances.is_empty() {
                if let Some(path) = ui_texture_path {
                    // OPTIMIZED: Only sort if more than one instance
                    if ui_instances.len() > 1 {
                        ui_instances.sort_by(|a, b| a.z_index.cmp(&b.z_index));
                    }
                    let world_offset = if !self.world_texture_groups.is_empty() {
                        self.world_texture_groups[0].1.len()
                    } else {
                        0
                    };
                    self.ui_texture_groups.push((path, ui_instances));
                    self.ui_texture_group_offsets.push((world_offset, self.ui_texture_groups[0].1.len()));
                    const INSTANCE_SIZE: usize = std::mem::size_of::<TextureInstance>();
                    let start_byte = world_offset * INSTANCE_SIZE;
                    let size_bytes = self.ui_texture_groups[0].1.len() * INSTANCE_SIZE;
                    self.ui_texture_buffer_ranges.push((start_byte as u64)..((start_byte + size_bytes) as u64));
                }
            }
        } else {
            // Fallback: multiple textures detected, use full grouping logic
            // Multiple textures detected, use full grouping logic
            self.world_texture_groups.clear();
            self.ui_texture_groups.clear();
            self.world_texture_group_offsets.clear();
            self.ui_texture_group_offsets.clear();
            self.world_texture_buffer_ranges.clear();
            self.ui_texture_buffer_ranges.clear();

            // Reuse temp_texture_map instead of allocating new HashMaps
            self.temp_texture_map.clear();
            let mut ui_texture_map: FxHashMap<String, Vec<TextureInstance>> = FxHashMap::default();

            for slot in &self.texture_instance_slots {
                if let Some((layer, instance, texture_path, _tex_size, _timestamp)) = slot {
                    match layer {
                        RenderLayer::World2D => {
                            self.temp_texture_map
                                .entry(texture_path.clone())
                                .or_insert_with(Vec::new)
                                .push(*instance);
                        }
                        RenderLayer::UI => {
                            ui_texture_map
                                .entry(texture_path.clone())
                                .or_insert_with(Vec::new)
                                .push(*instance);
                        }
                    }
                }
            }

            // Build world texture groups first - take ownership of temp_texture_map
            let world_map_owned = std::mem::take(&mut self.temp_texture_map);
            Self::build_texture_groups(
                world_map_owned,
                &mut self.world_texture_groups,
                &mut self.world_texture_group_offsets,
                &mut self.world_texture_buffer_ranges,
                0,
                &mut self.temp_sorted_groups,
            );

            // Calculate the offset AFTER the first call completes
            let world_total_instances: usize = self
                .world_texture_groups
                .iter()
                .map(|(_, instances)| instances.len())
                .sum();

            // Now build UI texture groups with the calculated offset
            Self::build_texture_groups(
                ui_texture_map,
                &mut self.ui_texture_groups,
                &mut self.ui_texture_group_offsets,
                &mut self.ui_texture_buffer_ranges,
                world_total_instances,
                &mut self.temp_sorted_groups,
            );
        }
    }

    fn upload_instances_to_gpu(&mut self, queue: &Queue) {
        // Upload rect instances - reuse temp vector if available
        // For small cases, use stack allocation
        let total_rects = self.world_rect_instances.len() + self.ui_rect_instances.len();
        if total_rects > 0 {
            // Use a temporary buffer only if needed, otherwise write directly
            if total_rects <= 64 {
                // Small case: use SmallVec for stack allocation
                let mut all_rect_instances: SmallVec<[RectInstance; 64]> = SmallVec::new();
                all_rect_instances.extend(self.world_rect_instances.iter().copied());
                all_rect_instances.extend(self.ui_rect_instances.iter().copied());
                queue.write_buffer(
                    &self.rect_instance_buffer,
                    0,
                    bytemuck::cast_slice(&all_rect_instances),
                );
            } else {
                // Large case: write in chunks to avoid large allocations
                // Write world first
                if !self.world_rect_instances.is_empty() {
                    queue.write_buffer(
                        &self.rect_instance_buffer,
                        0,
                        bytemuck::cast_slice(&self.world_rect_instances),
                    );
                }
                // Then write UI at offset
                if !self.ui_rect_instances.is_empty() {
                    let offset = (self.world_rect_instances.len() * std::mem::size_of::<RectInstance>()) as u64;
                    queue.write_buffer(
                        &self.rect_instance_buffer,
                        offset,
                        bytemuck::cast_slice(&self.ui_rect_instances),
                    );
                }
            }
        }

        // Upload texture instances
        self.temp_all_texture_instances.clear();
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
                eprintln!("Warning: {} texture instances queued, but buffer only supports {}. Truncating.", 
                    self.temp_all_texture_instances.len(), MAX_INSTANCES);
            }
            queue.write_buffer(
                &self.texture_instance_buffer,
                0,
                bytemuck::cast_slice(&self.temp_all_texture_instances[..instances_to_write]),
            );
        }
    }

    fn build_texture_groups(
        mut texture_map: FxHashMap<String, Vec<TextureInstance>>,
        groups: &mut Vec<(String, Vec<TextureInstance>)>,
        offsets: &mut Vec<(usize, usize)>,
        ranges: &mut Vec<Range<u64>>,
        buffer_offset: usize,
        temp_sorted_groups: &mut Vec<(String, Vec<TextureInstance>)>,
    ) {
        temp_sorted_groups.clear();
        temp_sorted_groups.extend(texture_map.drain());
        // OPTIMIZED: Only sort if more than one group
        if temp_sorted_groups.len() > 1 {
            temp_sorted_groups.sort_by(|a, b| {
                // OPTIMIZED: Use first instance z_index as proxy (most groups have same z_index)
                // This avoids iterating through all instances for each comparison
                let z_a = a.1.first().map(|c| c.z_index).unwrap_or(0);
                let z_b = b.1.first().map(|c| c.z_index).unwrap_or(0);
                z_a.cmp(&z_b)
                // Note: Timestamp sorting for texture groups would require storing timestamps separately
                // For now, just sort by z_index within groups
            });
        }

        let mut current_offset = buffer_offset;

        // OPTIMIZED: Pre-compute size_of constant to avoid repeated calculations
        const INSTANCE_SIZE: usize = std::mem::size_of::<TextureInstance>();
        
        for (path, mut instances) in temp_sorted_groups.drain(..) {
            // OPTIMIZED: Only sort if more than one instance (no-op for single instance)
            // Sort by z_index (timestamp sorting handled at higher level for texture groups)
            if instances.len() > 1 {
                instances.sort_by(|a, b| a.z_index.cmp(&b.z_index));
            }

            let count = instances.len();
            let start_byte = current_offset * INSTANCE_SIZE;
            let size_bytes = count * INSTANCE_SIZE;
            let range = (start_byte as u64)..((start_byte + size_bytes) as u64);

            groups.push((path, instances));
            offsets.push((current_offset, count));
            ranges.push(range);

            current_offset += count;
        }
    }

    fn render_rects(
        &self,
        instances: &[RectInstance],
        rpass: &mut RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
        vertex_buffer: &wgpu::Buffer,
    ) {
        if !instances.is_empty() {
            rpass.set_pipeline(&self.rect_instanced_pipeline);
            rpass.set_bind_group(0, camera_bind_group, &[]);
            rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.rect_instance_buffer.slice(..));
            rpass.draw(0..6, 0..instances.len() as u32);
        }
    }

    fn render_textures(
        &self,
        texture_groups: &[(String, Vec<TextureInstance>)],
        group_offsets: &[(usize, usize)],
        buffer_ranges: &[Range<u64>],
        rpass: &mut RenderPass<'_>,
        texture_manager: &mut TextureManager,
        device: &Device,
        queue: &Queue,
        camera_bind_group: &wgpu::BindGroup,
        vertex_buffer: &wgpu::Buffer,
    ) {
        for (i, (texture_path, _)) in texture_groups.iter().enumerate() {
            let (_, count) = group_offsets[i];

            if count > 0 {
                let tex_bg = texture_manager.get_or_create_bind_group(
                    texture_path,
                    device,
                    queue,
                    &self.texture_bind_group_layout,
                );

                rpass.set_pipeline(&self.texture_instanced_pipeline);
                rpass.set_bind_group(0, tex_bg, &[]);
                rpass.set_bind_group(1, camera_bind_group, &[]);
                rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
                rpass.set_vertex_buffer(
                    1,
                    self.texture_instance_buffer.slice(buffer_ranges[i].clone()),
                );
                rpass.draw(0..6, 0..count as u32);
            }
        }
    }

    fn render_text(
        &self,
        instances: &[FontInstance],
        rpass: &mut RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
        vertex_buffer: &wgpu::Buffer,
    ) {
        if !instances.is_empty() && self.font_bind_group.is_some() {
            rpass.set_pipeline(&self.font_instanced_pipeline);
            rpass.set_bind_group(0, self.font_bind_group.as_ref().unwrap(), &[]);
            rpass.set_bind_group(1, camera_bind_group, &[]);
            rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.font_instance_buffer.slice(..));
            rpass.draw(0..6, 0..instances.len() as u32);
        }
    }

    // [Pipeline creation methods remain the same - keeping them for completeness but truncating for space]
    fn create_rect_pipeline(
        device: &Device,
        camera_bgl: &BindGroupLayout,
        format: TextureFormat,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Rect Instanced Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/rect_instanced.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Rect Instanced Pipeline Layout"),
            bind_group_layouts: &[camera_bgl],
            immediate_size: 0,
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
                            VertexAttribute { offset: 0,  shader_location: 2, format: VertexFormat::Float32x3 },
                            VertexAttribute { offset: 12, shader_location: 3, format: VertexFormat::Float32x3 },
                            VertexAttribute { offset: 24, shader_location: 4, format: VertexFormat::Float32x3 },
                    
                            // color
                            VertexAttribute { offset: 36, shader_location: 5, format: VertexFormat::Float32x4 },
                    
                            // size, pivot
                            VertexAttribute { offset: 52, shader_location: 6, format: VertexFormat::Float32x2 },
                            VertexAttribute { offset: 60, shader_location: 7, format: VertexFormat::Float32x2 },
                    
                            // corner radii
                            VertexAttribute { offset: 68, shader_location: 8, format: VertexFormat::Float32x4 },
                            VertexAttribute { offset: 84, shader_location: 9, format: VertexFormat::Float32x4 },
                    
                            VertexAttribute { offset: 100, shader_location: 10, format: VertexFormat::Float32 },
                            VertexAttribute { offset: 104, shader_location: 11, format: VertexFormat::Uint32 },
                            VertexAttribute { offset: 108, shader_location: 12, format: VertexFormat::Sint32 },
                        ],
                    }
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
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    fn create_texture_pipeline(
        device: &Device,
        texture_bgl: &BindGroupLayout,
        camera_bgl: &BindGroupLayout,
        format: TextureFormat,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Sprite Instanced Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "shaders/sprite_instanced.wgsl"
            ))),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Sprite Instanced Pipeline Layout"),
            bind_group_layouts: &[texture_bgl, camera_bgl],
            immediate_size: 0,
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
                            VertexAttribute { offset: 0,  shader_location: 2, format: VertexFormat::Float32x3 },
                            VertexAttribute { offset: 12, shader_location: 3, format: VertexFormat::Float32x3 },
                            VertexAttribute { offset: 24, shader_location: 4, format: VertexFormat::Float32x3 },
                    
                            VertexAttribute { offset: 36, shader_location: 5, format: VertexFormat::Float32x2 },
                            VertexAttribute { offset: 44, shader_location: 6, format: VertexFormat::Sint32 },
                        ],
                    }
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
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    fn create_font_pipeline(
        device: &Device,
        font_texture_bind_group_layout: &BindGroupLayout,
        camera_bgl: &BindGroupLayout,
        format: TextureFormat,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Font Instanced Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/font_instanced.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Font Pipeline Layout"),
            bind_group_layouts: &[font_texture_bind_group_layout, camera_bgl],
            immediate_size: 0,
        });

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Font Instanced Pipeline"),
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
                                offset: 8,
                                shader_location: 1,
                                format: VertexFormat::Float32x2,
                            },
                        ],
                    },
                    VertexBufferLayout {
                        array_stride: std::mem::size_of::<FontInstance>() as _,
                        step_mode: VertexStepMode::Instance,
                        attributes: &[
                            VertexAttribute { offset: 0,  shader_location: 2, format: VertexFormat::Float32x3 },
                            VertexAttribute { offset: 12, shader_location: 3, format: VertexFormat::Float32x3 },
                            VertexAttribute { offset: 24, shader_location: 4, format: VertexFormat::Float32x3 },
                    
                            VertexAttribute { offset: 36, shader_location: 5, format: VertexFormat::Float32x4 },
                            VertexAttribute { offset: 52, shader_location: 6, format: VertexFormat::Float32x2 },
                            VertexAttribute { offset: 60, shader_location: 7, format: VertexFormat::Float32x2 },
                            VertexAttribute { offset: 68, shader_location: 8, format: VertexFormat::Sint32 },
                        ],
                    }
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
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    }
}
