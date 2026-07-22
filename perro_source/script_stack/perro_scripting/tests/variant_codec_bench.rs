//! Timing probes for the `#[derive(Variant)]` codec hot path.
//!
//! Not asserted-on numbers; run release for signal:
//! `cargo test -p perro_scripting --release --test variant_codec_bench -- --nocapture`

extern crate self as perro_api;

pub mod variant {
    pub use perro_variant::{DeriveVariant, SceneVariantResolver, Variant, VariantSchema};
}

use perro_scripting::Variant;
use perro_variant::{DeriveVariant, Variant as VariantValue};
use std::hint::black_box;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Variant)]
struct Vec3Like {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone, PartialEq, Variant)]
enum BotState {
    Idle,
    Charging(f32, Vec3Like),
    Fired { power: f32, direction: Vec3Like },
}

const ITERS: u32 = 200_000;

fn time_per_op(label: &str, mut op: impl FnMut()) {
    // Warmup pass keeps LazyLock init + allocator warm out of the measurement.
    for _ in 0..1_000 {
        op();
    }
    let start = Instant::now();
    for _ in 0..ITERS {
        op();
    }
    let per_op = start.elapsed().as_nanos() / ITERS as u128;
    println!("{label}: {per_op} ns/op");
}

#[test]
fn bench_enum_codec() {
    println!("size_of::<Variant>() = {}", size_of::<VariantValue>());

    let unit = BotState::Idle;
    let tuple = BotState::Charging(
        0.75,
        Vec3Like {
            x: 0.0,
            y: 0.2,
            z: -1.0,
        },
    );
    let named = BotState::Fired {
        power: 0.91,
        direction: Vec3Like {
            x: 0.05,
            y: 0.15,
            z: -0.98,
        },
    };

    time_per_op("enum unit   to_variant", || {
        black_box(<BotState as DeriveVariant>::to_variant(black_box(&unit)));
    });
    time_per_op("enum tuple  to_variant", || {
        black_box(<BotState as DeriveVariant>::to_variant(black_box(&tuple)));
    });
    time_per_op("enum struct to_variant", || {
        black_box(<BotState as DeriveVariant>::to_variant(black_box(&named)));
    });

    let unit_v = <BotState as DeriveVariant>::to_variant(&unit);
    let tuple_v = <BotState as DeriveVariant>::to_variant(&tuple);
    let named_v = <BotState as DeriveVariant>::to_variant(&named);

    time_per_op("enum unit   from_variant", || {
        black_box(<BotState as DeriveVariant>::from_variant(black_box(&unit_v)).unwrap());
    });
    time_per_op("enum tuple  from_variant", || {
        black_box(<BotState as DeriveVariant>::from_variant(black_box(&tuple_v)).unwrap());
    });
    time_per_op("enum struct from_variant", || {
        black_box(<BotState as DeriveVariant>::from_variant(black_box(&named_v)).unwrap());
    });

    time_per_op("enum tuple  roundtrip", || {
        let v = <BotState as DeriveVariant>::to_variant(black_box(&tuple));
        black_box(<BotState as DeriveVariant>::from_owned_variant(v).unwrap());
    });

    let numbers: Vec<VariantValue> = (0..64).map(|i| VariantValue::from(i as f64)).collect();
    let array = VariantValue::Array(numbers);
    time_per_op("Variant array[64 num] clone", || {
        black_box(black_box(&array).clone());
    });
}
