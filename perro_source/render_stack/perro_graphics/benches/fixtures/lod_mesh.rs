pub const LOD_MESH_SOURCE: &str = "__bench_lod_mesh__";

/// A 3-LOD mesh, hand-encoded as a raw-payload `pmesh` (nothing in-process
/// bakes LOD variants: `load_mesh_from_source` only ever reads them off an
/// asset). Positions are the 6 axis points of a unit octahedron, so the bbox
/// center is the origin and `bounds_radius` is exactly 1.0 -- which puts the
/// two live band edges (`LOD_DISTANCE_RADIUS_SCALES[0..2]`) at 36 and 54 world
/// units from the camera.
fn lod_test_mesh_bytes() -> &'static [u8] {
    use perro_asset_formats::pmesh;
    static BYTES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    BYTES
        .get_or_init(|| {
            const VERTS: [[f32; 3]; 6] = [
                [1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, -1.0],
            ];
            const FACES: [[u32; 3]; 8] = [
                [0, 2, 4],
                [2, 1, 4],
                [1, 3, 4],
                [3, 0, 4],
                [2, 0, 5],
                [1, 2, 5],
                [3, 1, 5],
                [0, 3, 5],
            ];
            // Triangle count per LOD; each LOD owns its own index block.
            const LOD_FACES: [usize; 3] = [8, 4, 2];

            let mut payload = Vec::new();
            for vertex in VERTS {
                for axis in vertex {
                    payload.extend_from_slice(&axis.to_le_bytes());
                }
            }
            let mut starts = [0u32; LOD_FACES.len()];
            let mut counts = [0u32; LOD_FACES.len()];
            let mut cursor = 0u32;
            for (lod, faces) in LOD_FACES.iter().copied().enumerate() {
                starts[lod] = cursor;
                counts[lod] = faces as u32 * 3;
                cursor += counts[lod];
                for face in FACES.iter().take(faces) {
                    for index in face {
                        payload.extend_from_slice(&index.to_le_bytes());
                    }
                }
            }
            // One surface per LOD...
            for (start, count) in starts.iter().zip(counts.iter()) {
                payload.extend_from_slice(&start.to_le_bytes());
                payload.extend_from_slice(&count.to_le_bytes());
            }
            // ...and the LOD table pointing at it (no meshlets).
            for (lod, (start, count)) in starts.iter().zip(counts.iter()).enumerate() {
                for word in [*start, *count, lod as u32, 1, 0, 0] {
                    payload.extend_from_slice(&word.to_le_bytes());
                }
            }

            let mut out = Vec::with_capacity(41 + payload.len());
            out.extend_from_slice(pmesh::MAGIC);
            out.extend_from_slice(&pmesh::VERSION_V2.to_le_bytes());
            out.extend_from_slice(&pmesh::FLAG_PAYLOAD_RAW.to_le_bytes());
            out.extend_from_slice(&(VERTS.len() as u32).to_le_bytes());
            out.extend_from_slice(&cursor.to_le_bytes());
            out.extend_from_slice(&(LOD_FACES.len() as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes()); // meshlets
            out.extend_from_slice(&(LOD_FACES.len() as u32).to_le_bytes());
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes()); // blend shapes
            out.extend_from_slice(&payload);
            out
        })
        .as_slice()
}

pub fn lod_test_mesh_lookup(path_hash: u64) -> &'static [u8] {
    if path_hash == perro_ids::string_to_u64(LOD_MESH_SOURCE) {
        lod_test_mesh_bytes()
    } else {
        &[]
    }
}
