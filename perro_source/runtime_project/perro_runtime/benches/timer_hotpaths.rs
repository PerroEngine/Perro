use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use perro_ids::{SignalID, TimerID};
use perro_runtime::Runtime;
use std::time::Duration;

fn fill(runtime: &mut Runtime, count: usize, duration: Duration) {
    for index in 0..count {
        runtime.bench_timer_start(
            TimerID::from_u64(index as u64 + 1),
            SignalID::from_u64(index as u64 + 1),
            duration,
        );
    }
}

fn bench_timer_hotpaths(c: &mut Criterion) {
    let mut idle = c.benchmark_group("timers_idle_tick");
    for count in [1_000usize, 10_000, 100_000] {
        let mut runtime = Runtime::new();
        fill(&mut runtime, count, Duration::from_secs(60));
        idle.throughput(Throughput::Elements(count as u64));
        idle.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| black_box(runtime.bench_timer_advance(1.0 / 60.0)))
        });
    }
    idle.finish();

    let mut expire = c.benchmark_group("timers_same_frame_expiry");
    for count in [1_000usize, 10_000, 100_000] {
        expire.throughput(Throughput::Elements(count as u64));
        expire.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched(
                || {
                    let mut runtime = Runtime::new();
                    fill(&mut runtime, count, Duration::from_millis(1));
                    runtime
                },
                |mut runtime| black_box(runtime.bench_timer_advance(0.001)),
                BatchSize::LargeInput,
            )
        });
    }
    expire.finish();

    let mut resets = c.benchmark_group("timers_reset_storm");
    for count in [1_000usize, 10_000, 100_000] {
        resets.throughput(Throughput::Elements(count as u64));
        resets.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched(
                Runtime::new,
                |mut runtime| {
                    for index in 0..count {
                        runtime.bench_timer_start(
                            TimerID::from_u64(1),
                            SignalID::from_u64(1),
                            Duration::from_nanos(index as u64 + 1),
                        );
                    }
                    black_box(runtime.bench_timer_counts())
                },
                BatchSize::LargeInput,
            )
        });
    }
    resets.finish();
}

criterion_group!(benches, bench_timer_hotpaths);
criterion_main!(benches);
