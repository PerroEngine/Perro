//! Editor helper A/B probe.
//!
//! Run from repo root:
//! `rustc tools/editor_perf_probe.rs -O -o target/editor_perf_probe.exe`
//! `target/editor_perf_probe.exe`

use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

mod current_files {
    include!("../perro_editor/res/scripts/assets/editor_files.rs");
}
mod current_watch {
    include!("../perro_editor/res/scripts/assets/editor_file_watch.rs");
}

fn old_changed_paths(before: &[String], after: &[String]) -> Vec<String> {
    let before = before
        .iter()
        .filter_map(|sig| sig.split('|').next().map(|path| (path, sig.as_str())))
        .collect::<BTreeMap<_, _>>();
    let after = after
        .iter()
        .filter_map(|sig| sig.split('|').next().map(|path| (path, sig.as_str())))
        .collect::<BTreeMap<_, _>>();
    let mut out = Vec::new();
    for (path, sig) in &after {
        if before.get(path) != Some(sig) {
            out.push((*path).to_string());
        }
    }
    for path in before.keys() {
        if !after.contains_key(path) {
            out.push((*path).to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn old_sort_key(path: &str) -> String {
    let label = path
        .trim_start_matches("res://")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    let folder = path.ends_with('/');
    let mut key = String::new();
    for (idx, part) in label.split('/').filter(|part| !part.is_empty()).enumerate() {
        if idx > 0 {
            key.push('/');
        }
        key.push_str(part);
        if folder && idx == label.trim_end_matches('/').matches('/').count() {
            key.push('\0');
        } else {
            key.push('\x01');
        }
    }
    key
}

fn signatures(count: usize) -> Vec<String> {
    let mut out = (0..count)
        .map(|idx| {
            let dir = idx / 100;
            format!(
                "res/dir_{dir:04}/asset_{idx:06}.scn|{}|{}|0",
                idx + 20,
                idx + 1
            )
        })
        .collect::<Vec<_>>();
    out.sort();
    out
}

fn changed_fixture(base: &[String]) -> Vec<String> {
    let mut out = base.to_vec();
    for idx in (0..out.len()).step_by(100) {
        let path = out[idx].split('|').next().unwrap_or_default();
        out[idx] = format!("{path}|999|999|0");
    }
    out.sort();
    out
}

fn median_ms(samples: &[u128]) -> f64 {
    let mut samples = samples.to_vec();
    samples.sort_unstable();
    samples[samples.len() / 2] as f64 / 1_000_000.0
}

fn bench_diff(count: usize, changed: bool) {
    let before = signatures(count);
    let after = if changed {
        changed_fixture(&before)
    } else {
        before.clone()
    };
    assert_eq!(
        old_changed_paths(&before, &after),
        current_watch::changed_paths(&before, &after)
    );
    let mut old_samples = Vec::new();
    let mut new_samples = Vec::new();
    for _ in 0..18 {
        let start = Instant::now();
        black_box(old_changed_paths(black_box(&before), black_box(&after)));
        old_samples.push(start.elapsed().as_nanos());
        let start = Instant::now();
        black_box(current_watch::changed_paths(
            black_box(&before),
            black_box(&after),
        ));
        new_samples.push(start.elapsed().as_nanos());
    }
    println!(
        "diff {count} {} old={:.3}ms new={:.3}ms ratio={:.2}x",
        if changed { "1% changed" } else { "unchanged" },
        median_ms(&old_samples),
        median_ms(&new_samples),
        median_ms(&old_samples) / median_ms(&new_samples),
    );
}

fn bench_sort(count: usize) {
    let paths = (0..count)
        .map(|idx| {
            if idx % 100 == 0 {
                format!("res://dir_{:04}/", idx / 100)
            } else {
                format!("res://dir_{:04}/asset_{idx:06}.scn", idx / 100)
            }
        })
        .collect::<Vec<_>>();
    let mut new_paths = paths.clone();
    new_paths.sort_by_cached_key(|path| current_files::res_browser_sort_key(path));
    for (idx, path) in new_paths.iter().enumerate() {
        if path.ends_with('/') {
            assert!(new_paths[idx + 1..]
                .iter()
                .take_while(|child| child.starts_with(path.trim_end_matches('/')))
                .all(|child| child.starts_with(path.trim_end_matches('/'))));
        }
    }
    let mut old_samples = Vec::new();
    let mut new_samples = Vec::new();
    for _ in 0..18 {
        let mut old_input = paths.clone();
        let start = Instant::now();
        old_input.sort_by_cached_key(|path| old_sort_key(path));
        old_samples.push(start.elapsed().as_nanos());
        black_box(old_input);
        let mut new_input = paths.clone();
        let start = Instant::now();
        new_input.sort_by_cached_key(|path| current_files::res_browser_sort_key(path));
        new_samples.push(start.elapsed().as_nanos());
        black_box(new_input);
    }
    println!(
        "sort {count} old={:.3}ms new={:.3}ms ratio={:.2}x",
        median_ms(&old_samples),
        median_ms(&new_samples),
        median_ms(&old_samples) / median_ms(&new_samples),
    );
}

fn main() {
    bench_diff(10_000, false);
    bench_diff(10_000, true);
    bench_diff(50_000, false);
    bench_diff(50_000, true);
    bench_sort(10_000);
    bench_sort(50_000);
}
