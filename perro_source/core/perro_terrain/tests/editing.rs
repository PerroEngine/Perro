use perro_structs::Vector3;
use perro_terrain::{ChunkCoord, TerrainChunk};
use std::collections::HashMap;

#[test]
fn center_vertex_on_flat_plane_snaps_to_existing_grid_vertex() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let result = chunk
        .insert_vertex(Vector3::new(0.0, 0.0, 0.0))
        .expect("insert should succeed");

    assert!(result.removed_as_coplanar);
    assert_eq!(chunk.vertex_count(), 65 * 65);
    assert_eq!(chunk.triangle_count(), 64 * 64 * 2);
    assert!(chunk.validate(1.0e-6).is_ok());
}

#[test]
fn center_vertex_with_height_updates_existing_grid_vertex() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let result = chunk
        .insert_vertex(Vector3::new(0.0, 8.0, 0.0))
        .expect("insert should succeed");

    assert!(!result.removed_as_coplanar);
    assert_eq!(chunk.vertex_count(), 65 * 65);
    assert_eq!(chunk.triangle_count(), 64 * 64 * 2);
    assert!(
        chunk
            .vertices()
            .iter()
            .any(|v| v.position.x.abs() <= 1.0e-6
                && v.position.z.abs() <= 1.0e-6
                && (v.position.y - 8.0).abs() <= 1.0e-6)
    );
    assert!(chunk.validate(1.0e-6).is_ok());
}

#[test]
fn repeated_inserts_preserve_valid_non_manifold_free_topology() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let inserts = [
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(8.0, 0.0, 0.0),
        Vector3::new(-8.0, 0.0, 6.0),
        Vector3::new(0.0, 4.0, 0.0),
        Vector3::new(12.0, 2.0, -10.0),
        Vector3::new(-14.0, 1.5, 9.0),
    ];
    for p in inserts {
        let _ = chunk.insert_vertex(p).expect("insert should succeed");
    }

    assert!(chunk.validate(1.0e-6).is_ok());
    assert!(is_manifold(chunk.triangles()));
}

fn is_manifold(tris: &[perro_terrain::Triangle]) -> bool {
    let mut edge_counts: HashMap<(usize, usize), usize> = HashMap::new();
    for tri in tris {
        for (a, b) in [(tri.a, tri.b), (tri.b, tri.c), (tri.c, tri.a)] {
            let key = if a < b { (a, b) } else { (b, a) };
            let count = edge_counts.entry(key).or_insert(0);
            *count += 1;
            if *count > 2 {
                return false;
            }
        }
    }
    true
}
