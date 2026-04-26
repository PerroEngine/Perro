extern crate self as perro_api;

pub mod variant {
    pub use perro_variant::{Variant, VariantCodec, VariantSchema};
}

use perro_scripting::Variant;
use perro_variant::{VariantCodec, VariantSchema};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Variant)]
struct Vec3Like {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone, PartialEq, Variant)]
struct AimTuning {
    gravity_scale: f32,
    wind_bias: Vec3Like,
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Variant)]
struct BotProfile {
    name: String,
    enabled: bool,
    tuning: AimTuning,
    overrides: BTreeMap<Arc<str>, i32>,
    focus: Option<Vec3Like>,
}

#[derive(Debug, Clone, PartialEq, Variant)]
enum BotState {
    Idle,
    Charging(f32, Vec3Like),
    Fired { power: f32, direction: Vec3Like },
}

#[derive(Debug, Clone, PartialEq, Variant)]
struct BrainSnapshot {
    profile: BotProfile,
    state: BotState,
}

fn sample_profile() -> BotProfile {
    let mut overrides = BTreeMap::<Arc<str>, i32>::new();
    overrides.insert(Arc::<str>::from("aggression"), 7);
    overrides.insert(Arc::<str>::from("patience"), 3);
    BotProfile {
        name: "Bot-A".to_string(),
        enabled: true,
        tuning: AimTuning {
            gravity_scale: 0.93,
            wind_bias: Vec3Like {
                x: 0.1,
                y: 0.0,
                z: -0.05,
            },
            tags: vec!["ranked".to_string(), "archery".to_string()],
        },
        overrides,
        focus: Some(Vec3Like {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        }),
    }
}

#[test]
fn custom_struct_roundtrip_variant_codec() {
    let value = sample_profile();
    let encoded = <BotProfile as VariantCodec>::to_variant(&value);
    let decoded = <BotProfile as VariantCodec>::from_variant(&encoded).expect("decode BotProfile");
    assert_eq!(value, decoded);
}

#[test]
fn custom_enum_roundtrip_variant_codec_all_variants() {
    let values = vec![
        BotState::Idle,
        BotState::Charging(
            0.75,
            Vec3Like {
                x: 0.0,
                y: 0.2,
                z: -1.0,
            },
        ),
        BotState::Fired {
            power: 0.91,
            direction: Vec3Like {
                x: 0.05,
                y: 0.15,
                z: -0.98,
            },
        },
    ];

    for value in values {
        let encoded = <BotState as VariantCodec>::to_variant(&value);
        let decoded = <BotState as VariantCodec>::from_variant(&encoded).expect("decode BotState");
        assert_eq!(value, decoded);
    }
}

#[test]
fn complex_nested_struct_and_enum_roundtrip_variant_codec() {
    let value = BrainSnapshot {
        profile: sample_profile(),
        state: BotState::Fired {
            power: 1.0,
            direction: Vec3Like {
                x: 0.0,
                y: 0.12,
                z: -0.99,
            },
        },
    };

    let encoded = <BrainSnapshot as VariantCodec>::to_variant(&value);
    let decoded =
        <BrainSnapshot as VariantCodec>::from_variant(&encoded).expect("decode BrainSnapshot");
    assert_eq!(value, decoded);
}

#[test]
fn enum_encoding_shape_contains_variant_tag_and_data() {
    let value = BotState::Charging(
        0.42,
        Vec3Like {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
    );
    let encoded = <BotState as VariantCodec>::to_variant(&value);
    let obj = encoded.as_object().expect("enum encodes as object");

    assert_eq!(
        obj.get("__variant").and_then(|v| v.as_str()),
        Some("Charging")
    );
    assert!(obj.get("__data").is_some());
}

#[test]
fn derive_variant_emits_schema_field_names_for_structs() {
    assert_eq!(Vec3Like::field_names(), &["x", "y", "z"]);
    assert_eq!(
        BotProfile::field_names(),
        &["name", "enabled", "tuning", "overrides", "focus"]
    );
}

#[test]
fn derive_variant_schema_for_enum_defaults_to_empty() {
    assert!(BotState::field_names().is_empty());
}
