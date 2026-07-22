extern crate self as perro_api;

use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use perro_scripting::Variant;
use perro_variant::{DeriveVariant, Variant as VariantValue};
use std::collections::BTreeMap;
use std::sync::Arc;

pub mod variant {
    pub use perro_variant::{DeriveVariant, SceneVariantResolver, Variant, VariantSchema};
}

#[derive(Clone, PartialEq, Variant)]
#[variant(mode = "object")]
struct ObjectVec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, PartialEq, Variant)]
#[variant(mode = "array")]
struct ArrayVec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, PartialEq, Variant)]
#[variant(tag = "string")]
enum StringTagState {
    Idle,
    Move(ObjectVec3),
}

#[derive(Clone, PartialEq, Variant)]
#[variant(tag = "u16")]
enum U16TagState {
    Idle,
    Move(ArrayVec3),
}

#[derive(Clone, PartialEq, Variant)]
#[variant(mode = "array")]
struct RuntimeVec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, PartialEq, Variant)]
#[variant(mode = "array")]
struct RuntimeTuning {
    gravity_scale: f32,
    wind_bias: RuntimeVec3,
    tags: Vec<String>,
}

#[derive(Clone, PartialEq, Variant)]
#[variant(mode = "array")]
struct RuntimeProfile {
    name: String,
    enabled: bool,
    tuning: RuntimeTuning,
    overrides: BTreeMap<Arc<str>, i32>,
    focus: Option<RuntimeVec3>,
}

#[derive(Clone, PartialEq, Variant)]
#[variant(tag = "u16")]
enum RuntimeState {
    Idle,
    Charging(f32, RuntimeVec3),
    Fired { power: f32, direction: RuntimeVec3 },
}

fn runtime_profile() -> RuntimeProfile {
    let mut overrides = BTreeMap::<Arc<str>, i32>::new();
    overrides.insert(Arc::<str>::from("aggression"), 7);
    overrides.insert(Arc::<str>::from("patience"), 3);
    RuntimeProfile {
        name: "Bot-A".to_string(),
        enabled: true,
        tuning: RuntimeTuning {
            gravity_scale: 0.93,
            wind_bias: RuntimeVec3 {
                x: 0.1,
                y: 0.0,
                z: -0.05,
            },
            tags: vec!["ranked".to_string(), "archery".to_string()],
        },
        overrides,
        focus: Some(RuntimeVec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        }),
    }
}

fn bench_variant_modes(c: &mut Criterion) {
    let object = ObjectVec3 {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let array = ArrayVec3 {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let st_string = StringTagState::Move(object.clone());
    let st_u16 = U16TagState::Move(array.clone());
    let profile = runtime_profile();
    let state_tuple = RuntimeState::Charging(
        0.75,
        RuntimeVec3 {
            x: 0.0,
            y: 0.2,
            z: -1.0,
        },
    );
    let state_struct = RuntimeState::Fired {
        power: 0.91,
        direction: RuntimeVec3 {
            x: 0.05,
            y: 0.15,
            z: -0.98,
        },
    };

    c.bench_function("struct_object_encode", |b| {
        b.iter(|| {
            black_box(<ObjectVec3 as DeriveVariant>::to_variant(black_box(
                &object,
            )))
        })
    });
    c.bench_function("struct_array_encode", |b| {
        b.iter(|| black_box(<ArrayVec3 as DeriveVariant>::to_variant(black_box(&array))))
    });

    let object_encoded = <ObjectVec3 as DeriveVariant>::to_variant(&object);
    let array_encoded = <ArrayVec3 as DeriveVariant>::to_variant(&array);
    c.bench_function("struct_object_decode", |b| {
        b.iter(|| {
            black_box(<ObjectVec3 as DeriveVariant>::from_variant(black_box(
                &object_encoded,
            )))
        })
    });
    c.bench_function("struct_array_decode", |b| {
        b.iter(|| {
            black_box(<ArrayVec3 as DeriveVariant>::from_variant(black_box(
                &array_encoded,
            )))
        })
    });

    c.bench_function("enum_string_tag_encode", |b| {
        b.iter(|| {
            black_box(<StringTagState as DeriveVariant>::to_variant(black_box(
                &st_string,
            )))
        })
    });
    c.bench_function("enum_u16_tag_encode", |b| {
        b.iter(|| {
            black_box(<U16TagState as DeriveVariant>::to_variant(black_box(
                &st_u16,
            )))
        })
    });

    let st_string_encoded = VariantValue::from(st_string.clone());
    let st_u16_encoded = VariantValue::from(st_u16.clone());
    c.bench_function("enum_string_tag_decode_parse", |b| {
        b.iter(|| {
            black_box(
                black_box(&st_string_encoded)
                    .parse::<StringTagState>()
                    .expect("parse StringTagState"),
            )
        })
    });
    c.bench_function("enum_u16_tag_decode_parse", |b| {
        b.iter(|| {
            black_box(
                black_box(&st_u16_encoded)
                    .parse::<U16TagState>()
                    .expect("parse U16TagState"),
            )
        })
    });

    let profile_encoded = VariantValue::from(profile.clone());
    let state_tuple_encoded = VariantValue::from(state_tuple.clone());
    let state_struct_encoded = VariantValue::from(state_struct.clone());
    let arc_str_encoded = VariantValue::from(Arc::<str>::from("dynamic-state-label"));
    c.bench_function("arc_str_borrow_decode", |b| {
        b.iter(|| {
            black_box(
                black_box(&arc_str_encoded)
                    .parse::<Arc<str>>()
                    .expect("borrow parse Arc<str>"),
            )
        })
    });
    c.bench_function("arc_str_owned_decode", |b| {
        b.iter_batched(
            || arc_str_encoded.clone(),
            |value| {
                black_box(
                    value
                        .into_parse::<Arc<str>>()
                        .expect("owned parse Arc<str>"),
                )
            },
            BatchSize::SmallInput,
        )
    });
    c.bench_function("custom_struct_decode_parse", |b| {
        b.iter(|| {
            black_box(
                black_box(&profile_encoded)
                    .parse::<RuntimeProfile>()
                    .expect("parse RuntimeProfile"),
            )
        })
    });
    c.bench_function("custom_struct_clone_decode_parse", |b| {
        b.iter(|| {
            black_box(
                black_box(profile_encoded.clone())
                    .into_parse::<RuntimeProfile>()
                    .expect("clone + parse RuntimeProfile"),
            )
        })
    });
    c.bench_function("custom_enum_tuple_decode_parse", |b| {
        b.iter(|| {
            black_box(
                black_box(&state_tuple_encoded)
                    .parse::<RuntimeState>()
                    .expect("parse RuntimeState tuple"),
            )
        })
    });
    c.bench_function("custom_enum_struct_decode_parse", |b| {
        b.iter(|| {
            black_box(
                black_box(&state_struct_encoded)
                    .parse::<RuntimeState>()
                    .expect("parse RuntimeState struct"),
            )
        })
    });
    c.bench_function("custom_struct_into_variant_parse", |b| {
        b.iter_batched(
            || profile.clone(),
            |profile| {
                let encoded = VariantValue::from(black_box(profile));
                black_box(
                    encoded
                        .into_parse::<RuntimeProfile>()
                        .expect("into_parse RuntimeProfile"),
                )
            },
            BatchSize::SmallInput,
        )
    });
    c.bench_function("custom_enum_into_variant_parse", |b| {
        b.iter_batched(
            || state_struct.clone(),
            |state_struct| {
                let encoded = VariantValue::from(black_box(state_struct));
                black_box(
                    encoded
                        .into_parse::<RuntimeState>()
                        .expect("into_parse RuntimeState struct"),
                )
            },
            BatchSize::SmallInput,
        )
    });
    c.bench_function("custom_struct_clone_into_variant_parse", |b| {
        b.iter(|| {
            let encoded = VariantValue::from(black_box(profile.clone()));
            black_box(
                encoded
                    .into_parse::<RuntimeProfile>()
                    .expect("into_parse RuntimeProfile"),
            )
        })
    });
    c.bench_function("custom_enum_clone_into_variant_parse", |b| {
        b.iter(|| {
            let encoded = VariantValue::from(black_box(state_struct.clone()));
            black_box(
                encoded
                    .into_parse::<RuntimeState>()
                    .expect("into_parse RuntimeState struct"),
            )
        })
    });
}

criterion_group!(benches, bench_variant_modes);
criterion_main!(benches);
