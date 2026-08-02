use crate::{
    backend::StaticTextureLookup,
    gpu_shrink::{ShrinkTracker, shrink_buffer_preserving},
    resources::ResourceStore,
    shared_textures::{
        SharedGpuTexture, SharedTextureColorSpace, SharedTextureKey, SharedTextureStore,
    },
    texture_mips::{sampler_descriptor, write_texture_base_level},
};
use ahash::{AHashMap, AHashSet};
use bytemuck::{Pod, Zeroable};
use epaint::{ClippedPrimitive, ImageData, Primitive, TextureId, textures::TexturesDelta};
use perro_ids::TextureID;
use perro_structs::TextureFilterMode;
use std::borrow::Cow;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::Arc;

#[path = "gpu/helpers.rs"]
mod helpers;
#[path = "gpu/shaders.rs"]
mod shaders;

use helpers::*;
use shaders::*;

// The UI pass has no MSAA: epaint's feathered vector edges and glyphs render
// oversized here and the linear composite sampler minifies them for edge AA.
// 1 disables supersampling (still renders through the intermediate + composite
// pass; skipping that entirely needs a direct-to-surface pipeline variant).
const UI_SUPERSAMPLE_SCALE: u32 = 2;
// sRGB8 stores the linear composite input at 1/4 the memory of Rgba16Float.
// Camera-stream pixels composited into UI clamp to SDR in this intermediate,
// so HDR headroom inside UI-embedded viewports does not survive it.
const UI_SUPERSAMPLE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const UI_HARFBUZZ_TEXTURE_ID: TextureId = TextureId::Managed(1);
// Consecutive GC ticks with no UI pass encoded before the supersample target is
// released; it recreates on demand (and the recreation forces a redraw).
const UI_TARGET_IDLE_RELEASE_TICKS: u32 = 3;
// Shrink floors for the mesh buffers: below these the GC leaves them alone.
const UI_MIN_VERTEX_BYTES: usize = 8 * 1024;
const UI_MIN_INDEX_BYTES: usize = 4 * 1024;

#[repr(C)]
#[derive(Clone, Copy, Debug, Zeroable, Pod)]
struct UiVertexGpu {
    pos: [f32; 2],
    uv: [f32; 2],
    depth_test: [f32; 2],
    color: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Zeroable, Pod)]
struct UiUniform {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

struct UiTextureGpu {
    // owned upload (font atlases only; images share via the Gpu-level store).
    texture: Option<wgpu::Texture>,
    // shared upload handle for image textures (None for fonts + externals).
    shared: Option<Arc<SharedGpuTexture>>,
    bind_group: wgpu::BindGroup,
    size: [u32; 2],
}

struct UiSupersampleTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    size: [u32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UiMeshGpu {
    index_start: u32,
    index_count: u32,
    clip_rect: [u32; 4],
    texture_id: TextureId,
}

#[derive(Clone, Copy, Default)]
struct UiPerfCounters {
    draw_calls: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct UiMeshSignature {
    hash: u64,
    mesh_count: usize,
    vertex_count: usize,
    index_count: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct UiMeshTotals {
    mesh_count: usize,
    vertex_count: usize,
    index_count: usize,
}

pub struct GpuUi {
    pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    scene_depth_bind_group_layout: wgpu::BindGroupLayout,
    _dummy_depth_texture: wgpu::Texture,
    dummy_depth_view: wgpu::TextureView,
    // scene-depth bind group + the depth-view generation it was built from.
    // 0 = dummy view (no 3D pass); Gpu3D bumps its generation whenever the
    // prepass depth view is recreated, so a stale group can never survive.
    scene_depth_bind_group: Option<wgpu::BindGroup>,
    scene_depth_bind_group_generation: u64,
    // device.limits() clones the whole limits struct; the only field the
    // per-frame supersample math needs never changes for a device.
    max_texture_dimension_2d: u32,
    sampler: wgpu::Sampler,
    texture_filter: TextureFilterMode,
    font_texture: Option<UiTextureGpu>,
    harfbuzz_font_texture: Option<UiTextureGpu>,
    image_textures: AHashMap<TextureID, UiTextureGpu>,
    // stream texture ids (webcam/video): built single-level so per-frame base
    // writes update in place instead of rebuilding the whole image texture.
    stream_texture_ids: AHashSet<TextureID>,
    supersample_target: Option<UiSupersampleTarget>,
    meshes: Vec<UiMeshGpu>,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    vertex_capacity_bytes: u64,
    index_capacity_bytes: u64,
    vertices: Vec<UiVertexGpu>,
    indices: Vec<u32>,
    prepared_mesh_signature: Option<UiMeshSignature>,
    // Keeps every Arc whose pointer the current signature hashed alive until
    // the next signature is computed (ABA guard for the ptr-identity hash).
    signature_pins: Vec<Arc<ClippedPrimitive>>,
    prepared_revision: u64,
    prepared_viewport: [u32; 2],
    // Retained supersample raster. The target keeps the previous frame's UI
    // pixels, so a frame whose UI did not change re-encodes only the cheap
    // full-screen composite instead of re-rasterizing up to 4x swapchain
    // pixels. `supersample_dirty` is the catch-all invalidation flag for
    // content that mutates under a stable mesh signature (texture deltas,
    // texture eviction, stream writes); `rasterized_*` pin the state the
    // retained pixels were produced from.
    supersample_dirty: bool,
    rasterized_signature: Option<UiMeshSignature>,
    rasterized_render_viewport: [u32; 2],
    // Prepared meshes that sample per-frame-mutable inputs can never be
    // retained: scene depth changes whenever the 3D pass runs, and webcam /
    // camera-stream textures are rewritten in place under the same texture id.
    prepared_uses_depth_test: bool,
    prepared_uses_live_texture: bool,
    // Ids bound to externally owned views (camera-stream outputs): the view
    // stays put while its pixels are re-rendered every frame.
    external_image_texture_ids: AHashSet<TextureID>,
    shrink_vertices: ShrinkTracker,
    shrink_indices: ShrinkTracker,
    supersample_idle_ticks: u32,
    used_since_shrink_tick: bool,
    ui_supersample_redraws: u64,
    ui_supersample_composites: u64,
    // Per-adapter render pixel budget (low-memory adapters cap at 1080p); the
    // supersample target obeys it instead of only the global frame cap.
    max_render_pixels: u64,
    perf_counters: UiPerfCounters,
}

pub struct UiPrepareInput<'a> {
    pub resources: &'a ResourceStore,
    pub(crate) shared_textures: &'a mut SharedTextureStore,
    pub viewport: [u32; 2],
    pub primitives: &'a [Arc<ClippedPrimitive>],
    pub primitive_depths: &'a [Option<Arc<[f32]>>],
    pub textures_delta: &'a TexturesDelta,
    pub texture_size: [u32; 2],
    pub revision: u64,
    pub static_texture_lookup: Option<StaticTextureLookup>,
}

struct UiMeshSignatureInput<'a> {
    resources: &'a ResourceStore,
    shared_textures: &'a mut SharedTextureStore,
    primitives: &'a [Arc<ClippedPrimitive>],
    render_viewport: [u32; 2],
    render_scale: [f32; 2],
    static_texture_lookup: Option<StaticTextureLookup>,
}

impl GpuUi {
    pub fn new(
        device: &wgpu::Device,
        output_format: wgpu::TextureFormat,
        texture_filter: TextureFilterMode,
    ) -> Self {
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
            size: std::mem::size_of::<UiUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_ui_uniform_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
        let scene_depth_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_ui_scene_depth_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });
        let dummy_depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_ui_dummy_scene_depth"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy_depth_view =
            dummy_depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&sampler_descriptor(
            "perro_ui_sampler",
            texture_filter,
            wgpu::AddressMode::ClampToEdge,
        ));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_ui_pipeline_layout"),
            bind_group_layouts: &[
                Some(&uniform_bind_group_layout),
                Some(&texture_bind_group_layout),
                Some(&scene_depth_bind_group_layout),
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
                buffers: &[Some(wgpu::VertexBufferLayout {
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
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 16,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 24,
                            shader_location: 3,
                        },
                    ],
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main_linear_framebuffer"),
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
            scene_depth_bind_group_layout,
            _dummy_depth_texture: dummy_depth_texture,
            dummy_depth_view,
            scene_depth_bind_group: None,
            scene_depth_bind_group_generation: 0,
            max_texture_dimension_2d: device.limits().max_texture_dimension_2d,
            sampler,
            texture_filter,
            font_texture: None,
            harfbuzz_font_texture: None,
            image_textures: AHashMap::new(),
            stream_texture_ids: AHashSet::new(),
            supersample_target: None,
            meshes: Vec::new(),
            vertex_buffer: None,
            index_buffer: None,
            vertex_capacity_bytes: 0,
            index_capacity_bytes: 0,
            vertices: Vec::new(),
            indices: Vec::new(),
            prepared_mesh_signature: None,
            signature_pins: Vec::new(),
            prepared_revision: u64::MAX,
            prepared_viewport: [0, 0],
            supersample_dirty: true,
            rasterized_signature: None,
            rasterized_render_viewport: [0, 0],
            prepared_uses_depth_test: false,
            prepared_uses_live_texture: false,
            external_image_texture_ids: AHashSet::new(),
            shrink_vertices: ShrinkTracker::default(),
            shrink_indices: ShrinkTracker::default(),
            supersample_idle_ticks: 0,
            used_since_shrink_tick: false,
            ui_supersample_redraws: 0,
            ui_supersample_composites: 0,
            max_render_pixels: u64::MAX,
            perf_counters: UiPerfCounters::default(),
        }
    }

    pub fn set_max_render_pixels(&mut self, max_render_pixels: u64) {
        self.max_render_pixels = max_render_pixels.max(1);
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input: UiPrepareInput<'_>,
    ) {
        let UiPrepareInput {
            resources,
            shared_textures,
            viewport,
            primitives,
            primitive_depths,
            textures_delta,
            texture_size,
            revision,
            static_texture_lookup,
        } = input;
        let viewport = [viewport[0].max(1), viewport[1].max(1)];
        let render_viewport = supersampled_size(
            viewport,
            self.max_texture_dimension_2d,
            self.max_render_pixels,
        );
        let render_scale = viewport_scale(viewport, render_viewport);
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
            bytemuck::bytes_of(&UiUniform {
                screen_size: [render_viewport[0] as f32, render_viewport[1] as f32],
                _pad: [0.0, 0.0],
            }),
        );
        // Atlas pixels change under an unchanged mesh signature (a partial
        // glyph delta reuses the same primitives), so any delta drops the
        // retained raster.
        if !textures_delta.set.is_empty() || !textures_delta.free.is_empty() {
            self.supersample_dirty = true;
        }
        for (texture_id, delta) in &textures_delta.set {
            if *texture_id == TextureId::default() {
                self.apply_font_delta(device, queue, delta, texture_size);
            } else if *texture_id == UI_HARFBUZZ_TEXTURE_ID {
                self.apply_harfbuzz_font_delta(device, queue, delta);
            }
        }
        // frees land after the sets, matching the epaint contract (an id is
        // never set + freed in one delta).
        for texture_id in &textures_delta.free {
            self.free_managed_texture(*texture_id);
        }
        let (mesh_signature, mesh_totals) = self.mesh_signature(
            device,
            queue,
            UiMeshSignatureInput {
                resources,
                shared_textures: &mut *shared_textures,
                primitives,
                render_viewport,
                render_scale,
                static_texture_lookup,
            },
        );
        // ABA guard: the signature hashes `Arc::as_ptr` per primitive. Pin the
        // hashed Arcs for as long as the signature can gate a skip, so a
        // dropped primitive's address can never be reused by a different
        // primitive that would then collide with the retained signature.
        self.signature_pins.clear();
        self.signature_pins.extend_from_slice(primitives);
        if self.prepared_mesh_signature == Some(mesh_signature) {
            self.perf_counters.draw_calls = self.meshes.len() as u32;
            self.prepared_revision = revision;
            self.prepared_viewport = viewport;
            return;
        }
        self.meshes.clear();
        self.vertices.clear();
        self.indices.clear();
        self.meshes.reserve(mesh_totals.mesh_count);
        self.vertices.reserve(mesh_totals.vertex_count);
        self.indices.reserve(mesh_totals.index_count);
        self.perf_counters = UiPerfCounters::default();
        let mut uses_depth_test = false;
        let mut uses_live_texture = false;
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            let Primitive::Mesh(mesh) = &primitive.primitive else {
                continue;
            };
            if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                continue;
            }
            if mesh.texture_id != TextureId::default()
                && mesh.texture_id != UI_HARFBUZZ_TEXTURE_ID
                && !self.ensure_image_texture(
                    device,
                    queue,
                    shared_textures,
                    resources,
                    mesh.texture_id,
                    static_texture_lookup,
                )
            {
                continue;
            }
            let clip_rect = clip_rect_scaled(primitive, render_viewport, render_scale);
            if clip_rect[2] == 0 || clip_rect[3] == 0 {
                continue;
            }
            let vertex_offset = self.vertices.len().min(u32::MAX as usize) as u32;
            let index_start = self.indices.len().min(u32::MAX as usize) as u32;
            self.indices.extend(
                mesh.indices
                    .iter()
                    .map(|index| index.saturating_add(vertex_offset)),
            );
            let depths = primitive_depths
                .get(primitive_index)
                .and_then(Option::as_deref)
                .filter(|depths| depths.len() == mesh.vertices.len());
            uses_depth_test |= depths.is_some();
            uses_live_texture |= self.is_live_texture(mesh.texture_id);
            self.vertices
                .extend(
                    mesh.vertices
                        .iter()
                        .enumerate()
                        .map(|(index, vertex)| UiVertexGpu {
                            pos: [
                                vertex.pos.x * render_scale[0],
                                vertex.pos.y * render_scale[1],
                            ],
                            uv: [vertex.uv.x, vertex.uv.y],
                            depth_test: depths.map_or([0.0, 0.0], |depths| [depths[index], 1.0]),
                            color: vertex.color.to_array(),
                        }),
                );
            let index_count = mesh.indices.len().min(u32::MAX as usize) as u32;
            push_ui_mesh(
                &mut self.meshes,
                index_start,
                index_count,
                clip_rect,
                mesh.texture_id,
            );
        }
        self.perf_counters.draw_calls = self.meshes.len() as u32;
        self.upload_mesh_buffers(device, queue);
        self.prepared_mesh_signature = Some(mesh_signature);
        self.prepared_uses_depth_test = uses_depth_test;
        self.prepared_uses_live_texture = uses_live_texture;
        self.prepared_revision = revision;
        self.prepared_viewport = viewport;
    }

    /// True when the mesh samples a texture whose pixels are rewritten under a
    /// stable id: webcam/video streams and camera-stream (subviewport) outputs.
    fn is_live_texture(&self, texture_id: TextureId) -> bool {
        let TextureId::User(raw) = texture_id else {
            return false;
        };
        let key = TextureID::from_u64(raw);
        self.stream_texture_ids.contains(&key) || self.external_image_texture_ids.contains(&key)
    }

    pub fn render_pass(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        viewport: [u32; 2],
        // generation identifies the view; None => the dummy depth view.
        scene_depth: Option<(&wgpu::TextureView, u64)>,
    ) {
        if self.meshes.is_empty() {
            return;
        }
        if self.vertex_buffer.is_none() || self.index_buffer.is_none() {
            return;
        }
        self.used_since_shrink_tick = true;
        let viewport = [viewport[0].max(1), viewport[1].max(1)];
        let render_viewport = supersampled_size(
            viewport,
            self.max_texture_dimension_2d,
            self.max_render_pixels,
        );
        let target_created = self.ensure_supersample_target(device, render_viewport);
        if self.supersample_target.is_none() {
            return;
        }
        // rebuilt only when the bound view changes (incl None <-> Some); the
        // group was otherwise recreated every single frame.
        let scene_depth_generation = scene_depth.map_or(0, |(_, generation)| generation);
        if self.scene_depth_bind_group.is_none()
            || self.scene_depth_bind_group_generation != scene_depth_generation
        {
            let view = scene_depth.map_or(&self.dummy_depth_view, |(view, _)| view);
            self.scene_depth_bind_group =
                Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("perro_ui_scene_depth_bg"),
                    layout: &self.scene_depth_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    }],
                }));
            self.scene_depth_bind_group_generation = scene_depth_generation;
        }
        // The supersample target retains the previous frame's pixels, so the
        // raster is re-encoded only when something it depends on moved:
        //   - the target was just (re)created (its contents are undefined),
        //   - the prepared mesh set changed (or was invalidated to `None`),
        //   - the render viewport / supersample size changed,
        //   - a texture delta, eviction, external rebind or stream write
        //     landed (`supersample_dirty`),
        //   - the prepared UI samples per-frame-mutable inputs: scene depth
        //     (depth-tested Label3D-style meshes; the depth buffer is rewritten
        //     every 3D frame at an unchanged view generation) or a
        //     webcam/camera-stream texture rewritten under a stable id.
        // Otherwise only the full-screen composite is encoded.
        let needs_raster = target_created
            || self.supersample_dirty
            || self.prepared_mesh_signature.is_none()
            || self.rasterized_signature != self.prepared_mesh_signature
            || self.rasterized_render_viewport != render_viewport
            || self.prepared_uses_depth_test
            || self.prepared_uses_live_texture;
        if needs_raster {
            self.ui_supersample_redraws = self.ui_supersample_redraws.wrapping_add(1);
            self.rasterized_signature = self.prepared_mesh_signature;
            self.rasterized_render_viewport = render_viewport;
            self.supersample_dirty = false;
        }
        self.ui_supersample_composites = self.ui_supersample_composites.wrapping_add(1);
        let (Some(vertex_buffer), Some(index_buffer)) =
            (self.vertex_buffer.as_ref(), self.index_buffer.as_ref())
        else {
            return;
        };
        let Some(target) = self.supersample_target.as_ref() else {
            return;
        };
        let Some(scene_depth_bind_group) = self.scene_depth_bind_group.as_ref() else {
            return;
        };
        if needs_raster {
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
            pass.set_bind_group(2, scene_depth_bind_group, &[]);
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
        }

        // Always composited: the swapchain image is fresh every frame, so the
        // retained target only skips the raster, never the blend onto output.
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
        self.prepared_mesh_signature = None;
        self.signature_pins.clear();
        self.prepared_revision = u64::MAX;
        self.prepared_uses_depth_test = false;
        self.prepared_uses_live_texture = false;
        self.supersample_dirty = true;
        // The buffers keep their high-water capacity; tell the GC the live
        // content is now empty so it can decay them.
        self.shrink_vertices.note_used(0);
        self.shrink_indices.note_used(0);
    }

    /// Supersample rasters encoded since creation. A frame whose UI is
    /// unchanged bumps only `ui_supersample_composites`.
    pub fn ui_supersample_redraws(&self) -> u64 {
        self.ui_supersample_redraws
    }

    /// UI composite passes encoded since creation (one per rendered frame).
    pub fn ui_supersample_composites(&self) -> u64 {
        self.ui_supersample_composites
    }

    pub fn supersample_target_allocated(&self) -> bool {
        self.supersample_target.is_some()
    }

    pub fn mesh_buffer_capacity_bytes(&self) -> [u64; 2] {
        [self.vertex_capacity_bytes, self.index_capacity_bytes]
    }

    pub fn mesh_mirror_capacity(&self) -> [usize; 2] {
        [self.vertices.capacity(), self.indices.capacity()]
    }

    /// Periodic GC tick: decay the mesh buffers (GPU + CPU mirrors) toward
    /// current usage, and release the supersample target once the UI pass has
    /// not been encoded for `UI_TARGET_IDLE_RELEASE_TICKS` ticks. The target
    /// recreates on demand and its recreation forces a redraw.
    pub fn shrink_tick(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if let Some(new_capacity) = self
            .shrink_vertices
            .tick(self.vertex_capacity_bytes as usize, UI_MIN_VERTEX_BYTES)
        {
            let new_capacity = align_buffer_bytes(new_capacity);
            if let Some(buffer) = self.vertex_buffer.take() {
                self.vertex_buffer = Some(shrink_buffer_preserving(
                    device,
                    queue,
                    &buffer,
                    "perro_ui_vertices",
                    new_capacity as u64,
                    wgpu::BufferUsages::VERTEX
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                ));
            }
            self.vertex_capacity_bytes = new_capacity as u64;
            self.vertices
                .shrink_to(new_capacity / std::mem::size_of::<UiVertexGpu>());
        }
        if let Some(new_capacity) = self
            .shrink_indices
            .tick(self.index_capacity_bytes as usize, UI_MIN_INDEX_BYTES)
        {
            let new_capacity = align_buffer_bytes(new_capacity);
            if let Some(buffer) = self.index_buffer.take() {
                self.index_buffer = Some(shrink_buffer_preserving(
                    device,
                    queue,
                    &buffer,
                    "perro_ui_indices",
                    new_capacity as u64,
                    wgpu::BufferUsages::INDEX
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                ));
            }
            self.index_capacity_bytes = new_capacity as u64;
            self.indices
                .shrink_to(new_capacity / std::mem::size_of::<u32>());
        }
        if self.used_since_shrink_tick {
            self.used_since_shrink_tick = false;
            self.supersample_idle_ticks = 0;
            return;
        }
        self.supersample_idle_ticks = self.supersample_idle_ticks.saturating_add(1);
        if self.supersample_idle_ticks >= UI_TARGET_IDLE_RELEASE_TICKS {
            self.supersample_idle_ticks = 0;
            self.supersample_target = None;
            self.supersample_dirty = true;
        }
    }

    fn mesh_signature(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input: UiMeshSignatureInput<'_>,
    ) -> (UiMeshSignature, UiMeshTotals) {
        let UiMeshSignatureInput {
            resources,
            shared_textures,
            primitives,
            render_viewport,
            render_scale,
            static_texture_lookup,
        } = input;
        hash_renderable_meshes(primitives, render_viewport, render_scale, |texture_id| {
            texture_id == TextureId::default()
                || texture_id == UI_HARFBUZZ_TEXTURE_ID
                || self.ensure_image_texture(
                    device,
                    queue,
                    shared_textures,
                    resources,
                    texture_id,
                    static_texture_lookup,
                )
        })
    }

    fn upload_mesh_buffers(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&self.vertices);
        let index_bytes: &[u8] = bytemuck::cast_slice(&self.indices);
        self.shrink_vertices.note_used(vertex_bytes.len());
        self.shrink_indices.note_used(index_bytes.len());
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

    /// Returns true when the target was (re)created, i.e. its contents are
    /// undefined and the retained raster must be redrawn.
    fn ensure_supersample_target(&mut self, device: &wgpu::Device, size: [u32; 2]) -> bool {
        let size = [size[0].max(1), size[1].max(1)];
        if self
            .supersample_target
            .as_ref()
            .is_some_and(|target| target.size == size)
        {
            return false;
        }
        // PERRO_STREAM_LOG=1 prints the UI raster target the sub-view composite
        // lands in. Pairs with the per-stream target log so the full
        // on-screen -> UI-space -> stream-target chain is readable, not inferred
        // from UI_SUPERSAMPLE_SCALE. Allocation path only.
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var("PERRO_STREAM_LOG").is_ok() {
            eprintln!(
                "[perro][gfx] ui supersample target size={}x{} (scale={})",
                size[0], size[1], UI_SUPERSAMPLE_SCALE
            );
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
        true
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
        // Color32 is Pod over [u8; 4]: reinterpret the atlas pixels in place
        // instead of copying them into a scratch Vec every delta.
        let rgba: &[u8] = bytemuck::cast_slice(image.pixels.as_slice());
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
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            if let Some(copy_size) = font_texture_copy_extent(
                self.font_texture.as_ref().map(|texture| texture.size),
                required_size,
                delta.pos.is_some(),
            ) && let Some(old_texture) = self
                .font_texture
                .as_ref()
                .and_then(|texture| texture.texture.as_ref())
            {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("perro_ui_font_grow_copy"),
                });
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: old_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: copy_size[0],
                        height: copy_size[1],
                        depth_or_array_layers: 1,
                    },
                );
                queue.submit(std::iter::once(encoder.finish()));
            }
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
                texture: Some(texture),
                shared: None,
                bind_group,
                size: required_size,
            });
        }
        let Some(font_texture) = self.font_texture.as_ref() else {
            return;
        };
        let Some(font_texture_gpu) = font_texture.texture.as_ref() else {
            return;
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: font_texture_gpu,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: origin[0] as u32,
                    y: origin[1] as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
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

    fn apply_harfbuzz_font_delta(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        delta: &epaint::ImageDelta,
    ) {
        let ImageData::Color(image) = &delta.image;
        let size = [image.size[0] as u32, image.size[1] as u32];
        let origin = delta.pos.unwrap_or([0, 0]);
        let required_size = font_delta_required_size(size, origin, size);
        // Color32 is Pod over [u8; 4]: reinterpret the atlas pixels in place
        // instead of copying them into a scratch Vec every delta.
        let rgba: &[u8] = bytemuck::cast_slice(image.pixels.as_slice());
        let needs_texture = match &self.harfbuzz_font_texture {
            Some(texture) => {
                delta.pos.is_none()
                    || texture.size[0] < required_size[0]
                    || texture.size[1] < required_size[1]
            }
            None => true,
        };
        if needs_texture {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_ui_harfbuzz_font_texture"),
                size: wgpu::Extent3d {
                    width: required_size[0].max(1),
                    height: required_size[1].max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            if let Some(copy_size) = font_texture_copy_extent(
                self.harfbuzz_font_texture
                    .as_ref()
                    .map(|texture| texture.size),
                required_size,
                delta.pos.is_some(),
            ) && let Some(old_texture) = self
                .harfbuzz_font_texture
                .as_ref()
                .and_then(|texture| texture.texture.as_ref())
            {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("perro_ui_harfbuzz_font_grow_copy"),
                });
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: old_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: copy_size[0],
                        height: copy_size[1],
                        depth_or_array_layers: 1,
                    },
                );
                queue.submit(std::iter::once(encoder.finish()));
            }
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_ui_harfbuzz_font_bg"),
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
            self.harfbuzz_font_texture = Some(UiTextureGpu {
                texture: Some(texture),
                shared: None,
                bind_group,
                size: required_size,
            });
        }
        let Some(font_texture) = self.harfbuzz_font_texture.as_ref() else {
            return;
        };
        let Some(font_texture_gpu) = font_texture.texture.as_ref() else {
            return;
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: font_texture_gpu,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: origin[0] as u32,
                    y: origin[1] as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
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
        if texture_id == UI_HARFBUZZ_TEXTURE_ID {
            return self
                .harfbuzz_font_texture
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

    /// Release a texture the painter says it will never reference again
    /// (`textures_delta.free`): atlas rebuilds and evicted user images.
    fn free_managed_texture(&mut self, texture_id: TextureId) {
        if texture_id == TextureId::default() {
            self.font_texture = None;
        } else if texture_id == UI_HARFBUZZ_TEXTURE_ID {
            self.harfbuzz_font_texture = None;
        } else if let TextureId::User(raw) = texture_id {
            self.image_textures.remove(&TextureID::from_u64(raw));
        } else {
            return;
        }
        self.prepared_mesh_signature = None;
        self.signature_pins.clear();
        self.prepared_revision = u64::MAX;
        self.supersample_dirty = true;
    }

    pub fn invalidate_image_texture(&mut self, texture: TextureID) {
        self.image_textures.remove(&texture);
        self.external_image_texture_ids.remove(&texture);
        self.prepared_mesh_signature = None;
        self.signature_pins.clear();
        self.prepared_revision = u64::MAX;
        self.supersample_dirty = true;
    }

    pub fn set_stream_texture(&mut self, texture: TextureID, is_stream: bool) {
        if is_stream {
            self.stream_texture_ids.insert(texture);
        } else if self.stream_texture_ids.remove(&texture) {
            self.image_textures.remove(&texture);
        }
        self.supersample_dirty = true;
    }

    /// In-place base-level upload for a resident UI stream image texture. Returns
    /// false when no matching-dimension cache exists so the caller can rebuild.
    pub fn write_stream_texture(
        &mut self,
        queue: &wgpu::Queue,
        texture: TextureID,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> bool {
        let Some(cached) = self.image_textures.get(&texture) else {
            return false;
        };
        let Some(shared) = cached.shared.as_ref() else {
            return false;
        };
        if cached.size != [width, height] {
            return false;
        }
        write_texture_base_level(queue, &shared.texture, width, height, rgba);
        // Same texture id, new pixels: the retained raster is stale even though
        // the mesh signature is unchanged.
        self.supersample_dirty = true;
        true
    }

    fn ensure_image_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shared_textures: &mut SharedTextureStore,
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
        // stream textures skip mip chains: base level updates in place each frame.
        let filter = if self.stream_texture_ids.contains(&texture_key) {
            TextureFilterMode::Linear
        } else {
            self.texture_filter
        };
        let key = SharedTextureKey::from_source(source, SharedTextureColorSpace::Srgb, filter);
        let shared = match shared_textures.get(&key) {
            // another consumer already uploaded this source: reuse, no decode.
            Some(shared) => shared,
            None => {
                // resident CPU copy, or re-decode from source when the idle
                // sweep already reclaimed the bytes.
                let redecoded;
                let decoded = match resources.decoded_texture_data(texture_key) {
                    Some(decoded) if decoded.has_pixels() => decoded,
                    _ => {
                        let Some(restored) = crate::backend::decode_texture_source_rgba(
                            source,
                            static_texture_lookup,
                        ) else {
                            return false;
                        };
                        redecoded = restored;
                        &redecoded
                    }
                };
                shared_textures.ensure_rgba(
                    device,
                    queue,
                    key,
                    &decoded.rgba,
                    decoded.width,
                    decoded.height,
                )
            }
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_ui_image_bg"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shared.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let size = [shared.width, shared.height];
        self.image_textures.insert(
            texture_key,
            UiTextureGpu {
                texture: None,
                shared: Some(shared),
                bind_group,
                size,
            },
        );
        true
    }

    pub fn upsert_external_image_texture(
        &mut self,
        device: &wgpu::Device,
        texture_key: TextureID,
        view: wgpu::TextureView,
        size: [u32; 2],
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_ui_external_image_bg"),
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
                texture: None,
                shared: None,
                bind_group,
                size: [size[0].max(1), size[1].max(1)],
            },
        );
        // The view is externally owned (camera-stream output) and re-rendered
        // every frame under this same id, so meshes sampling it can never be
        // retained.
        self.external_image_texture_ids.insert(texture_key);
        self.prepared_mesh_signature = None;
        self.signature_pins.clear();
        self.prepared_revision = u64::MAX;
        self.supersample_dirty = true;
    }
}

/// wgpu buffer sizes must be a multiple of `COPY_BUFFER_ALIGNMENT`.
fn align_buffer_bytes(bytes: usize) -> usize {
    bytes.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT as usize)
}

fn hash_f32(value: f32, hasher: &mut impl Hasher) {
    value.to_bits().hash(hasher);
}

fn hash_renderable_meshes(
    primitives: &[Arc<ClippedPrimitive>],
    render_viewport: [u32; 2],
    render_scale: [f32; 2],
    mut texture_available: impl FnMut(TextureId) -> bool,
) -> (UiMeshSignature, UiMeshTotals) {
    // Fixed-seed ahash (fast, deterministic across frames) instead of SipHash.
    let mut hasher =
        ahash::RandomState::with_seeds(0x5eed_0001, 0x5eed_0002, 0x5eed_0003, 0x5eed_0004)
            .build_hasher();
    let mut totals = UiMeshTotals::default();
    render_viewport.hash(&mut hasher);
    hash_f32(render_scale[0], &mut hasher);
    hash_f32(render_scale[1], &mut hasher);
    for primitive in primitives {
        let Primitive::Mesh(mesh) = &primitive.primitive else {
            continue;
        };
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            continue;
        }
        if !texture_available(mesh.texture_id) {
            continue;
        }
        let clip_rect = clip_rect_scaled(primitive, render_viewport, render_scale);
        if clip_rect[2] == 0 || clip_rect[3] == 0 {
            continue;
        }
        let index_start = totals.index_count.min(u32::MAX as usize) as u32;
        index_start.hash(&mut hasher);
        clip_rect.hash(&mut hasher);
        hash_texture_id(mesh.texture_id, &mut hasher);
        // The painter hands out the same Arc for a node whose tessellation is
        // unchanged and a fresh Arc for any mutated (or text) node, so pointer
        // identity plus lengths proxy the vertex/index bytes without walking
        // them. Position scaling is folded in via render_scale hashed above.
        (Arc::as_ptr(primitive) as usize).hash(&mut hasher);
        mesh.vertices.len().hash(&mut hasher);
        mesh.indices.len().hash(&mut hasher);
        totals.mesh_count = totals.mesh_count.saturating_add(1);
        totals.vertex_count = totals.vertex_count.saturating_add(mesh.vertices.len());
        totals.index_count = totals.index_count.saturating_add(mesh.indices.len());
    }
    (
        UiMeshSignature {
            hash: hasher.finish(),
            mesh_count: totals.mesh_count,
            vertex_count: totals.vertex_count,
            index_count: totals.index_count,
        },
        totals,
    )
}

#[cfg(test)]
fn ui_mesh_signature_for_test(
    primitives: &[Arc<ClippedPrimitive>],
    render_viewport: [u32; 2],
    render_scale: [f32; 2],
) -> UiMeshSignature {
    hash_renderable_meshes(primitives, render_viewport, render_scale, |_| true).0
}

fn hash_texture_id(texture_id: TextureId, hasher: &mut impl Hasher) {
    match texture_id {
        TextureId::Managed(id) => {
            0_u8.hash(hasher);
            id.hash(hasher);
        }
        TextureId::User(id) => {
            1_u8.hash(hasher);
            id.hash(hasher);
        }
    }
}

fn push_ui_mesh(
    meshes: &mut Vec<UiMeshGpu>,
    index_start: u32,
    index_count: u32,
    clip_rect: [u32; 4],
    texture_id: TextureId,
) {
    if let Some(last) = meshes.last_mut()
        && last.clip_rect == clip_rect
        && last.texture_id == texture_id
        && last.index_start.saturating_add(last.index_count) == index_start
    {
        last.index_count = last.index_count.saturating_add(index_count);
        return;
    }
    meshes.push(UiMeshGpu {
        index_start,
        index_count,
        clip_rect,
        texture_id,
    });
}

#[cfg(test)]
#[path = "../../tests/unit/ui_gpu_tests.rs"]
mod ui_gpu_tests;

#[cfg(test)]
mod tests {
    use super::{UI_SUPERSAMPLE_FORMAT, UiMeshGpu, push_ui_mesh, ui_mesh_signature_for_test};
    use epaint::{ClippedPrimitive, Color32, Mesh, Primitive, Rect, TextureId, Vertex, pos2};

    #[test]
    fn compact_meshes_only_merge_exact_clip_and_texture() {
        let mut meshes = Vec::new();
        push_ui_mesh(&mut meshes, 0, 3, [0, 0, 8, 8], TextureId::Managed(1));
        push_ui_mesh(&mut meshes, 3, 6, [0, 0, 8, 8], TextureId::Managed(1));
        push_ui_mesh(&mut meshes, 9, 3, [1, 0, 8, 8], TextureId::Managed(1));
        push_ui_mesh(&mut meshes, 12, 3, [1, 0, 8, 8], TextureId::Managed(2));

        assert_eq!(meshes.len(), 3);
        assert_eq!(
            meshes[0],
            UiMeshGpu {
                index_start: 0,
                index_count: 9,
                clip_rect: [0, 0, 8, 8],
                texture_id: TextureId::Managed(1),
            }
        );
    }

    #[test]
    fn mesh_signature_matches_unchanged_mesh_data() {
        let primitives = [primitive(TextureId::Managed(1), 0.0)];
        let first = ui_mesh_signature_for_test(&primitives, [100, 100], [1.0, 1.0]);
        let second = ui_mesh_signature_for_test(&primitives, [100, 100], [1.0, 1.0]);

        assert_eq!(first, second);
    }

    #[test]
    fn mesh_signature_changes_with_texture_or_viewport() {
        let primitives = [primitive(TextureId::Managed(1), 0.0)];
        let texture_changed = [primitive(TextureId::Managed(2), 0.0)];

        let base = ui_mesh_signature_for_test(&primitives, [100, 100], [1.0, 1.0]);
        let texture = ui_mesh_signature_for_test(&texture_changed, [100, 100], [1.0, 1.0]);
        let viewport = ui_mesh_signature_for_test(&primitives, [200, 100], [2.0, 1.0]);

        assert_ne!(base, texture);
        assert_ne!(base, viewport);
    }

    fn premultiplied_over(src: [f32; 4], dst: [f32; 4]) -> [f32; 4] {
        let inv_alpha = 1.0 - src[3];
        [
            src[0] + dst[0] * inv_alpha,
            src[1] + dst[1] * inv_alpha,
            src[2] + dst[2] * inv_alpha,
            src[3] + dst[3] * inv_alpha,
        ]
    }

    #[test]
    fn transparent_viewport_with_opaque_mesh_is_visible() {
        let clear = [0.0, 0.0, 0.0, 0.0];
        let mesh = [2.5, 0.5, 0.25, 1.0];

        assert_eq!(premultiplied_over(mesh, clear), mesh);
    }

    #[test]
    fn transparent_background_stays_zero_alpha() {
        assert_eq!(premultiplied_over([0.0; 4], [0.0; 4]), [0.0; 4]);
    }

    #[test]
    fn opaque_pixel_stays_one_alpha() {
        let pixel = premultiplied_over([0.25, 0.5, 0.75, 1.0], [0.1; 4]);
        assert_eq!(pixel[3], 1.0);
    }

    #[test]
    fn supersample_target_stays_srgb_encoded_linear_input() {
        // Composite samples this target and expects linear values back, which
        // holds for sRGB-view formats (decode on sample) and float formats.
        assert!(UI_SUPERSAMPLE_FORMAT.is_srgb());
    }

    fn primitive(texture_id: TextureId, x: f32) -> std::sync::Arc<ClippedPrimitive> {
        let mut mesh = Mesh::with_texture(texture_id);
        mesh.vertices = vec![vertex(x, 0.0), vertex(x + 1.0, 0.0), vertex(x + 1.0, 1.0)];
        mesh.indices = vec![0, 1, 2];
        std::sync::Arc::new(ClippedPrimitive {
            clip_rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0)),
            primitive: Primitive::Mesh(mesh),
        })
    }

    fn vertex(x: f32, y: f32) -> Vertex {
        Vertex {
            pos: pos2(x, y),
            uv: pos2(x, y),
            color: Color32::WHITE,
        }
    }
}
