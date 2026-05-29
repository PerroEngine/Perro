use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

include!(concat!(
    env!("OUT_DIR"),
    "/static_lookup_scale_generated.rs"
));

#[inline(never)]
fn binary_lookup(table: &[(u64, u32)], key: u64) -> Option<u32> {
    table
        .binary_search_by_key(&key, |(candidate, _)| *candidate)
        .ok()
        .map(|index| table[index].1)
}

fn bench_lookup_size(
    c: &mut Criterion,
    size: usize,
    probes: &'static [u64; 4],
    table: &'static [(u64, u32)],
    match_lookup: fn(u64) -> Option<u32>,
) {
    let mut group = c.benchmark_group("static_lookup_scale");

    group.bench_function(BenchmarkId::new("match", size), |b| {
        let mut index = 0usize;
        b.iter(|| {
            let key = probes[index & 3];
            index = index.wrapping_add(1);
            black_box(match_lookup(black_box(key)))
        })
    });

    group.bench_function(BenchmarkId::new("binary_search", size), |b| {
        let mut index = 0usize;
        b.iter(|| {
            let key = probes[index & 3];
            index = index.wrapping_add(1);
            black_box(binary_lookup(black_box(table), black_box(key)))
        })
    });

    group.finish();
}

fn bench_static_lookup_scale(c: &mut Criterion) {
    bench_lookup_size(c, 32, &PROBES_32, &STATIC_TABLE_32, match_lookup_32);
    bench_lookup_size(c, 128, &PROBES_128, &STATIC_TABLE_128, match_lookup_128);
    bench_lookup_size(c, 512, &PROBES_512, &STATIC_TABLE_512, match_lookup_512);
    bench_lookup_size(c, 2048, &PROBES_2048, &STATIC_TABLE_2048, match_lookup_2048);
    bench_lookup_size(c, 4096, &PROBES_4096, &STATIC_TABLE_4096, match_lookup_4096);
}

criterion_group!(benches, bench_static_lookup_scale);
criterion_main!(benches);
