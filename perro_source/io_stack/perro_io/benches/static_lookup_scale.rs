use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

include!(concat!(
    env!("OUT_DIR"),
    "/static_lookup_scale_generated.rs"
));

type LookupFn = fn(u64) -> Option<u32>;

struct LookupFns {
    match_lookup: LookupFn,
    aos_lookup: LookupFn,
    soa_lookup: LookupFn,
    soa_slice_lookup: LookupFn,
    soa_lower_bound_lookup: LookupFn,
}

fn bench_lookup_size(
    c: &mut Criterion,
    size: usize,
    probes: &'static [u64; 4],
    lookups: LookupFns,
) {
    let mut group = c.benchmark_group("static_lookup_scale");

    group.bench_function(BenchmarkId::new("match", size), |b| {
        let mut index = 0usize;
        b.iter(|| {
            let key = probes[index & 3];
            index = index.wrapping_add(1);
            black_box((lookups.match_lookup)(black_box(key)))
        })
    });

    group.bench_function(BenchmarkId::new("aos_hand", size), |b| {
        let mut index = 0usize;
        b.iter(|| {
            let key = probes[index & 3];
            index = index.wrapping_add(1);
            black_box((lookups.aos_lookup)(black_box(key)))
        })
    });

    group.bench_function(BenchmarkId::new("soa_hand", size), |b| {
        let mut index = 0usize;
        b.iter(|| {
            let key = probes[index & 3];
            index = index.wrapping_add(1);
            black_box((lookups.soa_lookup)(black_box(key)))
        })
    });

    group.bench_function(BenchmarkId::new("soa_slice", size), |b| {
        let mut index = 0usize;
        b.iter(|| {
            let key = probes[index & 3];
            index = index.wrapping_add(1);
            black_box((lookups.soa_slice_lookup)(black_box(key)))
        })
    });

    group.bench_function(BenchmarkId::new("soa_lower_bound", size), |b| {
        let mut index = 0usize;
        b.iter(|| {
            let key = probes[index & 3];
            index = index.wrapping_add(1);
            black_box((lookups.soa_lower_bound_lookup)(black_box(key)))
        })
    });

    group.finish();
}

fn bench_static_lookup_scale(c: &mut Criterion) {
    bench_lookup_size(
        c,
        8,
        &PROBES_8,
        LookupFns {
            match_lookup: match_lookup_8,
            aos_lookup: aos_lookup_8,
            soa_lookup: soa_lookup_8,
            soa_slice_lookup: soa_slice_lookup_8,
            soa_lower_bound_lookup: soa_lower_bound_lookup_8,
        },
    );
    bench_lookup_size(
        c,
        9,
        &PROBES_9,
        LookupFns {
            match_lookup: match_lookup_9,
            aos_lookup: aos_lookup_9,
            soa_lookup: soa_lookup_9,
            soa_slice_lookup: soa_slice_lookup_9,
            soa_lower_bound_lookup: soa_lower_bound_lookup_9,
        },
    );
    bench_lookup_size(
        c,
        32,
        &PROBES_32,
        LookupFns {
            match_lookup: match_lookup_32,
            aos_lookup: aos_lookup_32,
            soa_lookup: soa_lookup_32,
            soa_slice_lookup: soa_slice_lookup_32,
            soa_lower_bound_lookup: soa_lower_bound_lookup_32,
        },
    );
    bench_lookup_size(
        c,
        128,
        &PROBES_128,
        LookupFns {
            match_lookup: match_lookup_128,
            aos_lookup: aos_lookup_128,
            soa_lookup: soa_lookup_128,
            soa_slice_lookup: soa_slice_lookup_128,
            soa_lower_bound_lookup: soa_lower_bound_lookup_128,
        },
    );
    bench_lookup_size(
        c,
        512,
        &PROBES_512,
        LookupFns {
            match_lookup: match_lookup_512,
            aos_lookup: aos_lookup_512,
            soa_lookup: soa_lookup_512,
            soa_slice_lookup: soa_slice_lookup_512,
            soa_lower_bound_lookup: soa_lower_bound_lookup_512,
        },
    );
    bench_lookup_size(
        c,
        2048,
        &PROBES_2048,
        LookupFns {
            match_lookup: match_lookup_2048,
            aos_lookup: aos_lookup_2048,
            soa_lookup: soa_lookup_2048,
            soa_slice_lookup: soa_slice_lookup_2048,
            soa_lower_bound_lookup: soa_lower_bound_lookup_2048,
        },
    );
    bench_lookup_size(
        c,
        4096,
        &PROBES_4096,
        LookupFns {
            match_lookup: match_lookup_4096,
            aos_lookup: aos_lookup_4096,
            soa_lookup: soa_lookup_4096,
            soa_slice_lookup: soa_slice_lookup_4096,
            soa_lower_bound_lookup: soa_lower_bound_lookup_4096,
        },
    );
}

criterion_group!(benches, bench_static_lookup_scale);
criterion_main!(benches);
