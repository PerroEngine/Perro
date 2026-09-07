// Baseline-compatible: append `#[cfg(test)] #[path = "world_perf_tests.rs"]
// mod perf_tests;` to world.rs in both trees. Public World calls stay unchanged.
use super::*;
use std::{hint::black_box, time::Instant};

#[test]
#[ignore = "manual localhost release timing probe; run serially"]
fn time_world_highwater_sparse_full_and_churn() {
    for (highwater, stride, iterations) in [(4096, 4096, 10_000), (128, 1, 100), (4096, 128, 200)] {
        let mut world = NetworkWorld::new();
        let ids = (0..highwater)
            .map(|_| world.bind_udp("127.0.0.1:0").expect("UDP fixture"))
            .collect::<Vec<_>>();
        for (index, id) in ids.iter().copied().enumerate() {
            if index % stride != 0 {
                assert!(world.remove_udp(id));
            }
        }
        assert!(world.poll_events(1, 64).is_empty());
        let mut samples = Vec::new();
        for _ in 0..20 {
            let start = Instant::now();
            for _ in 0..iterations {
                if stride == 128 {
                    let id = world.bind_udp("127.0.0.1:0").expect("churn UDP");
                    black_box(world.poll_events(1, 64));
                    assert!(world.remove_udp(id));
                } else {
                    black_box(world.poll_events(1, 64));
                }
            }
            samples.push(start.elapsed().as_nanos());
        }
        assert!(world.poll_events(1, 64).is_empty());
        println!(
            "{{\"case\":\"world_highwater_{highwater}_stride{stride}\",\"operations_per_sample\":{iterations},\"samples_ns\":{samples:?}}}"
        );
    }
}
