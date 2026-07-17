//! Deterministic mutation-sweep robustness test for the collision trimesh
//! decoder. Collision sources can come from runtime-loaded packs, so mutated
//! bytes must decode to `None` or a trimesh whose triangle indices all point
//! at real vertices (the physics engine indexes them unchecked downstream).

use super::*;

fn mutation_sweep(valid: &[u8], mut check: impl FnMut(&[u8])) {
    for len in 0..valid.len() {
        check(&valid[..len]);
    }
    let mut buf = valid.to_vec();
    for i in 0..buf.len() {
        let orig = buf[i];
        for v in [0x00, 0x01, 0x7F, 0x80, 0xFF] {
            buf[i] = v;
            check(&buf);
        }
        buf[i] = orig;
    }
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as u32
    };
    for _ in 0..2000 {
        let mut mutated = valid.to_vec();
        for _ in 0..(1 + (next() % 8) as usize) {
            let idx = next() as usize % mutated.len();
            mutated[idx] = (next() & 0xFF) as u8;
        }
        check(&mutated);
    }
}

/// Legacy V1 pos+index trimesh: 3 vertices, 1 triangle, raw payload.
fn valid_trimesh_bytes() -> Vec<u8> {
    let mut payload = Vec::new();
    for pos in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        for c in pos {
            payload.extend_from_slice(&c.to_le_bytes());
        }
    }
    for idx in [0u32, 1, 2] {
        payload.extend_from_slice(&idx.to_le_bytes());
    }
    let mut out = Vec::with_capacity(33 + payload.len());
    out.extend_from_slice(b"PMESH");
    out.extend_from_slice(&PMESH_VERSION.to_le_bytes());
    out.extend_from_slice(&PMESH_FLAG_PAYLOAD_RAW.to_le_bytes());
    out.extend_from_slice(&3u32.to_le_bytes()); // vertices
    out.extend_from_slice(&3u32.to_le_bytes()); // indices
    out.extend_from_slice(&[0u8; 8]); // unused header words
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

fn check_trimesh(bytes: &[u8]) {
    if let Some((vertices, triangles)) = decode_pmesh_trimesh(bytes, 1.0, 1.0, 1.0) {
        assert!(vertices.len() >= 3);
        assert!(!triangles.is_empty());
        for tri in &triangles {
            for &idx in tri.iter() {
                assert!(
                    (idx as usize) < vertices.len(),
                    "triangle index out of vertex bounds"
                );
            }
        }
    }
}

#[test]
fn trimesh_decoder_survives_mutation_sweep() {
    let valid = valid_trimesh_bytes();
    let (vertices, triangles) =
        decode_pmesh_trimesh(&valid, 1.0, 1.0, 1.0).expect("baseline must decode");
    assert_eq!(vertices.len(), 3);
    assert_eq!(triangles.len(), 1);
    mutation_sweep(&valid, check_trimesh);
}
