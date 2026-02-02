//! Static mesh data: Zstd-compressed vertex + index bytes (.pmesh); decompress and upload at runtime.
//! One entry per mesh; keys are model path (single mesh) or model.glb:index (multiple). Release uses embedded .pmesh, dev loads from disk (GLTF/GLB).

use wgpu::{Device, Queue};

use crate::rendering::renderer_3d::{Mesh, Vertex3D};

/// Static mesh data: Zstd-compressed vertex + index bytes (.pmesh); decompress and upload at runtime.
#[derive(Debug, Clone)]
pub struct StaticMeshData {
    pub vertex_count: u32,
    pub index_count: u32,
    pub bounds_center: [f32; 3],
    pub bounds_radius: f32,
    /// Zstd-compressed: vertex bytes (Vertex3D[]) then index bytes (u32[]).
    pub mesh_bytes: &'static [u8],
}

impl StaticMeshData {
    /// Decompress .pmesh and upload to GPU; returns a Mesh ready for MeshManager.
    pub fn to_mesh(&self, device: &Device, _queue: &Queue) -> Mesh {
        use bytemuck::cast_slice;
        use wgpu::util::DeviceExt;

        let decompressed =
            zstd::stream::decode_all(self.mesh_bytes).expect("static mesh Zstd decompress");
        let vertex_byte_len = (self.vertex_count as usize) * std::mem::size_of::<Vertex3D>();
        let (vertex_slice, index_slice) = decompressed.split_at(vertex_byte_len);
        let vertices: &[Vertex3D] = cast_slice(vertex_slice);
        let indices: &[u32] = cast_slice(index_slice);

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("StaticMesh VB"),
            contents: cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("StaticMesh IB"),
            contents: cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Mesh {
            vertex_buffer: vb,
            index_buffer: Some(ib),
            index_count: self.index_count,
            vertex_count: self.vertex_count,
            bounds_center: glam::Vec3::from_array(self.bounds_center),
            bounds_radius: self.bounds_radius,
        }
    }
}
