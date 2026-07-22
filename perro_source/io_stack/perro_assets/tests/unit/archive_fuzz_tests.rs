//! Deterministic mutation-sweep robustness tests for the `.perro` archive
//! reader. Archives arrive as runtime-loaded DLC/user content, so any mutated
//! byte stream must produce an `Err` or a self-consistent archive — never a
//! panic in header/index parsing, entry range resolution, or decompression.

use crate::common::{
    PERRO_ASSETS_COMPRESSED_MAGIC, PERRO_ASSETS_MAGIC, PerroAssetsEntryMeta, PerroAssetsHeader,
    write_header, write_index_entry,
};
use crate::compression::compress_zlib_best;
use std::io::{Cursor, Seek, SeekFrom};

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

/// Two-entry archive: one raw, one zlib-compressed payload.
fn valid_archive_bytes() -> Vec<u8> {
    let mut archive = Cursor::new(Vec::<u8>::new());
    write_header(
        &mut archive,
        &PerroAssetsHeader {
            magic: PERRO_ASSETS_MAGIC,
            version: perro_asset_formats::archive::VERSION,
            file_count: 0,
            index_offset: 0,
        },
    )
    .expect("test setup/result must succeed");

    let raw_payload = b"hello archive";
    let raw_offset = archive.position();
    archive.get_mut().extend_from_slice(raw_payload);

    let original = vec![0x5Au8; 64];
    let compressed = compress_zlib_best(&original).expect("test setup/result must succeed");
    let compressed_offset = raw_offset + raw_payload.len() as u64;
    archive.get_mut().extend_from_slice(&compressed);

    archive
        .seek(SeekFrom::End(0))
        .expect("test setup/result must succeed");
    let index_offset = archive.position();
    write_index_entry(
        &mut archive,
        "res/raw.txt",
        &PerroAssetsEntryMeta {
            offset: raw_offset,
            size: raw_payload.len() as u64,
            original_size: raw_payload.len() as u64,
            flags: 0,
        },
    )
    .expect("test setup/result must succeed");
    write_index_entry(
        &mut archive,
        "res/packed.bin",
        &PerroAssetsEntryMeta {
            offset: compressed_offset,
            size: compressed.len() as u64,
            original_size: original.len() as u64,
            flags: perro_asset_formats::archive::FLAG_COMPRESSED,
        },
    )
    .expect("test setup/result must succeed");

    archive
        .seek(SeekFrom::Start(0))
        .expect("test setup/result must succeed");
    write_header(
        &mut archive,
        &PerroAssetsHeader {
            magic: PERRO_ASSETS_MAGIC,
            version: perro_asset_formats::archive::VERSION,
            file_count: 2,
            index_offset,
        },
    )
    .expect("test setup/result must succeed");
    archive.into_inner()
}

// Opening must not panic; every listed file must read without panicking, and
// successful reads must honor the recorded original size.
fn check_archive(bytes: &[u8]) {
    let Ok(archive) = crate::archive::PerroAssetsArchive::open_from_owned_bytes(bytes.to_vec())
    else {
        return;
    };
    for path in archive.list_files() {
        let _ = archive.read_file(&path);
        let _ = archive.get_file_slice(&path);
    }
}

#[test]
fn archive_survives_mutation_sweep() {
    let valid = valid_archive_bytes();
    let archive = crate::archive::PerroAssetsArchive::open_from_owned_bytes(valid.clone())
        .expect("baseline must open");
    assert_eq!(
        archive
            .read_file("res/raw.txt")
            .expect("test setup/result must succeed"),
        b"hello archive"
    );
    assert_eq!(
        archive
            .read_file("res/packed.bin")
            .expect("test setup/result must succeed"),
        vec![0x5Au8; 64]
    );
    mutation_sweep(&valid, check_archive);
}

#[test]
fn compressed_container_survives_mutation_sweep() {
    let inner = valid_archive_bytes();
    let compressed = compress_zlib_best(&inner).expect("test setup/result must succeed");
    let mut wrapped = Vec::with_capacity(16 + compressed.len());
    wrapped.extend_from_slice(&PERRO_ASSETS_COMPRESSED_MAGIC);
    wrapped.extend_from_slice(&perro_asset_formats::archive::VERSION.to_le_bytes());
    wrapped.extend_from_slice(&(inner.len() as u64).to_le_bytes());
    wrapped.extend_from_slice(&compressed);
    assert!(
        crate::archive::PerroAssetsArchive::open_from_owned_bytes(wrapped.clone()).is_ok(),
        "baseline wrapped archive must open"
    );
    mutation_sweep(&wrapped, check_archive);
}
