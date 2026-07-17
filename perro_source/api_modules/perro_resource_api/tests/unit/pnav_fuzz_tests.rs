//! Deterministic mutation-sweep robustness test for the `.pnav` navmesh
//! parser. Navmeshes load from runtime packs; mutated bytes (including
//! invalid UTF-8 and mangled numeric fields) must parse to `Err`, never panic.

use super::{parse_pnav_bytes, parse_pnav_resource_bytes};

const VALID_PNAV: &[u8] = b"pnav 1\nv 0 0 0\nv 1 0 0\nv 0 0 1\nv 4 0 0\nv 5 0 0\nv 4 0 1\ntri 0 1 2 area=2\ntri 3 4 5 area=3\nlink 0.2 0 0.2 4.2 0 0.2 cost=1.5\n";

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

#[test]
fn pnav_parser_survives_mutation_sweep() {
    assert!(parse_pnav_bytes(VALID_PNAV).is_ok(), "baseline must parse");
    assert!(
        parse_pnav_resource_bytes(VALID_PNAV).is_ok(),
        "baseline must parse as resource"
    );
    mutation_sweep(VALID_PNAV, |bytes| {
        let _ = parse_pnav_bytes(bytes);
        let _ = parse_pnav_resource_bytes(bytes);
    });
}
