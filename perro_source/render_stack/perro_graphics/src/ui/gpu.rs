use crate::{backend::StaticTextureLookup, resources::ResourceStore};
use ahash::AHashMap;
use bytemuck::{Pod, Zeroable};
use epaint::{ClippedPrimitive, ImageData, Primitive, TextureId, textures::TexturesDelta};
use perro_asset_formats::ptex::{
    FLAG_FORMAT_MASK as PTEX_FLAG_FORMAT_MASK, FLAG_FORMAT_R8 as PTEX_FLAG_FORMAT_R8,
    FLAG_FORMAT_RGB8 as PTEX_FLAG_FORMAT_RGB8, FLAG_FORMAT_RGBA8 as PTEX_FLAG_FORMAT_RGBA8,
    FLAG_PAYLOAD_RAW as PTEX_FLAG_PAYLOAD_RAW, MAGIC as PTEX_MAGIC, VERSION as PTEX_VERSION,
};
use perro_ids::TextureID;
use perro_io::{decompress_zlib, load_asset};
use std::borrow::Cow;

const UI_SUPERSAMPLE_SCALE: u32 = 2;
const UI_SUPERSAMPLE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[repr(C)]
#[derive(Clone, Copy, Debug, Zeroable, Pod)]
struct UiVertexGpu {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Zeroable, Pod)]
struct UiUniformGpu {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

struct UiTextureGpu {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: [u32; 2],
}

struct UiSupersampleTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    size: [u32; 2],
}

struct UiMeshGpu {
    index_start: u32,
    index_count: u32,
    clip_rect: [u32; 4],
    texture_id: TextureId,
}

pub struct GpuUi {
    pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    font_texture: Option<UiTextureGpu>,
    image_textures: AHashMap<TextureID, UiTextureGpu>,
    supersample_target: Option<UiSupersampleTarget>,
    meshes: Vec<UiMeshGpu>,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    vertex_capacity_bytes: u64,
    index_capacity_bytes: u64,
    vertices: Vec<UiVertexGpu>,
    indices: Vec<u32>,
    prepared_revision: u64,
    prepared_viewport: [u32; 2],
}

pub struct UiPrepareInput<'a> {
    pub resources: &'a ResourceStore,
    pub viewport: [u32; 2],
    pub primitives: &'a [ClippedPrimitive],
    pub textures_delta: &'a TexturesDelta,
    pub texture_size: [u32; 2],
    pub revision: u64,
    pub static_texture_lookup: Option<StaticTextureLookup>,
}

impl GpuUi {
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_ui_epaint_shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(UI_SHADER)),
        });
        let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_ui_composite_shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(UI_COMPOSITE_SHADER)),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_ui_uniform"),
            size: std::mem::size_of::<UiUniformGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_ui_uniform_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_ui_uniform_bg"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_ui_texture_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
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
        let composite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_ui_composite_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("perro_ui_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_ui_pipeline_layout"),
            bind_group_layouts: &[
                Some(&uniform_bind_group_layout),
                Some(&texture_bind_group_layout),
            ],
            immediate_size: 0,
        });
        let composite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_ui_composite_pipeline_layout"),
                bind_group_layouts: &[Some(&composite_bind_group_layout)],
                immediate_size: 0,
            });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_ui_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertexGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 16,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main_gamma_framebuffer"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: UI_SUPERSAMPLE_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_ui_composite_pipeline"),
            layout: Some(&composite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &composite_shader,
                entry_point: Some("vs_composite"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &composite_shader,
                entry_point: Some(if output_format.is_srgb() {
                    "fs_composite_linear_framebuffer"
                } else {
                    "fs_composite_gamma_framebuffer"
                }),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            composite_pipeline,
            uniform_buffer,
            uniform_bind_group,
            texture_bind_group_layout,
            composite_bind_group_layout,
            sampler,
            font_texture: None,
            image_textures: AHashMap::new(),
            supersample_target: None,
            meshes: Vec::new(),
            vertex_buffer: None,
            index_buffer: None,
            vertex_capacity_bytes: 0,
            index_capacity_bytes: 0,
            vertices: Vec::new(),
            indices: Vec::new(),
            prepared_revision: u64::MAX,
            prepared_viewport: [0, 0],
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input: UiPrepareInput<'_>,
    ) {
        let UiPrepareInput {
            resources,
            viewport,
            primitives,
            textures_delta,
            texture_size,
            revision,
            static_texture_lookup,
        } = input;
        let viewport = [viewport[0].max(1), viewport[1].max(1)];
        let render_viewport = supersampled_size(viewport);
        if self.prepared_revision == revision
            && self.prepared_viewport == viewport
            && textures_delta.set.is_empty()
            && textures_delta.free.is_empty()
        {
            return;
        }
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&UiUniformGpu {
                screen_size: [render_viewport[0] as f32, render_viewport[1] as f32],
                _pad: [0.0, 0.0],
            }),
        );
        for (texture_id, delta) in &textures_delta.set {
            if *texture_id == TextureId::default() {
                self.apply_font_delta(device, queue, delta, texture_size);
            }
        }
        self.meshes.clear();
        self.vertices.clear();
        self.indices.clear();
        for primitive in primitives {
            let Primitive::Mesh(mesh) = &primitive.primitive else {
                continue;
            };
            if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                continue;
            }
            if mesh.texture_id != TextureId::default()
                && !self.ensure_image_texture(
                    device,
                    queue,
                    resources,
                    mesh.texture_id,
                    static_texture_lookup,
                )
            {
                continue;
            }
            let clip_rect = clip_rect_scaled(primitive, render_viewport, UI_SUPERSAMPLE_SCALE);
            if clip_rect[2] == 0 || clip_rect[3] == 0 {
                continue;
            }
            let vertex_offset = self.vertices.len().min(u32::MAX as usize) as u32;
            let index_start = self.indices.len().min(u32::MAX as usize) as u32;
            self.vertices.reserve(mesh.vertices.len());
            self.indices.reserve(mesh.indices.len());
            self.indices.extend(
                mesh.indices
                    .iter()
                    .map(|index| index.saturating_add(vertex_offset)),
            );
            self.vertices
                .extend(mesh.vertices.iter().map(|vertex| UiVertexGpu {
                    pos: [
                        vertex.pos.x * UI_SUPERSAMPLE_SCALE as f32,
                        vertex.pos.y * UI_SUPERSAMPLE_SCALE as f32,
                    ],
                    uv: [vertex.uv.x, vertex.uv.y],
                    color: vertex.color.to_array(),
                }));
            let index_count = mesh.indices.len().min(u32::MAX as usize) as u32;
            if let Some(last) = self.meshes.last_mut()
                && last.clip_rect == clip_rect
                && last.texture_id == mesh.texture_id
                && last.index_start.saturating_add(last.index_count) == index_start
            {
                last.index_count = last.index_count.saturating_add(index_count);
                continue;
            }
            self.meshes.push(UiMeshGpu {
                index_start,
                index_count,
                clip_rect,
                texture_id: mesh.texture_id,
            });
        }
        self.upload_mesh_buffers(device, queue);
        self.prepared_revision = revision;
        self.prepared_viewport = viewport;
    }

    pub fn render_pass(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        viewport: [u32; 2],
    ) {
        if self.meshes.is_empty() {
            return;
        }
        let render_viewport = supersampled_size(viewport);
        self.ensure_supersample_target(device, render_viewport);
        let (Some(vertex_buffer), Some(index_buffer)) =
            (self.vertex_buffer.as_ref(), self.index_buffer.as_ref())
        else {
            return;
        };
        let Some(target) = self.supersample_target.as_ref() else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_ui_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        for mesh in &self.meshes {
            if mesh.clip_rect[2] == 0 || mesh.clip_rect[3] == 0 {
                continue;
            }
            let Some(bind_group) = self.ui_texture_bind_group(mesh.texture_id) else {
                continue;
            };
            pass.set_bind_group(1, bind_group, &[]);
            pass.set_scissor_rect(
                mesh.clip_rect[0],
                mesh.clip_rect[1],
                mesh.clip_rect[2].min(render_viewport[0]),
                mesh.clip_rect[3].min(render_viewport[1]),
            );
            let start = mesh.index_start;
            pass.draw_indexed(start..start.saturating_add(mesh.index_count), 0, 0..1);
        }
        drop(pass);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_ui_composite_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
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
        pass.set_pipeline(&self.composite_pipeline);
        pass.set_bind_group(0, &target.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    pub fn draw_call_count(&self) -> u32 {
        self.meshes.len().min(u32::MAX as usize) as u32
    }

    pub fn clear(&mut self) {
        self.meshes.clear();
        self.vertices.clear();
        self.indices.clear();
        self.prepared_revision = u64::MAX;
    }

    fn upload_mesh_buffers(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let vertex_bytes = bytemuck::cast_slice(&self.vertices);
        let index_bytes = bytemuck::cast_slice(&self.indices);
        self.vertex_buffer = upload_or_grow_buffer(
            device,
            queue,
            self.vertex_buffer.take(),
            &mut self.vertex_capacity_bytes,
            "perro_ui_vertices",
            wgpu::BufferUsages::VERTEX,
            vertex_bytes,
        );
        self.index_buffer = upload_or_grow_buffer(
            device,
            queue,
            self.index_buffer.take(),
            &mut self.index_capacity_bytes,
            "perro_ui_indices",
            wgpu::BufferUsages::INDEX,
            index_bytes,
        );
    }

    fn ensure_supersample_target(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        let size = [size[0].max(1), size[1].max(1)];
        if self
            .supersample_target
            .as_ref()
            .is_some_and(|target| target.size == size)
        {
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_ui_supersample_texture"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: UI_SUPERSAMPLE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_ui_supersample_bg"),
            layout: &self.composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.supersample_target = Some(UiSupersampleTarget {
            _texture: texture,
            view,
            bind_group,
            size,
        });
    }

    fn apply_font_delta(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        delta: &epaint::ImageDelta,
        texture_size: [u32; 2],
    ) {
        let ImageData::Color(image) = &delta.image;
        let size = [image.size[0] as u32, image.size[1] as u32];
        let origin = delta.pos.unwrap_or([0, 0]);
        let required_size = font_delta_required_size(size, origin, texture_size);
        let mut rgba = Vec::with_capacity(image.pixels.len() * 4);
        for pixel in &image.pixels {
            rgba.extend_from_slice(&pixel.to_array());
        }
        let needs_texture = match &self.font_texture {
            Some(texture) => {
                delta.pos.is_none()
                    || texture.size[0] < required_size[0]
                    || texture.size[1] < required_size[1]
            }
            None => true,
        };
        if needs_texture {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_ui_font_texture"),
                size: wgpu::Extent3d {
                    width: required_size[0].max(1),
                    height: required_size[1].max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_ui_font_bg"),
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.font_texture = Some(UiTextureGpu {
                texture,
                bind_group,
                size: required_size,
            });
        }
        let Some(font_texture) = self.font_texture.as_ref() else {
            return;
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &font_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: origin[0] as u32,
                    y: origin[1] as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size[0].max(1) * 4),
                rows_per_image: Some(size[1].max(1)),
            },
            wgpu::Extent3d {
                width: size[0].max(1),
                height: size[1].max(1),
                depth_or_array_layers: 1,
            },
        );
    }

    fn ui_texture_bind_group(&self, texture_id: TextureId) -> Option<&wgpu::BindGroup> {
        if texture_id == TextureId::default() {
            return self
                .font_texture
                .as_ref()
                .map(|texture| &texture.bind_group);
        }
        let TextureId::User(raw) = texture_id else {
            return None;
        };
        self.image_textures
            .get(&TextureID::from_u64(raw))
            .map(|texture| &texture.bind_group)
    }

    fn ensure_image_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        texture_id: TextureId,
        static_texture_lookup: Option<StaticTextureLookup>,
    ) -> bool {
        let TextureId::User(raw) = texture_id else {
            return false;
        };
        let texture_key = TextureID::from_u64(raw);
        if self.image_textures.contains_key(&texture_key) {
            return true;
        }
        let Some(source) = resources.texture_source(texture_key) else {
            return false;
        };
        let Some((rgba, width, height)) = load_ui_texture_rgba(source, static_texture_lookup)
        else {
            return false;
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_ui_image_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
                bytes_per_row: Some(width.max(1) * 4),
                rows_per_image: Some(height.max(1)),
            },
            wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_ui_image_bg"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.image_textures.insert(
            texture_key,
            UiTextureGpu {
                texture,
                bind_group,
                size: [width, height],
            },
        );
        true
    }
}

fn load_ui_texture_rgba(
    source: &str,
    static_texture_lookup: Option<StaticTextureLookup>,
) -> Option<(Vec<u8>, u32, u32)> {
    if source == "__default__" {
        return Some((vec![255, 255, 255, 255], 1, 1));
    }
    if let Some(lookup) = static_texture_lookup {
        let source_hash = perro_ids::parse_hashed_source_uri(source)
            .unwrap_or_else(|| perro_ids::string_to_u64(source));
        let bytes = lookup(source_hash);
        if !bytes.is_empty() {
            if let Some(decoded) = decode_ptex(bytes) {
                return Some(decoded);
            }
            let image = image::load_from_memory(bytes).ok()?;
            let rgba = image.to_rgba8();
            let (width, height) = rgba.dimensions();
            return Some((rgba.into_raw(), width.max(1), height.max(1)));
        }
    }
    let bytes = load_asset(source).ok()?;
    let image = image::load_from_memory(&bytes).ok()?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some((rgba.into_raw(), width.max(1), height.max(1)))
}

fn font_delta_required_size(
    delta_size: [u32; 2],
    origin: [usize; 2],
    texture_size: [u32; 2],
) -> [u32; 2] {
    let origin_x = origin[0].min(u32::MAX as usize) as u32;
    let origin_y = origin[1].min(u32::MAX as usize) as u32;
    let required_width = origin_x.saturating_add(delta_size[0]);
    let required_height = origin_y.saturating_add(delta_size[1]);
    [
        texture_size[0].max(required_width).max(1),
        texture_size[1].max(required_height).max(1),
    ]
}

fn clip_rect_scaled(primitive: &ClippedPrimitive, viewport: [u32; 2], scale: u32) -> [u32; 4] {
    let scale = scale.max(1) as f32;
    let min_x = (primitive.clip_rect.min.x * scale).floor().max(0.0) as u32;
    let min_y = (primitive.clip_rect.min.y * scale).floor().max(0.0) as u32;
    let max_x = (primitive.clip_rect.max.x * scale)
        .ceil()
        .min(viewport[0] as f32)
        .max(min_x as f32) as u32;
    let max_y = (primitive.clip_rect.max.y * scale)
        .ceil()
        .min(viewport[1] as f32)
        .max(min_y as f32) as u32;
    [min_x, min_y, max_x - min_x, max_y - min_y]
}

fn supersampled_size(viewport: [u32; 2]) -> [u32; 2] {
    [
        viewport[0].max(1).saturating_mul(UI_SUPERSAMPLE_SCALE),
        viewport[1].max(1).saturating_mul(UI_SUPERSAMPLE_SCALE),
    ]
}

fn decode_ptex(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if bytes.len() < 24 || &bytes[0..4] != PTEX_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if version != PTEX_VERSION {
        return None;
    }
    let width = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    let flags = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    let raw_len = u32::from_le_bytes(bytes[20..24].try_into().ok()?);
    if flags & !(PTEX_FLAG_FORMAT_MASK | PTEX_FLAG_PAYLOAD_RAW) != 0 {
        return None;
    }
    let pixel_count = width.checked_mul(height)? as usize;
    let expected_raw_len = match flags & PTEX_FLAG_FORMAT_MASK {
        PTEX_FLAG_FORMAT_RGBA8 => pixel_count.checked_mul(4)?,
        PTEX_FLAG_FORMAT_RGB8 => pixel_count.checked_mul(3)?,
        PTEX_FLAG_FORMAT_R8 => pixel_count,
        _ => return None,
    };
    if raw_len as usize != expected_raw_len {
        return None;
    }
    let raw = if (flags & PTEX_FLAG_PAYLOAD_RAW) != 0 {
        bytes[24..].to_vec()
    } else {
        decompress_zlib(&bytes[24..]).ok()?
    };
    if raw.len() != expected_raw_len {
        return None;
    }
    let rgba = match flags & PTEX_FLAG_FORMAT_MASK {
        PTEX_FLAG_FORMAT_RGBA8 => raw,
        PTEX_FLAG_FORMAT_RGB8 => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for px in raw.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        PTEX_FLAG_FORMAT_R8 => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for &v in &raw {
                out.extend_from_slice(&[v, v, v, 255]);
            }
            out
        }
        _ => return None,
    };
    Some((rgba, width, height))
}

fn upload_or_grow_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    current: Option<wgpu::Buffer>,
    capacity_bytes: &mut u64,
    label: &'static str,
    usage: wgpu::BufferUsages,
    bytes: &[u8],
) -> Option<wgpu::Buffer> {
    if bytes.is_empty() {
        return current;
    }
    let required = bytes.len() as u64;
    if let Some(buffer) = current
        && *capacity_bytes >= required
    {
        queue.write_buffer(&buffer, 0, bytes);
        return Some(buffer);
    }
    let capacity = required.next_power_of_two();
    *capacity_bytes = capacity;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: capacity,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytes);
    Some(buffer)
}

const UI_SHADER: &str = r#"
struct UiUniform {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> ui: UiUniform;
@group(1) @binding(0) var font_tex: texture_2d<f32>;
@group(1) @binding(1) var font_sampler: sampler;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let x = (in.pos.x / max(ui.screen_size.x, 1.0)) * 2.0 - 1.0;
    let y = 1.0 - (in.pos.y / max(ui.screen_size.y, 1.0)) * 2.0;
    var out: VsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main_gamma_framebuffer(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(font_tex, font_sampler, in.uv) * in.color;
}

fn linear_from_gamma_rgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

@fragment
fn fs_main_linear_framebuffer(in: VsOut) -> @location(0) vec4<f32> {
    let gamma = textureSample(font_tex, font_sampler, in.uv) * in.color;
    return vec4<f32>(linear_from_gamma_rgb(gamma.rgb), gamma.a);
}
"#;

const UI_COMPOSITE_SHADER: &str = r#"
@group(0) @binding(0) var composite_tex: texture_2d<f32>;
@group(0) @binding(1) var composite_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_composite(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    let pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vertex_index], 0.0, 1.0);
    out.uv = uv[vertex_index];
    return out;
}

@fragment
fn fs_composite_gamma_framebuffer(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(composite_tex, composite_sampler, in.uv);
}

fn linear_from_gamma_rgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

@fragment
fn fs_composite_linear_framebuffer(in: VsOut) -> @location(0) vec4<f32> {
    let gamma = textureSample(composite_tex, composite_sampler, in.uv);
    return vec4<f32>(linear_from_gamma_rgb(gamma.rgb), gamma.a);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_delta_required_size_covers_partial_origin() {
        assert_eq!(
            font_delta_required_size([55, 12], [90, 4], [55, 16]),
            [145, 16]
        );
    }

    #[test]
    fn font_delta_required_size_keeps_atlas_size() {
        assert_eq!(
            font_delta_required_size([55, 12], [0, 0], [2048, 32]),
            [2048, 32]
        );
    }
}
