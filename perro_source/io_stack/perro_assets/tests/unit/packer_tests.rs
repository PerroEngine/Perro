use super::{build_compressed_perro_archive_from_entries, build_perro_assets_archive, should_skip};
use crate::archive::PerroAssetsArchive;
use crate::common::PERRO_ASSETS_COMPRESSED_MAGIC;
use std::collections::HashSet;
use std::fs;
use std::time::Duration;

#[test]
fn pmat_is_skipped_as_compiled_resource() {
    let extra = HashSet::new();
    assert!(should_skip("materials/mat.pmat", &extra));
    assert!(should_skip("particles/fire.ppart", &extra));
    assert!(should_skip("animations/run.panim", &extra));
    assert!(should_skip("animations/tree.panimtree", &extra));
    assert!(should_skip("rigs/hero.pskel", &extra));
    assert!(should_skip("rigs/ui.pskel2d", &extra));
    assert!(should_skip("tiles/world.ptileset", &extra));
    assert!(should_skip("ui/default.uistyle", &extra));
    assert!(!should_skip("chunks/0_0.pdata", &extra));
    assert!(!should_skip("data/settings.txt", &extra));
    assert!(should_skip("scene/main.scn", &extra));
    assert!(should_skip("mesh/robot.glb", &extra));
    assert!(should_skip("audio/music.ogg", &extra));
    assert!(should_skip("music/theme.mid", &extra));
    assert!(should_skip("music/theme.midi", &extra));
    assert!(should_skip("soundfonts/game.sf2", &extra));
    assert!(should_skip("shaders/custom.wgsl", &extra));
}

#[test]
fn compressed_archive_roundtrips() {
    let root = std::env::temp_dir().join(format!(
        "perro_assets_compressed_archive_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("test setup/result must succeed");
    let source = root.join("payload.bin");
    let output = root.join("pack.dlc");
    fs::write(&source, vec![b'x'; 4096]).expect("test setup/result must succeed");

    build_compressed_perro_archive_from_entries(
        &output,
        &[("res/payload.bin".to_string(), source.clone())],
    )
    .expect("test setup/result must succeed");

    let bytes = fs::read(&output).expect("test setup/result must succeed");
    assert_eq!(&bytes[..4], &PERRO_ASSETS_COMPRESSED_MAGIC);
    assert_eq!(
        u32::from_le_bytes(
            bytes[4..8]
                .try_into()
                .expect("test setup/result must succeed")
        ),
        1
    );
    let archive =
        PerroAssetsArchive::open_from_file(&output).expect("test setup/result must succeed");
    assert_eq!(
        archive
            .read_file("res/payload.bin")
            .expect("test setup/result must succeed"),
        vec![b'x'; 4096]
    );

    let _ = fs::remove_dir_all(&root);
}

fn set_source_mtime(path: &std::path::Path, mtime: std::time::SystemTime) {
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("test setup/result must succeed")
        .set_times(fs::FileTimes::new().set_modified(mtime))
        .expect("test setup/result must succeed");
}

#[test]
fn assets_archive_reuses_prev_compressed_bytes_on_stat_match() {
    let root = std::env::temp_dir().join(format!("perro_assets_incr_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let res_dir = root.join("res");
    fs::create_dir_all(&res_dir).expect("test setup/result must succeed");
    let source = res_dir.join("data.txt");
    let output = root.join("assets.perro");
    fs::write(&source, vec![b'a'; 512]).expect("test setup/result must succeed");

    build_perro_assets_archive(&output, &res_dir, &root, &[])
        .expect("test setup/result must succeed");
    let first = fs::read(&output).expect("test setup/result must succeed");
    let mtime = fs::metadata(&source)
        .expect("test setup/result must succeed")
        .modified()
        .expect("test setup/result must succeed");

    // Same len + restored mtime: the builder must serve the previous
    // archive's compressed bytes (stale 'a' payload proves no re-encode).
    fs::write(&source, vec![b'b'; 512]).expect("test setup/result must succeed");
    set_source_mtime(&source, mtime);
    build_perro_assets_archive(&output, &res_dir, &root, &[])
        .expect("test setup/result must succeed");
    assert_eq!(
        fs::read(&output).expect("test setup/result must succeed"),
        first,
        "stat hit must reuse"
    );

    // mtime moved: rebuild re-encodes and picks up the new bytes.
    set_source_mtime(&source, mtime + Duration::from_secs(5));
    build_perro_assets_archive(&output, &res_dir, &root, &[])
        .expect("test setup/result must succeed");
    let archive =
        PerroAssetsArchive::open_from_file(&output).expect("test setup/result must succeed");
    assert_eq!(
        archive
            .read_file("res/data.txt")
            .expect("test setup/result must succeed"),
        vec![b'b'; 512]
    );

    // Corrupt sidecar: falls back to a full rebuild, output stays valid.
    fs::write(root.join("assets.perro.stat"), "garbage").expect("test setup/result must succeed");
    build_perro_assets_archive(&output, &res_dir, &root, &[])
        .expect("test setup/result must succeed");
    let archive =
        PerroAssetsArchive::open_from_file(&output).expect("test setup/result must succeed");
    assert_eq!(
        archive
            .read_file("res/data.txt")
            .expect("test setup/result must succeed"),
        vec![b'b'; 512]
    );

    let _ = fs::remove_dir_all(&root);
}
