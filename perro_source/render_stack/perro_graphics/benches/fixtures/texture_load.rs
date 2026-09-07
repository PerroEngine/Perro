use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const EDGE: u32 = 512;
pub static LOOKUPS: AtomicUsize = AtomicUsize::new(0);
pub static PRIVATE_POOL_LOOKUPS: AtomicUsize = AtomicUsize::new(0);

pub fn source(index: usize) -> String {
    format!("res://bench/texture_{index}.ptex")
}

pub fn texture_lookup(hash: u64) -> &'static [u8] {
    static HASHES: OnceLock<Vec<u64>> = OnceLock::new();
    if !HASHES
        .get_or_init(|| {
            (0..32)
                .map(|i| perro_ids::string_to_u64(&source(i)))
                .collect()
        })
        .contains(&hash)
    {
        return &[];
    }
    LOOKUPS.fetch_add(1, Ordering::Relaxed);
    if std::thread::current()
        .name()
        .is_some_and(|name| name.starts_with("perro-texture-"))
    {
        PRIVATE_POOL_LOOKUPS.fetch_add(1, Ordering::Relaxed);
    }
    bytes()
}

pub fn bytes() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| {
        use perro_asset_formats::ptex;
        let mut pixels = Vec::with_capacity((EDGE * EDGE * 4) as usize);
        for y in 0..EDGE {
            for x in 0..EDGE {
                pixels.extend_from_slice(&[x as u8, y as u8, (x ^ y) as u8, 255]);
            }
        }
        let mut bytes = ptex::MAGIC.to_vec();
        for word in [
            ptex::VERSION_V1,
            EDGE,
            EDGE,
            ptex::FLAG_FORMAT_RGBA8,
            pixels.len() as u32,
        ] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend(perro_io::compress_zlib_best(&pixels).expect("fixture compression"));
        bytes
    })
}
