use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use perro_io::{ProjectRoot, set_project_root};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const ROW_COUNTS: &[usize] = &[1_000, 10_000, 50_000];
const MAX_PARSE_LOOPS: u64 = 2;

fn csv_loading_bench(c: &mut Criterion) {
    let root = unique_temp_dir("csv_loading_bench");
    let res_dir = root.join("res/data");
    fs::create_dir_all(&res_dir).expect("create bench csv dir");

    let mut files = Vec::new();
    for &rows in ROW_COUNTS {
        let bytes = bench_csv_source(rows).into_bytes();
        let path = res_dir.join(format!("items_{rows}.csv"));
        fs::write(&path, &bytes).expect("write bench csv");
        files.push((rows, path, bytes.len() as u64));
    }

    set_project_root(ProjectRoot::Disk {
        root: root.clone(),
        name: "CsvLoadingBench".to_string(),
    });

    bench_fs_read(c, &files);
    bench_fs_read_parse(c, &files);

    let _ = fs::remove_dir_all(root);
}

fn bench_fs_read(c: &mut Criterion, files: &[(usize, PathBuf, u64)]) {
    let mut group = c.benchmark_group("csv_loading/fs_read_only");
    for (rows, path, bytes) in files {
        group.throughput(Throughput::Bytes(*bytes));
        group.bench_with_input(BenchmarkId::from_parameter(rows), path, |b, path| {
            b.iter(|| black_box(fs::read(black_box(path)).expect("read bench csv")));
        });
    }
    group.finish();
}

fn bench_fs_read_parse(c: &mut Criterion, files: &[(usize, PathBuf, u64)]) {
    let mut group = c.benchmark_group("csv_loading/fs_read_parse_perrocsv");
    group.sample_size(10);
    for (rows, path, bytes) in files {
        group.throughput(Throughput::Bytes(*bytes));
        group.bench_with_input(BenchmarkId::from_parameter(rows), path, |b, path| {
            b.iter_custom(|iters| time_capped_load_parse(path, iters))
        });
    }
    group.finish();
}

fn time_capped_load_parse(path: &Path, iters: u64) -> Duration {
    let loops = iters.clamp(1, MAX_PARSE_LOOPS);
    let start = Instant::now();
    for _ in 0..loops {
        let bytes = fs::read(black_box(path)).expect("read bench csv");
        let csv = perro_csv::parse_csv_static(black_box(&bytes)).expect("parse bench csv");
        black_box(csv.row_count());
    }
    start.elapsed().mul_f64(iters as f64 / loops as f64)
}

fn bench_csv_source(rows: usize) -> String {
    let mut src = String::with_capacity(rows * 64);
    src.push_str("id,name,kind,power,cost,rarity,zone,flag\n");
    for row in 0..rows {
        src.push_str(&format!(
            "item_{row},Name {row},kind_{},{},{},rarity_{},zone_{},{}\n",
            row % 64,
            row % 1000,
            row % 500,
            row % 7,
            row % 32,
            row % 2
        ));
    }
    src
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("perro_{label}_{}_{}", std::process::id(), ts))
}

criterion_group!(benches, csv_loading_bench);
criterion_main!(benches);
