//! Baseline-compatible export probes. Copy this file, tests/support/mod.rs and
//! its Cargo target/dev dependency into the baseline checkout unchanged.
//! --case=material|archive|textures|shader|codegen|codegen_small
//! --samples=10 --threads=4 [--memory] [--first-call]
//! Time includes generation, output writes and temporary result destruction;
//! excludes input creation, cold-cache reset, output verification and fixture drop.
//! --first-call emits one sample without prewarming. Repeat the whole process
//! for independent first-call samples; this includes lazy executable hashing,
//! but excludes process/thread-pool startup and input fixture construction.
#[path = "../tests/support/mod.rs"]
mod support;

use perro_static_pipeline::{
    generate_static_animations, generate_static_csvs, generate_static_materials,
    generate_static_scenes, generate_static_textures,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicIsize, Ordering},
    time::Instant,
};
use support::{Fixture, outputs};

struct CountingAllocator;
static TRACK: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);

fn account(delta: isize) {
    if TRACK.load(Ordering::Relaxed) {
        let live = LIVE.fetch_add(delta, Ordering::Relaxed) + delta;
        PEAK.fetch_max(live, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            account(layout.size() as isize);
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            account(layout.size() as isize);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        account(-(layout.size() as isize));
        unsafe {
            System.dealloc(ptr, layout);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let out = unsafe { System.realloc(ptr, layout, size) };
        if !out.is_null() {
            account(size as isize - layout.size() as isize);
        }
        out
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str| args.iter().find_map(|arg| arg.strip_prefix(name));
    let case = value("--case=").unwrap_or("material");
    let samples: usize = value("--samples=")
        .unwrap_or("10")
        .parse()
        .expect("sample count");
    let threads: usize = value("--threads=")
        .unwrap_or("4")
        .parse()
        .expect("worker count");
    let memory = args.iter().any(|arg| arg == "--memory");
    let first_call = args.iter().any(|arg| arg == "--first-call");
    let samples = if first_call { 1 } else { samples };
    TRACK.store(memory, Ordering::Relaxed);
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .expect("worker pool");
    let fixture = Fixture::new();
    match case {
        "codegen" | "codegen_small" => {
            let small = case == "codegen_small";
            let mut scene =
                "$root = @Root\n[Root]\n[Node3D]\nposition = (1,2,3)\n[/Node3D]\n[/Root]\n"
                    .to_string();
            for i in 0..if small { 0 } else { 25 } {
                scene.push_str(&format!("[Child{i}]\nparent = @Root\n[Node3D]\nposition = ({i},0,0)\n[/Node3D]\n[/Child{i}]\n"));
            }
            let mut animation = "[Animation]\nname = \"Clip\"\nfps = 24\n[/Animation]\n[Objects]\n@Hero = Node3D\n[/Objects]\n".to_string();
            for i in 0..if small { 1 } else { 64 } {
                animation.push_str(&format!(
                    "[Frame{i}]\n@Hero {{ position = ({i},0,0) }}\n[/Frame{i}]\n"
                ));
            }
            for i in 0..if small { 1 } else { 100 } {
                fixture.write(&format!("scene{i}.scn"), scene.as_bytes());
                fixture.write(&format!("clip{i}.panim"), animation.as_bytes());
            }
            let mut csv = "key,value\n".to_string();
            for i in 0..if small { 1 } else { 25_000 } {
                csv.push_str(&format!("key{i},value{i}\n"));
            }
            fixture.write("data.csv", csv.as_bytes());
        }
        "material" => {
            fixture.image("texture.png", 1024);
            for i in 0..8 {
                fixture.model(&format!("mesh{i}.gltf"), "texture.png");
            }
        }
        "archive" => {
            let mut data = vec![0u8; 4 * 1024 * 1024];
            let mut state = 0x1234_5678u32;
            for word in data.chunks_exact_mut(4) {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                word.copy_from_slice(&state.to_le_bytes());
            }
            for i in 0..8 {
                fixture.write(&format!("data{i}.bin"), &data);
            }
        }
        "textures" => {
            fixture.image("texture0.png", 512);
            let png = std::fs::read(fixture.0.join("res/texture0.png")).expect("source PNG");
            for i in 1..64 {
                fixture.write(&format!("texture{i}.png"), &png);
            }
        }
        "shader" => {
            fixture.write("bake.wgsl", b"fn shade_material(in: FragmentInput) -> vec4<f32> { return vec4<f32>(in.uv, 0.25, 1.0); }\nfn bake_texture(in: BakeInput) -> vec4<f32> { return vec4<f32>(in.uv, 0.25, 1.0); }");
            for i in 0..10 {
                fixture.write(&format!("material{i}.pmat"), b"type = \"custom\"\nshader_path = \"res://bake.wgsl\"\nrelease_bake = true\nbake_resolution = (32, 32)\n");
            }
        }
        _ => panic!("unknown case"),
    }
    let tree = fixture.tree();
    let archive = fixture.0.join("assets.perro");
    let run = || match case {
        "codegen" | "codegen_small" => {
            generate_static_scenes(&fixture.0, &tree).expect("scene export");
            generate_static_animations(&fixture.0, &tree).expect("animation export");
            generate_static_csvs(&fixture.0).expect("CSV export");
        }
        "material" => generate_static_materials(&fixture.0, &tree).expect("material export"),
        "archive" => perro_assets::packer::build_perro_assets_archive(
            &archive,
            &fixture.0.join("res"),
            &fixture.0,
            &[],
        )
        .expect("archive export"),
        "textures" | "shader" => {
            generate_static_textures(&fixture.0, &tree).expect("texture export")
        }
        _ => unreachable!(),
    };
    if !first_call {
        run();
    }
    let mut nanos = Vec::with_capacity(samples);
    let mut peak_bytes = Vec::with_capacity(samples);
    for _ in 0..samples {
        if matches!(case, "textures" | "shader") {
            fixture.clear_outputs();
        }
        let base = LIVE.load(Ordering::Relaxed);
        PEAK.store(base, Ordering::Relaxed);
        let start = Instant::now();
        run();
        let elapsed = start.elapsed().as_nanos();
        let peak = PEAK.load(Ordering::Relaxed) - base;
        nanos.push(elapsed);
        peak_bytes.push(peak);
    }
    let mut checksum = 0xcbf29ce484222325u64;
    let data = if case == "archive" {
        std::collections::BTreeMap::from([(
            "assets.perro".to_string(),
            std::fs::read(archive).expect("verify archive"),
        )])
    } else {
        outputs(&fixture.0.join(".perro"))
    };
    assert!(!data.is_empty());
    if matches!(case, "codegen" | "codegen_small") {
        assert_eq!(data.len(), 3, "scene, animation and CSV codegen outputs");
        for name in ["scenes.rs", "animations.rs", "csvs.rs"] {
            let (_, bytes) = data
                .iter()
                .find(|(path, _)| path.ends_with(&format!("/{name}")))
                .expect("generated codegen output");
            assert!(bytes.starts_with(b"// Auto-generated by Perro Static Pipeline."));
        }
    }
    for (name, bytes) in &data {
        for byte in name.bytes().chain(bytes.iter().copied()) {
            checksum = (checksum ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    println!(
        "{{\"case\":\"{case}\",\"threads\":{threads},\"first_call\":{first_call},\"memory_instrumented\":{memory},\"samples_ns\":{nanos:?},\"peak_requested_bytes\":{peak_bytes:?},\"output_files\":{},\"output_checksum\":\"{checksum:016x}\"}}",
        data.len()
    );
}
