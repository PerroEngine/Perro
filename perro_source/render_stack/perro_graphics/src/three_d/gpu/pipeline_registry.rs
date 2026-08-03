//! Lazy, shared render-pipeline storage for `Gpu3D`.
//!
//! Historically every `Gpu3D::new` eagerly compiled ~64 render pipelines (all
//! material/depth/shadow/mask/sky variants), and every camera-stream subview
//! built a *second* full copy. Pipelines depend only on
//! `(device, color format, sample count, bind group layouts)`, none of which
//! are per-instance, so this module hoists them into a `PipelineRegistry`:
//!
//! * **Lazy (pay-for-what-you-use):** each pipeline family lives behind a
//!   `OnceLock` and compiles its shader module + pipelines on first draw that
//!   needs it. The first-use hitch is accepted by design (engine rule); the
//!   hot-path cost afterwards is a single atomic-load fast path.
//! * **Shared (one copy per `(format, sample_count)`):** the main `Gpu3D` and
//!   every camera-stream `Gpu3D` with matching format/samples share one
//!   `Arc<PipelineRegistry>` via [`PipelineRegistryCache`], mirroring how
//!   `SharedTextureStore` de-duplicates per-stream textures. Instances with a
//!   different sample count get their own registry keyed accordingly, but
//!   still share the bind group layouts (layout identity is what wgpu checks
//!   for bind group <-> pipeline compatibility, and BGLs are format-agnostic).
//!
//! Family grouping: pipelines are built in culled + double-sided pairs
//! (`PipelinePair`) because the draw code always selects between exactly those
//! two per state; opaque vs. blend variants of a family are *separate* cells so
//! a scene with no transparency never compiles the blend pipelines. Grouping
//! finer than a pair would double the option checks for no memory win; grouping
//! coarser (whole shader family) would compile blend/overlay variants most
//! scenes never use. Depth-prepass/shadow pipelines are also lazy: 2D/UI-only
//! games (and streams that never draw 3D geometry) then pay for zero 3D
//! pipelines, and the once-cell fast path is too cheap to justify keeping them
//! eager.
//!
//! Per-instance state intentionally NOT in the registry: custom-shader and
//! builtin-variant pipeline maps (content-dependent, LRU-evicted per instance),
//! compute pipelines (frustum/hi-z/multimesh cull - small, and their bind
//! groups are per-instance), and every buffer/texture/bind group.

use super::*;
use std::sync::{Mutex, OnceLock};

/// The culled/double-sided pipeline pair every draw family selects between.
pub struct PipelinePair {
    pub(crate) culled: wgpu::RenderPipeline,
    pub(crate) double_sided: wgpu::RenderPipeline,
}

impl PipelinePair {
    #[inline]
    pub(crate) fn select(&self, double_sided: bool) -> &wgpu::RenderPipeline {
        if double_sided {
            &self.double_sided
        } else {
            &self.culled
        }
    }
}

fn decal_buffer_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn decal_texture_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    }
}

fn decal_sampler_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

/// Bind group layouts shared by every `Gpu3D` instance and every
/// `PipelineRegistry` on one device. BGLs do not depend on color format or
/// sample count, so one set serves all registries; keeping the *same objects*
/// everywhere guarantees bind groups built by any instance are compatible with
/// pipelines built by any registry (and survive a sample-count registry swap).
pub struct SharedBindGroupLayouts3D {
    pub(crate) camera: wgpu::BindGroupLayout,
    pub(crate) water_camera: wgpu::BindGroupLayout,
    pub(crate) rigid_camera: wgpu::BindGroupLayout,
    pub(crate) multimesh: wgpu::BindGroupLayout,
    pub(crate) material_texture: wgpu::BindGroupLayout,
    pub(crate) ibl: wgpu::BindGroupLayout,
    pub(crate) shadow: wgpu::BindGroupLayout,
    pub(crate) sky: wgpu::BindGroupLayout,
    pub(crate) mesh_blend_mask_id: wgpu::BindGroupLayout,
    pub(crate) mesh_blend_seam: wgpu::BindGroupLayout,
}

impl SharedBindGroupLayouts3D {
    fn new(device: &wgpu::Device) -> Self {
        let camera = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_camera3d_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<Scene3DUniform>() as u64)
                                .expect("camera uniform size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                storage_ro_entry(1, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage_ro_entry(2, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage_ro_entry(3, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage_ro_entry(4, wgpu::ShaderStages::VERTEX),
                storage_ro_entry(5, wgpu::ShaderStages::VERTEX),
                storage_ro_entry(6, wgpu::ShaderStages::VERTEX),
                decal_buffer_layout_entry(7),
                decal_texture_layout_entry(8),
                decal_sampler_layout_entry(9),
            ],
        });
        let water_camera = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_water_camera3d_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Scene3DUniform>() as u64)
                            .expect("camera uniform size must be non-zero"),
                    ),
                },
                count: None,
            }],
        });
        let rigid_camera = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_camera3d_rigid_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<Scene3DUniform>() as u64)
                                .expect("camera uniform size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                storage_ro_entry(1, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage_ro_entry(2, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage_ro_entry(3, wgpu::ShaderStages::VERTEX),
                storage_ro_entry(4, wgpu::ShaderStages::VERTEX),
                storage_ro_entry(5, wgpu::ShaderStages::VERTEX),
                storage_ro_entry(6, wgpu::ShaderStages::VERTEX),
                decal_buffer_layout_entry(7),
                decal_texture_layout_entry(8),
                decal_sampler_layout_entry(9),
            ],
        });
        let multimesh = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_multimesh_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<Scene3DUniform>() as u64)
                                .expect("scene size"),
                        ),
                    },
                    count: None,
                },
                storage_ro_entry(1, wgpu::ShaderStages::VERTEX),
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
                storage_ro_entry(3, wgpu::ShaderStages::VERTEX),
                storage_ro_entry(4, wgpu::ShaderStages::VERTEX),
                storage_ro_entry(5, wgpu::ShaderStages::VERTEX),
                storage_ro_entry(6, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage_ro_entry(7, wgpu::ShaderStages::VERTEX_FRAGMENT),
                // 8: visible-instance indices (identity or cull-compacted).
                storage_ro_entry(8, wgpu::ShaderStages::VERTEX),
                // 9: multimesh instance payloads (fetched by the vertex shader).
                storage_ro_entry(9, wgpu::ShaderStages::VERTEX),
                decal_buffer_layout_entry(10),
                decal_texture_layout_entry(11),
                decal_sampler_layout_entry(12),
                wgpu::BindGroupLayoutEntry {
                    binding: 13,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let material_texture = {
            let mut entries = Vec::with_capacity(MATERIAL_TEXTURE_SET_SIZE + 1);
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
            for binding in 1..=(MATERIAL_TEXTURE_SET_SIZE as u32) {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                });
            }
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_material_texture_bgl"),
                entries: &entries,
            })
        };
        let shadow = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_shadow3d_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<ShadowUniform>() as u64)
                                .expect("shadow uniform size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let sky = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_sky3d_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<SkyUniform>() as u64)
                                .expect("sky uniform size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let ibl = environment::create_environment_bgl(device);
        let mesh_blend_mask_id = mesh_blend_screen::create_mesh_blend_mask_id_bgl(device);
        let mesh_blend_seam = mesh_blend_screen::create_mesh_blend_seam_bgl(device);
        Self {
            camera,
            water_camera,
            rigid_camera,
            multimesh,
            material_texture,
            ibl,
            shadow,
            sky,
            mesh_blend_mask_id,
            mesh_blend_seam,
        }
    }
}

fn storage_ro_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// Lazily-built render pipelines for one `(color format, sample count)`.
/// Shared behind an `Arc` between the main `Gpu3D` and camera-stream `Gpu3D`s.
pub struct PipelineRegistry {
    // wgpu::Device is internally refcounted; the clone keeps getters
    // self-contained so lazy builds need no device threading at draw time.
    device: wgpu::Device,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    bgls: Arc<SharedBindGroupLayouts3D>,
    // Pipeline layouts are tiny; built eagerly so lazy builders and the
    // per-instance custom/variant pipeline paths can borrow them freely.
    material_layout: wgpu::PipelineLayout,
    rigid_material_layout: wgpu::PipelineLayout,
    multimesh_layout: wgpu::PipelineLayout,
    multimesh_depth_layout: wgpu::PipelineLayout,
    depth_layout: wgpu::PipelineLayout,
    rigid_depth_layout: wgpu::PipelineLayout,
    mask_multimesh_layout: wgpu::PipelineLayout,
    mask_rigid_layout: wgpu::PipelineLayout,
    mask_skinned_layout: wgpu::PipelineLayout,
    sky_layout: wgpu::PipelineLayout,
    // Lazy pipeline cells. One cell per family the draw code selects as a
    // unit; each cell compiles its shader module on first use and drops it
    // right after (module IR is the expensive part to keep resident).
    skinned_standard: OnceLock<PipelinePair>,
    skinned_standard_blend: OnceLock<PipelinePair>,
    skinned_unlit: OnceLock<PipelinePair>,
    skinned_unlit_blend: OnceLock<PipelinePair>,
    skinned_toon: OnceLock<PipelinePair>,
    skinned_toon_blend: OnceLock<PipelinePair>,
    skinned_overlay: OnceLock<PipelinePair>,
    rigid_standard: OnceLock<PipelinePair>,
    rigid_standard_blend: OnceLock<PipelinePair>,
    rigid_unlit: OnceLock<PipelinePair>,
    rigid_unlit_blend: OnceLock<PipelinePair>,
    rigid_toon: OnceLock<PipelinePair>,
    rigid_toon_blend: OnceLock<PipelinePair>,
    rigid_overlay: OnceLock<PipelinePair>,
    rigid_packed_lod: OnceLock<PipelinePair>,
    rigid_packed_lod_blend: OnceLock<PipelinePair>,
    multimesh: OnceLock<PipelinePair>,
    multimesh_blend: OnceLock<PipelinePair>,
    multimesh_covered: OnceLock<PipelinePair>,
    multimesh_mask: OnceLock<PipelinePair>,
    multimesh_depth_prepass: OnceLock<PipelinePair>,
    multimesh_shadow_depth: OnceLock<PipelinePair>,
    depth_prepass_skinned: OnceLock<PipelinePair>,
    depth_prepass_rigid: OnceLock<PipelinePair>,
    depth_prepass_rigid_packed_lod: OnceLock<PipelinePair>,
    shadow_depth_skinned: OnceLock<PipelinePair>,
    shadow_depth_rigid: OnceLock<PipelinePair>,
    shadow_depth_rigid_packed_lod: OnceLock<PipelinePair>,
    mask_rigid: OnceLock<PipelinePair>,
    mask_rigid_packed_lod: OnceLock<PipelinePair>,
    mask_skinned: OnceLock<PipelinePair>,
    sky: OnceLock<wgpu::RenderPipeline>,
    mesh_blend_seam: OnceLock<wgpu::RenderPipeline>,
}

impl PipelineRegistry {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        bgls: Arc<SharedBindGroupLayouts3D>,
    ) -> Self {
        let material_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_mesh_pipeline_layout"),
            bind_group_layouts: &[
                Some(&bgls.camera),
                Some(&bgls.material_texture),
                Some(&bgls.shadow),
                Some(&bgls.ibl),
            ],
            immediate_size: 0,
        });
        let rigid_material_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_mesh_pipeline_layout_rigid"),
                bind_group_layouts: &[
                    Some(&bgls.rigid_camera),
                    Some(&bgls.material_texture),
                    Some(&bgls.shadow),
                    Some(&bgls.ibl),
                ],
                immediate_size: 0,
            });
        let multimesh_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_multimesh_pipeline_layout"),
            bind_group_layouts: &[
                Some(&bgls.multimesh),
                Some(&bgls.material_texture),
                None,
                Some(&bgls.ibl),
            ],
            immediate_size: 0,
        });
        let multimesh_depth_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_multimesh_depth_pipeline_layout"),
                bind_group_layouts: &[Some(&bgls.multimesh)],
                immediate_size: 0,
            });
        let depth_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_depth_pipeline_layout"),
            bind_group_layouts: &[Some(&bgls.camera)],
            immediate_size: 0,
        });
        let rigid_depth_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_depth_pipeline_layout_rigid"),
            bind_group_layouts: &[Some(&bgls.rigid_camera)],
            immediate_size: 0,
        });
        let mask_multimesh_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_mesh_blend_mask_layout_multimesh"),
                bind_group_layouts: &[
                    Some(&bgls.multimesh),
                    Some(&bgls.material_texture),
                    Some(&bgls.mesh_blend_mask_id),
                ],
                immediate_size: 0,
            });
        let mask_rigid_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_mesh_blend_mask_layout_rigid"),
            bind_group_layouts: &[Some(&bgls.rigid_camera), Some(&bgls.mesh_blend_mask_id)],
            immediate_size: 0,
        });
        let mask_skinned_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_mesh_blend_mask_layout_skinned"),
            bind_group_layouts: &[Some(&bgls.camera), Some(&bgls.mesh_blend_mask_id)],
            immediate_size: 0,
        });
        let sky_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_sky3d_pipeline_layout"),
            bind_group_layouts: &[Some(&bgls.sky)],
            immediate_size: 0,
        });
        Self {
            device: device.clone(),
            color_format,
            sample_count,
            bgls,
            material_layout,
            rigid_material_layout,
            multimesh_layout,
            multimesh_depth_layout,
            depth_layout,
            rigid_depth_layout,
            mask_multimesh_layout,
            mask_rigid_layout,
            mask_skinned_layout,
            sky_layout,
            skinned_standard: OnceLock::new(),
            skinned_standard_blend: OnceLock::new(),
            skinned_unlit: OnceLock::new(),
            skinned_unlit_blend: OnceLock::new(),
            skinned_toon: OnceLock::new(),
            skinned_toon_blend: OnceLock::new(),
            skinned_overlay: OnceLock::new(),
            rigid_standard: OnceLock::new(),
            rigid_standard_blend: OnceLock::new(),
            rigid_unlit: OnceLock::new(),
            rigid_unlit_blend: OnceLock::new(),
            rigid_toon: OnceLock::new(),
            rigid_toon_blend: OnceLock::new(),
            rigid_overlay: OnceLock::new(),
            rigid_packed_lod: OnceLock::new(),
            rigid_packed_lod_blend: OnceLock::new(),
            multimesh: OnceLock::new(),
            multimesh_blend: OnceLock::new(),
            multimesh_covered: OnceLock::new(),
            multimesh_mask: OnceLock::new(),
            multimesh_depth_prepass: OnceLock::new(),
            multimesh_shadow_depth: OnceLock::new(),
            depth_prepass_skinned: OnceLock::new(),
            depth_prepass_rigid: OnceLock::new(),
            depth_prepass_rigid_packed_lod: OnceLock::new(),
            shadow_depth_skinned: OnceLock::new(),
            shadow_depth_rigid: OnceLock::new(),
            shadow_depth_rigid_packed_lod: OnceLock::new(),
            mask_rigid: OnceLock::new(),
            mask_rigid_packed_lod: OnceLock::new(),
            mask_skinned: OnceLock::new(),
            sky: OnceLock::new(),
            mesh_blend_seam: OnceLock::new(),
        }
    }

    #[inline]
    pub(crate) fn color_format(&self) -> wgpu::TextureFormat {
        self.color_format
    }

    #[inline]
    pub(crate) fn sample_count(&self) -> u32 {
        self.sample_count
    }

    pub(crate) fn bgls(&self) -> &SharedBindGroupLayouts3D {
        &self.bgls
    }

    pub(crate) fn material_layout(&self) -> &wgpu::PipelineLayout {
        &self.material_layout
    }

    pub(crate) fn rigid_material_layout(&self) -> &wgpu::PipelineLayout {
        &self.rigid_material_layout
    }

    pub(crate) fn multimesh_layout(&self) -> &wgpu::PipelineLayout {
        &self.multimesh_layout
    }

    pub(crate) fn sky_layout(&self) -> &wgpu::PipelineLayout {
        &self.sky_layout
    }

    fn pair(&self, build: impl Fn(Option<wgpu::Face>) -> wgpu::RenderPipeline) -> PipelinePair {
        PipelinePair {
            culled: build(Some(wgpu::Face::Back)),
            double_sided: build(None),
        }
    }

    pub(crate) fn skinned_standard(&self) -> &PipelinePair {
        self.skinned_standard.get_or_init(|| {
            let shader = create_mesh_shader_module_skinned(&self.device);
            self.pair(|cull| {
                create_pipeline_skinned(
                    &self.device,
                    &self.material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn skinned_standard_blend(&self) -> &PipelinePair {
        self.skinned_standard_blend.get_or_init(|| {
            let shader = create_mesh_shader_module_skinned(&self.device);
            self.pair(|cull| {
                create_pipeline_skinned_blend(
                    &self.device,
                    &self.material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn skinned_unlit(&self) -> &PipelinePair {
        self.skinned_unlit.get_or_init(|| {
            let shader = create_unlit_shader_module_skinned(&self.device);
            self.pair(|cull| {
                create_pipeline_skinned(
                    &self.device,
                    &self.material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn skinned_unlit_blend(&self) -> &PipelinePair {
        self.skinned_unlit_blend.get_or_init(|| {
            let shader = create_unlit_shader_module_skinned(&self.device);
            self.pair(|cull| {
                create_pipeline_skinned_blend(
                    &self.device,
                    &self.material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn skinned_toon(&self) -> &PipelinePair {
        self.skinned_toon.get_or_init(|| {
            let shader = create_toon_shader_module_skinned(&self.device);
            self.pair(|cull| {
                create_pipeline_skinned(
                    &self.device,
                    &self.material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn skinned_toon_blend(&self) -> &PipelinePair {
        self.skinned_toon_blend.get_or_init(|| {
            let shader = create_toon_shader_module_skinned(&self.device);
            self.pair(|cull| {
                create_pipeline_skinned_blend(
                    &self.device,
                    &self.material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn skinned_overlay(&self) -> &PipelinePair {
        self.skinned_overlay.get_or_init(|| {
            let shader = create_mesh_shader_module_skinned(&self.device);
            self.pair(|cull| {
                create_pipeline_overlay_skinned(
                    &self.device,
                    &self.material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn rigid_standard(&self) -> &PipelinePair {
        self.rigid_standard.get_or_init(|| {
            let shader = create_mesh_shader_module_rigid(&self.device);
            self.pair(|cull| {
                create_pipeline_rigid(
                    &self.device,
                    &self.rigid_material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn rigid_standard_blend(&self) -> &PipelinePair {
        self.rigid_standard_blend.get_or_init(|| {
            let shader = create_mesh_shader_module_rigid(&self.device);
            self.pair(|cull| {
                create_pipeline_rigid_blend(
                    &self.device,
                    &self.rigid_material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn rigid_unlit(&self) -> &PipelinePair {
        self.rigid_unlit.get_or_init(|| {
            let shader = create_unlit_shader_module_rigid(&self.device);
            self.pair(|cull| {
                create_pipeline_rigid(
                    &self.device,
                    &self.rigid_material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn rigid_unlit_blend(&self) -> &PipelinePair {
        self.rigid_unlit_blend.get_or_init(|| {
            let shader = create_unlit_shader_module_rigid(&self.device);
            self.pair(|cull| {
                create_pipeline_rigid_blend(
                    &self.device,
                    &self.rigid_material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn rigid_toon(&self) -> &PipelinePair {
        self.rigid_toon.get_or_init(|| {
            let shader = create_toon_shader_module_rigid(&self.device);
            self.pair(|cull| {
                create_pipeline_rigid(
                    &self.device,
                    &self.rigid_material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn rigid_toon_blend(&self) -> &PipelinePair {
        self.rigid_toon_blend.get_or_init(|| {
            let shader = create_toon_shader_module_rigid(&self.device);
            self.pair(|cull| {
                create_pipeline_rigid_blend(
                    &self.device,
                    &self.rigid_material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn rigid_overlay(&self) -> &PipelinePair {
        self.rigid_overlay.get_or_init(|| {
            let shader = create_mesh_shader_module_rigid(&self.device);
            self.pair(|cull| {
                create_pipeline_overlay_rigid(
                    &self.device,
                    &self.rigid_material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn rigid_packed_lod(&self) -> &PipelinePair {
        self.rigid_packed_lod.get_or_init(|| {
            let shader = create_mesh_shader_module_rigid_packed_lod(&self.device);
            self.pair(|cull| {
                create_pipeline_rigid_packed_lod(
                    &self.device,
                    &self.rigid_material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn rigid_packed_lod_blend(&self) -> &PipelinePair {
        self.rigid_packed_lod_blend.get_or_init(|| {
            let shader = create_mesh_shader_module_rigid_packed_lod(&self.device);
            self.pair(|cull| {
                create_pipeline_rigid_packed_lod_blend(
                    &self.device,
                    &self.rigid_material_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn multimesh(&self) -> &PipelinePair {
        self.multimesh.get_or_init(|| {
            let shader = create_multimesh_shader_module(&self.device);
            self.pair(|cull| {
                create_multimesh_pipeline(
                    &self.device,
                    &self.multimesh_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn multimesh_blend(&self) -> &PipelinePair {
        self.multimesh_blend.get_or_init(|| {
            let shader = create_multimesh_shader_module(&self.device);
            self.pair(|cull| {
                create_multimesh_blend_pipeline(
                    &self.device,
                    &self.multimesh_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn multimesh_covered(&self) -> &PipelinePair {
        self.multimesh_covered.get_or_init(|| {
            let shader = create_multimesh_shader_module(&self.device);
            self.pair(|cull| {
                create_multimesh_covered_pipeline(
                    &self.device,
                    &self.multimesh_layout,
                    &shader,
                    self.color_format,
                    self.sample_count,
                    cull,
                )
            })
        })
    }

    pub(crate) fn multimesh_mask(&self) -> &PipelinePair {
        self.multimesh_mask.get_or_init(|| {
            let shader = create_multimesh_shader_module(&self.device);
            self.pair(|cull| {
                create_multimesh_mask_pipeline(
                    &self.device,
                    &self.mask_multimesh_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn multimesh_depth_prepass(&self) -> &PipelinePair {
        self.multimesh_depth_prepass.get_or_init(|| {
            let shader = create_multimesh_shader_module(&self.device);
            self.pair(|cull| {
                create_multimesh_depth_prepass_pipeline(
                    &self.device,
                    &self.multimesh_depth_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn multimesh_shadow_depth(&self) -> &PipelinePair {
        self.multimesh_shadow_depth.get_or_init(|| {
            let shader = create_multimesh_shader_module(&self.device);
            self.pair(|cull| {
                create_multimesh_shadow_depth_pipeline(
                    &self.device,
                    &self.multimesh_depth_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn depth_prepass_skinned(&self) -> &PipelinePair {
        self.depth_prepass_skinned.get_or_init(|| {
            let shader = create_depth_prepass_shader_module_skinned(&self.device);
            self.pair(|cull| {
                create_depth_prepass_pipeline_skinned(
                    &self.device,
                    &self.depth_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn depth_prepass_rigid(&self) -> &PipelinePair {
        self.depth_prepass_rigid.get_or_init(|| {
            let shader = create_depth_prepass_shader_module_rigid(&self.device);
            self.pair(|cull| {
                create_depth_prepass_pipeline_rigid(
                    &self.device,
                    &self.rigid_depth_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn depth_prepass_rigid_packed_lod(&self) -> &PipelinePair {
        self.depth_prepass_rigid_packed_lod.get_or_init(|| {
            let shader = create_depth_prepass_shader_module_rigid_packed_lod(&self.device);
            self.pair(|cull| {
                create_depth_prepass_pipeline_rigid_packed_lod(
                    &self.device,
                    &self.rigid_depth_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn shadow_depth_skinned(&self) -> &PipelinePair {
        self.shadow_depth_skinned.get_or_init(|| {
            let shader = create_depth_prepass_shader_module_skinned(&self.device);
            self.pair(|cull| {
                create_shadow_depth_pipeline_skinned(
                    &self.device,
                    &self.depth_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn shadow_depth_rigid(&self) -> &PipelinePair {
        self.shadow_depth_rigid.get_or_init(|| {
            let shader = create_depth_prepass_shader_module_rigid(&self.device);
            self.pair(|cull| {
                create_shadow_depth_pipeline_rigid(
                    &self.device,
                    &self.rigid_depth_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn shadow_depth_rigid_packed_lod(&self) -> &PipelinePair {
        self.shadow_depth_rigid_packed_lod.get_or_init(|| {
            let shader = create_depth_prepass_shader_module_rigid_packed_lod(&self.device);
            self.pair(|cull| {
                create_shadow_depth_pipeline_rigid_packed_lod(
                    &self.device,
                    &self.rigid_depth_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn mask_rigid(&self) -> &PipelinePair {
        self.mask_rigid.get_or_init(|| {
            let shader = create_mesh_blend_mask_shader_module_rigid(&self.device);
            self.pair(|cull| {
                mesh_blend_screen::create_mesh_blend_mask_pipeline_rigid(
                    &self.device,
                    &self.mask_rigid_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn mask_rigid_packed_lod(&self) -> &PipelinePair {
        self.mask_rigid_packed_lod.get_or_init(|| {
            let shader = create_mesh_blend_mask_shader_module_rigid_packed_lod(&self.device);
            self.pair(|cull| {
                mesh_blend_screen::create_mesh_blend_mask_pipeline_rigid_packed_lod(
                    &self.device,
                    &self.mask_rigid_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn mask_skinned(&self) -> &PipelinePair {
        self.mask_skinned.get_or_init(|| {
            let shader = create_mesh_blend_mask_shader_module_skinned(&self.device);
            self.pair(|cull| {
                mesh_blend_screen::create_mesh_blend_mask_pipeline_skinned(
                    &self.device,
                    &self.mask_skinned_layout,
                    &shader,
                    cull,
                )
            })
        })
    }

    pub(crate) fn sky(&self) -> &wgpu::RenderPipeline {
        self.sky.get_or_init(|| {
            let shader = create_sky_shader_module(&self.device);
            create_sky_pipeline(
                &self.device,
                &self.sky_layout,
                &shader,
                self.color_format,
                self.sample_count,
            )
        })
    }

    pub(crate) fn mesh_blend_seam(&self) -> &wgpu::RenderPipeline {
        self.mesh_blend_seam.get_or_init(|| {
            mesh_blend_screen::create_mesh_blend_seam_pipeline(
                &self.device,
                &self.bgls.mesh_blend_seam,
                self.color_format,
            )
        })
    }

    /// Compile the next not-yet-built *base* family; ret false when all base
    /// families exist. Base = what nearly every 3D scene hits in its first
    /// frames: rigid mesh + its depth/shadow passes, the multimesh trio, sky.
    /// Driven by the budgeted warm drain so these land under the startup
    /// splash instead of the first visible draw; blend/skinned/mask variants
    /// stay on the lazy first-use path by design (see module doc).
    pub(crate) fn warm_next_base_family(&self) -> bool {
        if self.rigid_standard.get().is_none() {
            self.rigid_standard();
            return true;
        }
        if self.depth_prepass_rigid.get().is_none() {
            self.depth_prepass_rigid();
            return true;
        }
        if self.shadow_depth_rigid.get().is_none() {
            self.shadow_depth_rigid();
            return true;
        }
        if self.sky.get().is_none() {
            self.sky();
            return true;
        }
        if self.multimesh.get().is_none() {
            self.multimesh();
            return true;
        }
        if self.multimesh_depth_prepass.get().is_none() {
            self.multimesh_depth_prepass();
            return true;
        }
        if self.multimesh_shadow_depth.get().is_none() {
            self.multimesh_shadow_depth();
            return true;
        }
        false
    }
}

/// One registry per `(color format, sample count)` on a device, all sharing
/// one [`SharedBindGroupLayouts3D`]. Owned by the backend `Gpu`; handed to the
/// main `Gpu3D` and every camera-stream `Gpu3D` so matching instances share
/// pipelines instead of duplicating the whole set.
type RegistryKey = (wgpu::TextureFormat, u32);

#[derive(Default)]
pub struct PipelineRegistryCache {
    bgls: OnceLock<Arc<SharedBindGroupLayouts3D>>,
    // Tiny keyspace (1-2 entries in practice); linear scan beats a map.
    registries: Mutex<Vec<(RegistryKey, Arc<PipelineRegistry>)>>,
}

impl PipelineRegistryCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get_or_create(
        &self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Arc<PipelineRegistry> {
        let sample_count = sample_count.max(1);
        let bgls = self
            .bgls
            .get_or_init(|| Arc::new(SharedBindGroupLayouts3D::new(device)));
        let mut registries = self
            .registries
            .lock()
            .expect("pipeline registry cache poisoned");
        // Drop registries no Gpu3D references anymore (sample-count changes
        // leave the old (format, samples) entry behind otherwise).
        registries.retain(|(_, registry)| Arc::strong_count(registry) > 1);
        if let Some((_, registry)) = registries
            .iter()
            .find(|(key, _)| *key == (color_format, sample_count))
        {
            return registry.clone();
        }
        let registry = Arc::new(PipelineRegistry::new(
            device,
            color_format,
            sample_count,
            bgls.clone(),
        ));
        registries.push(((color_format, sample_count), registry.clone()));
        registry
    }
}
