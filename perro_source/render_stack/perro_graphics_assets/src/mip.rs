//! CPU mip-chain generation, shared by the runtime uploader and the static
//! texture bake so both produce identical levels.
//!
//! The sRGB conversions here are the hot loop of texture upload: a 2048^2 chain
//! needs 12 decodes and 3 encodes per generated texel. Both directions are
//! table-driven; see the table docs for why the encode needs a guess plus a
//! correction rather than a plain lookup or a plain search.

use perro_structs::TextureFilterMode;

pub struct RgbaMipLevel {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[inline]
pub fn rgba_mip_level_count(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height).max(1);
    u32::BITS - max_dim.leading_zeros()
}

pub fn build_rgba_levels_for_filter(
    rgba: &[u8],
    width: u32,
    height: u32,
    filter: TextureFilterMode,
) -> Vec<RgbaMipLevel> {
    let width = width.max(1);
    let height = height.max(1);
    let base_len = width as usize * height as usize * 4;
    if rgba.len() < base_len {
        return fallback_mip_chain();
    }
    let base = rgba[..base_len].to_vec();
    if filter.uses_mipmaps() {
        build_rgba_mip_chain_from_base(base, width, height)
    } else {
        vec![RgbaMipLevel {
            rgba: base,
            width,
            height,
        }]
    }
}

pub fn build_rgba_levels_for_filter_owned(
    mut rgba: Vec<u8>,
    width: u32,
    height: u32,
    filter: TextureFilterMode,
) -> Vec<RgbaMipLevel> {
    let width = width.max(1);
    let height = height.max(1);
    let base_len = width as usize * height as usize * 4;
    if rgba.len() < base_len {
        return fallback_mip_chain();
    }
    rgba.truncate(base_len);
    if filter.uses_mipmaps() {
        build_rgba_mip_chain_from_base(rgba, width, height)
    } else {
        vec![RgbaMipLevel {
            rgba,
            width,
            height,
        }]
    }
}

fn build_rgba_mip_chain_from_base(rgba: Vec<u8>, width: u32, height: u32) -> Vec<RgbaMipLevel> {
    let mut levels = Vec::with_capacity(rgba_mip_level_count(width, height) as usize);
    levels.push(RgbaMipLevel {
        rgba,
        width,
        height,
    });

    while let Some(prev) = levels
        .last()
        .filter(|level| level.width > 1 || level.height > 1)
    {
        let mut next = Vec::new();
        let (next_width, next_height) =
            downsample_rgba_into(&prev.rgba, prev.width, prev.height, &mut next);
        levels.push(RgbaMipLevel {
            rgba: next,
            width: next_width,
            height: next_height,
        });
    }

    levels
}

/// 2x2 box-downsample one RGBA level into `dst` (cleared + resized in place so
/// scratch buffers reuse their allocation). Color averages in linear space,
/// alpha averages directly. Returns the downsampled dimensions.
pub fn downsample_rgba_into(src: &[u8], width: u32, height: u32, dst: &mut Vec<u8>) -> (u32, u32) {
    let next_width = (width / 2).max(1);
    let next_height = (height / 2).max(1);
    dst.clear();
    dst.resize(next_width as usize * next_height as usize * 4, 0);

    // Hoisted: both tables are behind a `OnceLock`, and the inner loop would
    // otherwise pay the acquire load 15 times per output texel.
    let decode = srgb_decode_table();
    let encode = srgb_encode_tables();

    for y in 0..next_height {
        for x in 0..next_width {
            let sx = x * 2;
            let sy = y * 2;
            let x1 = (sx + 1).min(width - 1);
            let y1 = (sy + 1).min(height - 1);
            let samples = [(sx, sy), (x1, sy), (sx, y1), (x1, y1)];
            let dst_at = ((y * next_width + x) * 4) as usize;

            let alpha_sum = samples.iter().fold(0u32, |acc, &(px, py)| {
                let src_at = ((py * width + px) * 4) as usize + 3;
                acc + src[src_at] as u32
            });
            let alpha = ((alpha_sum + 2) / 4) as u8;
            for c in 0..3 {
                let sum = samples.iter().fold(0.0f32, |acc, &(px, py)| {
                    let src_at = ((py * width + px) * 4) as usize + c;
                    acc + decode[src[src_at] as usize]
                });
                dst[dst_at + c] = linear_to_srgb_u8_with(encode, sum * 0.25);
            }
            dst[dst_at + 3] = alpha;
        }
    }

    (next_width, next_height)
}

/// sRGB -> linear for every u8 input, computed once.
///
/// The downsampler needs 12 decodes and 3 encodes per output texel, and both
/// directions used to run `powf`. A 2048^2 chain is ~1.4M output texels, so
/// that was ~21M transcendental calls per mipped upload.
fn srgb_decode_table() -> &'static [f32; 256] {
    static TABLE: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0.0f32; 256];
        for (value, slot) in table.iter_mut().enumerate() {
            *slot = srgb_unit_to_linear(value as f32 / 255.0);
        }
        table
    })
}

/// Encode tables: a `sqrt(v)`-indexed guess plus the exact step boundaries.
///
/// A direct table alone cannot be exact — quantized bins straddle output-step
/// boundaries, so values sitting on a boundary land one step off. A binary
/// search over the boundaries alone IS exact but costs eight unpredictable
/// branches per channel, which measured as the dominant cost once `powf` was
/// gone.
///
/// So: index a coarse table for a guess, then fix it against the two boundaries
/// that could matter. `sqrt` bins put the resolution where the encode curve is
/// steep (~3300 steps per unit linear near black), keeping the guess within one
/// step everywhere, which is what makes the two-comparison correction total.
/// Result is bit-identical to the `powf` math at a sqrt, a load, and two
/// predictable compares.
const ENCODE_LUT_LEN: usize = 4096;

struct SrgbEncodeTables {
    /// Guess byte for each `sqrt(v)` bin; within one step of exact.
    guess: [u8; ENCODE_LUT_LEN],
    /// `boundary[i]` is the linear value where the encode rounds from `i` up
    /// to `i + 1`, i.e. the linear value encoding to exactly `i + 0.5`.
    boundary: [f32; 255],
}

fn srgb_encode_tables() -> &'static SrgbEncodeTables {
    static TABLES: std::sync::OnceLock<SrgbEncodeTables> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let mut guess = [0u8; ENCODE_LUT_LEN];
        for (index, slot) in guess.iter_mut().enumerate() {
            let root = index as f32 / (ENCODE_LUT_LEN - 1) as f32;
            *slot = linear_to_srgb_u8_exact(root * root);
        }
        let mut boundary = [0.0f32; 255];
        for (index, slot) in boundary.iter_mut().enumerate() {
            *slot = srgb_unit_to_linear((index as f32 + 0.5) / 255.0);
        }
        SrgbEncodeTables { guess, boundary }
    })
}

#[inline]
fn srgb_unit_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Table-build reference: the original `powf` encode.
fn linear_to_srgb_u8_exact(v: f32) -> u8 {
    let c = if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[inline]
fn linear_to_srgb_u8_with(tables: &SrgbEncodeTables, v: f32) -> u8 {
    // NaN must land on 0 like the old clamp did, so test for "definitely
    // positive" rather than negating a comparison.
    if v.is_nan() || v <= 0.0 {
        return 0;
    }
    if v >= 1.0 {
        return 255;
    }
    let index = (v.sqrt() * (ENCODE_LUT_LEN - 1) as f32 + 0.5) as usize;
    let guess = tables.guess[index.min(ENCODE_LUT_LEN - 1)] as usize;
    // Correct the one step the bin quantization can cost.
    if guess < 255 && v >= tables.boundary[guess] {
        return (guess + 1) as u8;
    }
    if guess > 0 && v < tables.boundary[guess - 1] {
        return (guess - 1) as u8;
    }
    guess as u8
}

fn fallback_mip_chain() -> Vec<RgbaMipLevel> {
    vec![RgbaMipLevel {
        rgba: vec![255, 255, 255, 255],
        width: 1,
        height: 1,
    }]
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Reference implementation the LUTs replaced. Kept here so the swap is
    /// checked against the real math, not against itself.
    fn reference_linear_to_srgb_u8(v: f32) -> u8 {
        let c = if v <= 0.0031308 {
            v * 12.92
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (c.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    fn reference_srgb_u8_to_linear(v: u8) -> f32 {
        let c = v as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    #[test]
    fn srgb_luts_match_the_powf_math_they_replaced() {
        let decode = srgb_decode_table();
        for value in 0..=255u8 {
            assert_eq!(decode[value as usize], reference_srgb_u8_to_linear(value));
        }

        // Every 4-sample average of table entries is reachable, so sweep the
        // encode side densely across the unit range.
        let encode = srgb_encode_tables();
        for step in 0..=20_000u32 {
            let v = step as f32 / 20_000.0;
            assert_eq!(
                linear_to_srgb_u8_with(encode, v),
                reference_linear_to_srgb_u8(v),
                "linear {v}"
            );
        }
        // A uniform 2x2 block averages to exactly one decode-table entry, so
        // flat color must survive downsampling unchanged. This is the property
        // that would show up as banding if the tables were off.
        for value in 0..=255u8 {
            assert_eq!(linear_to_srgb_u8_with(encode, decode[value as usize]), value);
        }

        assert_eq!(linear_to_srgb_u8_with(encode, -1.0), 0);
        assert_eq!(linear_to_srgb_u8_with(encode, f32::NAN), 0);
        assert_eq!(linear_to_srgb_u8_with(encode, 2.0), 255);
    }

    /// The pre-LUT downsampler, for the bench probe's baseline column.
    fn reference_downsample_into(
        src: &[u8],
        width: u32,
        height: u32,
        dst: &mut Vec<u8>,
    ) -> (u32, u32) {
        let next_width = (width / 2).max(1);
        let next_height = (height / 2).max(1);
        dst.clear();
        dst.resize(next_width as usize * next_height as usize * 4, 0);
        for y in 0..next_height {
            for x in 0..next_width {
                let sx = x * 2;
                let sy = y * 2;
                let x1 = (sx + 1).min(width - 1);
                let y1 = (sy + 1).min(height - 1);
                let samples = [(sx, sy), (x1, sy), (sx, y1), (x1, y1)];
                let dst_at = ((y * next_width + x) * 4) as usize;
                let alpha_sum = samples.iter().fold(0u32, |acc, &(px, py)| {
                    acc + src[((py * width + px) * 4) as usize + 3] as u32
                });
                for c in 0..3 {
                    let sum = samples.iter().fold(0.0f32, |acc, &(px, py)| {
                        acc + reference_srgb_u8_to_linear(src[((py * width + px) * 4) as usize + c])
                    });
                    dst[dst_at + c] = reference_linear_to_srgb_u8(sum * 0.25);
                }
                dst[dst_at + 3] = ((alpha_sum + 2) / 4) as u8;
            }
        }
        (next_width, next_height)
    }

    type DownsampleFn = fn(&[u8], u32, u32, &mut Vec<u8>) -> (u32, u32);

    fn time_chain(base: &[u8], dim: u32, downsample: DownsampleFn) -> std::time::Duration {
        let start = std::time::Instant::now();
        let mut src = base.to_vec();
        let (mut w, mut h) = (dim, dim);
        let mut dst = Vec::new();
        while w > 1 || h > 1 {
            let (nw, nh) = downsample(&src, w, h, &mut dst);
            std::mem::swap(&mut src, &mut dst);
            w = nw;
            h = nh;
        }
        std::hint::black_box(&src);
        start.elapsed()
    }

    #[test]
    #[ignore = "bench probe; run with --release --ignored --nocapture"]
    fn mip_chain_build_bench() {
        if cfg!(debug_assertions) {
            eprintln!("warn run this with --release for useful numbers");
        }
        for dim in [512u32, 1024, 2048] {
            let base = (0..dim as usize * dim as usize * 4)
                .map(|i| (i % 251) as u8)
                .collect::<Vec<u8>>();
            let _ = time_chain(&base, dim, downsample_rgba_into);
            let now = time_chain(&base, dim, downsample_rgba_into);
            let before = time_chain(&base, dim, reference_downsample_into);
            eprintln!(
                "mip_chain {dim}x{dim}: powf {:>8.3} ms -> lut {:>8.3} ms",
                before.as_secs_f64() * 1000.0,
                now.as_secs_f64() * 1000.0
            );
        }
    }

    #[test]
    fn rgba_mip_level_count_tracks_max_dim() {
        assert_eq!(rgba_mip_level_count(1, 1), 1);
        assert_eq!(rgba_mip_level_count(2, 1), 2);
        assert_eq!(rgba_mip_level_count(3, 5), 3);
        assert_eq!(rgba_mip_level_count(256, 128), 9);
    }

    #[test]
    fn build_rgba_mip_chain_halves_until_one() {
        let rgba = vec![128u8; 4 * 4 * 2];
        let levels = build_rgba_levels_for_filter(&rgba, 4, 2, TextureFilterMode::LinearMipmap);
        let dims: Vec<(u32, u32)> = levels
            .iter()
            .map(|level| (level.width, level.height))
            .collect();
        assert_eq!(dims, vec![(4, 2), (2, 1), (1, 1)]);
    }

}
