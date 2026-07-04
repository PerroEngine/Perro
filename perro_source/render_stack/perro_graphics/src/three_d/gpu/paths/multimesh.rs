use super::*;

const MULTIMESH_MESH_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        offset: 0,
        shader_location: 0,
        format: wgpu::VertexFormat::Float32x3,
    },
    wgpu::VertexAttribute {
        offset: 12,
        shader_location: 1,
        format: wgpu::VertexFormat::Snorm16x4,
    },
];

pub(super) fn multimesh_mesh_vertex_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RigidMeshVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &MULTIMESH_MESH_VERTEX_ATTRIBUTES,
    }
}

pub(super) fn create_multimesh_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    create_multimesh_pipeline_with_depth_write(
        device,
        pipeline_layout,
        shader,
        color_format,
        sample_count,
        cull_mode,
        true,
        wgpu::CompareFunction::LessEqual,
        "fs_main",
    )
}

// Prepass-covered variant: depth already primed, so drop depth writes and keep
// LessEqual so surviving fragments still pass. Mirrors pipeline_for_batch.
pub(super) fn create_multimesh_covered_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    create_multimesh_pipeline_with_depth_write(
        device,
        pipeline_layout,
        shader,
        color_format,
        sample_count,
        cull_mode,
        false,
        wgpu::CompareFunction::LessEqual,
        "fs_main",
    )
}

// Depth-only prepass pipeline: vertex-only (vs_depth), no color target.
pub(super) fn create_multimesh_depth_prepass_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_multimesh_depth_prepass_pipeline"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_depth"),
            buffers: &[multimesh_mesh_vertex_layout()],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_depth"),
            targets: &[],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_PREPASS_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

// Shadow-depth pipeline: same vertex-only path as the prepass but writes into a
// shadow layer (SHADOW_DEPTH_FORMAT, biased). Bind group 0 carries the shadow
// layer's camera scene uniform so vs_depth projects into the light's view.
pub(super) fn create_multimesh_shadow_depth_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_multimesh_shadow_depth_pipeline"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_depth"),
            buffers: &[multimesh_mesh_vertex_layout()],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_depth"),
            targets: &[],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: SHADOW_DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: SHADOW_MAP_DEPTH_BIAS_CONST,
                slope_scale: SHADOW_MAP_DEPTH_BIAS_SLOPE,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub(super) fn create_multimesh_blend_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    create_multimesh_pipeline_with_depth_write(
        device,
        pipeline_layout,
        shader,
        color_format,
        sample_count,
        cull_mode,
        false,
        wgpu::CompareFunction::LessEqual,
        "fs_main",
    )
}

#[allow(clippy::too_many_arguments)]
fn create_multimesh_pipeline_with_depth_write(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    cull_mode: Option<wgpu::Face>,
    depth_write_enabled: bool,
    depth_compare: wgpu::CompareFunction,
    fragment_entry: &'static str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_multimesh_pipeline"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[multimesh_mesh_vertex_layout()],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::RED
                    | wgpu::ColorWrites::GREEN
                    | wgpu::ColorWrites::BLUE,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: crate::scene_depth_format(sample_count),
            depth_write_enabled: Some(depth_write_enabled),
            depth_compare: Some(depth_compare),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

#[inline]
pub(super) fn pack_unorm4x8(v: [f32; 4]) -> u32 {
    perro_structs::UnitVector4::new(v).to_le_u32()
}
