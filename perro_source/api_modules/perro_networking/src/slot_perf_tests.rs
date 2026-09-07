// Baseline-compatible: append `#[cfg(test)] #[path = "slot_perf_tests.rs"]
// mod perf_tests;` to slot.rs in both trees. Infer storage through Default so
// the same fixture covers the old Vec and the new private slot store.
use super::*;
use std::{hint::black_box, time::Instant};

#[test]
#[ignore = "manual release timing probe; run serially"]
fn time_slot_insert_and_min_free_churn() {
    for count in [16u32, 10_000] {
        let mut samples = Vec::new();
        for _ in 0..20 {
            let mut slots = Default::default();
            let start = Instant::now();
            for value in 0..count {
                black_box(insert_slot(&mut slots, black_box(value)));
            }
            samples.push(start.elapsed().as_nanos());
            assert_eq!(
                *get_slot(&slots, count - 1, "probe").expect("last slot"),
                count - 1
            );
            black_box(&slots);
        }
        println!(
            "{{\"case\":\"slot_insert_{count}\",\"operations_per_sample\":{count},\"samples_ns\":{samples:?}}}"
        );

        let mut slots = Default::default();
        for value in 0..count {
            insert_slot(&mut slots, value);
        }
        for id in [0, count - 1] {
            let mut samples = Vec::new();
            for _ in 0..20 {
                let start = Instant::now();
                for _ in 0..10_000 {
                    black_box(remove_slot(&mut slots, black_box(id)));
                    black_box(insert_slot(&mut slots, black_box(id)));
                }
                samples.push(start.elapsed().as_nanos());
                assert_eq!(*get_slot(&slots, id, "probe").expect("reused slot"), id);
            }
            println!(
                "{{\"case\":\"slot_churn_{count}_id{id}\",\"operations_per_sample\":10000,\"samples_ns\":{samples:?}}}"
            );
        }
    }
}
