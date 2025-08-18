#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];

    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

// A simple quad covering [-0.5, -0.5] to [0.5, 0.5] with UV coords
const QUAD_VERTICES: &[Vertex] = &[
    Vertex { position: [-0.5, -0.5], uv: [0.0, 1.0] },
    Vertex { position: [0.5, -0.5], uv: [1.0, 1.0] },
    Vertex { position: [0.5, 0.5], uv: [1.0, 0.0] },
    Vertex { position: [-0.5, -0.5], uv: [0.0, 1.0] },
    Vertex { position: [0.5, 0.5], uv: [1.0, 0.0] },
    Vertex { position: [-0.5, 0.5], uv: [0.0, 0.0] },
];
