use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_io::asset_io::{ProjectRoot, StaticResourceLookups, load_asset, set_project_root};

const EMPTY_ARCHIVE: &[u8] = &[
    b'P', b'R', b'A', b'1', 1, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0,
];

const TEXTURE_PATH: &str = "res://textures/bench.png";
const MESH_PATH: &str = "res://meshes/bench.pmesh";
const AUDIO_PATH: &str = "res://audio/bench.ogg";
const TEXTURE_HASH: u64 = perro_ids::string_to_u64(TEXTURE_PATH);
const MESH_HASH: u64 = perro_ids::string_to_u64(MESH_PATH);
const AUDIO_HASH: u64 = perro_ids::string_to_u64(AUDIO_PATH);
const DUMMY_HASH_00: u64 = perro_ids::string_to_u64("res://bench/dummy_00.bin");
const DUMMY_HASH_01: u64 = perro_ids::string_to_u64("res://bench/dummy_01.bin");
const DUMMY_HASH_02: u64 = perro_ids::string_to_u64("res://bench/dummy_02.bin");
const DUMMY_HASH_03: u64 = perro_ids::string_to_u64("res://bench/dummy_03.bin");
const DUMMY_HASH_04: u64 = perro_ids::string_to_u64("res://bench/dummy_04.bin");
const DUMMY_HASH_05: u64 = perro_ids::string_to_u64("res://bench/dummy_05.bin");
const DUMMY_HASH_06: u64 = perro_ids::string_to_u64("res://bench/dummy_06.bin");
const DUMMY_HASH_07: u64 = perro_ids::string_to_u64("res://bench/dummy_07.bin");
const DUMMY_HASH_08: u64 = perro_ids::string_to_u64("res://bench/dummy_08.bin");
const DUMMY_HASH_09: u64 = perro_ids::string_to_u64("res://bench/dummy_09.bin");
const DUMMY_HASH_10: u64 = perro_ids::string_to_u64("res://bench/dummy_10.bin");
const DUMMY_HASH_11: u64 = perro_ids::string_to_u64("res://bench/dummy_11.bin");
const DUMMY_HASH_12: u64 = perro_ids::string_to_u64("res://bench/dummy_12.bin");
const DUMMY_HASH_13: u64 = perro_ids::string_to_u64("res://bench/dummy_13.bin");
const DUMMY_HASH_14: u64 = perro_ids::string_to_u64("res://bench/dummy_14.bin");
const DUMMY_HASH_15: u64 = perro_ids::string_to_u64("res://bench/dummy_15.bin");
const DUMMY_HASH_16: u64 = perro_ids::string_to_u64("res://bench/dummy_16.bin");
const DUMMY_HASH_17: u64 = perro_ids::string_to_u64("res://bench/dummy_17.bin");
const DUMMY_HASH_18: u64 = perro_ids::string_to_u64("res://bench/dummy_18.bin");
const DUMMY_HASH_19: u64 = perro_ids::string_to_u64("res://bench/dummy_19.bin");
const DUMMY_HASH_20: u64 = perro_ids::string_to_u64("res://bench/dummy_20.bin");
const DUMMY_HASH_21: u64 = perro_ids::string_to_u64("res://bench/dummy_21.bin");
const DUMMY_HASH_22: u64 = perro_ids::string_to_u64("res://bench/dummy_22.bin");
const DUMMY_HASH_23: u64 = perro_ids::string_to_u64("res://bench/dummy_23.bin");
const DUMMY_HASH_24: u64 = perro_ids::string_to_u64("res://bench/dummy_24.bin");
const DUMMY_HASH_25: u64 = perro_ids::string_to_u64("res://bench/dummy_25.bin");
const DUMMY_HASH_26: u64 = perro_ids::string_to_u64("res://bench/dummy_26.bin");
const DUMMY_HASH_27: u64 = perro_ids::string_to_u64("res://bench/dummy_27.bin");
const DUMMY_HASH_28: u64 = perro_ids::string_to_u64("res://bench/dummy_28.bin");
const DUMMY_HASH_29: u64 = perro_ids::string_to_u64("res://bench/dummy_29.bin");
const DUMMY_HASH_30: u64 = perro_ids::string_to_u64("res://bench/dummy_30.bin");
const DUMMY_HASH_31: u64 = perro_ids::string_to_u64("res://bench/dummy_31.bin");

static TEXTURE_BYTES: [u8; 4 * 1024] = [7; 4 * 1024];
static MESH_BYTES: [u8; 64 * 1024] = [13; 64 * 1024];
static AUDIO_BYTES: [u8; 16 * 1024] = [29; 16 * 1024];

fn lookup_texture(path_hash: u64) -> &'static [u8] {
    match path_hash {
        DUMMY_HASH_00 => b"0",
        DUMMY_HASH_01 => b"1",
        DUMMY_HASH_02 => b"2",
        DUMMY_HASH_03 => b"3",
        DUMMY_HASH_04 => b"4",
        DUMMY_HASH_05 => b"5",
        DUMMY_HASH_06 => b"6",
        DUMMY_HASH_07 => b"7",
        DUMMY_HASH_08 => b"8",
        DUMMY_HASH_09 => b"9",
        DUMMY_HASH_10 => b"10",
        DUMMY_HASH_11 => b"11",
        DUMMY_HASH_12 => b"12",
        DUMMY_HASH_13 => b"13",
        DUMMY_HASH_14 => b"14",
        DUMMY_HASH_15 => b"15",
        TEXTURE_HASH => &TEXTURE_BYTES,
        DUMMY_HASH_16 => b"16",
        DUMMY_HASH_17 => b"17",
        DUMMY_HASH_18 => b"18",
        DUMMY_HASH_19 => b"19",
        DUMMY_HASH_20 => b"20",
        DUMMY_HASH_21 => b"21",
        DUMMY_HASH_22 => b"22",
        DUMMY_HASH_23 => b"23",
        DUMMY_HASH_24 => b"24",
        DUMMY_HASH_25 => b"25",
        DUMMY_HASH_26 => b"26",
        DUMMY_HASH_27 => b"27",
        DUMMY_HASH_28 => b"28",
        DUMMY_HASH_29 => b"29",
        DUMMY_HASH_30 => b"30",
        DUMMY_HASH_31 => b"31",
        _ => b"",
    }
}

fn lookup_mesh(path_hash: u64) -> &'static [u8] {
    match path_hash {
        DUMMY_HASH_00 => b"0",
        DUMMY_HASH_01 => b"1",
        DUMMY_HASH_02 => b"2",
        DUMMY_HASH_03 => b"3",
        DUMMY_HASH_04 => b"4",
        DUMMY_HASH_05 => b"5",
        DUMMY_HASH_06 => b"6",
        DUMMY_HASH_07 => b"7",
        DUMMY_HASH_08 => b"8",
        DUMMY_HASH_09 => b"9",
        DUMMY_HASH_10 => b"10",
        DUMMY_HASH_11 => b"11",
        DUMMY_HASH_12 => b"12",
        DUMMY_HASH_13 => b"13",
        DUMMY_HASH_14 => b"14",
        DUMMY_HASH_15 => b"15",
        MESH_HASH => &MESH_BYTES,
        DUMMY_HASH_16 => b"16",
        DUMMY_HASH_17 => b"17",
        DUMMY_HASH_18 => b"18",
        DUMMY_HASH_19 => b"19",
        DUMMY_HASH_20 => b"20",
        DUMMY_HASH_21 => b"21",
        DUMMY_HASH_22 => b"22",
        DUMMY_HASH_23 => b"23",
        DUMMY_HASH_24 => b"24",
        DUMMY_HASH_25 => b"25",
        DUMMY_HASH_26 => b"26",
        DUMMY_HASH_27 => b"27",
        DUMMY_HASH_28 => b"28",
        DUMMY_HASH_29 => b"29",
        DUMMY_HASH_30 => b"30",
        DUMMY_HASH_31 => b"31",
        _ => b"",
    }
}

fn lookup_audio(path_hash: u64) -> &'static [u8] {
    match path_hash {
        DUMMY_HASH_00 => b"0",
        DUMMY_HASH_01 => b"1",
        DUMMY_HASH_02 => b"2",
        DUMMY_HASH_03 => b"3",
        DUMMY_HASH_04 => b"4",
        DUMMY_HASH_05 => b"5",
        DUMMY_HASH_06 => b"6",
        DUMMY_HASH_07 => b"7",
        DUMMY_HASH_08 => b"8",
        DUMMY_HASH_09 => b"9",
        DUMMY_HASH_10 => b"10",
        DUMMY_HASH_11 => b"11",
        DUMMY_HASH_12 => b"12",
        DUMMY_HASH_13 => b"13",
        DUMMY_HASH_14 => b"14",
        DUMMY_HASH_15 => b"15",
        AUDIO_HASH => &AUDIO_BYTES,
        DUMMY_HASH_16 => b"16",
        DUMMY_HASH_17 => b"17",
        DUMMY_HASH_18 => b"18",
        DUMMY_HASH_19 => b"19",
        DUMMY_HASH_20 => b"20",
        DUMMY_HASH_21 => b"21",
        DUMMY_HASH_22 => b"22",
        DUMMY_HASH_23 => b"23",
        DUMMY_HASH_24 => b"24",
        DUMMY_HASH_25 => b"25",
        DUMMY_HASH_26 => b"26",
        DUMMY_HASH_27 => b"27",
        DUMMY_HASH_28 => b"28",
        DUMMY_HASH_29 => b"29",
        DUMMY_HASH_30 => b"30",
        DUMMY_HASH_31 => b"31",
        _ => b"",
    }
}

fn install_static_project_root() {
    set_project_root(ProjectRoot::PerroAssets {
        data: EMPTY_ARCHIVE,
        name: "Static Asset Bench".to_string(),
        static_resource_lookups: StaticResourceLookups {
            texture_lookup: Some(lookup_texture),
            mesh_lookup: Some(lookup_mesh),
            audio_lookup: Some(lookup_audio),
            ..StaticResourceLookups::default()
        },
    });
}

fn bench_static_asset_load(c: &mut Criterion) {
    install_static_project_root();

    c.bench_function("texture_lookup_match", |b| {
        b.iter(|| {
            let hash = black_box(perro_ids::string_to_u64(TEXTURE_PATH));
            black_box(lookup_texture(hash).len())
        })
    });
    c.bench_function("mesh_lookup_match", |b| {
        b.iter(|| {
            let hash = black_box(perro_ids::string_to_u64(MESH_PATH));
            black_box(lookup_mesh(hash).len())
        })
    });
    c.bench_function("audio_lookup_match", |b| {
        b.iter(|| {
            let hash = black_box(perro_ids::string_to_u64(AUDIO_PATH));
            black_box(lookup_audio(hash).len())
        })
    });

    c.bench_function("texture_load_static_vec", |b| {
        b.iter(|| black_box(load_asset(black_box(TEXTURE_PATH)).unwrap()))
    });
    c.bench_function("mesh_load_static_vec", |b| {
        b.iter(|| black_box(load_asset(black_box(MESH_PATH)).unwrap()))
    });
    c.bench_function("audio_load_static_vec", |b| {
        b.iter(|| black_box(load_asset(black_box(AUDIO_PATH)).unwrap()))
    });
}

criterion_group!(benches, bench_static_asset_load);
criterion_main!(benches);
