use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use perro_jobs::{join, par_map, spawn};

fn cpu_work(mut value: u64) -> u64 {
    for _ in 0..256 {
        value = value
            .wrapping_mul(6_364_136_223_846_793_005)
            .rotate_left(13)
            ^ 1;
    }
    value
}

fn bench_spawn(c: &mut Criterion) {
    c.bench_function("perro_jobs/spawn_take", |bench| {
        bench.iter(|| {
            spawn(|| cpu_work(black_box(7)))
                .take()
                .expect("test setup must succeed")
        })
    });
}

fn bench_join(c: &mut Criterion) {
    c.bench_function("perro_jobs/join", |bench| {
        bench.iter(|| join(|| cpu_work(black_box(7)), || cpu_work(black_box(11))))
    });
}

fn bench_par_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("perro_jobs/par_map");
    for count in [256, 4_096] {
        group.throughput(Throughput::Elements(count));
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &count,
            |bench, &count| bench.iter(|| par_map((0..count).collect(), cpu_work)),
        );
    }
    group.finish();
}

criterion_group!(benches, bench_spawn, bench_join, bench_par_map);
criterion_main!(benches);
