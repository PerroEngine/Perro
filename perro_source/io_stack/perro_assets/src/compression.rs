use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::io::{self, Read, Write};

const DEFAULT_MAX_DECOMPRESSED_BYTES: usize = 1024 * 1024 * 1024;

// Preallocation ceiling for the decompression output buffer: callers passing
// an exact expected size (well under this) get a single allocation; callers
// using the permissive default limit never preallocate gigabytes up front.
const MAX_DECOMPRESS_PREALLOC_BYTES: usize = 16 * 1024 * 1024;

pub fn compress_zlib_best(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    encoder.finish()
}

/// Fast-level zlib for latency-sensitive packing (e.g. mic packets), trading a
/// few percent of size for a much cheaper encode.
pub fn compress_zlib_fast(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(
        Vec::with_capacity((data.len() / 2).max(64)),
        Compression::fast(),
    );
    encoder.write_all(data)?;
    encoder.finish()
}

pub fn decompress_zlib(data: &[u8]) -> io::Result<Vec<u8>> {
    decompress_zlib_limited(data, DEFAULT_MAX_DECOMPRESSED_BYTES)
}

pub fn decompress_zlib_limited(data: &[u8], max_output_bytes: usize) -> io::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    // Sized hint bounded by the input's plausible expansion and a hard
    // ceiling, so the common exact-size case avoids realloc-growth without
    // trusting a hostile limit for a giant upfront allocation.
    let prealloc = max_output_bytes
        .min(data.len().saturating_mul(8))
        .min(MAX_DECOMPRESS_PREALLOC_BYTES);
    let mut out = Vec::with_capacity(prealloc);
    let limit = (max_output_bytes as u64).saturating_add(1);
    decoder.by_ref().take(limit).read_to_end(&mut out)?;
    if out.len() > max_output_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "zlib output exceeds limit",
        ));
    }
    Ok(out)
}
