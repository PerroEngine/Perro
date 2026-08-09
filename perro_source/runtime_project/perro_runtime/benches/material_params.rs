//! Cost of animating custom material params from script at frame rate.
//!
//! The scripting model leans on per-frame param writes being cheap. The backend
//! already guards against pipeline churn (only shape changes rebuild), so what
//! is left to measure is the runtime-side cost of the read-modify-write cycle a
//! script actually performs: `get_material_data` deep-clones the material,
//! the script mutates one param, `write_material_data` re-queues it.
//!
//! `param_count` is swept because the clone is proportional to it.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use perro_runtime::BenchSceneSpawner;
use std::time::Duration;

/// Params on the material being animated.
const PARAM_COUNTS: [usize; 4] = [1, 8, 32, 128];
/// Distinct materials written per frame, i.e. how many entities animate.
const WRITES_PER_FRAME: [usize; 4] = [1, 16, 256, 2048];

fn bench_param_write_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("material_param_write_cycle");
    group.measurement_time(Duration::from_secs(5));

    for param_count in PARAM_COUNTS {
        for writes in WRITES_PER_FRAME {
            group.throughput(Throughput::Elements(writes as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("params_{param_count}"), writes),
                &(param_count, writes),
                |b, &(param_count, writes)| {
                    let mut spawner = BenchSceneSpawner::new();
                    let id = spawner.bench_create_custom_material(param_count);
                    // Drop the create command so the queue starts empty.
                    let _ = spawner.bench_drain_queued_commands();
                    b.iter(|| {
                        let ok = spawner.bench_material_param_write_cycle(black_box(id), writes);
                        // A frame boundary drains the queue; without this the
                        // bench would measure unbounded queue growth instead.
                        let drained = spawner.bench_drain_queued_commands();
                        black_box((ok, drained))
                    });
                },
            );
        }
    }
    group.finish();
}

/// Same sweep through the narrow setter, to show the param-count term go away.
fn bench_set_param(c: &mut Criterion) {
    let mut group = c.benchmark_group("material_set_param");
    group.measurement_time(Duration::from_secs(5));

    for param_count in PARAM_COUNTS {
        for writes in WRITES_PER_FRAME {
            group.throughput(Throughput::Elements(writes as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("params_{param_count}"), writes),
                &(param_count, writes),
                |b, &(param_count, writes)| {
                    let mut spawner = BenchSceneSpawner::new();
                    let id = spawner.bench_create_custom_material(param_count);
                    let _ = spawner.bench_drain_queued_commands();
                    b.iter(|| {
                        let ok = spawner.bench_material_set_param(black_box(id), writes);
                        let drained = spawner.bench_drain_queued_commands();
                        black_box((ok, drained))
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_param_write_cycle, bench_set_param);
criterion_main!(benches);
