//! Procedural generators for the two SMAA lookup textures.
//!
//! SMAA's blending-weight pass needs two precomputed tables: AreaTex
//! (160x560 RG8, area coverage per edge pattern/distance pair) and SearchTex
//! (64x16 R8, extra steps for the last search iteration). The reference
//! implementation ships them as binary blobs; we generate them at first use
//! with a Rust port of the reference generators (`Scripts/AreaTex.py` and
//! `Scripts/SearchTex.py` from the SMAA distribution) and cache the bytes in
//! a `OnceLock`, so no binary assets are vendored.
//!
//! Coverage: FULL ortho + diagonal patterns at subsample offset 0 — that is
//! everything SMAA 1x reads. The remaining subsample blocks of AreaTex (6
//! extra ortho rows, 4 extra diag rows, used only by SMAA T2x/S2x) stay
//! zero; generating them would only matter if a temporal/spatial multi-
//! sample SMAA mode is added later.

use std::sync::OnceLock;

pub(crate) const AREA_TEX_WIDTH: u32 = 160;
pub(crate) const AREA_TEX_HEIGHT: u32 = 560;
pub(crate) const SEARCH_TEX_WIDTH: u32 = 64;
pub(crate) const SEARCH_TEX_HEIGHT: u32 = 16;

/// AreaTex bytes, tightly packed RG8 rows (2 bytes per texel).
pub(crate) fn area_tex_bytes() -> &'static [u8] {
    static CACHE: OnceLock<Vec<u8>> = OnceLock::new();
    CACHE.get_or_init(generate_area_tex)
}

/// SearchTex bytes, tightly packed R8 rows (1 byte per texel).
pub(crate) fn search_tex_bytes() -> &'static [u8] {
    static CACHE: OnceLock<Vec<u8>> = OnceLock::new();
    CACHE.get_or_init(generate_search_tex)
}

// ---------------------------------------------------------------------------
// AreaTex
// ---------------------------------------------------------------------------

/// Ortho subtexture side: distances 0..16 per axis (compressed, see below).
const SIZE_ORTHO: usize = 16;
/// Diag subtexture side: distances 0..20 per axis (stored linearly).
const SIZE_DIAG: usize = 20;
/// Supersampling grid used to integrate diagonal coverage.
const SAMPLES_DIAG: usize = 30;
/// Maximum distance for smoothing u-shapes (reference SMOOTH_MAX_DISTANCE).
const SMOOTH_MAX_DISTANCE: f32 = 32.0;

/// Subtexture slot per ortho pattern; slots encode the shader-side
/// `round(4 * edge_value)` for edge values 0, 0.25, 0.75, 1.0 -> 0, 1, 3, 4.
const EDGES_ORTHO: [(usize, usize); 16] = [
    (0, 0),
    (3, 0),
    (0, 3),
    (3, 3),
    (1, 0),
    (4, 0),
    (1, 3),
    (4, 3),
    (0, 1),
    (3, 1),
    (0, 4),
    (3, 4),
    (1, 1),
    (4, 1),
    (1, 4),
    (4, 4),
];

/// Subtexture slot per diag pattern (edge values 0..3 map 1:1).
const EDGES_DIAG: [(usize, usize); 16] = [
    (0, 0),
    (1, 0),
    (0, 2),
    (1, 2),
    (2, 0),
    (3, 0),
    (2, 2),
    (3, 2),
    (0, 1),
    (1, 1),
    (0, 3),
    (1, 3),
    (2, 1),
    (3, 1),
    (2, 3),
    (3, 3),
];

fn quantize(v: f32) -> u8 {
    (255.0 * v.clamp(0.0, 1.0)).round() as u8
}

fn generate_area_tex() -> Vec<u8> {
    let w = AREA_TEX_WIDTH as usize;
    let mut tex = vec![0u8; w * AREA_TEX_HEIGHT as usize * 2];

    // Ortho half (left 80 columns), subsample block 0 (rows 0..80).
    for (pattern, &(e1, e2)) in EDGES_ORTHO.iter().enumerate() {
        for left in 0..SIZE_ORTHO {
            for right in 0..SIZE_ORTHO {
                // The 16x16 subtexture compresses distances quadratically:
                // texel (left, right) stores the area at pixel distances
                // (left^2, right^2); the shader indexes with sqrt(distance)
                // and bilinear filtering interpolates in between.
                let (a0, a1) = area_ortho(pattern, (left * left) as f32, (right * right) as f32);
                let x = e1 * SIZE_ORTHO + left;
                let y = e2 * SIZE_ORTHO + right;
                let i = (y * w + x) * 2;
                tex[i] = quantize(a0);
                tex[i + 1] = quantize(a1);
            }
        }
    }

    // Diag half (columns 80..160), subsample block 0 (rows 0..80).
    for (pattern, &(e1, e2)) in EDGES_DIAG.iter().enumerate() {
        for left in 0..SIZE_DIAG {
            for right in 0..SIZE_DIAG {
                let (a0, a1) = area_diag(pattern, left as f32, right as f32);
                let x = 5 * SIZE_ORTHO + e1 * SIZE_DIAG + left;
                let y = e2 * SIZE_DIAG + right;
                let i = (y * w + x) * 2;
                tex[i] = quantize(a0);
                tex[i + 1] = quantize(a1);
            }
        }
    }

    tex
}

/// Area under the line p1->p2 for the pixel column [x, x+1], split into the
/// (below, above) pair the shader blends with. Direct port of the reference
/// `area()` helper inside `areaortho`.
fn area_under(p1: [f32; 2], p2: [f32; 2], x: f32) -> (f32, f32) {
    let d = [p2[0] - p1[0], p2[1] - p1[1]];
    let x1 = x;
    let x2 = x + 1.0;
    let y1 = p1[1] + d[1] * (x1 - p1[0]) / d[0];
    let y2 = p1[1] + d[1] * (x2 - p1[0]) / d[0];

    let inside = (x1 >= p1[0] && x1 < p2[0]) || (x2 > p1[0] && x2 <= p2[0]);
    if !inside {
        return (0.0, 0.0);
    }

    let is_trapezoid = y1.signum() == y2.signum() || y1.abs() < 1e-4 || y2.abs() < 1e-4;
    if is_trapezoid {
        let a = (y1 + y2) / 2.0;
        if a < 0.0 {
            (a.abs(), 0.0)
        } else {
            (0.0, a.abs())
        }
    } else {
        // The line crosses zero inside the column: two triangles.
        let xi = -p1[1] * d[0] / d[1] + p1[0];
        let a1 = if xi > p1[0] {
            y1 * xi.fract() / 2.0
        } else {
            0.0
        };
        let a2 = if xi < p2[0] {
            y2 * (1.0 - xi.fract()) / 2.0
        } else {
            0.0
        };
        let a = if a1.abs() > a2.abs() { a1 } else { -a2 };
        if a < 0.0 {
            (a1.abs(), a2.abs())
        } else {
            (a2.abs(), a1.abs())
        }
    }
}

/// Smoothing for small u-shapes (patterns 3 and 12): blend towards the
/// sqrt-shaped response for short lines.
fn smooth_area(d: f32, a1: (f32, f32), a2: (f32, f32)) -> ((f32, f32), (f32, f32)) {
    let sqrt_pair = |a: (f32, f32)| ((a.0 * 2.0).sqrt() * 0.5, (a.1 * 2.0).sqrt() * 0.5);
    let lerp_pair =
        |b: (f32, f32), a: (f32, f32), p: f32| (b.0 + (a.0 - b.0) * p, b.1 + (a.1 - b.1) * p);
    let b1 = sqrt_pair(a1);
    let b2 = sqrt_pair(a2);
    let p = (d / SMOOTH_MAX_DISTANCE).clamp(0.0, 1.0);
    (lerp_pair(b1, a1, p), lerp_pair(b2, a2, p))
}

/// Horizontal/vertical pattern areas at subsample offset 0 (SMAA 1x). The
/// reference generator's `offset != 0` branches (patterns 6 and 9) collapse
/// to the plain single-line case at offset 0.
fn area_ortho(pattern: usize, left: f32, right: f32) -> (f32, f32) {
    let d = left + right + 1.0;
    let o1 = 0.5;
    let o2 = -0.5;

    match pattern {
        0 | 5 | 10 | 15 => (0.0, 0.0),
        // L shapes only blend on the crossing-edge side, and only when they
        // converge with the unfiltered side (left <=/>= right), to avoid
        // artifacts against pattern 0.
        1 => {
            if left <= right {
                area_under([0.0, o2], [d / 2.0, 0.0], left)
            } else {
                (0.0, 0.0)
            }
        }
        2 => {
            if left >= right {
                area_under([d / 2.0, 0.0], [d, o2], left)
            } else {
                (0.0, 0.0)
            }
        }
        3 => {
            let a1 = area_under([0.0, o2], [d / 2.0, 0.0], left);
            let a2 = area_under([d / 2.0, 0.0], [d, o2], left);
            let (a1, a2) = smooth_area(d, a1, a2);
            (a1.0 + a2.0, a1.1 + a2.1)
        }
        4 => {
            if left <= right {
                area_under([0.0, o1], [d / 2.0, 0.0], left)
            } else {
                (0.0, 0.0)
            }
        }
        6 | 7 | 14 => area_under([0.0, o1], [d, o2], left),
        8 => {
            if left >= right {
                area_under([d / 2.0, 0.0], [d, o1], left)
            } else {
                (0.0, 0.0)
            }
        }
        9 | 11 | 13 => area_under([0.0, o2], [d, o1], left),
        12 => {
            let a1 = area_under([0.0, o1], [d / 2.0, 0.0], left);
            let a2 = area_under([d / 2.0, 0.0], [d, o1], left);
            let (a1, a2) = smooth_area(d, a1, a2);
            (a1.0 + a2.0, a1.1 + a2.1)
        }
        _ => (0.0, 0.0),
    }
}

/// Fraction of the unit pixel square at `p` on the positive side of the line
/// p1->p2, integrated on a 30x30 sample grid (reference `area1`).
fn coverage_diag(p1: [f32; 2], p2: [f32; 2], p: [f32; 2]) -> f32 {
    let degenerate = p1 == p2;
    let xm = (p1[0] + p2[0]) / 2.0;
    let ym = (p1[1] + p2[1]) / 2.0;
    let a = p2[1] - p1[1];
    let b = p1[0] - p2[0];
    let mut count = 0u32;
    for x in 0..SAMPLES_DIAG {
        for y in 0..SAMPLES_DIAG {
            let px = p[0] + x as f32 / (SAMPLES_DIAG as f32 - 1.0);
            let py = p[1] + y as f32 / (SAMPLES_DIAG as f32 - 1.0);
            let inside = degenerate || a * (px - xm) + b * (py - ym) > 0.0;
            if inside {
                count += 1;
            }
        }
    }
    count as f32 / (SAMPLES_DIAG * SAMPLES_DIAG) as f32
}

/// Coverage pair for the two pixels the shader blends across a diagonal
/// (reference `area` inside `areadiag`, at subsample offset (0, 0)).
fn area_pair_diag(p1: [f32; 2], p2: [f32; 2], left: f32) -> (f32, f32) {
    let a1 = coverage_diag(p1, p2, [1.0 + left, 0.0 + left]);
    let a2 = coverage_diag(p1, p2, [1.0 + left, 1.0 + left]);
    (1.0 - a1, a2)
}

/// Diagonal pattern areas at subsample offset (0, 0) (SMAA 1x).
fn area_diag(pattern: usize, left: f32, right: f32) -> (f32, f32) {
    let d = left + right + 1.0;
    let ap = |p1: [f32; 2], p2: [f32; 2]| area_pair_diag(p1, [p2[0] + d, p2[1] + d], left);
    let avg = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);

    match pattern {
        0 => avg(ap([1.0, 1.0], [1.0, 1.0]), ap([1.0, 0.0], [1.0, 0.0])),
        1 => avg(ap([1.0, 0.0], [0.0, 0.0]), ap([1.0, 0.0], [1.0, 0.0])),
        2 => avg(ap([0.0, 0.0], [1.0, 0.0]), ap([1.0, 0.0], [1.0, 0.0])),
        3 => ap([1.0, 0.0], [1.0, 0.0]),
        4 => avg(ap([1.0, 1.0], [0.0, 0.0]), ap([1.0, 1.0], [1.0, 0.0])),
        5 => avg(ap([1.0, 1.0], [0.0, 0.0]), ap([1.0, 0.0], [1.0, 0.0])),
        6 => ap([1.0, 1.0], [1.0, 0.0]),
        7 => avg(ap([1.0, 1.0], [1.0, 0.0]), ap([1.0, 0.0], [1.0, 0.0])),
        8 => avg(ap([0.0, 0.0], [1.0, 1.0]), ap([1.0, 0.0], [1.0, 1.0])),
        9 => ap([1.0, 0.0], [1.0, 1.0]),
        10 => avg(ap([0.0, 0.0], [1.0, 1.0]), ap([1.0, 0.0], [1.0, 0.0])),
        11 => avg(ap([1.0, 0.0], [1.0, 1.0]), ap([1.0, 0.0], [1.0, 0.0])),
        12 => ap([1.0, 1.0], [1.0, 1.0]),
        13 => avg(ap([1.0, 1.0], [1.0, 1.0]), ap([1.0, 0.0], [1.0, 1.0])),
        14 => avg(ap([1.0, 1.0], [1.0, 1.0]), ap([1.0, 1.0], [1.0, 0.0])),
        15 => avg(ap([1.0, 1.0], [1.0, 1.0]), ap([1.0, 0.0], [1.0, 0.0])),
        _ => (0.0, 0.0),
    }
}

// ---------------------------------------------------------------------------
// SearchTex
// ---------------------------------------------------------------------------

/// Bilinear-fetch key for an edge combination: the search loop samples edges
/// at (-0.25, -0.125) between texels, so each fetched value encodes 4 edge
/// bits. The key doubles as the x/y pixel index of the pre-crop table
/// (values are e0 + 3*e1 + 7*e2 + 21*e3 in 1/32 units).
fn bilinear_index(e: [u8; 4]) -> usize {
    e[0] as usize + 3 * e[1] as usize + 7 * e[2] as usize + 21 * e[3] as usize
}

/// Extra distance for the last step of searches to the left.
fn delta_left(left: [u8; 4], top: [u8; 4]) -> u8 {
    let mut d = 0;
    // If there is an edge, continue.
    if top[3] == 1 {
        d += 1;
    }
    // If an edge was previously found, there is another edge and no crossing
    // edges, continue.
    if d == 1 && top[2] == 1 && left[1] != 1 && left[3] != 1 {
        d += 1;
    }
    d
}

/// Extra distance for the last step of searches to the right.
fn delta_right(left: [u8; 4], top: [u8; 4]) -> u8 {
    let mut d = 0;
    // If there is an edge and no crossing edges, continue.
    if top[3] == 1 && left[1] != 1 && left[3] != 1 {
        d += 1;
    }
    if d == 1 && top[2] == 1 && left[0] != 1 && left[2] != 1 {
        d += 1;
    }
    d
}

fn generate_search_tex() -> Vec<u8> {
    // Pre-crop table: 66x33, left half = searches to the left, right half =
    // searches to the right (matches the reference generator).
    let mut img = vec![0u8; 66 * 33];
    for l in 0..16u32 {
        let left = [
            (l >> 3 & 1) as u8,
            (l >> 2 & 1) as u8,
            (l >> 1 & 1) as u8,
            (l & 1) as u8,
        ];
        for t in 0..16u32 {
            let top = [
                (t >> 3 & 1) as u8,
                (t >> 2 & 1) as u8,
                (t >> 1 & 1) as u8,
                (t & 1) as u8,
            ];
            let x = bilinear_index(left);
            let y = bilinear_index(top);
            // Values 0/1/2 stored as n * 127 (the shader multiplies the
            // unorm sample by 255/127 to recover the step count).
            img[y * 66 + x] = 127 * delta_left(left, top);
            img[y * 66 + 33 + x] = 127 * delta_right(left, top);
        }
    }

    // Crop to the used 64x16 area (rows 17..33) and flip vertically, exactly
    // like the reference (the shader's SearchTex math assumes this layout).
    let mut out = vec![0u8; 64 * 16];
    for y in 17..33 {
        let out_y = 15 - (y - 17);
        for x in 0..64 {
            out[out_y * 64 + x] = img[y * 66 + x];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_tex_dimensions_and_known_texels() {
        let tex = area_tex_bytes();
        assert_eq!(tex.len(), (AREA_TEX_WIDTH * AREA_TEX_HEIGHT * 2) as usize);

        // Ortho pattern 1 (slot (3, 0)), distances (0, 0): the reference
        // areaortho returns (0.125, 0.0) -> bytes (32, 0) at texel (48, 0).
        let i = (48) * 2;
        assert_eq!(tex[i], 32);
        assert_eq!(tex[i + 1], 0);

        // Pattern 0 (slot (0, 0)) never blends: its whole subtexture is 0.
        for left in 0..SIZE_ORTHO {
            for right in 0..SIZE_ORTHO {
                let i = (right * AREA_TEX_WIDTH as usize + left) * 2;
                assert_eq!(tex[i], 0);
                assert_eq!(tex[i + 1], 0);
            }
        }

        // Diag pattern 3 (slot (1, 2)), distances (0, 0) is a genuine
        // diagonal: nonzero coverage on the first channel.
        let i = (40 * AREA_TEX_WIDTH as usize + 100) * 2;
        assert!(tex[i] > 0, "diag half must be generated (got 0)");

        // The unused subsample blocks (SMAA T2x/S2x only) stay zero.
        let below = (80 * AREA_TEX_WIDTH as usize) * 2;
        assert!(tex[below..].iter().all(|&b| b == 0));
    }

    #[test]
    fn search_tex_dimensions_and_values() {
        let tex = search_tex_bytes();
        assert_eq!(tex.len(), (SEARCH_TEX_WIDTH * SEARCH_TEX_HEIGHT) as usize);
        // Step counts are 0, 1 or 2, stored as n * 127.
        assert!(tex.iter().all(|&b| b == 0 || b == 127 || b == 254));
        assert!(tex.contains(&127));
        // left = [0,0,0,0], top = [0,0,1,1] gives delta 2 at pre-crop
        // (0, 28) -> post-crop-and-flip (0, 4).
        assert_eq!(tex[4 * SEARCH_TEX_WIDTH as usize], 254);
    }
}
