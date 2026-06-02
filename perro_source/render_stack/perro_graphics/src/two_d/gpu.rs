use super::renderer::{Camera2DUniform, RectInstanceGpu, RectUploadPlan, append_point_particles};
use super::shaders::{
    create_point_light_2d_shader_module, create_rect_shader_module, create_sprite_shader_module,
};
use crate::backend::StaticTextureLookup;
use crate::resources::ResourceStore;
use crate::texture_mips::{build_rgba_levels_for_filter, sampler_descriptor, write_rgba_mip_chain};
use ahash::AHashMap;
use bytemuck::{Pod, Zeroable};
use perro_ids::{NodeID, TextureID};
use perro_render_bridge::{Light2DState, PointParticles2DState, Sprite2DCommand};
use perro_structs::TextureFilterMode;
use wgpu::util::DeviceExt;

#[path = "gpu/helpers.rs"]
mod helpers;

use helpers::*;

const VIRTUAL_WIDTH: f32 = 1920.0;
const VIRTUAL_HEIGHT: f32 = 1080.0;
const SPRITE_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct QuadVertex {
    pos: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SpriteVertex {
    local_pos: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SpriteInstanceGpu {
    transform_0: [f32; 2],
    transform_1: [f32; 2],
    translation: [f32; 2],
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    size: [f32; 2],
    z_index: i32,
    tint: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Light2DGpu {
    position: [f32; 2],
    range: f32,
    z_index: i32,
    color: [f32; 3],
    intensity: f32,
    direction: [f32; 2],
    inner_cos: f32,
    outer_cos: f32,
    kind: u32,
    pad: [u32; 3],
}

#[derive(Clone)]
struct SpriteBatch {
    texture: TextureID,
    bind_group: wgpu::BindGroup,
    instance_start: u32,
    instance_count: u32,
}

struct SpriteBatchCandidate {
    texture: TextureID,
    texture_key: u64,
    z_index: i32,
    original_order: usize,
    instance_index: usize,
}

struct CachedSpriteTexture {
    _texture: Option<wgpu::Texture>,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, PartialEq)]
struct SpritePrepareKey {
    revision: u64,
    camera: Camera2DUniform,
}

#[derive(Clone, Copy, Default)]
struct SpritePerfCounters {
    draw_batches: u32,
    bind_group_switches: u32,
}

pub struct Gpu2D {
    camera_bgl: wgpu::BindGroupLayout,
    texture_bgl: wgpu::BindGroupLayout,
    rect_pipeline: wgpu::RenderPipeline,
    sprite_pipeline: wgpu::RenderPipeline,
    point_light_pipeline: wgpu::RenderPipeline,
    rect_vertex_buffer: wgpu::Buffer,
    rect_instance_buffer: wgpu::Buffer,
    rect_instance_capacity: usize,
    sprite_vertex_buffer: wgpu::Buffer,
    sprite_instance_buffer: wgpu::Buffer,
    sprite_instance_capacity: usize,
    point_light_instance_buffer: wgpu::Buffer,
    point_light_instance_capacity: usize,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    sprite_instances: Vec<SpriteInstanceGpu>,
    sprite_stage_instances: Vec<SpriteInstanceGpu>,
    sprite_batch_candidates: Vec<SpriteBatchCandidate>,
    point_light_instances: Vec<Light2DGpu>,
    stream_particle_rects: Vec<RectInstanceGpu>,
    stream_particle_eval_stack: Vec<f32>,
    sprite_batches: Vec<SpriteBatch>,
    sprite_textures: AHashMap<TextureID, CachedSpriteTexture>,
    texture_filter: TextureFilterMode,
    last_camera: Option<Camera2DUniform>,
    last_sprite_prepare: Option<SpritePrepareKey>,
    sprite_perf: SpritePerfCounters,
}

pub struct Prepare2D<'a> {
    pub resources: &'a ResourceStore,
    pub camera: Camera2DUniform,
    pub rects: &'a [RectInstanceGpu],
    pub upload: &'a RectUploadPlan,
    pub sprites: &'a [Sprite2DCommand],
    pub sprites_revision: u64,
    pub force_sprite_prepare: bool,
    pub point_lights: &'a [Light2DState],
    pub static_texture_lookup: Option<StaticTextureLookup>,
}

impl Gpu2D {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        texture_filter: TextureFilterMode,
    ) -> Self {
        let rect_shader = create_rect_shader_module(device);
        let sprite_shader = create_sprite_shader_module(device);
        let point_light_shader = create_point_light_2d_shader_module(device);
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_camera2d_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Camera2DUniform>() as u64)
                            .expect("camera uniform size must be non-zero"),
                    ),
                },
                count: None,
            }],
        });

        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_sprite_texture_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_camera2d_buffer"),
            size: std::mem::size_of::<Camera2DUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_camera2d_bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let rect_pipeline = create_rect_pipeline(
            device,
            &camera_bgl,
            &rect_shader,
            color_format,
            sample_count,
        );
        let sprite_pipeline = create_sprite_pipeline(
            device,
            &camera_bgl,
            &texture_bgl,
            &sprite_shader,
            color_format,
            sample_count,
        );
        let point_light_pipeline = create_point_light_pipeline(
            device,
            &camera_bgl,
            &point_light_shader,
            color_format,
            sample_count,
        );

        let rect_quad = [
            QuadVertex { pos: [-0.5, -0.5] },
            QuadVertex { pos: [0.5, -0.5] },
            QuadVertex { pos: [0.5, 0.5] },
            QuadVertex { pos: [-0.5, -0.5] },
            QuadVertex { pos: [0.5, 0.5] },
            QuadVertex { pos: [-0.5, 0.5] },
        ];
        let rect_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_quad_vertices"),
            contents: bytemuck::cast_slice(&rect_quad),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let sprite_quad = [
            SpriteVertex {
                local_pos: [-0.5, -0.5],
                uv: [0.0, 1.0],
            },
            SpriteVertex {
                local_pos: [0.5, -0.5],
                uv: [1.0, 1.0],
            },
            SpriteVertex {
                local_pos: [0.5, 0.5],
                uv: [1.0, 0.0],
            },
            SpriteVertex {
                local_pos: [-0.5, -0.5],
                uv: [0.0, 1.0],
            },
            SpriteVertex {
                local_pos: [0.5, 0.5],
                uv: [1.0, 0.0],
            },
            SpriteVertex {
                local_pos: [-0.5, 0.5],
                uv: [0.0, 0.0],
            },
        ];
        let sprite_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_sprite_vertices"),
            contents: bytemuck::cast_slice(&sprite_quad),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let rect_instance_capacity = 256usize;
        let rect_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_rect_instances"),
            size: (rect_instance_capacity * std::mem::size_of::<RectInstanceGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sprite_instance_capacity = 256usize;
        let sprite_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_sprite_instances"),
            size: (sprite_instance_capacity * std::mem::size_of::<SpriteInstanceGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let point_light_instance_capacity = 64usize;
        let point_light_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_point_light_2d_instances"),
            size: (point_light_instance_capacity * std::mem::size_of::<Light2DGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            camera_bgl,
            texture_bgl,
            rect_pipeline,
            sprite_pipeline,
            point_light_pipeline,
            rect_vertex_buffer,
            rect_instance_buffer,
            rect_instance_capacity,
            sprite_vertex_buffer,
            sprite_instance_buffer,
            sprite_instance_capacity,
            point_light_instance_buffer,
            point_light_instance_capacity,
            camera_buffer,
            camera_bind_group,
            sprite_instances: Vec::new(),
            sprite_stage_instances: Vec::new(),
            sprite_batch_candidates: Vec::new(),
            point_light_instances: Vec::new(),
            stream_particle_rects: Vec::new(),
            stream_particle_eval_stack: Vec::new(),
            sprite_batches: Vec::new(),
            sprite_textures: AHashMap::new(),
            texture_filter,
            last_camera: None,
            last_sprite_prepare: None,
            sprite_perf: SpritePerfCounters::default(),
        }
    }

    pub fn set_sample_count(
        &mut self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) {
        let rect_shader = create_rect_shader_module(device);
        let sprite_shader = create_sprite_shader_module(device);
        let point_light_shader = create_point_light_2d_shader_module(device);
        self.rect_pipeline = create_rect_pipeline(
            device,
            &self.camera_bgl,
            &rect_shader,
            color_format,
            sample_count,
        );
        self.sprite_pipeline = create_sprite_pipeline(
            device,
            &self.camera_bgl,
            &self.texture_bgl,
            &sprite_shader,
            color_format,
            sample_count,
        );
        self.point_light_pipeline = create_point_light_pipeline(
            device,
            &self.camera_bgl,
            &point_light_shader,
            color_format,
            sample_count,
        );
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: Prepare2D<'_>) {
        let Prepare2D {
            resources,
            camera,
            rects,
            upload,
            sprites,
            sprites_revision,
            force_sprite_prepare,
            point_lights,
            static_texture_lookup,
        } = frame;
        if force_sprite_prepare {
            self.sprite_textures
                .retain(|texture_id, _| resources.has_texture(*texture_id));
            self.last_sprite_prepare = None;
        }
        self.ensure_rect_instance_capacity(device, upload.draw_count);
        self.ensure_point_light_instance_capacity(device, point_lights.len());
        if self.last_camera != Some(camera) {
            queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera));
            self.last_camera = Some(camera);
        }
        if upload.full_reupload {
            if !rects.is_empty() {
                queue.write_buffer(&self.rect_instance_buffer, 0, bytemuck::cast_slice(rects));
            }
        } else {
            let stride = std::mem::size_of::<RectInstanceGpu>() as u64;
            for range in &upload.dirty_ranges {
                if range.start >= range.end || range.end > rects.len() {
                    continue;
                }
                let offset = range.start as u64 * stride;
                let start = range.start;
                let end = range.end;
                queue.write_buffer(
                    &self.rect_instance_buffer,
                    offset,
                    bytemuck::cast_slice(&rects[start..end]),
                );
            }
        }

        let sprite_key = SpritePrepareKey {
            revision: sprites_revision,
            camera,
        };
        if self.last_sprite_prepare != Some(sprite_key) {
            self.ensure_sprite_instance_capacity(device, sprites.len());
            self.sprite_instances.clear();
            self.sprite_stage_instances.clear();
            self.sprite_batches.clear();
            self.sprite_batch_candidates.clear();
            self.sprite_stage_instances.reserve(sprites.len());
            self.sprite_batches.reserve(sprites.len());
            self.sprite_batch_candidates.reserve(sprites.len());
            for sprite in sprites {
                let texture = if let Some(texture) = self.sprite_textures.get(&sprite.texture) {
                    texture
                } else {
                    if !self.ensure_sprite_texture(
                        device,
                        queue,
                        resources,
                        sprite.texture,
                        static_texture_lookup,
                    ) {
                        continue;
                    }
                    let Some(texture) = self.sprite_textures.get(&sprite.texture) else {
                        continue;
                    };
                    texture
                };
                let (sprite_size, uv_min, uv_max) =
                    resolve_sprite_geometry(sprite, texture.width, texture.height);
                if !sprite_intersects_screen(sprite, sprite_size[0], sprite_size[1], &camera) {
                    continue;
                }
                self.sprite_stage_instances.push(SpriteInstanceGpu {
                    transform_0: [sprite.model[0][0], sprite.model[0][1]],
                    transform_1: [sprite.model[1][0], sprite.model[1][1]],
                    translation: [sprite.model[2][0], sprite.model[2][1]],
                    uv_min,
                    uv_max,
                    size: sprite_size,
                    z_index: sprite.z_index,
                    tint: color_to_unorm8(sprite.tint.into()),
                });
                self.sprite_batch_candidates.push(SpriteBatchCandidate {
                    texture: sprite.texture,
                    texture_key: sprite.texture.as_u64(),
                    z_index: sprite.z_index,
                    original_order: self.sprite_batch_candidates.len(),
                    instance_index: self.sprite_stage_instances.len() - 1,
                });
            }
            let candidates_sorted =
                sprite_batch_candidates_sorted(self.sprite_batch_candidates.as_slice());
            self.sprite_perf = SpritePerfCounters::default();
            if candidates_sorted {
                std::mem::swap(&mut self.sprite_instances, &mut self.sprite_stage_instances);
            } else {
                sort_sprite_batch_candidates(self.sprite_batch_candidates.as_mut_slice());
                self.sprite_instances
                    .reserve(self.sprite_batch_candidates.len());
                for candidate in self.sprite_batch_candidates.iter() {
                    self.sprite_instances
                        .push(self.sprite_stage_instances[candidate.instance_index]);
                }
            }
            for (idx, candidate) in self.sprite_batch_candidates.iter().enumerate() {
                let idx = idx as u32;
                if let Some(batch) = self.sprite_batches.last_mut()
                    && batch.texture == candidate.texture
                    && batch.instance_start + batch.instance_count == idx
                {
                    batch.instance_count += 1;
                    continue;
                }
                let bind_group = self
                    .sprite_textures
                    .get(&candidate.texture)
                    .map(|texture| texture.bind_group.clone())
                    .expect("sprite texture cache must contain prepared texture");
                self.sprite_batches.push(SpriteBatch {
                    texture: candidate.texture,
                    bind_group,
                    instance_start: idx,
                    instance_count: 1,
                });
            }
            self.sprite_perf.draw_batches = self.sprite_batches.len() as u32;
            self.sprite_perf.bind_group_switches = self.sprite_batches.len() as u32;
            if !self.sprite_instances.is_empty() {
                queue.write_buffer(
                    &self.sprite_instance_buffer,
                    0,
                    bytemuck::cast_slice(&self.sprite_instances),
                );
            }
            self.last_sprite_prepare = Some(sprite_key);
        }

        self.point_light_instances.clear();
        self.point_light_instances.reserve(point_lights.len());
        for light in point_lights {
            if let Some(light) = light_2d_gpu(*light) {
                self.point_light_instances.push(light);
            }
        }
        if !self.point_light_instances.is_empty() {
            queue.write_buffer(
                &self.point_light_instance_buffer,
                0,
                bytemuck::cast_slice(&self.point_light_instances),
            );
        }
    }

    pub fn prepare_stream_point_particles(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        emitters: &[(NodeID, PointParticles2DState)],
    ) -> u32 {
        self.stream_particle_rects.clear();
        self.stream_particle_eval_stack.clear();
        for (_, emitter) in emitters {
            append_point_particles(
                &mut self.stream_particle_rects,
                &mut self.stream_particle_eval_stack,
                emitter,
            );
        }
        self.ensure_rect_instance_capacity(device, self.stream_particle_rects.len());
        if !self.stream_particle_rects.is_empty() {
            queue.write_buffer(
                &self.rect_instance_buffer,
                0,
                bytemuck::cast_slice(&self.stream_particle_rects),
            );
        }
        self.stream_particle_rects.len() as u32
    }

    pub fn upsert_external_texture(
        &mut self,
        device: &wgpu::Device,
        texture_key: TextureID,
        view: wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let sampler = device.create_sampler(&sampler_descriptor(
            "perro_external_sprite_sampler",
            self.texture_filter,
            wgpu::AddressMode::ClampToEdge,
        ));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_external_sprite_texture_bg"),
            layout: &self.texture_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        self.sprite_textures.insert(
            texture_key,
            CachedSpriteTexture {
                _texture: None,
                _view: view,
                _sampler: sampler,
                bind_group,
                width: width.max(1),
                height: height.max(1),
            },
        );
        self.last_sprite_prepare = None;
    }

    pub fn render_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        resolve_target: Option<&wgpu::TextureView>,
        rect_draw_count: u32,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_rect_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target,
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

        if rect_draw_count > 0 {
            pass.set_pipeline(&self.rect_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, self.rect_instance_buffer.slice(..));
            pass.draw(0..6, 0..rect_draw_count);
        }

        if !self.sprite_batches.is_empty() {
            pass.set_pipeline(&self.sprite_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.sprite_vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, self.sprite_instance_buffer.slice(..));
            for batch in &self.sprite_batches {
                pass.set_bind_group(1, &batch.bind_group, &[]);
                pass.draw(
                    0..6,
                    batch.instance_start..batch.instance_start + batch.instance_count,
                );
            }
        }

        if !self.point_light_instances.is_empty() {
            pass.set_pipeline(&self.point_light_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, self.point_light_instance_buffer.slice(..));
            pass.draw(0..6, 0..self.point_light_instances.len() as u32);
        }
    }

    pub(crate) fn camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }

    pub(crate) fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bgl
    }

    #[inline]
    pub fn draw_call_count(&self, rect_draw_count: u32) -> u32 {
        u32::from(rect_draw_count > 0)
            + self.sprite_batches.len() as u32
            + u32::from(!self.point_light_instances.is_empty())
    }

    fn ensure_sprite_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        texture_key: TextureID,
        _static_texture_lookup: Option<StaticTextureLookup>,
    ) -> bool {
        if self.sprite_textures.contains_key(&texture_key) {
            return true;
        }
        if resources.texture_source(texture_key).is_none() {
            return false;
        }

        let Some(decoded) = resources.decoded_texture_data(texture_key) else {
            return false;
        };
        let width = decoded.width;
        let height = decoded.height;
        let mips = build_rgba_levels_for_filter(&decoded.rgba, width, height, self.texture_filter);

        let gpu_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_sprite_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mips.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SPRITE_TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        write_rgba_mip_chain(queue, &gpu_texture, &mips);
        let view = gpu_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&sampler_descriptor(
            "perro_sprite_sampler",
            self.texture_filter,
            wgpu::AddressMode::ClampToEdge,
        ));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_sprite_texture_bg"),
            layout: &self.texture_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        self.sprite_textures.insert(
            texture_key,
            CachedSpriteTexture {
                _texture: Some(gpu_texture),
                _view: view,
                _sampler: sampler,
                bind_group,
                width,
                height,
            },
        );
        true
    }

    fn ensure_rect_instance_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.rect_instance_capacity {
            return;
        }
        let mut new_capacity = self.rect_instance_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.rect_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_rect_instances"),
            size: (new_capacity * std::mem::size_of::<RectInstanceGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.rect_instance_capacity = new_capacity;
    }

    fn ensure_sprite_instance_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.sprite_instance_capacity {
            return;
        }
        let mut new_capacity = self.sprite_instance_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.sprite_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_sprite_instances"),
            size: (new_capacity * std::mem::size_of::<SpriteInstanceGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.sprite_instance_capacity = new_capacity;
    }

    fn ensure_point_light_instance_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.point_light_instance_capacity {
            return;
        }
        let mut new_capacity = self.point_light_instance_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.point_light_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_point_light_2d_instances"),
            size: (new_capacity * std::mem::size_of::<Light2DGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.point_light_instance_capacity = new_capacity;
    }

    pub fn virtual_size() -> [f32; 2] {
        [VIRTUAL_WIDTH, VIRTUAL_HEIGHT]
    }
}

fn sort_sprite_batch_candidates(candidates: &mut [SpriteBatchCandidate]) {
    candidates.sort_unstable_by(|a, b| {
        sprite_batch_sort_key(a.z_index, a.texture_key, a.original_order).cmp(
            &sprite_batch_sort_key(b.z_index, b.texture_key, b.original_order),
        )
    });
}

fn sprite_batch_candidates_sorted(candidates: &[SpriteBatchCandidate]) -> bool {
    candidates.windows(2).all(|pair| {
        sprite_batch_sort_key(pair[0].z_index, pair[0].texture_key, pair[0].original_order)
            <= sprite_batch_sort_key(pair[1].z_index, pair[1].texture_key, pair[1].original_order)
    })
}

#[inline]
fn sprite_batch_sort_key(
    z_index: i32,
    texture_key: u64,
    original_order: usize,
) -> (i32, u64, usize) {
    (z_index, texture_key, original_order)
}

fn resolve_sprite_geometry(
    sprite: &Sprite2DCommand,
    texture_width: u32,
    texture_height: u32,
) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let texture_size = [texture_width.max(1) as f32, texture_height.max(1) as f32];
    if sprite.size[0].is_finite()
        && sprite.size[1].is_finite()
        && sprite.size[0] > 0.0
        && sprite.size[1] > 0.0
    {
        (sprite.size, sprite.uv_min, sprite.uv_max)
    } else {
        (texture_size, [0.0, 0.0], texture_size)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SpriteBatchCandidate, resolve_sprite_geometry, sprite_batch_candidates_sorted,
        sprite_batch_sort_key,
    };
    use perro_ids::TextureID;
    use perro_render_bridge::Sprite2DCommand;

    #[test]
    fn sprite_sort_keeps_z_buckets_and_groups_textures() {
        let tex_a = TextureID::from_parts(1, 0);
        let tex_b = TextureID::from_parts(2, 0);
        let mut keys = vec![
            sprite_batch_sort_key(2, tex_b.as_u64(), 0),
            sprite_batch_sort_key(1, tex_a.as_u64(), 1),
            sprite_batch_sort_key(1, tex_b.as_u64(), 2),
            sprite_batch_sort_key(2, tex_a.as_u64(), 3),
        ];
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                (1, tex_a.as_u64(), 1),
                (1, tex_b.as_u64(), 2),
                (2, tex_a.as_u64(), 3),
                (2, tex_b.as_u64(), 0),
            ]
        );
    }

    #[test]
    fn sprite_batch_candidates_sorted_detects_fast_path() {
        let tex_a = TextureID::from_parts(1, 0);
        let tex_b = TextureID::from_parts(2, 0);
        let sorted = vec![
            SpriteBatchCandidate {
                texture: tex_a,
                texture_key: tex_a.as_u64(),
                z_index: 1,
                original_order: 0,
                instance_index: 0,
            },
            SpriteBatchCandidate {
                texture: tex_b,
                texture_key: tex_b.as_u64(),
                z_index: 1,
                original_order: 1,
                instance_index: 1,
            },
            SpriteBatchCandidate {
                texture: tex_b,
                texture_key: tex_b.as_u64(),
                z_index: 2,
                original_order: 2,
                instance_index: 2,
            },
        ];
        let unsorted = vec![
            SpriteBatchCandidate {
                texture: tex_b,
                texture_key: tex_b.as_u64(),
                z_index: 2,
                original_order: 0,
                instance_index: 0,
            },
            SpriteBatchCandidate {
                texture: tex_a,
                texture_key: tex_a.as_u64(),
                z_index: 1,
                original_order: 1,
                instance_index: 1,
            },
        ];
        assert!(sprite_batch_candidates_sorted(&sorted));
        assert!(!sprite_batch_candidates_sorted(&unsorted));
    }

    #[test]
    fn sprite_size_falls_back_to_texture_dimensions() {
        let sprite = Sprite2DCommand {
            size: [0.0, 0.0],
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            ..Sprite2DCommand::default()
        };
        assert_eq!(
            resolve_sprite_geometry(&sprite, 32, 64),
            ([32.0, 64.0], [0.0, 0.0], [32.0, 64.0])
        );

        let sprite = Sprite2DCommand {
            size: [16.0, 8.0],
            uv_min: [4.0, 6.0],
            uv_max: [20.0, 14.0],
            ..Sprite2DCommand::default()
        };
        assert_eq!(
            resolve_sprite_geometry(&sprite, 32, 64),
            ([16.0, 8.0], [4.0, 6.0], [20.0, 14.0])
        );
    }
}
