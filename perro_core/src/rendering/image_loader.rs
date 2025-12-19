//! Optimized image loading for dev mode
//! Uses format-specific decoders for faster loading

use image::{DynamicImage, GenericImageView, RgbaImage};

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

    // Allocate buffer for RGBA data
    let mut rgba_data = vec![0u8; (width * height * 4) as usize];

    // Read the image data
    reader
        .next_frame(&mut rgba_data)
        .map_err(|e| format!("Failed to read PNG frame: {}", e))?;

    // Convert to RgbaImage
    let rgba_image = RgbaImage::from_raw(width, height, rgba_data)
        .ok_or_else(|| "Failed to create RgbaImage from PNG data".to_string())?;

    Ok((rgba_image, width, height))
}

fn decode_jpeg_fast(bytes: &[u8]) -> Result<(RgbaImage, u32, u32), String> {
    use jpeg_decoder::Decoder;

    let mut decoder = Decoder::new(bytes);
    let pixels = decoder
        .decode()
        .map_err(|e| format!("Failed to decode JPEG: {}", e))?;

    let info = decoder.info().ok_or_else(|| "JPEG decoder missing info".to_string())?;
    let width = info.width as u32;
    let height = info.height as u32;

    // Convert to RGBA8
    let rgba_data = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            // Convert RGB to RGBA
            pixels
                .chunks_exact(3)
                .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255u8])
                .collect()
        }
        jpeg_decoder::PixelFormat::L8 => {
            // Convert grayscale to RGBA
            pixels
                .iter()
                .flat_map(|&l| [l, l, l, 255u8])
                .collect()
        }
        jpeg_decoder::PixelFormat::L16 => {
            // Convert 16-bit grayscale to RGBA (downscale to 8-bit)
            pixels
                .chunks_exact(2)
                .flat_map(|bytes| {
                    // Convert little-endian u16 to u8 (downscale)
                    let l16 = u16::from_le_bytes([bytes[0], bytes[1]]);
                    let l8 = (l16 >> 8) as u8; // Take upper 8 bits
                    [l8, l8, l8, 255u8]
                })
                .collect()
        }
        jpeg_decoder::PixelFormat::CMYK32 => {
            // Convert CMYK to RGBA (approximate)
            pixels
                .chunks_exact(4)
                .flat_map(|cmyk| {
                    let c = cmyk[0] as f32 / 255.0;
                    let m = cmyk[1] as f32 / 255.0;
                    let y = cmyk[2] as f32 / 255.0;
                    let k = cmyk[3] as f32 / 255.0;
                    let r = ((1.0 - c) * (1.0 - k) * 255.0) as u8;
                    let g = ((1.0 - m) * (1.0 - k) * 255.0) as u8;
                    let b = ((1.0 - y) * (1.0 - k) * 255.0) as u8;
                    [r, g, b, 255u8]
                })
                .collect()
        }
    };

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
