// File: perro_source\render_stack\perro_graphics\src\postprocess\mod.rs

use crate::backend::StaticShaderLookup;
use crate::postprocess::shaders::{build_post_shader, create_builtin_shader_module};
use bytemuck::{Pod, Zeroable};
use perro_io::load_asset;
use perro_render_bridge::{Camera3DState, CameraProjectionState};
use perro_structs::{CustomPostParam, CustomPostParamValue, PostProcessEffect};
use std::collections::HashMap;

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
    custom_params: Vec<[f32; 4]>,
}

pub struct PostProcessContext<'a> {
    pub(crate) device: &'a wgpu::Device,
    pub(crate) queue: &'a wgpu::Queue,
    pub(crate) output_view: &'a wgpu::TextureView,
    pub(crate) camera: &'a Camera3DState,
    pub(crate) static_shader_lookup: Option<StaticShaderLookup>,
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
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    builtin_pipeline: wgpu::RenderPipeline,
    custom_pipelines: HashMap<String, wgpu::RenderPipeline>,
    uniform_buffer: wgpu::Buffer,
    uniform_stride: u64,
    uniform_capacity: usize,
    params_buffer: wgpu::Buffer,
    params_capacity: usize,
    frame_counter: u64,
}

struct PostBindGroupDesc<'a> {
    bgl: &'a wgpu::BindGroupLayout,
    input_view: &'a wgpu::TextureView,
    sampler: &'a wgpu::Sampler,
    depth_view: &'a wgpu::TextureView,
    uniform_buffer: &'a wgpu::Buffer,
    uniform_size_bytes: u64,
    params_buffer: &'a wgpu::Buffer,
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
        ],
    })
}

impl PostProcessor {
    pub fn new(
        device: &wgpu::Device,
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
            sampler,
            bgl,
            builtin_pipeline,
            custom_pipelines: HashMap::new(),
            uniform_buffer,
            uniform_stride,
            uniform_capacity,
            params_buffer,
            params_capacity,
            frame_counter: 0,
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
    }

    pub fn scene_view(&self) -> &wgpu::TextureView {
        &self.scene_view
    }

    pub fn uses_depth(effects: &[PostProcessEffect]) -> bool {
        effects
            .iter()
            .any(|e| matches!(e, PostProcessEffect::Custom { .. }))
    }

    pub fn has_effects(effects: &[PostProcessEffect]) -> bool {
        !effects.is_empty()
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
            static_shader_lookup,
        } = ctx;

        let PostProcessChainData {
            input_view,
            depth_view,
            effects,
        } = chain_data;

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
        self.ensure_params_capacity(device, max_params);
        self.ensure_uniform_capacity(device, effects.len().max(1));

        let mut current_input = *input_view;
        let mut input_kind = 0u8; // 0=external input_view, 1=ping_a, 2=ping_b
        let mut bind_group_input: Option<wgpu::BindGroup> = None;
        let mut bind_group_ping_a: Option<wgpu::BindGroup> = None;
        let mut bind_group_ping_b: Option<wgpu::BindGroup> = None;
        let mut use_ping_a = true;
        for (index, effect) in effects.iter().enumerate() {
            let last = index + 1 == effects.len();

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
                resolution: [width, height],
                inv_resolution: [inv_width, inv_height],
                near,
                far,
                time: [time, time],
            };
            let uniform_offset = index as u64 * self.uniform_stride;
            let uniform_dynamic_offset =
                u32::try_from(uniform_offset).expect("post uniform dynamic offset overflow");
            queue.write_buffer(
                &self.uniform_buffer,
                uniform_offset,
                bytemuck::bytes_of(&uniform),
            );

            let target_view = if last {
                *output_view
            } else if use_ping_a {
                &self.ping_a_view
            } else {
                &self.ping_b_view
            };
            let pipeline = match effect {
                PostProcessEffect::Custom { shader_path, .. } => self
                    .custom_pipelines
                    .get(shader_path.as_ref())
                    .unwrap_or(&self.builtin_pipeline),
                _ => &self.builtin_pipeline,
            };

            let bind_group = match input_kind {
                0 => bind_group_input.get_or_insert_with(|| {
                    create_post_bind_group(
                        device,
                        PostBindGroupDesc {
                            bgl: &self.bgl,
                            input_view: current_input,
                            sampler: &self.sampler,
                            depth_view,
                            uniform_buffer: &self.uniform_buffer,
                            uniform_size_bytes: std::mem::size_of::<PostUniform>() as u64,
                            params_buffer: &self.params_buffer,
                        },
                    )
                }),
                1 => bind_group_ping_a.get_or_insert_with(|| {
                    create_post_bind_group(
                        device,
                        PostBindGroupDesc {
                            bgl: &self.bgl,
                            input_view: &self.ping_a_view,
                            sampler: &self.sampler,
                            depth_view,
                            uniform_buffer: &self.uniform_buffer,
                            uniform_size_bytes: std::mem::size_of::<PostUniform>() as u64,
                            params_buffer: &self.params_buffer,
                        },
                    )
                }),
                _ => bind_group_ping_b.get_or_insert_with(|| {
                    create_post_bind_group(
                        device,
                        PostBindGroupDesc {
                            bgl: &self.bgl,
                            input_view: &self.ping_b_view,
                            sampler: &self.sampler,
                            depth_view,
                            uniform_buffer: &self.uniform_buffer,
                            uniform_size_bytes: std::mem::size_of::<PostUniform>() as u64,
                            params_buffer: &self.params_buffer,
                        },
                    )
                }),
            };

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_post_pass"),
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
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &*bind_group, &[uniform_dynamic_offset]);
                pass.draw(0..3, 0..1);
            }

            current_input = target_view;
            input_kind = if use_ping_a { 1 } else { 2 };
            use_ping_a = !use_ping_a;
        }
    }

    fn ensure_custom_pipeline(
        &mut self,
        device: &wgpu::Device,
        shader_path: &str,
        static_shader_lookup: Option<StaticShaderLookup>,
    ) -> Option<&wgpu::RenderPipeline> {
        if self.custom_pipelines.contains_key(shader_path) {
            return self.custom_pipelines.get(shader_path);
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_post_pipeline_layout"),
            bind_group_layouts: &[Some(&self.bgl)],
            immediate_size: 0,
        });
        let pipeline = create_pipeline(device, &pipeline_layout, &shader, self.format);
        self.custom_pipelines
            .insert(shader_path.to_string(), pipeline);
        self.custom_pipelines.get(shader_path)
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
    }
}

#[inline]
fn align_up_uniform(value: u64, alignment: u64) -> u64 {
    if alignment <= 1 {
        return value.max(1);
    }
    value.div_ceil(alignment) * alignment
}

fn create_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_post_pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_color_target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn projection_uniform_params(camera: &Camera3DState) -> (u32, f32, f32) {
    match camera.projection {
        CameraProjectionState::Perspective { near, far, .. } => (0, near, far),
        CameraProjectionState::Orthographic { near, far, .. } => (1, near, far),
        CameraProjectionState::Frustum { near, far, .. } => (2, near, far),
    }
}

// Refactored to return the new struct, simplifying the return type
fn encode_effect_params(effect: &PostProcessEffect) -> EncodedEffectParams {
    match effect {
        PostProcessEffect::Blur { strength } => EncodedEffectParams {
            effect_type: EFFECT_BLUR,
            params0: [*strength, 0.0, 0.0, 0.0],
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: Vec::new(),
        },
        PostProcessEffect::Pixelate { size } => EncodedEffectParams {
            effect_type: EFFECT_PIXELATE,
            params0: [*size, 0.0, 0.0, 0.0],
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: Vec::new(),
        },
        PostProcessEffect::Warp { waves, strength } => EncodedEffectParams {
            effect_type: EFFECT_WARP,
            params0: [*waves, *strength, 0.0, 0.0],
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: Vec::new(),
        },
        PostProcessEffect::Vignette {
            strength,
            radius,
            softness,
        } => EncodedEffectParams {
            effect_type: EFFECT_VIGNETTE,
            params0: [*strength, *radius, *softness, 0.0],
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: Vec::new(),
        },
        PostProcessEffect::Crt {
            scanline_strength,
            curvature,
            chromatic,
            vignette,
        } => EncodedEffectParams {
            effect_type: EFFECT_CRT,
            params0: [*scanline_strength, *curvature, *chromatic, *vignette],
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: Vec::new(),
        },
        PostProcessEffect::ColorFilter { color, strength } => EncodedEffectParams {
            effect_type: EFFECT_COLOR_FILTER,
            params0: [color[0], color[1], color[2], *strength],
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: Vec::new(),
        },
        PostProcessEffect::ReverseFilter {
            color,
            strength,
            softness,
        } => EncodedEffectParams {
            effect_type: EFFECT_REVERSE_FILTER,
            params0: [color[0], color[1], color[2], *strength],
            params1: [*softness, 0.0, 0.0, 0.0],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: Vec::new(),
        },
        PostProcessEffect::Bloom {
            strength,
            threshold,
            radius,
        } => EncodedEffectParams {
            effect_type: EFFECT_BLOOM,
            params0: [*strength, *threshold, *radius, 0.0],
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: Vec::new(),
        },
        PostProcessEffect::Saturate { amount } => EncodedEffectParams {
            effect_type: EFFECT_SATURATE,
            params0: [*amount, 0.0, 0.0, 0.0],
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: Vec::new(),
        },
        PostProcessEffect::BlackWhite { amount } => EncodedEffectParams {
            effect_type: EFFECT_BLACK_WHITE,
            params0: [*amount, 0.0, 0.0, 0.0],
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: Vec::new(),
        },
        PostProcessEffect::Custom { params, .. } => EncodedEffectParams {
            effect_type: EFFECT_CUSTOM,
            params0: [0.0; 4],
            params1: [0.0; 4],
            params2: [0.0; 4],
            params3: [0.0; 4],
            custom_params: params.iter().map(encode_custom_param_value).collect(),
        },
    }
}

fn encode_custom_param_value(value: &CustomPostParam) -> [f32; 4] {
    match &value.value {
        CustomPostParamValue::F32(v) => [*v, 0.0, 0.0, 0.0],
        CustomPostParamValue::I32(v) => [*v as f32, 0.0, 0.0, 0.0],
        CustomPostParamValue::Bool(v) => [if *v { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        CustomPostParamValue::Vec2(v) => [v[0], v[1], 0.0, 0.0],
        CustomPostParamValue::Vec3(v) => [v[0], v[1], v[2], 0.0],
        CustomPostParamValue::Vec4(v) => *v,
    }
}
