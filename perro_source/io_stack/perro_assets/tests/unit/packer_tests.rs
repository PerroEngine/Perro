use super::{build_compressed_perro_archive_from_entries, should_skip};
use crate::archive::PerroAssetsArchive;
use crate::common::PERRO_ASSETS_COMPRESSED_MAGIC;
use std::collections::HashSet;
use std::fs;

#[test]
fn pmat_is_skipped_as_compiled_resource() {
    let extra = HashSet::new();
    assert!(should_skip("materials/mat.pmat", &extra));
    assert!(should_skip("particles/fire.ppart", &extra));
    assert!(should_skip("animations/run.panim", &extra));
    assert!(!should_skip("chunks/0_0.pdata", &extra));
    assert!(!should_skip("data/settings.txt", &extra));
    assert!(should_skip("scene/main.scn", &extra));
    assert!(should_skip("mesh/robot.glb", &extra));
    assert!(should_skip("audio/music.ogg", &extra));
    assert!(should_skip("shaders/custom.wgsl", &extra));
}

#[test]
fn compressed_archive_roundtrips() {
    let root = std::env::temp_dir().join(format!(
        "perro_assets_compressed_archive_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let source = root.join("payload.bin");
    let output = root.join("pack.dlc");
    fs::write(&source, vec![b'x'; 4096]).unwrap();

    build_compressed_perro_archive_from_entries(
        &output,
        &[("res/payload.bin".to_string(), source.clone())],
    )
    .unwrap();

    let bytes = fs::read(&output).unwrap();
    assert_eq!(&bytes[..4], &PERRO_ASSETS_COMPRESSED_MAGIC);
    let archive = PerroAssetsArchive::open_from_file(&output).unwrap();
    assert_eq!(
        archive.read_file("res/payload.bin").unwrap(),
        vec![b'x'; 4096]
    );

    let _ = fs::remove_dir_all(&root);
}
