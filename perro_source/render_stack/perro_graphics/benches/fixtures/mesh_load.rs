use std::sync::{
    OnceLock,
    atomic::{AtomicUsize, Ordering},
};

pub const SOURCES: [(usize, &str); 2] = [
    (1_000, "res://bench/load_1000.pmesh"),
    (10_000, "res://bench/load_10000.pmesh"),
];
pub static LOOKUPS: AtomicUsize = AtomicUsize::new(0);

pub fn mesh_lookup(hash: u64) -> &'static [u8] {
    static BYTES: OnceLock<[Vec<u8>; 2]> = OnceLock::new();
    let meshes = BYTES.get_or_init(|| SOURCES.map(|(triangles, _)| mesh_bytes(triangles)));
    for (index, (_, source)) in SOURCES.iter().enumerate() {
        if hash == perro_ids::string_to_u64(source) {
            LOOKUPS.fetch_add(1, Ordering::Relaxed);
            return &meshes[index];
        }
    }
    &[]
}

fn mesh_bytes(triangles: usize) -> Vec<u8> {
    use perro_asset_formats::pmesh;
    let vertices = (triangles * 3) as u32;
    let mut raw = Vec::with_capacity(vertices as usize * 16 + 8);
    for index in 0..vertices {
        for coordinate in [
            (index / 3) as f32,
            (index % 3 == 1) as u8 as f32,
            (index % 3 == 2) as u8 as f32,
        ] {
            raw.extend_from_slice(&coordinate.to_le_bytes());
        }
    }
    for index in 0..vertices {
        raw.extend_from_slice(&index.to_le_bytes());
    }
    raw.extend_from_slice(&0u32.to_le_bytes());
    raw.extend_from_slice(&vertices.to_le_bytes());
    let mut bytes = pmesh::MAGIC.to_vec();
    for word in [
        pmesh::VERSION_V2,
        0,
        vertices,
        vertices,
        1,
        0,
        0,
        raw.len() as u32,
        0,
    ] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend(perro_io::compress_zlib_best(&raw).expect("compress mesh fixture"));
    bytes
}
