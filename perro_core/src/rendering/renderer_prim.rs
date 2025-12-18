use bytemuck::cast_slice;
use std::borrow::Cow;
use std::{
    collections::HashMap,
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
    font::FontAtlas,
    rendering::TextureManager,
    structs2d::{Transform2D, Vector2},
    ui_elements::ui_container::CornerRadius,
    vertex::Vertex,
};

const MAX_INSTANCES: usize = 10000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderLayer {
    World2D,
    UI,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable, PartialEq)]
pub struct RectInstance {
    transform_0: [f32; 4],
    transform_1: [f32; 4],
    transform_2: [f32; 4],
    transform_3: [f32; 4],
    color: [f32; 4],
    size: [f32; 2],
    pivot: [f32; 2],
    corner_radius_xy: [f32; 4],
    corner_radius_zw: [f32; 4],
    border_thickness: f32,
    is_border: u32,
    z_index: i32,
    _pad: f32,
}

impl Default for RectInstance {
    fn default() -> Self {
        Self {
            transform_0: [0.0; 4],
            transform_1: [0.0; 4],
            transform_2: [0.0; 4],
            transform_3: [0.0; 4],
            color: [0.0; 4],
            size: [0.0; 2],
            pivot: [0.0; 2],
            corner_radius_xy: [0.0; 4],
            corner_radius_zw: [0.0; 4],
            border_thickness: 0.0,
            is_border: 0,
            z_index: 0,
            _pad: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable, PartialEq)]
pub struct TextureInstance {
    transform_0: [f32; 4],
    transform_1: [f32; 4],
    transform_2: [f32; 4],
    transform_3: [f32; 4],
    pivot: [f32; 2],
    z_index: i32,
    _pad: f32,
}

impl Default for TextureInstance {
    fn default() -> Self {
        Self {
            transform_0: [0.0; 4],
            transform_1: [0.0; 4],
            transform_2: [0.0; 4],
            transform_3: [0.0; 4],
            pivot: [0.0; 2],
            z_index: 0,
            _pad: 0.0,
        }
    }
}

#[repr(C)]
#[derive(PartialEq, Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FontInstance {
    transform_0: [f32; 4],
    transform_1: [f32; 4],
    transform_2: [f32; 4],
    transform_3: [f32; 4],
    color: [f32; 4],
    uv_offset: [f32; 2],
    uv_size: [f32; 2],
    z_index: i32,
    _pad: [f32; 3],
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
    rect_instance_slots: Vec<Option<(RenderLayer, RectInstance)>>,
    rect_uuid_to_slot: HashMap<uuid::Uuid, usize>,
    free_rect_slots: Vec<usize>,
    rect_dirty_ranges: Vec<Range<usize>>,

    // Optimized texture storage
    texture_instance_slots: Vec<Option<(RenderLayer, TextureInstance, String)>>,
    texture_uuid_to_slot: HashMap<uuid::Uuid, usize>,
    free_texture_slots: Vec<usize>,
    texture_dirty_ranges: Vec<Range<usize>>,

    // Text storage (less critical to optimize since text changes more frequently)
    cached_text: HashMap<uuid::Uuid, (RenderLayer, Vec<FontInstance>)>,

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

    temp_texture_map: HashMap<String, Vec<TextureInstance>>,
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
}

impl PrimitiveRenderer {
    pub fn new(device: &Device, camera_bgl: &BindGroupLayout, format: TextureFormat) -> Self {
        println!("🔳 Primitive Renderer initialized - starting setup");
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

        println!("🔳 Creating rect pipeline...");
        let rect_instanced_pipeline = Self::create_rect_pipeline(device, camera_bgl, format);
        println!("🔳 Creating texture pipeline...");
        let texture_instanced_pipeline =
            Self::create_texture_pipeline(device, &texture_bind_group_layout, camera_bgl, format);
        println!("🔳 Creating font pipeline...");
        let font_instanced_pipeline =
            Self::create_font_pipeline(device, &font_bind_group_layout, camera_bgl, format);
        println!("🔳 All pipelines created successfully");

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
            rect_uuid_to_slot: HashMap::new(),
            free_rect_slots: Vec::new(),
            rect_dirty_ranges: Vec::new(),

            texture_instance_slots: Vec::with_capacity(MAX_INSTANCES),
            texture_uuid_to_slot: HashMap::new(),
            free_texture_slots: Vec::new(),
            texture_dirty_ranges: Vec::new(),

            cached_text: HashMap::new(),

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
            temp_texture_map: HashMap::new(),
            temp_sorted_groups: Vec::new(),
            temp_all_texture_instances: Vec::new(),
            temp_all_font_instances: Vec::new(),

            // Batching optimization
            last_rebuild_time: Instant::now(),
            dirty_count: 0,
            max_rebuild_interval: Duration::from_millis(16), // ~60 FPS max
            dirty_threshold: 32,                             // Rebuild when 32+ elements are dirty

            instances_need_rebuild: false,
            text_instances_need_rebuild: false,
        }
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
    ) {
        let new_instance = self.create_rect_instance(
            transform,
            size,
            pivot,
            color,
            corner_radius,
            border_thickness,
            is_border,
            z_index,
        );

        // Check if this rect already exists
        if let Some(&slot) = self.rect_uuid_to_slot.get(&uuid) {
            // Update existing slot if changed
            if let Some(ref mut existing) = self.rect_instance_slots[slot] {
                if existing.0 != layer || existing.1 != new_instance {
                    existing.0 = layer;
                    existing.1 = new_instance;
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

            self.rect_instance_slots[slot] = Some((layer, new_instance));
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
        texture_manager: &mut crate::rendering::TextureManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        // Load or fetch cached texture info
        let tex = texture_manager.get_or_load_texture_sync(texture_path, device, queue);
        let tex_size = Vector2::new(tex.width as f32, tex.height as f32);

        // Create a *new* version for rendering
        let adjusted_transform = Transform2D {
            position: transform.position,
            rotation: transform.rotation,
            scale: Vector2::new(
                transform.scale.x * tex_size.x,
                transform.scale.y * tex_size.y,
            ),
        };

        let new_instance = self.create_texture_instance(adjusted_transform, pivot, z_index);
        let texture_path = texture_path.to_string();

        // The rest stays exactly the same
        if let Some(&slot) = self.texture_uuid_to_slot.get(&uuid) {
            if let Some(ref mut existing) = self.texture_instance_slots[slot] {
                // Always update if layer, instance, or texture path changed
                // Use a more robust comparison for TextureInstance to handle floating point precision
                let instance_changed = existing.1.transform_0 != new_instance.transform_0
                    || existing.1.transform_1 != new_instance.transform_1
                    || existing.1.transform_2 != new_instance.transform_2
                    || existing.1.transform_3 != new_instance.transform_3
                    || existing.1.pivot != new_instance.pivot
                    || existing.1.z_index != new_instance.z_index;

                if existing.0 != layer || instance_changed || existing.2 != texture_path {
                    existing.0 = layer;
                    existing.1 = new_instance;
                    existing.2 = texture_path;
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

            self.texture_instance_slots[slot] = Some((layer, new_instance, texture_path));
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
    ) {
        if let Some(ref atlas) = self.font_atlas {
            let mut cursor_x = transform.position.x;
            let baseline_y = transform.position.y;
            let mut instances = Vec::with_capacity(text.len());

            let scale = font_size / atlas.design_size;

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
                        let gx = cursor_x + g.bearing_x * scale;
                        let gy = baseline_y - g.bearing_y * scale + atlas.ascent * scale;

                        let cx = gx + gw * 0.5;
                        let cy = gy + gh * 0.5;

                        let glyph_transform = Transform2D {
                            position: Vector2::new(cx, cy),
                            rotation: 0.0,
                            scale: Vector2::new(gw, gh),
                        };

                        let tfm = glyph_transform.to_mat4().to_cols_array();
                        let instance = FontInstance {
                            transform_0: [tfm[0], tfm[1], tfm[2], tfm[3]],
                            transform_1: [tfm[4], tfm[5], tfm[6], tfm[7]],
                            transform_2: [tfm[8], tfm[9], tfm[10], tfm[11]],
                            transform_3: [tfm[12], tfm[13], tfm[14], tfm[15]],
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
                self.cached_text.insert(uuid, (layer, instances));
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
        let mut consolidated = Vec::with_capacity(self.rect_dirty_ranges.len());

        let mut current = self.rect_dirty_ranges[0].clone();
        for range in self.rect_dirty_ranges.iter().skip(1) {
            if range.start <= current.end {
                current.end = current.end.max(range.end);
            } else {
                consolidated.push(current);
                current = range.clone();
            }
        }
        consolidated.push(current);

        self.rect_dirty_ranges = consolidated;
    }

    fn consolidate_texture_dirty_ranges(&mut self) {
        if self.texture_dirty_ranges.len() <= 1 {
            return;
        }

        self.texture_dirty_ranges.sort_by_key(|r| r.start);
        let mut consolidated = Vec::with_capacity(self.texture_dirty_ranges.len());

        let mut current = self.texture_dirty_ranges[0].clone();
        for range in self.texture_dirty_ranges.iter().skip(1) {
            if range.start <= current.end {
                current.end = current.end.max(range.end);
            } else {
                consolidated.push(current);
                current = range.clone();
            }
        }
        consolidated.push(current);

        self.texture_dirty_ranges = consolidated;
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
            wgpu::ImageCopyTexture {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &font_atlas.bitmap,
            wgpu::ImageDataLayout {
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
            mipmap_filter: wgpu::FilterMode::Nearest,
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
        let transform_array = xf_no_scale.to_mat4().to_cols_array();

        RectInstance {
            transform_0: [
                transform_array[0],
                transform_array[1],
                transform_array[2],
                transform_array[3],
            ],
            transform_1: [
                transform_array[4],
                transform_array[5],
                transform_array[6],
                transform_array[7],
            ],
            transform_2: [
                transform_array[8],
                transform_array[9],
                transform_array[10],
                transform_array[11],
            ],
            transform_3: [
                transform_array[12],
                transform_array[13],
                transform_array[14],
                transform_array[15],
            ],
            color: color_lin,
            size: [scaled_size_x, scaled_size_y],
            pivot: [pivot.x, pivot.y],
            corner_radius_xy,
            corner_radius_zw,
            border_thickness,
            is_border: if is_border { 1 } else { 0 },
            z_index,
            _pad: 0.0,
        }
    }

    fn create_texture_instance(
        &self,
        transform: Transform2D,
        pivot: Vector2,
        z_index: i32,
    ) -> TextureInstance {
        let transform_array = transform.to_mat4().to_cols_array();
        TextureInstance {
            transform_0: [
                transform_array[0],
                transform_array[1],
                transform_array[2],
                transform_array[3],
            ],
            transform_1: [
                transform_array[4],
                transform_array[5],
                transform_array[6],
                transform_array[7],
            ],
            transform_2: [
                transform_array[8],
                transform_array[9],
                transform_array[10],
                transform_array[11],
            ],
            transform_3: [
                transform_array[12],
                transform_array[13],
                transform_array[14],
                transform_array[15],
            ],
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

        for slot in &self.rect_instance_slots {
            if let Some((layer, instance)) = slot {
                match layer {
                    RenderLayer::World2D => self.world_rect_instances.push(*instance),
                    RenderLayer::UI => self.ui_rect_instances.push(*instance),
                }
            }
        }

        self.world_rect_instances
            .sort_by(|a, b| a.z_index.cmp(&b.z_index));
        self.ui_rect_instances
            .sort_by(|a, b| a.z_index.cmp(&b.z_index));

        self.rebuild_texture_groups_by_layer();
        self.upload_instances_to_gpu(queue);

        // Clear dirty ranges after upload
        self.rect_dirty_ranges.clear();
        self.texture_dirty_ranges.clear();
    }

    fn rebuild_text_instances(&mut self, queue: &Queue) {
        self.world_text_instances.clear();
        self.ui_text_instances.clear();

        for (layer, instances) in self.cached_text.values() {
            match layer {
                RenderLayer::World2D => {
                    self.world_text_instances.extend(instances.iter().cloned());
                }
                RenderLayer::UI => {
                    self.ui_text_instances.extend(instances.iter().cloned());
                }
            }
        }

        self.world_text_instances
            .sort_by(|a, b| a.z_index.cmp(&b.z_index));
        self.ui_text_instances
            .sort_by(|a, b| a.z_index.cmp(&b.z_index));

        self.temp_all_font_instances.clear();
        self.temp_all_font_instances
            .extend(&self.world_text_instances);
        self.temp_all_font_instances.extend(&self.ui_text_instances);

        if !self.temp_all_font_instances.is_empty() {
            queue.write_buffer(
                &self.font_instance_buffer,
                0,
                bytemuck::cast_slice(&self.temp_all_font_instances),
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

        let mut world_texture_map: HashMap<String, Vec<TextureInstance>> = HashMap::new();
        let mut ui_texture_map: HashMap<String, Vec<TextureInstance>> = HashMap::new();

        for slot in &self.texture_instance_slots {
            if let Some((layer, instance, texture_path)) = slot {
                match layer {
                    RenderLayer::World2D => {
                        world_texture_map
                            .entry(texture_path.clone())
                            .or_default()
                            .push(*instance);
                    }
                    RenderLayer::UI => {
                        ui_texture_map
                            .entry(texture_path.clone())
                            .or_default()
                            .push(*instance);
                    }
                }
            }
        }

        // Build world texture groups first
        Self::build_texture_groups(
            world_texture_map,
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

    fn upload_instances_to_gpu(&mut self, queue: &Queue) {
        // Upload rect instances
        let mut all_rect_instances: Vec<RectInstance> = Vec::new();
        all_rect_instances.extend(&self.world_rect_instances);
        all_rect_instances.extend(&self.ui_rect_instances);

        if !all_rect_instances.is_empty() {
            queue.write_buffer(
                &self.rect_instance_buffer,
                0,
                bytemuck::cast_slice(&all_rect_instances),
            );
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
            queue.write_buffer(
                &self.texture_instance_buffer,
                0,
                bytemuck::cast_slice(&self.temp_all_texture_instances),
            );
        }
    }

    fn build_texture_groups(
        mut texture_map: HashMap<String, Vec<TextureInstance>>,
        groups: &mut Vec<(String, Vec<TextureInstance>)>,
        offsets: &mut Vec<(usize, usize)>,
        ranges: &mut Vec<Range<u64>>,
        buffer_offset: usize,
        temp_sorted_groups: &mut Vec<(String, Vec<TextureInstance>)>,
    ) {
        temp_sorted_groups.clear();
        temp_sorted_groups.extend(texture_map.drain());
        temp_sorted_groups.sort_by(|a, b| {
            let min_z_a = a.1.iter().map(|c| c.z_index).min().unwrap_or(0);
            let min_z_b = b.1.iter().map(|c| c.z_index).min().unwrap_or(0);
            min_z_a.cmp(&min_z_b)
        });

        let mut current_offset = buffer_offset;

        for (path, mut instances) in temp_sorted_groups.drain(..) {
            instances.sort_by(|a, b| a.z_index.cmp(&b.z_index));

            let count = instances.len();
            let start_byte = current_offset * std::mem::size_of::<TextureInstance>();
            let size_bytes = count * std::mem::size_of::<TextureInstance>();
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
        println!("🔳 Loading rect shader...");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Rect Instanced Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/rect_instanced.wgsl"))),
        });
        println!("🔳 Rect shader loaded, creating pipeline...");

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
                            VertexAttribute {
                                offset: 0,
                                shader_location: 2,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 16,
                                shader_location: 3,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 32,
                                shader_location: 4,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 48,
                                shader_location: 5,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 64,
                                shader_location: 6,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 80,
                                shader_location: 7,
                                format: VertexFormat::Float32x2,
                            },
                            VertexAttribute {
                                offset: 88,
                                shader_location: 8,
                                format: VertexFormat::Float32x2,
                            },
                            VertexAttribute {
                                offset: 96,
                                shader_location: 9,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 112,
                                shader_location: 10,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 128,
                                shader_location: 11,
                                format: VertexFormat::Float32,
                            },
                            VertexAttribute {
                                offset: 132,
                                shader_location: 12,
                                format: VertexFormat::Uint32,
                            },
                            VertexAttribute {
                                offset: 136,
                                shader_location: 13,
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
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
    }

    fn create_texture_pipeline(
        device: &Device,
        texture_bgl: &BindGroupLayout,
        camera_bgl: &BindGroupLayout,
        format: TextureFormat,
    ) -> RenderPipeline {
        println!("🔳 Loading texture shader...");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Sprite Instanced Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "shaders/sprite_instanced.wgsl"
            ))),
        });
        println!("🔳 Texture shader loaded, creating pipeline...");

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Sprite Instanced Pipeline Layout"),
            bind_group_layouts: &[texture_bgl, camera_bgl],
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
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 16,
                                shader_location: 3,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 32,
                                shader_location: 4,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 48,
                                shader_location: 5,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 64,
                                shader_location: 6,
                                format: VertexFormat::Float32x2,
                            },
                            VertexAttribute {
                                offset: 72,
                                shader_location: 7,
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
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
    }

    fn create_font_pipeline(
        device: &Device,
        font_texture_bind_group_layout: &BindGroupLayout,
        camera_bgl: &BindGroupLayout,
        format: TextureFormat,
    ) -> RenderPipeline {
        println!("🔳 Loading font shader...");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Font Instanced Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/font_instanced.wgsl"))),
        });
        println!("🔳 Font shader loaded, creating pipeline...");

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Font Pipeline Layout"),
            bind_group_layouts: &[font_texture_bind_group_layout, camera_bgl],
            push_constant_ranges: &[],
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
                            VertexAttribute {
                                offset: 0,
                                shader_location: 2,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 16,
                                shader_location: 3,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 32,
                                shader_location: 4,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 48,
                                shader_location: 5,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 64,
                                shader_location: 6,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: 80,
                                shader_location: 7,
                                format: VertexFormat::Float32x2,
                            },
                            VertexAttribute {
                                offset: 88,
                                shader_location: 8,
                                format: VertexFormat::Float32x2,
                            },
                            VertexAttribute {
                                offset: 96,
                                shader_location: 9,
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
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
    }
}
