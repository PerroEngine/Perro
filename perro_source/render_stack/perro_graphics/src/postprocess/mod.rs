// File: perro_source\render_stack\perro_graphics\src\postprocess\mod.rs

use crate::backend::{StaticShaderLookup, StaticTextureLookup};
use crate::postprocess::shaders::{build_post_shader, create_builtin_shader_module};
use bytemuck::{Pod, Zeroable};
use perro_graphics_assets::{decode_image_rgba, decode_ptex};
use perro_io::load_asset;
use perro_render_bridge::{Camera3DState, CameraProjectionState};
use perro_structs::{CustomPostParam, CustomPostParamValue, PostProcessEffect};
use std::collections::HashMap;
use std::ops::Range;

mod shaders;

const EFFECT_CUSTOM: u32 = 0;
const EFFECT_BLUR: u32 = 1;
const EFFECT_PIXELATE: u32 = 2;
const EFFECT_WARP: u32 = 3;
const EFFECT_VIGNETTE: u32 = 4;
const EFFECT_CRT: u32 = 5;
const EFFECT_COLOR_FILTER: u32 = 6;
const EFFECT_REVERSE_FILTER: u32 = 7;
const EFFECT_BLOOM: u32 = 8;
const EFFECT_SATURATE: u32 = 9;
const EFFECT_BLACK_WHITE: u32 = 10;
const EFFECT_COLOR_GRADE: u32 = 11;
const EFFECT_LUT_2D: u32 = 12;
const EFFECT_LUT_3D: u32 = 13;
// Internal sub-pass effect type: bloom bright-pass + downsample (not exposed via
// PostProcessEffect). Bloom composite reuses EFFECT_BLOOM.
const EFFECT_BLOOM_BRIGHT: u32 = 14;

// Extra uniform slots reserved after the per-effect region for multi-pass effect
// sub-passes. Bloom uses the most (4).
const SUBPASS_UNIFORM_SLOTS: usize = 4;

// Internal effect type for a merged run of cheap per-pixel color ops.
const EFFECT_MERGED: u32 = 15;
const EFFECT_CHROMA_KEY: u32 = 16;

/// Per-frame uniform fields shared across an effect's sub-passes.
struct PostUniformFrameCtx {
    projection_mode: u32,
    near: f32,
    far: f32,
    time: f32,
}

/// One executable step in the resolved chain. Consecutive cheap color ops fold
/// into a single Merged step so they cost one pass instead of one pass each.
enum ChainStep {
    Single(usize),
    /// A merged run of color ops. `ops` is the op count; `descriptors` packs
    /// each op as a header vec4 ([type, param_vec4_count, _, _]) followed by
    /// that many param vec4s in the retained merged descriptor scratch.
    Merged {
        ops: u32,
        descriptors: Range<usize>,
    },
}

/// True for per-pixel color ops that can fold into one merged pass. Includes
/// color_grade (5 param vec4s) via the variable-length descriptor format.
fn is_mergeable_color_op(effect: &PostProcessEffect) -> bool {
    matches!(
        effect,
        PostProcessEffect::ColorFilter { .. }
            | PostProcessEffect::ReverseFilter { .. }
            | PostProcessEffect::ChromaKey { .. }
            | PostProcessEffect::Saturate { .. }
            | PostProcessEffect::BlackWhite { .. }
            | PostProcessEffect::Vignette { .. }
            | PostProcessEffect::ColorGrade { .. }
    )
}

/// Append one op as header + param vec4s. Cheap ops carry 2 param vec4s;
/// color_grade carries 5.
fn encode_merged_op(effect: &PostProcessEffect, descriptors: &mut Vec<[f32; 4]>) {
    let e = encode_effect_params(effect);
    if matches!(effect, PostProcessEffect::ColorGrade { .. }) {
        descriptors.push([e.effect_type as f32, 5.0, 0.0, 0.0]);
        descriptors.extend_from_slice(&[e.params0, e.params1, e.params2, e.params3, e.params4]);
    } else {
        descriptors.push([e.effect_type as f32, 2.0, 0.0, 0.0]);
        descriptors.extend_from_slice(&[e.params0, e.params1]);
    }
}

/// Collapse consecutive mergeable ops into Merged steps; everything else stays
/// Single. Retained scratch vectors keep repeated frames allocation-free.
fn build_chain_steps_into(
    effects: &[PostProcessEffect],
    steps: &mut Vec<ChainStep>,
    merged_descriptors: &mut Vec<[f32; 4]>,
) {
    steps.clear();
    merged_descriptors.clear();
    steps.reserve(effects.len().saturating_sub(steps.len()));
    let mut i = 0;
    while i < effects.len() {
        if matches!(effects[i], PostProcessEffect::Exposure { .. }) {
            i += 1;
            continue;
        }
        if is_mergeable_color_op(&effects[i]) {
            let start = i;
            while i < effects.len() && is_mergeable_color_op(&effects[i]) {
                i += 1;
            }
            if i - start >= 2 {
                let descriptor_start = merged_descriptors.len();
                for effect in &effects[start..i] {
                    encode_merged_op(effect, merged_descriptors);
                }
                let descriptor_end = merged_descriptors.len();
                steps.push(ChainStep::Merged {
                    ops: (i - start) as u32,
                    descriptors: descriptor_start..descriptor_end,
                });
                continue;
            }
            // Single mergeable op: keep the normal path.
            steps.push(ChainStep::Single(start));
            i = start + 1;
            continue;
        }
        steps.push(ChainStep::Single(i));
        i += 1;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostUniform {
    effect_type: u32,
    param_count: u32,
    projection_mode: u32,
    _pad0: u32,
    params0: [f32; 4],
    params1: [f32; 4],
    params2: [f32; 4],
    params3: [f32; 4],
    params4: [f32; 4],
    params5: [f32; 4],
    resolution: [f32; 2],
    inv_resolution: [f32; 2],
    near: f32,
    far: f32,
    time: [f32; 2],
}

struct EncodedEffectParams {
    effect_type: u32,
    params0: [f32; 4],
    params1: [f32; 4],
    params2: [f32; 4],
    params3: [f32; 4],
    params4: [f32; 4],
    params5: [f32; 4],
    custom_params: Vec<[f32; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum PostInputKind {
    External,
    PingA,
    PingB,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PostBindGroupKey {
    input_kind: PostInputKind,
    external_input_view_key: u64,
    depth_view_key: u64,
    uniform_buffer_generation: u32,
    params_buffer_generation: u32,
    lut_2d_key: u64,
    lut_3d_key: u64,
}

#[derive(Clone, Copy)]
struct PostViewKeys {
    external_input: u64,
    depth: u64,
}

#[derive(Clone, Copy, Default)]
struct PostPerfCounters {
    bind_group_hits: u32,
    bind_group_misses: u32,
}

pub struct PostProcessContext<'a> {
    pub(crate) device: &'a wgpu::Device,
    pub(crate) queue: &'a wgpu::Queue,
    pub(crate) output_view: &'a wgpu::TextureView,
    pub(crate) camera: &'a Camera3DState,
    pub(crate) external_input_view_key: u64,
    pub(crate) depth_view_key: u64,
    pub(crate) static_shader_lookup: Option<StaticShaderLookup>,
    pub(crate) static_texture_lookup: Option<StaticTextureLookup>,
}

pub struct PostProcessChainData<'a> {
    pub(crate) input_view: &'a wgpu::TextureView,
    pub(crate) depth_view: &'a wgpu::TextureView,
    pub(crate) effects: &'a [PostProcessEffect],
}

pub struct PostProcessor {
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    scene_texture: wgpu::Texture,
    scene_view: wgpu::TextureView,
    ping_a: wgpu::Texture,
    ping_a_view: wgpu::TextureView,
    ping_b: wgpu::Texture,
    ping_b_view: wgpu::TextureView,
    // Lazily-allocated scratch targets for multi-pass effects. Sized from the
    // main targets and dropped on resize. blur_scratch is full-res (separable
    // blur intermediate); bloom half targets are half-res (downsampled bloom).
    blur_scratch: Option<CachedPostTexture>,
    bloom_half_a: Option<CachedPostTexture>,
    bloom_half_b: Option<CachedPostTexture>,
    sampler: wgpu::Sampler,
    _default_lut_2d_texture: wgpu::Texture,
    default_lut_2d_view: wgpu::TextureView,
    _default_lut_3d_texture: wgpu::Texture,
    default_lut_3d_view: wgpu::TextureView,
    lut_2d_textures: HashMap<u64, CachedPostTexture>,
    lut_3d_textures: HashMap<u64, CachedPostTexture>,
    bgl: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    builtin_pipeline: wgpu::RenderPipeline,
    custom_pipelines: HashMap<u64, wgpu::RenderPipeline>,
    post_bind_groups: HashMap<PostBindGroupKey, wgpu::BindGroup>,
    uniform_buffer: wgpu::Buffer,
    uniform_stride: u64,
    uniform_capacity: usize,
    uniform_buffer_generation: u32,
    params_buffer: wgpu::Buffer,
    params_capacity: usize,
    params_buffer_generation: u32,
    lut_generation: u32,
    frame_counter: u64,
    perf_counters: PostPerfCounters,
    chain_steps_scratch: Vec<ChainStep>,
    merged_descriptors_scratch: Vec<[f32; 4]>,
}

struct PostBindGroupDesc<'a> {
    bgl: &'a wgpu::BindGroupLayout,
    input_view: &'a wgpu::TextureView,
    sampler: &'a wgpu::Sampler,
    depth_view: &'a wgpu::TextureView,
    uniform_buffer: &'a wgpu::Buffer,
    uniform_size_bytes: u64,
    params_buffer: &'a wgpu::Buffer,
    lut_2d_view: &'a wgpu::TextureView,
    lut_3d_view: &'a wgpu::TextureView,
}

struct CachedPostTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

#[inline]
fn create_post_bind_group(device: &wgpu::Device, desc: PostBindGroupDesc<'_>) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_post_bg"),
        layout: desc.bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(desc.input_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(desc.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(desc.depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: desc.uniform_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(desc.uniform_size_bytes)
                            .expect("post uniform size must be non-zero"),
                    ),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: desc.params_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(desc.lut_2d_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(desc.lut_3d_view),
            },
        ],
    })
}

impl PostProcessor {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let (scene_texture, scene_view) =
            create_color_target(device, format, width, height, "perro_post_scene");
        let (ping_a, ping_a_view) =
            create_color_target(device, format, width, height, "perro_post_ping_a");
        let (ping_b, ping_b_view) =
            create_color_target(device, format, width, height, "perro_post_ping_b");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("perro_post_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let (default_lut_2d_texture, default_lut_2d_view) = create_default_lut_2d(device, queue);
        let (default_lut_3d_texture, default_lut_3d_view) = create_default_lut_3d(device, queue);
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_post_bgl"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<PostUniform>() as u64)
                                .expect("post uniform size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let uniform_size_bytes = std::mem::size_of::<PostUniform>() as u64;
        let uniform_stride = align_up_uniform(
            uniform_size_bytes,
            device.limits().min_uniform_buffer_offset_alignment as u64,
        );
        let uniform_capacity = 1usize;
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_post_uniforms"),
            size: uniform_stride * uniform_capacity as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_capacity = 1usize;
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_post_params"),
            size: (params_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_post_pipeline_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let shader = create_builtin_shader_module(device);
        let builtin_pipeline = create_pipeline(device, &pipeline_layout, &shader, format);

        Self {
            format,
            width,
            height,
            scene_texture,
            scene_view,
            ping_a,
            ping_a_view,
            ping_b,
            ping_b_view,
            blur_scratch: None,
            bloom_half_a: None,
            bloom_half_b: None,
            sampler,
            _default_lut_2d_texture: default_lut_2d_texture,
            default_lut_2d_view,
            _default_lut_3d_texture: default_lut_3d_texture,
            default_lut_3d_view,
            lut_2d_textures: HashMap::new(),
            lut_3d_textures: HashMap::new(),
            bgl,
            pipeline_layout,
            builtin_pipeline,
            custom_pipelines: HashMap::new(),
            post_bind_groups: HashMap::new(),
            uniform_buffer,
            uniform_stride,
            uniform_capacity,
            uniform_buffer_generation: 1,
            params_buffer,
            params_capacity,
            params_buffer_generation: 1,
            lut_generation: 1,
            frame_counter: 0,
            perf_counters: PostPerfCounters::default(),
            chain_steps_scratch: Vec::new(),
            merged_descriptors_scratch: Vec::new(),
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        let (scene_texture, scene_view) =
            create_color_target(device, self.format, width, height, "perro_post_scene");
        let (ping_a, ping_a_view) =
            create_color_target(device, self.format, width, height, "perro_post_ping_a");
        let (ping_b, ping_b_view) =
            create_color_target(device, self.format, width, height, "perro_post_ping_b");
        self.scene_texture = scene_texture;
        self.scene_view = scene_view;
        self.ping_a = ping_a;
        self.ping_a_view = ping_a_view;
        self.ping_b = ping_b;
        self.ping_b_view = ping_b_view;
        // Drop scratch targets; they reallocate lazily at the new size.
        self.blur_scratch = None;
        self.bloom_half_a = None;
        self.bloom_half_b = None;
        self.post_bind_groups.clear();
    }

    pub fn scene_view(&self) -> &wgpu::TextureView {
        &self.scene_view
    }

    pub fn scene_texture(&self) -> &wgpu::Texture {
        &self.scene_texture
    }

    pub fn uses_depth(effects: &[PostProcessEffect]) -> bool {
        effects
            .iter()
            .any(|e| matches!(e, PostProcessEffect::Custom { .. }))
    }

    pub fn has_effects(effects: &[PostProcessEffect]) -> bool {
        effects
            .iter()
            .any(|effect| !matches!(effect, PostProcessEffect::Exposure { .. }))
    }

    // Refactored apply_chain to reduce argument count and improve clarity
    pub fn apply_chain(
        &mut self,
        ctx: &PostProcessContext,
        chain_data: &PostProcessChainData,
        encoder: &mut wgpu::CommandEncoder, // Encoder is passed mutably for the render pass
    ) {
        // Destructure context and chain data for easier access
        let PostProcessContext {
            device,
            queue,
            output_view,
            camera,
            external_input_view_key,
            depth_view_key,
            static_shader_lookup,
            static_texture_lookup,
        } = ctx;

        let PostProcessChainData {
            input_view,
            depth_view,
            effects,
        } = chain_data;
        let view_keys = PostViewKeys {
            external_input: *external_input_view_key,
            depth: *depth_view_key,
        };

        if effects.is_empty() {
            return;
        }

        let (projection_mode, near, far) = projection_uniform_params(camera);
        let width = self.width.max(1) as f32;
        let height = self.height.max(1) as f32;
        let inv_width = 1.0 / width;
        let inv_height = 1.0 / height;
        self.frame_counter = self.frame_counter.wrapping_add(1);
        let time = self.frame_counter as f32;
        self.perf_counters = PostPerfCounters::default();

        // Resolve the chain into executable steps, folding consecutive cheap
        // color ops into single merged passes.
        let mut steps = std::mem::take(&mut self.chain_steps_scratch);
        let mut merged_descriptors = std::mem::take(&mut self.merged_descriptors_scratch);
        build_chain_steps_into(effects, &mut steps, &mut merged_descriptors);

        let mut max_params = 0usize;
        for effect in *effects {
            if let PostProcessEffect::Custom {
                shader_path,
                params,
            } = effect
            {
                self.ensure_custom_pipeline(device, shader_path.as_ref(), *static_shader_lookup);
                max_params = max_params.max(params.len());
            }
        }
        // Merged passes pack all their descriptor vec4s into the params buffer
        // at distinct offsets, so capacity must cover the total.
        let merged_params: usize = steps
            .iter()
            .map(|step| match step {
                ChainStep::Merged { descriptors, .. } => descriptors.len(),
                ChainStep::Single(_) => 0,
            })
            .sum();
        // Custom effects read custom_params from offset 0 for param_count vec4s;
        // reserve that region and place merged descriptors after it so a Custom
        // effect and a merged step in the same chain never alias.
        let merged_region_start = max_params;
        self.ensure_params_capacity(device, merged_region_start + merged_params);
        // Extra uniform slots after the per-step region for multi-pass effect
        // sub-passes (bloom uses up to 4). Only one effect runs at a time, so a
        // single shared scratch region suffices.
        let subpass_base = steps.len();
        self.ensure_uniform_capacity(device, steps.len() + SUBPASS_UNIFORM_SLOTS);

        let uniform_ctx = PostUniformFrameCtx {
            projection_mode,
            near,
            far,
            time,
        };

        let mut input_kind = 0u8; // 0=external input_view, 1=ping_a, 2=ping_b
        let mut use_ping_a = true;
        // Vec4 index into the params buffer where the next merged step packs
        // its descriptors; distinct per step (and past the Custom region) to
        // avoid same-submit buffer aliasing.
        let mut merged_vec4_base = merged_region_start as u32;
        for (index, step) in steps.iter().enumerate() {
            let last = index + 1 == steps.len();
            let current_input = match input_kind {
                0 => (*input_view).clone(),
                1 => self.ping_a_view.clone(),
                _ => self.ping_b_view.clone(),
            };

            let target_view = if last {
                (*output_view).clone()
            } else if use_ping_a {
                self.ping_a_view.clone()
            } else {
                self.ping_b_view.clone()
            };

            // Merged run of cheap color ops: one pass applies the whole run.
            if let ChainStep::Merged { ops, descriptors } = step {
                let descriptors = &merged_descriptors[descriptors.clone()];
                let vec4_base = merged_vec4_base;
                merged_vec4_base += descriptors.len() as u32;
                queue.write_buffer(
                    &self.params_buffer,
                    vec4_base as u64 * std::mem::size_of::<[f32; 4]>() as u64,
                    bytemuck::cast_slice(descriptors),
                );
                let uniform = PostUniform {
                    effect_type: EFFECT_MERGED,
                    param_count: *ops,
                    projection_mode,
                    _pad0: 0,
                    params0: [vec4_base as f32, 0.0, 0.0, 0.0],
                    params1: [0.0; 4],
                    params2: [0.0; 4],
                    params3: [0.0; 4],
                    params4: [0.0; 4],
                    params5: [0.0; 4],
                    resolution: [width, height],
                    inv_resolution: [inv_width, inv_height],
                    near,
                    far,
                    time: [time, time],
                };
                let uniform_offset = index as u64 * self.uniform_stride;
                let Ok(dynamic_offset) = u32::try_from(uniform_offset) else {
                    continue;
                };
                queue.write_buffer(
                    &self.uniform_buffer,
                    uniform_offset,
                    bytemuck::bytes_of(&uniform),
                );
                let input_pk = match input_kind {
                    0 => PostInputKind::External,
                    1 => PostInputKind::PingA,
                    _ => PostInputKind::PingB,
                };
                let bind_group =
                    self.merged_bind_group(device, input_pk, &current_input, depth_view, view_keys);
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("perro_post_merged_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &target_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_pipeline(&self.builtin_pipeline);
                    pass.set_bind_group(0, &bind_group, &[dynamic_offset]);
                    pass.draw(0..3, 0..1);
                }
                input_kind = if last {
                    input_kind
                } else if use_ping_a {
                    1
                } else {
                    2
                };
                use_ping_a = !use_ping_a;
                continue;
            }

            let ChainStep::Single(effect_index) = step else {
                continue;
            };
            let effect = &effects[*effect_index];
            self.ensure_lut_texture(device, queue, effect, *static_texture_lookup);

            // Multi-pass effects (separable blur, downsampled bloom) run their
            // own sub-pass chain and write the final result into target_view.
            if let PostProcessEffect::Blur { strength } = effect {
                self.run_blur_effect(
                    device,
                    queue,
                    encoder,
                    &uniform_ctx,
                    subpass_base,
                    *strength,
                    &current_input,
                    depth_view,
                    &target_view,
                );
                input_kind = if last {
                    input_kind
                } else if use_ping_a {
                    1
                } else {
                    2
                };
                use_ping_a = !use_ping_a;
                continue;
            }
            if let PostProcessEffect::Bloom {
                strength,
                threshold,
                radius,
            } = effect
            {
                self.run_bloom_effect(
                    device,
                    queue,
                    encoder,
                    &uniform_ctx,
                    subpass_base,
                    *strength,
                    *threshold,
                    *radius,
                    &current_input,
                    depth_view,
                    &target_view,
                );
                input_kind = if last {
                    input_kind
                } else if use_ping_a {
                    1
                } else {
                    2
                };
                use_ping_a = !use_ping_a;
                continue;
            }

            // Use the new struct for encoded params
            let encoded_params = encode_effect_params(effect);
            let param_count = encoded_params.custom_params.len() as u32;
            if !encoded_params.custom_params.is_empty() {
                queue.write_buffer(
                    &self.params_buffer,
                    0,
                    bytemuck::cast_slice(&encoded_params.custom_params),
                );
            }
            let uniform = PostUniform {
                effect_type: encoded_params.effect_type,
                param_count,
                projection_mode,
                _pad0: 0,
                params0: encoded_params.params0,
                params1: encoded_params.params1,
                params2: encoded_params.params2,
                params3: encoded_params.params3,
                params4: encoded_params.params4,
                params5: encoded_params.params5,
                resolution: [width, height],
                inv_resolution: [inv_width, inv_height],
                near,
                far,
                time: [time, time],
            };
            let uniform_offset = index as u64 * self.uniform_stride;
            let Ok(uniform_dynamic_offset) = u32::try_from(uniform_offset) else {
                continue;
            };
            queue.write_buffer(
                &self.uniform_buffer,
                uniform_offset,
                bytemuck::bytes_of(&uniform),
            );

            let bind_group = self.bind_group_for_effect(
                device,
                effect,
                match input_kind {
                    0 => PostInputKind::External,
                    1 => PostInputKind::PingA,
                    _ => PostInputKind::PingB,
                },
                &current_input,
                depth_view,
                view_keys,
            );
            let pipeline = match effect {
                PostProcessEffect::Custom { shader_path, .. } => self
                    .custom_pipelines
                    .get(&post_shader_key(shader_path.as_ref()))
                    .unwrap_or(&self.builtin_pipeline),
                _ => &self.builtin_pipeline,
            };

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_post_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &bind_group, &[uniform_dynamic_offset]);
                pass.draw(0..3, 0..1);
            }

            input_kind = if last {
                input_kind
            } else if use_ping_a {
                1
            } else {
                2
            };
            use_ping_a = !use_ping_a;
        }
        self.chain_steps_scratch = steps;
        self.merged_descriptors_scratch = merged_descriptors;
    }

    /// Ensure the full-res blur scratch target exists.
    fn ensure_blur_scratch(&mut self, device: &wgpu::Device) -> Option<wgpu::TextureView> {
        if self.blur_scratch.is_none() {
            let (texture, view) = create_color_target(
                device,
                self.format,
                self.width,
                self.height,
                "perro_post_blur_scratch",
            );
            self.blur_scratch = Some(CachedPostTexture { texture, view });
        }
        self.blur_scratch
            .as_ref()
            .map(|scratch| scratch.view.clone())
    }

    /// Ensure the two half-res bloom targets exist.
    fn ensure_bloom_targets(
        &mut self,
        device: &wgpu::Device,
    ) -> Option<(wgpu::TextureView, wgpu::TextureView)> {
        let hw = (self.width / 2).max(1);
        let hh = (self.height / 2).max(1);
        if self.bloom_half_a.is_none() {
            let (texture, view) =
                create_color_target(device, self.format, hw, hh, "perro_post_bloom_a");
            self.bloom_half_a = Some(CachedPostTexture { texture, view });
        }
        if self.bloom_half_b.is_none() {
            let (texture, view) =
                create_color_target(device, self.format, hw, hh, "perro_post_bloom_b");
            self.bloom_half_b = Some(CachedPostTexture { texture, view });
        }
        let view_a = self.bloom_half_a.as_ref()?.view.clone();
        let view_b = self.bloom_half_b.as_ref()?.view.clone();
        Some((view_a, view_b))
    }

    /// Record one full-screen builtin pass into `target`, using a transient
    /// (uncached) bind group. Used by the multi-pass effect drivers.
    #[allow(clippy::too_many_arguments)]
    fn record_sub_pass(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        ctx: &PostUniformFrameCtx,
        uniform_slot: usize,
        effect_type: u32,
        params0: [f32; 4],
        target_dims: [u32; 2],
        input_view: &wgpu::TextureView,
        second_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        target_view: &wgpu::TextureView,
    ) {
        let width = target_dims[0].max(1) as f32;
        let height = target_dims[1].max(1) as f32;
        let uniform = PostUniform {
            effect_type,
            param_count: 0,
            projection_mode: ctx.projection_mode,
            _pad0: 0,
            params0,
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            params4: [0.0; 4],
            params5: [0.0; 4],
            resolution: [width, height],
            inv_resolution: [1.0 / width, 1.0 / height],
            near: ctx.near,
            far: ctx.far,
            time: [ctx.time, ctx.time],
        };
        let uniform_offset = uniform_slot as u64 * self.uniform_stride;
        let Ok(dynamic_offset) = u32::try_from(uniform_offset) else {
            return;
        };
        queue.write_buffer(
            &self.uniform_buffer,
            uniform_offset,
            bytemuck::bytes_of(&uniform),
        );

        let bind_group = create_post_bind_group(
            device,
            PostBindGroupDesc {
                bgl: &self.bgl,
                input_view,
                sampler: &self.sampler,
                depth_view,
                uniform_buffer: &self.uniform_buffer,
                uniform_size_bytes: std::mem::size_of::<PostUniform>() as u64,
                params_buffer: &self.params_buffer,
                // second_view rides in the lut_2d slot (bloom composite reads it).
                lut_2d_view: second_view,
                lut_3d_view: &self.default_lut_3d_view,
            },
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_post_subpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.builtin_pipeline);
        pass.set_bind_group(0, &bind_group, &[dynamic_offset]);
        pass.draw(0..3, 0..1);
    }

    /// Separable gaussian blur: horizontal pass into blur_scratch, vertical pass
    /// into the effect's output target.
    #[allow(clippy::too_many_arguments)]
    fn run_blur_effect(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        ctx: &PostUniformFrameCtx,
        subpass_base: usize,
        strength: f32,
        input_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        target_view: &wgpu::TextureView,
    ) {
        let Some(scratch) = self.ensure_blur_scratch(device) else {
            return;
        };
        let dims = [self.width, self.height];
        let default_lut = self.default_lut_2d_view.clone();
        // Horizontal (axis 0): input -> scratch.
        self.record_sub_pass(
            device,
            queue,
            encoder,
            ctx,
            subpass_base,
            EFFECT_BLUR,
            [strength, 0.0, 0.0, 0.0],
            dims,
            input_view,
            &default_lut,
            depth_view,
            &scratch,
        );
        // Vertical (axis 1): scratch -> target.
        self.record_sub_pass(
            device,
            queue,
            encoder,
            ctx,
            subpass_base + 1,
            EFFECT_BLUR,
            [strength, 1.0, 0.0, 0.0],
            dims,
            &scratch,
            &default_lut,
            depth_view,
            target_view,
        );
    }

    /// Downsampled bloom: bright-pass into half-res A, separable blur A<->B at
    /// half res, then composite the upsampled bloom over the full-res scene.
    #[allow(clippy::too_many_arguments)]
    fn run_bloom_effect(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        ctx: &PostUniformFrameCtx,
        subpass_base: usize,
        strength: f32,
        threshold: f32,
        radius: f32,
        input_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        target_view: &wgpu::TextureView,
    ) {
        let Some((view_a, view_b)) = self.ensure_bloom_targets(device) else {
            return;
        };
        let full = [self.width, self.height];
        let half = [(self.width / 2).max(1), (self.height / 2).max(1)];
        let default_lut = self.default_lut_2d_view.clone();
        // Bright-pass + downsample: full-res input -> half-res A.
        self.record_sub_pass(
            device,
            queue,
            encoder,
            ctx,
            subpass_base,
            EFFECT_BLOOM_BRIGHT,
            [0.0, threshold, 0.0, 0.0],
            half,
            input_view,
            &default_lut,
            depth_view,
            &view_a,
        );
        // Blur horizontal: A -> B (half res).
        self.record_sub_pass(
            device,
            queue,
            encoder,
            ctx,
            subpass_base + 1,
            EFFECT_BLUR,
            [radius.max(0.0), 0.0, 0.0, 0.0],
            half,
            &view_a,
            &default_lut,
            depth_view,
            &view_b,
        );
        // Blur vertical: B -> A (half res).
        self.record_sub_pass(
            device,
            queue,
            encoder,
            ctx,
            subpass_base + 2,
            EFFECT_BLUR,
            [radius.max(0.0), 1.0, 0.0, 0.0],
            half,
            &view_b,
            &default_lut,
            depth_view,
            &view_a,
        );
        // Composite: full-res scene (input_tex) + upsampled bloom A (lut slot).
        self.record_sub_pass(
            device,
            queue,
            encoder,
            ctx,
            subpass_base + 3,
            EFFECT_BLOOM,
            [strength, 0.0, 0.0, 0.0],
            full,
            input_view,
            &view_a,
            depth_view,
            target_view,
        );
    }

    fn ensure_custom_pipeline(
        &mut self,
        device: &wgpu::Device,
        shader_path: &str,
        static_shader_lookup: Option<StaticShaderLookup>,
    ) -> Option<&wgpu::RenderPipeline> {
        let shader_key = post_shader_key(shader_path);
        if self.custom_pipelines.contains_key(&shader_key) {
            return self.custom_pipelines.get(&shader_key);
        }
        let src = if let Some(lookup) = static_shader_lookup {
            let shader_hash = perro_ids::parse_hashed_source_uri(shader_path)
                .unwrap_or_else(|| perro_ids::string_to_u64(shader_path));
            let src = lookup(shader_hash);
            (!src.is_empty()).then(|| src.to_string())
        } else {
            None
        }
        .or_else(|| {
            let bytes = load_asset(shader_path).ok()?;
            let src = std::str::from_utf8(&bytes).ok()?;
            Some(src.to_string())
        })?;
        let wgsl = build_post_shader(&src);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_post_custom"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
        let pipeline = create_pipeline(device, &self.pipeline_layout, &shader, self.format);
        self.custom_pipelines.insert(shader_key, pipeline);
        self.custom_pipelines.get(&shader_key)
    }

    fn ensure_lut_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effect: &PostProcessEffect,
        static_texture_lookup: Option<StaticTextureLookup>,
    ) {
        match effect {
            PostProcessEffect::Lut2D {
                texture_path, size, ..
            } => {
                let key = lut_key(texture_path.as_ref(), *size);
                if self.lut_2d_textures.contains_key(&key) {
                    return;
                }
                let Some((rgba, width, height)) =
                    load_post_texture_rgba(texture_path.as_ref(), static_texture_lookup)
                else {
                    return;
                };
                let texture = create_post_lut_2d(device, queue, rgba, width, height);
                self.lut_2d_textures.insert(key, texture);
                self.bump_lut_generation();
            }
            PostProcessEffect::Lut3D {
                texture_path, size, ..
            } => {
                let key = lut_key(texture_path.as_ref(), *size);
                if self.lut_3d_textures.contains_key(&key) {
                    return;
                }
                let Some((rgba, width, height)) =
                    load_post_texture_rgba(texture_path.as_ref(), static_texture_lookup)
                else {
                    return;
                };
                let Some((rgba_3d, size)) = flattened_lut_to_3d(rgba, width, height, *size) else {
                    return;
                };
                let texture = create_post_lut_3d(device, queue, rgba_3d, size);
                self.lut_3d_textures.insert(key, texture);
                self.bump_lut_generation();
            }
            _ => {}
        }
    }

    fn lut_views(&self, effect: &PostProcessEffect) -> (&wgpu::TextureView, &wgpu::TextureView) {
        match effect {
            PostProcessEffect::Lut2D {
                texture_path, size, ..
            } => {
                let key = lut_key(texture_path.as_ref(), *size);
                let view = self
                    .lut_2d_textures
                    .get(&key)
                    .map(|texture| &texture.view)
                    .unwrap_or(&self.default_lut_2d_view);
                (view, &self.default_lut_3d_view)
            }
            PostProcessEffect::Lut3D {
                texture_path, size, ..
            } => {
                let key = lut_key(texture_path.as_ref(), *size);
                let view = self
                    .lut_3d_textures
                    .get(&key)
                    .map(|texture| &texture.view)
                    .unwrap_or(&self.default_lut_3d_view);
                (&self.default_lut_2d_view, view)
            }
            _ => (&self.default_lut_2d_view, &self.default_lut_3d_view),
        }
    }

    fn ensure_params_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.params_capacity {
            return;
        }
        let mut new_capacity = self.params_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_post_params"),
            size: (new_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.params_capacity = new_capacity;
        self.params_buffer_generation = next_generation(self.params_buffer_generation);
        self.post_bind_groups.clear();
    }

    fn ensure_uniform_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.uniform_capacity {
            return;
        }
        let mut new_capacity = self.uniform_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_post_uniforms"),
            size: self.uniform_stride * new_capacity as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.uniform_capacity = new_capacity;
        self.uniform_buffer_generation = next_generation(self.uniform_buffer_generation);
        self.post_bind_groups.clear();
    }

    fn bind_group_for_effect(
        &mut self,
        device: &wgpu::Device,
        effect: &PostProcessEffect,
        input_kind: PostInputKind,
        input_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        view_keys: PostViewKeys,
    ) -> wgpu::BindGroup {
        let lut_2d_key = lut_hash_2d(effect);
        let lut_3d_key = lut_hash_3d(effect);
        let key = PostBindGroupKey {
            input_kind,
            external_input_view_key: if input_kind == PostInputKind::External {
                view_keys.external_input
            } else {
                0
            },
            depth_view_key: view_keys.depth,
            uniform_buffer_generation: self.uniform_buffer_generation,
            params_buffer_generation: self.params_buffer_generation,
            lut_2d_key,
            lut_3d_key,
        };
        if let Some(bind_group) = self.post_bind_groups.get(&key) {
            self.perf_counters.bind_group_hits =
                self.perf_counters.bind_group_hits.saturating_add(1);
            return bind_group.clone();
        }
        let (lut_2d_view, lut_3d_view) = self.lut_views(effect);
        let bind_group = create_post_bind_group(
            device,
            PostBindGroupDesc {
                bgl: &self.bgl,
                input_view,
                sampler: &self.sampler,
                depth_view,
                uniform_buffer: &self.uniform_buffer,
                uniform_size_bytes: std::mem::size_of::<PostUniform>() as u64,
                params_buffer: &self.params_buffer,
                lut_2d_view,
                lut_3d_view,
            },
        );
        self.post_bind_groups.insert(key, bind_group.clone());
        self.perf_counters.bind_group_misses =
            self.perf_counters.bind_group_misses.saturating_add(1);
        bind_group
    }

    /// Bind group for a merged color-op pass. Uses default LUT views (merged
    /// ops never sample LUTs) and the shared uniform + params buffers.
    fn merged_bind_group(
        &mut self,
        device: &wgpu::Device,
        input_kind: PostInputKind,
        input_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        view_keys: PostViewKeys,
    ) -> wgpu::BindGroup {
        let key = PostBindGroupKey {
            input_kind,
            external_input_view_key: if input_kind == PostInputKind::External {
                view_keys.external_input
            } else {
                0
            },
            depth_view_key: view_keys.depth,
            uniform_buffer_generation: self.uniform_buffer_generation,
            params_buffer_generation: self.params_buffer_generation,
            // Distinct LUT keys so merged bind groups never collide with an
            // effect's cached bind group that happens to share inputs.
            lut_2d_key: u64::MAX,
            lut_3d_key: u64::MAX,
        };
        if let Some(bind_group) = self.post_bind_groups.get(&key) {
            return bind_group.clone();
        }
        let bind_group = create_post_bind_group(
            device,
            PostBindGroupDesc {
                bgl: &self.bgl,
                input_view,
                sampler: &self.sampler,
                depth_view,
                uniform_buffer: &self.uniform_buffer,
                uniform_size_bytes: std::mem::size_of::<PostUniform>() as u64,
                params_buffer: &self.params_buffer,
                lut_2d_view: &self.default_lut_2d_view,
                lut_3d_view: &self.default_lut_3d_view,
            },
        );
        self.post_bind_groups.insert(key, bind_group.clone());
        bind_group
    }

    fn bump_lut_generation(&mut self) {
        self.lut_generation = next_generation(self.lut_generation);
        self.post_bind_groups.clear();
    }
}

mod resources;
use resources::*;
mod encode;
use encode::*;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
