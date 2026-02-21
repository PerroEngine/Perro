use super::renderer::{Camera2DUniform, RectInstanceGpu, RectUploadPlan};
use super::shaders::{create_rect_shader_module, create_sprite_shader_module};
use crate::backend::StaticTextureLookup;
use crate::resources::ResourceStore;
use bytemuck::{Pod, Zeroable};
use perro_ids::TextureID;
use perro_io::{decompress_zlib, load_asset};
use perro_render_bridge::Sprite2DCommand;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

const VIRTUAL_WIDTH: f32 = 1920.0;
const VIRTUAL_HEIGHT: f32 = 1080.0;
const SPRITE_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const PTEX_MAGIC: &[u8; 4] = b"PTEX";

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
    transform_0: [f32; 3],
    transform_1: [f32; 3],
    transform_2: [f32; 3],
    z_index: i32,
}

#[derive(Clone, Copy)]
struct SpriteBatch {
    texture: TextureID,
    instance_start: u32,
    instance_count: u32,
}

struct CachedSpriteTexture {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
}

pub struct Gpu2D {
    camera_bgl: wgpu::BindGroupLayout,
    texture_bgl: wgpu::BindGroupLayout,
    rect_pipeline: wgpu::RenderPipeline,
    sprite_pipeline: wgpu::RenderPipeline,
    rect_vertex_buffer: wgpu::Buffer,
    rect_instance_buffer: wgpu::Buffer,
    rect_instance_capacity: usize,
    sprite_vertex_buffer: wgpu::Buffer,
    sprite_instance_buffer: wgpu::Buffer,
    sprite_instance_capacity: usize,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    sprite_instances: Vec<SpriteInstanceGpu>,
    sprite_batches: Vec<SpriteBatch>,
    sprite_textures: HashMap<TextureID, CachedSpriteTexture>,
    last_camera: Option<Camera2DUniform>,
}

impl Gpu2D {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Self {
        let rect_shader = create_rect_shader_module(device);
        let sprite_shader = create_sprite_shader_module(device);
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

        Self {
            camera_bgl,
            texture_bgl,
            rect_pipeline,
            sprite_pipeline,
            rect_vertex_buffer,
            rect_instance_buffer,
            rect_instance_capacity,
            sprite_vertex_buffer,
            sprite_instance_buffer,
            sprite_instance_capacity,
            camera_buffer,
            camera_bind_group,
            sprite_instances: Vec::new(),
            sprite_batches: Vec::new(),
            sprite_textures: HashMap::new(),
            last_camera: None,
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
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        camera: Camera2DUniform,
        rects: &[RectInstanceGpu],
        upload: &RectUploadPlan,
        sprites: &[Sprite2DCommand],
        static_texture_lookup: Option<StaticTextureLookup>,
    ) {
        self.ensure_rect_instance_capacity(device, upload.draw_count);
        self.ensure_sprite_instance_capacity(device, sprites.len());
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
                queue.write_buffer(
                    &self.rect_instance_buffer,
                    offset,
                    bytemuck::cast_slice(&rects[range.clone()]),
                );
            }
        }

        self.sprite_instances.clear();
        self.sprite_batches.clear();
        self.sprite_instances.reserve(sprites.len());
        self.sprite_batches.reserve(sprites.len());
        for sprite in sprites {
            if !self.ensure_sprite_texture(
                device,
                queue,
                resources,
                sprite.texture,
                static_texture_lookup,
            ) {
                continue;
            }
            let idx = self.sprite_instances.len() as u32;
            self.sprite_instances.push(SpriteInstanceGpu {
                transform_0: sprite.model[0],
                transform_1: sprite.model[1],
                transform_2: sprite.model[2],
                z_index: sprite.z_index,
            });
            if let Some(batch) = self.sprite_batches.last_mut() {
                if batch.texture == sprite.texture
                    && batch.instance_start + batch.instance_count == idx
                {
                    batch.instance_count += 1;
                    continue;
                }
            }
            self.sprite_batches.push(SpriteBatch {
                texture: sprite.texture,
                instance_start: idx,
                instance_count: 1,
            });
        }
        if !self.sprite_instances.is_empty() {
            queue.write_buffer(
                &self.sprite_instance_buffer,
                0,
                bytemuck::cast_slice(&self.sprite_instances),
            );
        }
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
                let Some(tex) = self.sprite_textures.get(&batch.texture) else {
                    continue;
                };
                pass.set_bind_group(1, &tex.bind_group, &[]);
                pass.draw(
                    0..6,
                    batch.instance_start..batch.instance_start + batch.instance_count,
                );
            }
        }
    }

    fn ensure_sprite_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        texture_id: TextureID,
        static_texture_lookup: Option<StaticTextureLookup>,
    ) -> bool {
        if self.sprite_textures.contains_key(&texture_id) {
            return true;
        }
        let Some(source) = resources.texture_source(texture_id) else {
            return false;
        };

        let (rgba, width, height) = if source == "__default__" {
            (vec![255u8, 255, 255, 255], 1u32, 1u32)
        } else if let Some(lookup) = static_texture_lookup {
            if let Some(bytes) = lookup(source) {
                let Some(decoded) = decode_ptex(bytes) else {
                    return false;
                };
                decoded
            } else {
                let Ok(bytes) = load_asset(source) else {
                    return false;
                };
                let Ok(image) = image::load_from_memory(&bytes) else {
                    return false;
                };
                let rgba = image.to_rgba8();
                let (w, h) = rgba.dimensions();
                (rgba.into_raw(), w.max(1), h.max(1))
            }
        } else {
            let Ok(bytes) = load_asset(source) else {
                return false;
            };
            let Ok(image) = image::load_from_memory(&bytes) else {
                return false;
            };
            let rgba = image.to_rgba8();
            let (w, h) = rgba.dimensions();
            (rgba.into_raw(), w.max(1), h.max(1))
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_sprite_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SPRITE_TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("perro_sprite_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
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
            texture_id,
            CachedSpriteTexture {
                _texture: texture,
                _view: view,
                _sampler: sampler,
                bind_group,
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

    pub fn virtual_size() -> [f32; 2] {
        [VIRTUAL_WIDTH, VIRTUAL_HEIGHT]
    }
}

fn decode_ptex(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if bytes.len() < 20 || &bytes[0..4] != PTEX_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if version != 1 {
        return None;
    }
    let width = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    let raw_len = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    let expected_len = width.checked_mul(height)?.checked_mul(4)?;
    if raw_len != expected_len {
        return None;
    }
    let Ok(rgba) = decompress_zlib(&bytes[20..]) else {
        return None;
    };
    if rgba.len() != raw_len as usize {
        return None;
    }
    Some((rgba, width, height))
}

fn create_rect_pipeline(
    device: &wgpu::Device,
    camera_bgl: &wgpu::BindGroupLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("perro_rect_pipeline_layout"),
        bind_group_layouts: &[camera_bgl],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_rect_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<QuadVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x2,
                    }],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectInstanceGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Sint32,
                        },
                    ],
                },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: sample_count.max(1),
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

fn create_sprite_pipeline(
    device: &wgpu::Device,
    camera_bgl: &wgpu::BindGroupLayout,
    texture_bgl: &wgpu::BindGroupLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("perro_sprite_pipeline_layout"),
        bind_group_layouts: &[camera_bgl, texture_bgl],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_sprite_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SpriteVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SpriteInstanceGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 36,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Sint32,
                        },
                    ],
                },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: sample_count.max(1),
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}
