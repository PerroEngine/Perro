use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_variant::{DeriveVariant, Variant};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

fn collect_borrowed(map: &HashMap<String, i32>) -> Variant {
    Variant::Object(
        map.iter()
            .map(|(key, value)| (Arc::<str>::from(key.as_str()), Variant::from(*value)))
            .collect(),
    )
}

fn collect_owned(map: HashMap<String, i32>) -> Variant {
    Variant::Object(
        map.into_iter()
            .map(|(key, value)| (Arc::<str>::from(key), Variant::from(value)))
            .collect(),
    )
}

fn bench_hashmap_variant(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_variant");
    for count in [8usize, 128, 4096] {
        let source: HashMap<String, i32> = (0..count)
            .map(|i| (format!("field_{i:04}"), i as i32))
            .collect();
        let ordered: BTreeMap<String, i32> = source.iter().map(|(k, v)| (k.clone(), *v)).collect();

        assert_eq!(source.to_variant(), collect_borrowed(&source));
        assert_eq!(source.clone().into_variant(), collect_owned(source.clone()));

        group.bench_function(BenchmarkId::new("borrowed_insert", count), |b| {
            b.iter(|| black_box(black_box(&source).to_variant()))
        });
        group.bench_function(BenchmarkId::new("borrowed_collect", count), |b| {
            b.iter(|| black_box(collect_borrowed(black_box(&source))))
        });
        group.bench_function(BenchmarkId::new("owned_insert", count), |b| {
            b.iter_batched(
                || source.clone(),
                |map| black_box(black_box(map).into_variant()),
                BatchSize::SmallInput,
            )
        });
        group.bench_function(BenchmarkId::new("owned_collect", count), |b| {
            b.iter_batched(
                || source.clone(),
                |map| black_box(collect_owned(black_box(map))),
                BatchSize::SmallInput,
            )
        });
        group.bench_function(BenchmarkId::new("btree_control", count), |b| {
            b.iter(|| black_box(black_box(&ordered).to_variant()))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_hashmap_variant);
criterion_main!(benches);
