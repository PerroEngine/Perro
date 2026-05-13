use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_structs::BitMask;

const N: usize = 65_536;

fn make_masks() -> Vec<BitMask> {
    (0..N)
        .map(|i| {
            BitMask::with([
                (i % 32 + 1) as u8,
                ((i * 3) % 32 + 1) as u8,
                ((i * 7) % 32 + 1) as u8,
            ])
        })
        .collect()
}

fn make_layer_sets() -> Vec<[u8; 4]> {
    (0..N)
        .map(|i| {
            [
                (i % 32 + 1) as u8,
                ((i * 5) % 32 + 1) as u8,
                ((i * 11) % 32 + 1) as u8,
                ((i * 17) % 32 + 1) as u8,
            ]
        })
        .collect()
}

fn bench_bitmask_query_ops(c: &mut Criterion) {
    let masks = make_masks();

    c.bench_function("perro_structs/bitmask_query_ops", |bench| {
        bench.iter(|| {
            let mut hits = 0usize;
            let mut bits = 0u32;
            for window in black_box(masks.windows(2)) {
                let a = window[0];
                let b = window[1];
                hits += a.intersects(b) as usize;
                hits += a.contains(a.intersection(b)) as usize;
                bits ^= a.union(b).bits();
            }
            black_box((hits, bits))
        })
    });
}

fn bench_bitmask_build_from_layers(c: &mut Criterion) {
    let layers = make_layer_sets();

    c.bench_function("perro_structs/bitmask_build_from_layers", |bench| {
        bench.iter(|| {
            let mut bits = 0u32;
            for &set in black_box(&layers) {
                bits ^= BitMask::from_layers(set).bits();
            }
            black_box(bits)
        })
    });
}

fn bench_bitmask_push_pop_layers(c: &mut Criterion) {
    let layers = make_layer_sets();

    c.bench_function("perro_structs/bitmask_push_pop_layers", |bench| {
        bench.iter(|| {
            let mut mask = BitMask::NONE;
            let mut bits = 0u32;
            for &set in black_box(&layers) {
                mask.push(set);
                mask.pop([set[1], set[3]]);
                bits ^= mask.bits();
            }
            black_box((mask, bits))
        })
    });
}

fn bench_bitmask_without_layers(c: &mut Criterion) {
    let layers = make_layer_sets();

    c.bench_function("perro_structs/bitmask_without_layers", |bench| {
        bench.iter(|| {
            let mut bits = 0u32;
            for &set in black_box(&layers) {
                bits ^= BitMask::without(set).bits();
            }
            black_box(bits)
        })
    });
}

criterion_group!(
    benches,
    bench_bitmask_query_ops,
    bench_bitmask_build_from_layers,
    bench_bitmask_push_pop_layers,
    bench_bitmask_without_layers
);
criterion_main!(benches);
