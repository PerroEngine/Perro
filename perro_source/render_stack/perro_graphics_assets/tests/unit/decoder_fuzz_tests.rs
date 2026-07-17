//! Deterministic mutation-sweep robustness tests for the binary asset
//! decoders. Runtime-loaded content (DLC packs, user projects) reaches these
//! decoders with untrusted bytes, so every mutated input must return `None`
//! or a self-consistent `Some` — never panic, never produce out-of-contract
//! values downstream consumers would index with.

use crate::{decode_pmesh, decode_ptex};
use perro_asset_formats::pmesh::{
    FLAG_PAYLOAD_RAW as PMESH_FLAG_PAYLOAD_RAW, MAGIC as PMESH_MAGIC,
    VERSION_V2 as PMESH_VERSION_V2,
};
use perro_asset_formats::ptex::{
    FLAG_FORMAT_RGBA8, FLAG_PAYLOAD_RAW as PTEX_FLAG_PAYLOAD_RAW, MAGIC as PTEX_MAGIC,
    MAX_RAW_BYTES as PTEX_MAX_RAW_BYTES, VERSION as PTEX_VERSION,
};
use perro_io::compress_zlib_best;

/// Exhaustive truncations, exhaustive single-byte substitutions with boundary
/// values, then seeded multi-byte mutations. Deterministic so CI failures
/// reproduce locally.
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

fn ptex_bytes(payload: &[u8], flags: u32, raw_len: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(24 + payload.len());
    out.extend_from_slice(PTEX_MAGIC);
    out.extend_from_slice(&PTEX_VERSION.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&raw_len.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

fn check_ptex(bytes: &[u8]) {
    if let Some((rgba, width, height)) = decode_ptex(bytes) {
        assert!(width > 0 && height > 0);
        let pixel_bytes = (width as u64).checked_mul(height as u64).unwrap() * 4;
        assert!(pixel_bytes <= PTEX_MAX_RAW_BYTES as u64 * 4);
        assert_eq!(rgba.len() as u64, pixel_bytes);
    }
}

#[test]
fn ptex_raw_payload_survives_mutation_sweep() {
    let raw: Vec<u8> = (0u8..16).collect();
    let valid = ptex_bytes(&raw, FLAG_FORMAT_RGBA8 | PTEX_FLAG_PAYLOAD_RAW, 16);
    assert!(decode_ptex(&valid).is_some(), "baseline must decode");
    mutation_sweep(&valid, check_ptex);
}

#[test]
fn ptex_compressed_payload_survives_mutation_sweep() {
    let raw: Vec<u8> = std::iter::repeat_n(0xABu8, 16).collect();
    let compressed = compress_zlib_best(&raw).expect("compress");
    let valid = ptex_bytes(&compressed, FLAG_FORMAT_RGBA8, 16);
    assert!(decode_ptex(&valid).is_some(), "baseline must decode");
    mutation_sweep(&valid, check_ptex);
}

/// Pos-only triangle mesh: 3 vertices, 3 indices, 1 surface, 1 meshlet, 1 lod.
fn pmesh_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    for pos in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        for c in pos {
            payload.extend_from_slice(&c.to_le_bytes());
        }
    }
    for idx in [0u32, 1, 2] {
        payload.extend_from_slice(&idx.to_le_bytes());
    }
    // surface: index_start=0, index_count=3
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&3u32.to_le_bytes());
    // meshlet: index_start, index_count, center xyz, radius
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&3u32.to_le_bytes());
    for c in [0.5f32, 0.5, 0.0, 1.0] {
        payload.extend_from_slice(&c.to_le_bytes());
    }
    // lod: index 0..3, surface 0..1, meshlet 0..1
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&3u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload
}

fn pmesh_bytes(payload: &[u8], flags: u32, raw_len: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(41 + payload.len());
    out.extend_from_slice(PMESH_MAGIC);
    out.extend_from_slice(&PMESH_VERSION_V2.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&3u32.to_le_bytes()); // vertices
    out.extend_from_slice(&3u32.to_le_bytes()); // indices
    out.extend_from_slice(&1u32.to_le_bytes()); // surfaces
    out.extend_from_slice(&1u32.to_le_bytes()); // meshlets
    out.extend_from_slice(&1u32.to_le_bytes()); // lods
    out.extend_from_slice(&raw_len.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // blend shapes
    out.extend_from_slice(payload);
    out
}

// Every lod range a renderer would draw from must stay inside the decoded
// vectors, whatever the input bytes were.
fn check_pmesh(bytes: &[u8]) {
    if let Some(mesh) = decode_pmesh(bytes) {
        for lod in &mesh.lods {
            assert!(
                lod.index_start as usize + lod.index_count as usize <= mesh.indices.len(),
                "lod index range exceeds decoded indices"
            );
            assert!(
                lod.surface_start as usize + lod.surface_count as usize
                    <= mesh.surface_ranges.len(),
                "lod surface range exceeds decoded surfaces"
            );
            assert!(
                lod.meshlet_start as usize + lod.meshlet_count as usize <= mesh.meshlets.len(),
                "lod meshlet range exceeds decoded meshlets"
            );
        }
    }
}

#[test]
fn pmesh_raw_payload_survives_mutation_sweep() {
    let payload = pmesh_payload();
    let valid = pmesh_bytes(&payload, PMESH_FLAG_PAYLOAD_RAW, payload.len() as u32);
    let mesh = decode_pmesh(&valid).expect("baseline must decode");
    assert_eq!(mesh.vertices.len(), 3);
    assert_eq!(mesh.indices, vec![0, 1, 2]);
    assert_eq!(mesh.lods.len(), 1);
    mutation_sweep(&valid, check_pmesh);
}

#[test]
fn pmesh_compressed_payload_survives_mutation_sweep() {
    let payload = pmesh_payload();
    let compressed = compress_zlib_best(&payload).expect("compress");
    let valid = pmesh_bytes(&compressed, 0, payload.len() as u32);
    assert!(decode_pmesh(&valid).is_some(), "baseline must decode");
    mutation_sweep(&valid, check_pmesh);
}
