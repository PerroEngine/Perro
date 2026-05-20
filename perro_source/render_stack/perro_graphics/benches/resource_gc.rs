use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use perro_graphics::ResourceStore;
use perro_render_bridge::Material3D;

fn store_with_used_resources(count: usize) -> ResourceStore {
    let mut store = ResourceStore::new();
    for i in 0..count {
        let texture = store.create_texture(&format!("res://bench/textures/{i}.png"), false);
        let mesh = store.create_mesh(&format!("res://bench/meshes/{i}.glb"), false);
        let material = store.create_material(
            Material3D::default(),
            Some(&format!("res://bench/materials/{i}.pmat")),
            false,
        );
        store.mark_texture_used(texture);
        store.mark_mesh_used(mesh);
        store.mark_material_used(material);
    }
    store
}

fn store_with_reserved_resources(count: usize) -> ResourceStore {
    let mut store = ResourceStore::new();
    for i in 0..count {
        store.create_texture(&format!("res://bench/textures/{i}.png"), true);
        store.create_mesh(&format!("res://bench/meshes/{i}.glb"), true);
        store.create_material(
            Material3D::default(),
            Some(&format!("res://bench/materials/{i}.pmat")),
            true,
        );
    }
    store
}

fn bench_resource_gc(c: &mut Criterion) {
    let mut group = c.benchmark_group("resource_gc");
    for count in [100_usize, 128, 1_024, 8_192] {
        group.throughput(Throughput::Elements((count * 3) as u64));
        group.bench_with_input(
            BenchmarkId::new("scan_reserved_keep", count),
            &count,
            |b, &count| {
                let mut store = store_with_reserved_resources(count);
                b.iter(|| black_box(store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES)));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("scan_candidates_keep", count),
            &count,
            |b, &count| {
                let mut store = store_with_used_resources(count);
                store.reset_ref_counts();
                b.iter(|| black_box(store.gc_unused(u32::MAX)));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("drop_all_ttl_1_with_setup", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || store_with_used_resources(count),
                    |mut store| {
                        store.reset_ref_counts();
                        black_box(store.gc_unused(1))
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_resource_gc);
criterion_main!(benches);
