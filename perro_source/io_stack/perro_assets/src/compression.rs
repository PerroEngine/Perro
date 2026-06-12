use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::io::{self, Read, Write};

const DEFAULT_MAX_DECOMPRESSED_BYTES: usize = 1024 * 1024 * 1024;

pub fn compress_zlib_best(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    encoder.finish()
}

pub fn decompress_zlib(data: &[u8]) -> io::Result<Vec<u8>> {
    decompress_zlib_limited(data, DEFAULT_MAX_DECOMPRESSED_BYTES)
}

pub fn decompress_zlib_limited(data: &[u8], max_output_bytes: usize) -> io::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut out = Vec::new();
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
