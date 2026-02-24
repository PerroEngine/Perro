use flate2::Compression;
use flate2::read::{DeflateDecoder, ZlibDecoder};
use flate2::write::{DeflateEncoder, ZlibEncoder};
use std::io::{self, Read, Write};

pub fn compress_deflate_best(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    encoder.finish()
}

pub fn decompress_deflate(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}

pub fn compress_zlib_best(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    encoder.finish()
}

pub fn decompress_zlib(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}
