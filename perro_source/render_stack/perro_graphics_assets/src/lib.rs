mod mesh;
mod texture;

#[cfg(test)]
#[path = "../tests/unit/decoder_fuzz_tests.rs"]
mod decoder_fuzz_tests;

pub use mesh::{
    BorrowedBlendShape, BorrowedMesh, DecodedLod, DecodedMesh, DecodedMeshlet, MeshBlendShape,
    MeshBlendShapeVertex, MeshRange, MeshVertex, StaticMeshBytesLookup, borrow_runtime_mesh,
    decode_gltf_mesh, decode_pmesh, load_mesh_from_source, load_mesh_from_source_no_dynamic_lods,
    load_mesh3d_from_bytes, load_mesh3d_from_source, validate_mesh_source,
};
pub use texture::{
    SVG_RASTER_SCALE, clear_svg_caches, decode_gltf_texture, decode_image_logical_size,
    decode_image_rgba, decode_image_rgba_arc, decode_image_rgba_max_size, decode_image_size,
    decode_ptex, encode_rgba_image, gltf_texture_source_from_mesh_source, load_texture_rgba,
    load_texture_rgba_arc, save_rgba_image,
};
