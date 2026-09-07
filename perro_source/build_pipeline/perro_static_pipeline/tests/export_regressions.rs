mod support;

use perro_static_pipeline::{
    generate_static_audios, generate_static_materials, generate_static_meshes,
    generate_static_skeletons, generate_static_textures,
};
use support::{Fixture, outputs};

#[test]
fn text_codegen_cache_keeps_inventory_and_recovers_missing_output() {
    use perro_static_pipeline::{
        begin_static_asset_inventory, generate_static_animations, generate_static_csvs,
        generate_static_scenes, take_static_asset_inventory,
    };
    let fixture = Fixture::new();
    fixture.write(
        "scene.scn",
        b"$root = @Root\n[Root]\n[Node3D]\nposition = (1,2,3)\n[/Node3D]\n[/Root]\n",
    );
    fixture.write("clip.panim", b"[Animation]\nname = \"Clip\"\nfps = 24\n[/Animation]\n[Objects]\n@Hero = Node3D\n[/Objects]\n[Frame0]\n@Hero { position = (0,0,0) }\n[/Frame0]\n");
    fixture.write("data.csv", b"key,value\none,1\n");
    // Exercise enabled caches, above the small-input bypass threshold.
    for index in 1..8 {
        for (stem, ext) in [("scene", "scn"), ("clip", "panim"), ("data", "csv")] {
            let bytes =
                std::fs::read(fixture.0.join(format!("res/{stem}.{ext}"))).expect("fixture source");
            fixture.write(&format!("{stem}{index}.{ext}"), &bytes);
        }
    }
    let tree = fixture.tree();
    let build = || {
        begin_static_asset_inventory();
        generate_static_scenes(&fixture.0, &tree).expect("scenes");
        generate_static_animations(&fixture.0, &tree).expect("animations");
        generate_static_csvs(&fixture.0).expect("CSVs");
        take_static_asset_inventory().expect("inventory")
    };
    let inventory = build();
    assert_eq!(inventory.len(), 24);
    for name in ["scenes", "animations", "csvs"] {
        assert!(
            fixture
                .0
                .join(format!(".perro/project/src/static/.{name}.rs.codegen"))
                .is_file()
        );
    }
    let first = outputs(&fixture.0.join(".perro"));
    assert_eq!(build(), inventory);
    assert_eq!(outputs(&fixture.0.join(".perro")), first);
    std::fs::remove_file(fixture.0.join(".perro/project/src/static/animations.rs"))
        .expect("remove generated output");
    assert_eq!(build(), inventory);
    assert_eq!(outputs(&fixture.0.join(".perro")), first);
    let csv_path = fixture.0.join("res/data.csv");
    let csv_time = std::fs::metadata(&csv_path)
        .expect("CSV metadata")
        .modified()
        .expect("CSV time");
    fixture.write("data.csv", b"key,value\ntwo,2\n");
    std::fs::File::options()
        .write(true)
        .open(&csv_path)
        .expect("CSV handle")
        .set_times(std::fs::FileTimes::new().set_modified(csv_time))
        .expect("restore CSV time");
    assert_eq!(build(), inventory);
    assert_ne!(
        outputs(&fixture.0.join(".perro")),
        first,
        "same-stat source edit must regenerate code"
    );
    fixture.write("data.csv", b"key,value\none,1,extra\n");
    assert!(
        generate_static_csvs(&fixture.0).is_err(),
        "changed invalid input cannot hit cache"
    );
}

#[test]
fn model_import_and_incremental_exports_keep_output_bytes() {
    let fixture = Fixture::new();
    fixture.image("texture.png", 8);
    fixture.model("mesh.gltf", "texture.png");
    let (document, buffers, images) =
        gltf::import(fixture.0.join("res/mesh.gltf")).expect("full reference import");
    assert_eq!(document.meshes().count(), 1);
    assert_eq!(buffers.len(), 1);
    assert_eq!(images.len(), 4);
    let tree = fixture.tree();
    let build = || {
        generate_static_materials(&fixture.0, &tree).expect("materials");
        generate_static_meshes(&fixture.0, &tree, false).expect("meshes");
        generate_static_skeletons(&fixture.0, &tree).expect("skeletons");
    };
    build();
    let first = outputs(&fixture.0.join(".perro"));
    assert!(first.keys().any(|path| path.ends_with(".pmesh")));
    build();
    assert_eq!(outputs(&fixture.0.join(".perro")), first);
    fixture.clear_outputs();
    build();
    assert_eq!(outputs(&fixture.0.join(".perro")), first);
}

#[test]
fn bounded_bakes_keep_serial_parallel_and_incremental_output() {
    let fixture = Fixture::new();
    for i in 0..7 {
        fixture.image(&format!("texture{i}.png"), 8);
    }
    // Different audio input extensions intentionally target the same output.
    fixture.write("same.ogg", b"first synthetic audio payload");
    fixture.write("same.wav", b"last synthetic audio payload");
    let tree = fixture.tree();
    let build = || {
        generate_static_textures(&fixture.0, &tree).expect("textures");
        generate_static_audios(&fixture.0, &tree).expect("audios");
    };
    let serial = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("serial pool");
    let parallel = rayon::ThreadPoolBuilder::new()
        .num_threads(3)
        .build()
        .expect("parallel pool");
    serial.install(build);
    let first = outputs(&fixture.0.join(".perro"));
    parallel.install(build);
    assert_eq!(outputs(&fixture.0.join(".perro")), first);
    fixture.clear_outputs();
    parallel.install(build);
    assert_eq!(outputs(&fixture.0.join(".perro")), first);
}
