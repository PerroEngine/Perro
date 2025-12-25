//! Optimized image loading for dev mode
//! Uses format-specific decoders for faster loading

use image::{GenericImageView, RgbaImage};

/// Decode image bytes directly to RGBA8, using fast format-specific decoders when possible
pub fn decode_image_fast(bytes: &[u8], path: &str) -> Result<(RgbaImage, u32, u32), String> {
    // Detect format from file extension first (fastest)
    let format = detect_format_from_path(path)
        .or_else(|| detect_format_from_magic_bytes(bytes));

    match format {
        Some(ImageFormat::Png) => decode_png_fast(bytes),
        Some(ImageFormat::Jpeg) => decode_jpeg_fast(bytes),
        Some(ImageFormat::WebP) | Some(ImageFormat::Bmp) | Some(ImageFormat::Gif) | Some(ImageFormat::Tga) | Some(ImageFormat::Ico) | None => {
            // Fall back to image crate for formats without fast decoders or unknown formats
            decode_with_image_crate(bytes)
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ImageFormat {
    Png,
    Jpeg,
    WebP,
    Bmp,
    Gif,
    Tga,
    Ico,
}

fn detect_format_from_path(path: &str) -> Option<ImageFormat> {
    let path_lower = path.to_lowercase();
    if path_lower.ends_with(".png") {
        Some(ImageFormat::Png)
    } else if path_lower.ends_with(".jpg") || path_lower.ends_with(".jpeg") {
        Some(ImageFormat::Jpeg)
    } else if path_lower.ends_with(".webp") {
        Some(ImageFormat::WebP)
    } else if path_lower.ends_with(".bmp") {
        Some(ImageFormat::Bmp)
    } else if path_lower.ends_with(".gif") {
        Some(ImageFormat::Gif)
    } else if path_lower.ends_with(".tga") {
        Some(ImageFormat::Tga)
    } else if path_lower.ends_with(".ico") {
        Some(ImageFormat::Ico)
    } else {
        None
    }
}

/// Read JPEG dimensions from SOF (Start of Frame) marker
/// Returns (width, height) if found, None otherwise
fn read_jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 20 {
        return None;
    }
    
    // JPEG starts with FF D8
    if bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    
    // Search for SOF markers (0xFFC0-0xFFC3)
    let mut i = 2;
    while i < bytes.len().saturating_sub(9) {
        if bytes[i] == 0xFF {
            let marker = bytes[i + 1];
            // SOF markers: 0xC0-0xC3 (baseline, extended sequential, progressive, lossless)
            if marker >= 0xC0 && marker <= 0xC3 {
                // SOF structure: FF [marker] [length_high] [length_low] [precision] [height_high] [height_low] [width_high] [width_low] ...
                if i + 8 < bytes.len() {
                    let height = ((bytes[i + 5] as u32) << 8) | (bytes[i + 6] as u32);
                    let width = ((bytes[i + 7] as u32) << 8) | (bytes[i + 8] as u32);
                    if width > 0 && height > 0 && width < 65536 && height < 65536 {
                        return Some((width, height));
                    }
                }
            }
            // Skip this marker segment
            if marker != 0xFF {
                i += 1;
            }
        }
        i += 1;
    }
    
    None
}

fn detect_format_from_magic_bytes(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.len() < 12 {
        return None;
    }

    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(ImageFormat::Png);
    }

    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(ImageFormat::Jpeg);
    }

    // WebP: RIFF...WEBP
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && bytes[8..12] == *b"WEBP" {
        return Some(ImageFormat::WebP);
    }

    // BMP: BM
    if bytes.starts_with(b"BM") {
        return Some(ImageFormat::Bmp);
    }

    // GIF: GIF87a or GIF89a
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(ImageFormat::Gif);
    }

    None
}

fn decode_png_fast(bytes: &[u8]) -> Result<(RgbaImage, u32, u32), String> {
    use png::Decoder;
    use std::io::Cursor;

    let decoder = Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("Failed to decode PNG: {}", e))?;

    let info = reader.info();
    let width = info.width;
    let height = info.height;

    // OPTIMIZED: Pre-allocate exact size needed, avoiding reallocations
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .and_then(|x| x.checked_mul(4))
        .ok_or_else(|| "Image dimensions too large".to_string())?;
    
    let mut rgba_data = Vec::with_capacity(pixel_count);
    rgba_data.resize(pixel_count, 0u8);

    // Read the image data directly into pre-allocated buffer
    reader
        .next_frame(&mut rgba_data)
        .map_err(|e| format!("Failed to read PNG frame: {}", e))?;

    // OPTIMIZED: Use from_raw which takes ownership, avoiding copy
    let rgba_image = RgbaImage::from_raw(width, height, rgba_data)
        .ok_or_else(|| "Failed to create RgbaImage from PNG data".to_string())?;

    Ok((rgba_image, width, height))
}

fn decode_jpeg_fast(bytes: &[u8]) -> Result<(RgbaImage, u32, u32), String> {
    use jpeg_decoder::Decoder;

    // Try to read dimensions from JPEG SOF header first
    let (width, height) = match read_jpeg_dimensions(bytes) {
        Some(dims) => dims,
        None => {
            // Can't read dimensions from header, fall back to image crate
            return decode_with_image_crate(bytes);
        }
    };

    let mut decoder = Decoder::new(bytes);
    
    // Decode the image
    let pixels = decoder
        .decode()
        .map_err(|e| format!("Failed to decode JPEG: {}", e))?;
    
    // Get pixel format from decoder info, or default to RGB24
    let pixel_format = decoder.info()
        .map(|info| info.pixel_format)
        .unwrap_or(jpeg_decoder::PixelFormat::RGB24);

    // OPTIMIZED: Pre-allocate exact size needed for RGBA output
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .and_then(|x| x.checked_mul(4))
        .ok_or_else(|| "Image dimensions too large".to_string())?;
    
    let mut rgba_data = Vec::with_capacity(pixel_count);

    // Convert to RGBA8 with optimized conversions
    match pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            // OPTIMIZED: Reserve capacity and use extend_from_slice pattern
            rgba_data.reserve_exact(pixel_count);
            for rgb in pixels.chunks_exact(3) {
                rgba_data.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255u8]);
            }
        }
        jpeg_decoder::PixelFormat::L8 => {
            // OPTIMIZED: Reserve and use direct iteration
            rgba_data.reserve_exact(pixel_count);
            for &l in pixels.iter() {
                rgba_data.extend_from_slice(&[l, l, l, 255u8]);
            }
        }
        jpeg_decoder::PixelFormat::L16 => {
            // OPTIMIZED: Convert 16-bit grayscale to RGBA (downscale to 8-bit)
            rgba_data.reserve_exact(pixel_count);
            for bytes in pixels.chunks_exact(2) {
                // Convert little-endian u16 to u8 (downscale)
                let l16 = u16::from_le_bytes([bytes[0], bytes[1]]);
                let l8 = (l16 >> 8) as u8; // Take upper 8 bits
                rgba_data.extend_from_slice(&[l8, l8, l8, 255u8]);
            }
        }
        jpeg_decoder::PixelFormat::CMYK32 => {
            // OPTIMIZED: Convert CMYK to RGBA (approximate)
            rgba_data.reserve_exact(pixel_count);
            for cmyk in pixels.chunks_exact(4) {
                let c = cmyk[0] as f32 / 255.0;
                let m = cmyk[1] as f32 / 255.0;
                let y = cmyk[2] as f32 / 255.0;
                let k = cmyk[3] as f32 / 255.0;
                let r = ((1.0 - c) * (1.0 - k) * 255.0) as u8;
                let g = ((1.0 - m) * (1.0 - k) * 255.0) as u8;
                let b = ((1.0 - y) * (1.0 - k) * 255.0) as u8;
                rgba_data.extend_from_slice(&[r, g, b, 255u8]);
            }
        }
    }

    // OPTIMIZED: Use from_raw which takes ownership, avoiding copy
    let rgba_image = RgbaImage::from_raw(width, height, rgba_data)
        .ok_or_else(|| "Failed to create RgbaImage from JPEG data".to_string())?;

    Ok((rgba_image, width, height))
}

fn decode_with_image_crate(bytes: &[u8]) -> Result<(RgbaImage, u32, u32), String> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("Failed to decode image: {}", e))?;
    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();
    Ok((rgba, width, height))
}

/// Load and decode image from bytes, optimized for dev mode
/// Returns RGBA8 image directly (avoids DynamicImage conversion overhead)
pub fn load_and_decode_image_fast(bytes: &[u8], path: &str) -> Result<(image::RgbaImage, u32, u32), String> {
    decode_image_fast(bytes, path)
}
