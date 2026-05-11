use std::time::Duration;
#[cfg(feature = "profile")]
use std::time::Instant;

pub(crate) fn decode_static_pawdio(blob: &[u8]) -> Result<(Vec<u8>, Duration), String> {
    const HEADER_LEN: usize = 18;
    if blob.is_empty() {
        return Ok((Vec::new(), Duration::ZERO));
    }
    if blob.len() < HEADER_LEN {
        return Err("static audio blob too small".to_string());
    }
    if &blob[..6] != perro_asset_formats::pawdio::MAGIC {
        return Ok((blob.to_vec(), Duration::ZERO));
    }
    let version = u32::from_le_bytes([blob[6], blob[7], blob[8], blob[9]]);
    if version != perro_asset_formats::pawdio::VERSION {
        return Err(format!("unsupported .pawdio version {version}"));
    }
    let flags = u32::from_le_bytes([blob[10], blob[11], blob[12], blob[13]]);
    let raw_len = u32::from_le_bytes([blob[14], blob[15], blob[16], blob[17]]) as usize;
    let payload = &blob[HEADER_LEN..];

    if (flags & perro_asset_formats::pawdio::FLAG_ZLIB) != 0 {
        #[cfg(feature = "profile")]
        let decompress_begin = Instant::now();
        let decompressed = perro_io::decompress_zlib(payload).map_err(|err| err.to_string())?;
        #[cfg(feature = "profile")]
        let decompress_elapsed = decompress_begin.elapsed();
        #[cfg(not(feature = "profile"))]
        let decompress_elapsed = Duration::ZERO;
        if decompressed.len() != raw_len {
            return Err(format!(
                "invalid .pawdio length: expected {raw_len}, got {}",
                decompressed.len()
            ));
        }
        return Ok((decompressed, decompress_elapsed));
    }

    if payload.len() != raw_len {
        return Err(format!(
            "invalid .pawdio raw payload length: expected {raw_len}, got {}",
            payload.len()
        ));
    }
    Ok((payload.to_vec(), Duration::ZERO))
}
