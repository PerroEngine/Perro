use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_nodes::{IKTarget2D, PhysicsBoneChain2D};
use perro_runtime::Runtime;
use perro_runtime::api::scripts::{bench_insert_state_script, bench_with_active_script};
use perro_runtime_api::sub_apis::{NodeAPI, SignalAPI};
use perro_variant::Variant;

fn teardown_fixture(
    signal_count: usize,
    target_signal_count: usize,
    methods_per_signal: usize,
) -> (Runtime, NodeID) {
    let mut runtime = Runtime::new();
    let other = NodeID::new(1);
    let target = NodeID::new(2);
    let other_method = ScriptMemberID::from_string("other");
    let target_methods = [
        "target_0", "target_1", "target_2", "target_3", "target_4", "target_5", "target_6",
        "target_7",
    ]
    .map(ScriptMemberID::from_string);
    for index in 0..signal_count {
        let signal = SignalID::from_u64(0x5100_0000_0000_0000 | index as u64);
        assert!(SignalAPI::signal_connect(
            &mut runtime,
            other,
            signal,
            other_method,
            &[],
        ));
        if index < target_signal_count {
            for method in target_methods.iter().take(methods_per_signal).copied() {
                assert!(SignalAPI::signal_connect(
                    &mut runtime,
                    target,
                    signal,
                    method,
                    &[],
                ));
            }
        }
    }
    perro_runtime::api::signals::bench_reset_signal_disconnect_counters(&mut runtime);
    (runtime, target)
}

fn bench_signal_script_teardown(c: &mut Criterion) {
    let mut group = c.benchmark_group("signal/script_teardown_8192_signals");
    for (name, target_signals, methods) in [
        ("no_connections", 0, 1),
        ("32_signals_1_method", 32, 1),
        ("32_signals_8_methods", 32, 8),
    ] {
        group.bench_function(name, |b| {
            b.iter_batched_ref(
                || teardown_fixture(8_192, target_signals, methods),
                |(runtime, target)| {
                    let removed = perro_runtime::api::signals::bench_disconnect_signal_script(
                        runtime, *target,
                    );
                    black_box(removed);
                    black_box(
                        perro_runtime::api::signals::bench_signal_disconnect_counters(runtime),
                    );
                },
                criterion::BatchSize::LargeInput,
            )
        });
    }
    group.finish();

    let mut runtime = Runtime::new();
    let target = NodeID::new(1);
    let signal = SignalID::from_string("connection_churn");
    let method = ScriptMemberID::from_string("handle");
    c.bench_function("signal/connect_disconnect_churn", |b| {
        b.iter(|| {
            assert!(SignalAPI::signal_connect(
                &mut runtime,
                target,
                signal,
                method,
                &[],
            ));
            assert!(SignalAPI::signal_disconnect(
                &mut runtime,
                target,
                signal,
                method,
            ));
        })
    });
}

fn bench_signal_connect(c: &mut Criterion) {
    let signal = SignalID::from_string("connect_target");
    let first_id = NodeID::new(1);
    let second_id = NodeID::new(2);
    let first_method = ScriptMemberID::from_string("first");
    let second_method = ScriptMemberID::from_string("second");
    let connect_params = [Variant::from(13_i32), Variant::from(17_i32)];
    let mut group = c.benchmark_group("signal/connect");

    group.bench_function("new_signal_empty_params", |b| {
        b.iter_batched_ref(
            Runtime::new,
            |runtime| {
                black_box(SignalAPI::signal_connect(
                    runtime,
                    first_id,
                    signal,
                    first_method,
                    &[],
                ));
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.bench_function("existing_signal_second_script_empty_params", |b| {
        b.iter_batched_ref(
            || {
                let mut runtime = Runtime::new();
                assert!(SignalAPI::signal_connect(
                    &mut runtime,
                    first_id,
                    signal,
                    first_method,
                    &[],
                ));
                runtime
            },
            |runtime| {
                black_box(SignalAPI::signal_connect(
                    runtime,
                    second_id,
                    signal,
                    first_method,
                    &[],
                ));
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.bench_function("existing_signal_second_method_params_2", |b| {
        b.iter_batched_ref(
            || {
                let mut runtime = Runtime::new();
                assert!(SignalAPI::signal_connect(
                    &mut runtime,
                    first_id,
                    signal,
                    first_method,
                    &[],
                ));
                runtime
            },
            |runtime| {
                black_box(SignalAPI::signal_connect(
                    runtime,
                    first_id,
                    signal,
                    second_method,
                    &connect_params,
                ));
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.bench_function("duplicate", |b| {
        let mut runtime = Runtime::new();
        assert!(SignalAPI::signal_connect(
            &mut runtime,
            first_id,
            signal,
            first_method,
            &[],
        ));
        b.iter(|| {
            black_box(SignalAPI::signal_connect(
                &mut runtime,
                first_id,
                signal,
                first_method,
                &[],
            ));
        })
    });

    group.bench_function("warm_16_add_one", |b| {
        b.iter_batched_ref(
            || {
                let mut runtime = Runtime::new();
                for index in 0..=16 {
                    let id = NodeID::new(index + 1);
                    perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, id);
                    if index < 16 {
                        assert!(SignalAPI::signal_connect(
                            &mut runtime,
                            id,
                            signal,
                            first_method,
                            &[],
                        ));
                    }
                }
                assert_eq!(SignalAPI::signal_emit(&mut runtime, signal, &[]), 16);
                runtime
            },
            |runtime| {
                black_box(SignalAPI::signal_connect(
                    runtime,
                    NodeID::new(17),
                    signal,
                    first_method,
                    &[],
                ));
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.bench_function("warm_16_add_one_emit", |b| {
        b.iter_batched_ref(
            || {
                let mut runtime = Runtime::new();
                for index in 0..=16 {
                    let id = NodeID::new(index + 1);
                    perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, id);
                    if index < 16 {
                        assert!(SignalAPI::signal_connect(
                            &mut runtime,
                            id,
                            signal,
                            first_method,
                            &[],
                        ));
                    }
                }
                assert_eq!(SignalAPI::signal_emit(&mut runtime, signal, &[]), 16);
                runtime
            },
            |runtime| {
                black_box(SignalAPI::signal_connect(
                    runtime,
                    NodeID::new(17),
                    signal,
                    first_method,
                    &[],
                ));
                black_box(SignalAPI::signal_emit(runtime, signal, &[]));
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.finish();
}

fn bench_internal_schedule_snapshots(c: &mut Criterion) {
    let mut group = c.benchmark_group("internal_schedule/full_pass");
    for count in [1_000, 10_000] {
        let mut update_runtime = Runtime::new();
        for _ in 0..count {
            let _ = NodeAPI::create::<IKTarget2D>(&mut update_runtime);
        }
        update_runtime.update(1.0 / 60.0);
        update_runtime.bench_reset_internal_schedule_snapshot_copies();
        group.bench_function(format!("update_stable_{count}"), |b| {
            b.iter(|| {
                update_runtime.update(black_box(1.0 / 60.0));
                black_box(update_runtime.bench_internal_schedule_snapshot_copies());
            })
        });

        let mut update_churn_runtime = Runtime::new();
        for _ in 0..count {
            let _ = NodeAPI::create::<IKTarget2D>(&mut update_churn_runtime);
        }
        update_churn_runtime.update(1.0 / 60.0);
        update_churn_runtime.bench_reset_internal_schedule_snapshot_copies();
        group.bench_function(format!("update_churn_{count}"), |b| {
            b.iter(|| {
                let added = NodeAPI::create::<IKTarget2D>(&mut update_churn_runtime);
                update_churn_runtime.update(black_box(1.0 / 60.0));
                assert!(NodeAPI::remove_node(&mut update_churn_runtime, added));
                black_box(update_churn_runtime.bench_internal_schedule_snapshot_copies());
            })
        });

        let mut fixed_runtime = Runtime::new();
        for _ in 0..count {
            let _ = NodeAPI::create::<PhysicsBoneChain2D>(&mut fixed_runtime);
        }
        fixed_runtime.fixed_update(1.0 / 60.0);
        fixed_runtime.bench_reset_internal_schedule_snapshot_copies();
        group.bench_function(format!("fixed_stable_{count}"), |b| {
            b.iter(|| {
                fixed_runtime.fixed_update(black_box(1.0 / 60.0));
                black_box(fixed_runtime.bench_internal_schedule_snapshot_copies());
            })
        });

        let mut fixed_churn_runtime = Runtime::new();
        for _ in 0..count {
            let _ = NodeAPI::create::<PhysicsBoneChain2D>(&mut fixed_churn_runtime);
        }
        fixed_churn_runtime.fixed_update(1.0 / 60.0);
        fixed_churn_runtime.bench_reset_internal_schedule_snapshot_copies();
        group.bench_function(format!("fixed_churn_{count}"), |b| {
            b.iter(|| {
                let added = NodeAPI::create::<PhysicsBoneChain2D>(&mut fixed_churn_runtime);
                fixed_churn_runtime.fixed_update(black_box(1.0 / 60.0));
                assert!(NodeAPI::remove_node(&mut fixed_churn_runtime, added));
                black_box(fixed_churn_runtime.bench_internal_schedule_snapshot_copies());
            })
        });
    }
    group.finish();
}

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

    group.bench_function("hit_1_connect_params_only", |b| {
        let mut runtime = Runtime::new();
        perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, NodeID::new(1));
        let signal = SignalID::from_string("single_connect_params_only");
        assert!(SignalAPI::signal_connect(
            &mut runtime,
            NodeID::new(1),
            signal,
            method,
            &connect_params,
        ));
        b.iter(|| black_box(SignalAPI::signal_emit(&mut runtime, signal, &[])))
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

    for listener_count in [2_u32, 4, 16, 64] {
        group.bench_function(format!("hit_{listener_count}_no_params"), |b| {
            let mut runtime = Runtime::new();
            let signal = SignalID::from_u64(0xE117_0000 | u64::from(listener_count));
            for index in 0..listener_count {
                let id = NodeID::new(index + 1);
                perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, id);
                assert!(SignalAPI::signal_connect(
                    &mut runtime,
                    id,
                    signal,
                    method,
                    &[],
                ));
            }
            assert_eq!(
                SignalAPI::signal_emit(&mut runtime, signal, &[]),
                listener_count as usize
            );
            b.iter(|| black_box(SignalAPI::signal_emit(&mut runtime, signal, &[])))
        });
    }

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

    group.bench_function("active_hit_1_no_params", |b| {
        let mut runtime = Runtime::new();
        let owner = NodeID::new(1);
        let target = NodeID::new(2);
        bench_insert_state_script(&mut runtime, owner);
        perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, target);
        let signal = SignalID::from_string("active_single_no_params");
        assert!(SignalAPI::signal_connect(
            &mut runtime,
            target,
            signal,
            method,
            &[],
        ));
        b.iter(|| {
            black_box(bench_with_active_script(&mut runtime, owner, |runtime| {
                SignalAPI::signal_emit(runtime, signal, &[])
            }))
        })
    });

    group.bench_function("active_hit_4_emit_plus_connect_params", |b| {
        let mut runtime = Runtime::new();
        let owner = NodeID::new(1);
        bench_insert_state_script(&mut runtime, owner);
        let signal = SignalID::from_string("active_four_emit_connect_params");
        for i in 0..4 {
            let id = NodeID::new(i + 2);
            perro_runtime::api::signals::bench_insert_noop_signal_script(&mut runtime, id);
            assert!(SignalAPI::signal_connect(
                &mut runtime,
                id,
                signal,
                method,
                &connect_params,
            ));
        }
        b.iter(|| {
            black_box(bench_with_active_script(&mut runtime, owner, |runtime| {
                SignalAPI::signal_emit(runtime, signal, &emit_params)
            }))
        })
    });

    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_signal_connect(c);
    bench_signal_emit_release_matrix(c);
    bench_signal_script_teardown(c);
    bench_internal_schedule_snapshots(c);
}

criterion_group! {
    name = signal_hotpaths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(signal_hotpaths);
