// Pure std fixture: also runnable with `rustc --edition 2021 --test -O
// perro_website/src/demo_sync.rs -o <temporary-exe>`. No website build needed.
use super::*;
use std::{hint::black_box, time::Instant};

// Exact pre-change build.rs sync/copy algorithm, kept here as a local baseline.
fn baseline_sync_dir(src: &Path, dst: &Path) -> io::Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    baseline_copy_dir(src, dst)
}

fn baseline_copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    for entry in fs::read_dir(src)? {
        let src_path = entry?.path();
        let file_name = src_path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "source has no file name"))?;
        let dst_path = dst.join(file_name);
        if src_path.is_dir() {
            fs::create_dir_all(&dst_path)?;
            baseline_copy_dir(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[test]
#[ignore = "manual release timing probe; run serially"]
fn time_unchanged_demo_bundle_sync() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "perro_demo_sync_perf_{}_{stamp}",
        std::process::id()
    ));
    let src = root.join("src");
    let before = root.join("before");
    let after = root.join("after");
    fs::create_dir_all(&src).expect("fixture directory");
    let bytes = (0..8 * 1024 * 1024usize)
        .map(|index| (index.wrapping_mul(31) ^ (index >> 8)) as u8)
        .collect::<Vec<_>>();
    for index in 0..6 {
        fs::write(src.join(format!("module{index}.wasm")), &bytes).expect("fixture WASM");
    }
    baseline_sync_dir(&src, &before).expect("baseline setup");
    sync_dir(&src, &after).expect("candidate setup");
    let stamp_before = fs::metadata(before.join("module0.wasm"))
        .expect("baseline stat")
        .modified()
        .expect("baseline time");
    let stamp_after = fs::metadata(after.join("module0.wasm"))
        .expect("candidate stat")
        .modified()
        .expect("candidate time");
    let mut baseline_samples = Vec::new();
    let mut candidate_samples = Vec::new();
    for index in 0..20 {
        for candidate in if index % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let start = Instant::now();
            if candidate {
                black_box(sync_dir(&src, &after)).expect("candidate sync");
                candidate_samples.push(start.elapsed().as_nanos());
            } else {
                black_box(baseline_sync_dir(&src, &before)).expect("baseline sync");
                baseline_samples.push(start.elapsed().as_nanos());
            }
        }
    }
    for index in 0..6 {
        let name = format!("module{index}.wasm");
        assert_eq!(
            fs::read(before.join(&name)).expect("baseline output"),
            bytes
        );
        assert_eq!(fs::read(after.join(name)).expect("candidate output"), bytes);
    }
    let baseline_mtime_same = fs::metadata(before.join("module0.wasm"))
        .expect("baseline stat")
        .modified()
        .expect("baseline time")
        == stamp_before;
    let candidate_mtime_same = fs::metadata(after.join("module0.wasm"))
        .expect("candidate stat")
        .modified()
        .expect("candidate time")
        == stamp_after;
    assert!(candidate_mtime_same);
    println!("{{\"case\":\"demo_bundle_sync_baseline\",\"bytes\":50331648,\"samples_ns\":{baseline_samples:?},\"mtime_same\":{baseline_mtime_same}}}");
    println!("{{\"case\":\"demo_bundle_sync_candidate\",\"bytes\":50331648,\"samples_ns\":{candidate_samples:?},\"mtime_same\":{candidate_mtime_same}}}");
    fs::remove_dir_all(root).expect("fixture cleanup");
}
