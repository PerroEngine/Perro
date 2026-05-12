use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::SignalAPI;
use perro_variant::Variant;

fn bench_signal_emit_release_matrix(c: &mut Criterion) {
    let method = ScriptMemberID::from_string("on_signal");
    let emit_params = [Variant::from(7_i32), Variant::from(11_i32)];
    let connect_params = [Variant::from(13_i32), Variant::from(17_i32)];

    let mut group = c.benchmark_group("signal/emit_matrix");

    group.bench_function("miss_empty_registry", |b| {
        let mut runtime = Runtime::new();
        let signal = SignalID::from_string("missing");
        b.iter(|| black_box(SignalAPI::signal_emit(&mut runtime, signal, &[])))
    });

    group.bench_function("hit_1_no_params", |b| {
        let mut runtime = Runtime::new();
        perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, NodeID::new(1));
        let signal = SignalID::from_string("single_no_params");
        assert!(SignalAPI::signal_connect(
            &mut runtime,
            NodeID::new(1),
            signal,
            method,
            &[],
        ));
        b.iter(|| black_box(SignalAPI::signal_emit(&mut runtime, signal, &[])))
    });

    group.bench_function("hit_1_emit_plus_connect_params", |b| {
        let mut runtime = Runtime::new();
        perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, NodeID::new(1));
        let signal = SignalID::from_string("single_emit_connect_params");
        assert!(SignalAPI::signal_connect(
            &mut runtime,
            NodeID::new(1),
            signal,
            method,
            &connect_params,
        ));
        b.iter(|| black_box(SignalAPI::signal_emit(&mut runtime, signal, &emit_params)))
    });

    group.bench_function("hit_4_emit_plus_connect_params", |b| {
        let mut runtime = Runtime::new();
        let signal = SignalID::from_string("four_emit_connect_params");
        for i in 0..4 {
            let id = NodeID::new(i + 1);
            perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, id);
            assert!(SignalAPI::signal_connect(
                &mut runtime,
                id,
                signal,
                method,
                &connect_params,
            ));
        }
        b.iter(|| black_box(SignalAPI::signal_emit(&mut runtime, signal, &emit_params)))
    });

    group.bench_function("hit_1_among_8192_signals", |b| {
        let mut runtime = Runtime::new();
        perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, NodeID::new(1));
        let mut signals = Vec::with_capacity(8_192);
        for i in 0..8_192 {
            let signal = SignalID::from_u64(0xCAFE_0000_0000_0000_u64 | i as u64);
            signals.push(signal);
            assert!(SignalAPI::signal_connect(
                &mut runtime,
                NodeID::new(1),
                signal,
                method,
                &[],
            ));
        }
        let signal = signals[signals.len() - 1];
        b.iter(|| black_box(SignalAPI::signal_emit(&mut runtime, signal, &[])))
    });

    group.bench_function("miss_among_8192_signals", |b| {
        let mut runtime = Runtime::new();
        perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, NodeID::new(1));
        for i in 0..8_192 {
            let signal = SignalID::from_u64(0xBEEF_0000_0000_0000_u64 | i as u64);
            assert!(SignalAPI::signal_connect(
                &mut runtime,
                NodeID::new(1),
                signal,
                method,
                &[],
            ));
        }
        let signal = SignalID::from_u64(0xDEAD_F00D);
        b.iter(|| black_box(SignalAPI::signal_emit(&mut runtime, signal, &[])))
    });

    group.bench_function("batch_1024_distinct_signals", |b| {
        let mut runtime = Runtime::new();
        perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, NodeID::new(1));
        let mut signals = Vec::with_capacity(1024);
        for i in 0..1024 {
            let signal = SignalID::from_u64(0xFA57_0000_0000_0000_u64 | i as u64);
            signals.push(signal);
            assert!(SignalAPI::signal_connect(
                &mut runtime,
                NodeID::new(1),
                signal,
                method,
                &[],
            ));
        }
        b.iter(|| {
            let mut calls = 0usize;
            for &signal in &signals {
                calls += SignalAPI::signal_emit(&mut runtime, signal, &[]);
            }
            black_box(calls)
        })
    });

    group.bench_function("frame_1000_distinct_signals", |b| {
        let mut runtime = Runtime::new();
        perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, NodeID::new(1));
        let mut signals = Vec::with_capacity(1000);
        for i in 0..1000 {
            let signal = SignalID::from_u64(0xF000_0000_0000_0000_u64 | i as u64);
            signals.push(signal);
            assert!(SignalAPI::signal_connect(
                &mut runtime,
                NodeID::new(1),
                signal,
                method,
                &[],
            ));
        }
        b.iter(|| {
            let mut calls = 0usize;
            for &signal in &signals {
                calls += SignalAPI::signal_emit(&mut runtime, signal, &[]);
            }
            black_box(calls)
        })
    });

    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_signal_emit_release_matrix(c);
}

criterion_group! {
    name = signal_hotpaths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(signal_hotpaths);
