pub mod asset_io;
pub mod brk;
pub mod compression;
pub mod dirs;
pub mod walkdir;

pub use asset_io::*;
pub use compression::*;
pub use dirs::*;
pub use walkdir::*;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::io::{self, Cursor, Seek, SeekFrom};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::brk::archive::BrkArchive;
    use crate::brk::common::{read_header, read_index_entry};
    use crate::brk::packer::build_brk;

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_test_dir() -> std::path::PathBuf {
        let seq = TEST_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("perro_io_test_{pid}_{nonce}_{seq}"))
    }

    fn print_compression_stats(
        path: &str,
        original: usize,
        compressed: usize,
        decompressed: usize,
    ) {
        let ratio = if original > 0 {
            (compressed as f64 / original as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "{:<40} | original: {:>10} | compressed: {:>10} ({:>5.1}%) | decompressed: {:>10}",
            path, original, compressed, ratio, decompressed
        );
    }

    fn print_archive_summary(archive_size: usize, total_original: usize, file_count: usize) {
        let ratio = if total_original > 0 {
            (archive_size as f64 / total_original as f64) * 100.0
        } else {
            0.0
        };
        println!("\n{}", "=".repeat(120));
        println!(
            "Archive Summary: {} files | Total original: {} bytes | Archive size: {} bytes | Overall ratio: {:.1}%",
            file_count, total_original, archive_size, ratio
        );
        println!("{}\n", "=".repeat(120));
    }

    #[test]
    fn brk_roundtrip_sizes() -> io::Result<()> {
        println!("\n=== BRK Roundtrip Test ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(res_dir.join("nested"))?;

        let mut expected: HashMap<String, Vec<u8>> = HashMap::new();

        let small_text = b"hello perro_io".to_vec();
        let repeated = vec![b'a'; 16 * 1024];
        let nested_text = b"nested file contents\n".repeat(128);

        expected.insert("res/hello.txt".to_string(), small_text.clone());
        expected.insert("res/data.bin".to_string(), repeated.clone());
        expected.insert("res/nested/notes.txt".to_string(), nested_text.clone());

        fs::write(res_dir.join("hello.txt"), &small_text)?;
        fs::write(res_dir.join("data.bin"), &repeated)?;
        fs::write(res_dir.join("nested/notes.txt"), &nested_text)?;

        let output = base.join("test.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;

        let mut index = Vec::with_capacity(header.file_count as usize);
        for _ in 0..header.file_count {
            let (path, meta) = read_index_entry(&mut cursor)?;
            index.push((path, meta));
        }

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        assert_eq!(index.len(), expected.len(), "unexpected file count");

        let mut total_original = 0;
        for (path, meta) in index {
            let decompressed = archive.read_file(&path)?;
            print_compression_stats(
                &path,
                meta.original_size as usize,
                meta.size as usize,
                decompressed.len(),
            );
            total_original += meta.original_size as usize;

            let expected_data = expected.get(&path).expect("missing expected data");
            assert_eq!(&decompressed, expected_data, "data mismatch for {path}");
        }

        print_archive_summary(archive_size, total_original, expected.len());

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_empty_archive() -> io::Result<()> {
        println!("\n=== Empty Archive Test ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(&res_dir)?;

        let output = base.join("empty.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;

        assert_eq!(header.file_count, 0, "empty archive should have 0 files");

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        assert!(archive.read_file("nonexistent.txt").is_err());

        print_archive_summary(archive_size, 0, 0);

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_single_file() -> io::Result<()> {
        println!("\n=== Single File Test ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(&res_dir)?;

        let content = b"single file test";
        fs::write(res_dir.join("single.txt"), content)?;

        let output = base.join("single.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;

        let (path, meta) = read_index_entry(&mut cursor)?;

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        let retrieved = archive.read_file(&path)?;
        print_compression_stats(
            &path,
            meta.original_size as usize,
            meta.size as usize,
            retrieved.len(),
        );

        assert_eq!(&retrieved, content);

        print_archive_summary(archive_size, meta.original_size as usize, 1);

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_large_file() -> io::Result<()> {
        println!("\n=== Large File Test (1MB) ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(&res_dir)?;

        // Create a 1MB file with pseudo-random data (using wrapping arithmetic to avoid overflow)
        let mut large_data = Vec::with_capacity(1024 * 1024);
        for i in 0_u32..(1024 * 1024) {
            large_data.push(((i.wrapping_mul(7919)) % 256) as u8);
        }

        fs::write(res_dir.join("large.bin"), &large_data)?;

        let output = base.join("large.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;

        let (path, meta) = read_index_entry(&mut cursor)?;

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        let retrieved = archive.read_file(&path)?;
        print_compression_stats(
            &path,
            meta.original_size as usize,
            meta.size as usize,
            retrieved.len(),
        );

        assert_eq!(&retrieved, &large_data);

        print_archive_summary(archive_size, meta.original_size as usize, 1);

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_binary_data() -> io::Result<()> {
        println!("\n=== Binary Data Test ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(&res_dir)?;

        let binary_data: Vec<u8> = (0..=255).cycle().take(1024).collect();
        fs::write(res_dir.join("binary.dat"), &binary_data)?;

        let output = base.join("binary.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;

        let (path, meta) = read_index_entry(&mut cursor)?;

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        let retrieved = archive.read_file(&path)?;
        print_compression_stats(
            &path,
            meta.original_size as usize,
            meta.size as usize,
            retrieved.len(),
        );

        assert_eq!(&retrieved, &binary_data);

        print_archive_summary(archive_size, meta.original_size as usize, 1);

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_deep_nesting() -> io::Result<()> {
        println!("\n=== Deep Nesting Test ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");

        let deep_path = res_dir.join("a/b/c/d/e/f/g");
        fs::create_dir_all(&deep_path)?;

        let content = b"deeply nested file";
        fs::write(deep_path.join("deep.txt"), content)?;

        let output = base.join("deep.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;

        let (path, meta) = read_index_entry(&mut cursor)?;

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        let retrieved = archive.read_file(&path)?;
        print_compression_stats(
            &path,
            meta.original_size as usize,
            meta.size as usize,
            retrieved.len(),
        );

        assert_eq!(&retrieved, content);

        print_archive_summary(archive_size, meta.original_size as usize, 1);

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_many_small_files() -> io::Result<()> {
        println!("\n=== Many Small Files Test (100 files) ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(&res_dir)?;

        let mut expected = HashMap::new();

        for i in 0..100 {
            let filename = format!("file_{:03}.txt", i);
            let content = format!("Content of file {}", i).into_bytes();
            fs::write(res_dir.join(&filename), &content)?;
            expected.insert(format!("res/{}", filename), content);
        }

        let output = base.join("many.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;

        assert_eq!(header.file_count, 100);

        cursor.seek(SeekFrom::Start(header.index_offset))?;
        let mut index = Vec::with_capacity(header.file_count as usize);
        for _ in 0..header.file_count {
            let (path, meta) = read_index_entry(&mut cursor)?;
            index.push((path, meta));
        }

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        let mut total_original = 0;
        for (path, meta) in &index {
            let retrieved = archive.read_file(path)?;
            print_compression_stats(
                path,
                meta.original_size as usize,
                meta.size as usize,
                retrieved.len(),
            );
            total_original += meta.original_size as usize;

            let expected_data = expected.get(path).unwrap();
            assert_eq!(&retrieved, expected_data, "mismatch for {}", path);
        }

        print_archive_summary(archive_size, total_original, 100);

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_empty_file() -> io::Result<()> {
        println!("\n=== Empty File Test ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(&res_dir)?;

        fs::write(res_dir.join("empty.txt"), b"")?;

        let output = base.join("empty_file.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;

        let (path, meta) = read_index_entry(&mut cursor)?;

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        let retrieved = archive.read_file(&path)?;
        print_compression_stats(
            &path,
            meta.original_size as usize,
            meta.size as usize,
            retrieved.len(),
        );

        assert_eq!(retrieved.len(), 0);

        print_archive_summary(archive_size, 0, 1);

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_mixed_file_sizes() -> io::Result<()> {
        println!("\n=== Mixed File Sizes Test ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(&res_dir)?;

        let mut expected = HashMap::new();

        let tiny = vec![b'x'];
        fs::write(res_dir.join("tiny.bin"), &tiny)?;
        expected.insert("res/tiny.bin".to_string(), tiny);

        let small = vec![b'a'; 100];
        fs::write(res_dir.join("small.bin"), &small)?;
        expected.insert("res/small.bin".to_string(), small);

        let medium = vec![b'b'; 10_000];
        fs::write(res_dir.join("medium.bin"), &medium)?;
        expected.insert("res/medium.bin".to_string(), medium);

        let large = vec![b'c'; 500_000];
        fs::write(res_dir.join("large.bin"), &large)?;
        expected.insert("res/large.bin".to_string(), large);

        let output = base.join("mixed.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;

        let mut index = Vec::with_capacity(header.file_count as usize);
        for _ in 0..header.file_count {
            let (path, meta) = read_index_entry(&mut cursor)?;
            index.push((path, meta));
        }

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        let mut total_original = 0;
        for (path, meta) in &index {
            let retrieved = archive.read_file(path)?;
            print_compression_stats(
                path,
                meta.original_size as usize,
                meta.size as usize,
                retrieved.len(),
            );
            total_original += meta.original_size as usize;

            let expected_data = expected.get(path).unwrap();
            assert_eq!(&retrieved, expected_data, "size mismatch for {}", path);
        }

        print_archive_summary(archive_size, total_original, expected.len());

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_compression_effectiveness() -> io::Result<()> {
        println!("\n=== Compression Effectiveness Test ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(&res_dir)?;

        let compressible = vec![b'A'; 100_000];
        fs::write(res_dir.join("compressible.bin"), &compressible)?;

        let output = base.join("compress_test.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;

        let (path, meta) = read_index_entry(&mut cursor)?;

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        let retrieved = archive.read_file(&path)?;
        print_compression_stats(
            &path,
            meta.original_size as usize,
            meta.size as usize,
            retrieved.len(),
        );

        assert!(
            archive_size < 10_000,
            "Expected compressed archive to be < 10KB, got {} bytes",
            archive_size
        );
        assert_eq!(&retrieved, &compressible);

        print_archive_summary(archive_size, meta.original_size as usize, 1);

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_nonexistent_file_access() -> io::Result<()> {
        println!("\n=== Nonexistent File Access Test ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(&res_dir)?;

        fs::write(res_dir.join("exists.txt"), b"content")?;

        let output = base.join("exists.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        println!("Archive size: {} bytes", archive_size);

        assert!(archive.read_file("res/exists.txt").is_ok());
        println!("✓ Successfully accessed existing file");

        assert!(archive.read_file("res/nonexistent.txt").is_err());
        println!("✓ Correctly failed on nonexistent file");

        assert!(archive.read_file("wrong/path/exists.txt").is_err());
        println!("✓ Correctly failed on wrong path");

        assert!(archive.read_file("").is_err());
        println!("✓ Correctly failed on empty path\n");

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn brk_unicode_content() -> io::Result<()> {
        println!("\n=== Unicode Content Test ===");
        let base = temp_test_dir();
        let res_dir = base.join("res");
        fs::create_dir_all(&res_dir)?;

        let unicode_content = "Hello 世界! 🦀 Rust\nمرحبا\nПривет".as_bytes().to_vec();
        fs::write(res_dir.join("unicode.txt"), &unicode_content)?;

        let output = base.join("unicode.brk");
        build_brk(&output, &res_dir, &base)?;

        let bytes = fs::read(&output)?;
        let archive_size = bytes.len();
        let mut cursor = Cursor::new(bytes.as_slice());
        let header = read_header(&mut cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;

        let (path, meta) = read_index_entry(&mut cursor)?;

        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let archive = BrkArchive::open_from_bytes(bytes)?;

        let retrieved = archive.read_file(&path)?;
        print_compression_stats(
            &path,
            meta.original_size as usize,
            meta.size as usize,
            retrieved.len(),
        );

        assert_eq!(&retrieved, &unicode_content);

        print_archive_summary(archive_size, meta.original_size as usize, 1);

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }
}
