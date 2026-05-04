extern crate self as perro_api;

pub mod variant {
    pub use perro_variant::{Variant, DeriveVariant, VariantSchema};
}

use perro_ids::ScriptMemberID;
use perro_scripting::Variant;
use perro_variant::{DeriveVariant, VariantSchema};
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

#[derive(Debug, Clone, PartialEq, Variant)]
#[variant(mode = "object")]
struct DeepLeaf {
    player_count: i32,
}

#[derive(Debug, Clone, PartialEq, Variant)]
#[variant(mode = "object")]
struct DeepMid {
    roster: DeepLeaf,
}

#[derive(Debug, Clone, PartialEq, Variant)]
#[variant(mode = "object")]
struct DeepState {
    players: DeepMid,
    top: i32,
}

#[derive(Debug, Clone, PartialEq, Variant)]
#[variant(mode = "array")]
struct CompactVec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone, PartialEq, Variant)]
#[variant(tag = "u16")]
enum CompactBotState {
    Idle,
    Charging(f32, CompactVec3),
    Fired { power: f32, direction: CompactVec3 },
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
    let encoded = <BotProfile as DeriveVariant>::to_variant(&value);
    let decoded = <BotProfile as DeriveVariant>::from_variant(&encoded).expect("decode BotProfile");
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
        let encoded = <BotState as DeriveVariant>::to_variant(&value);
        let decoded = <BotState as DeriveVariant>::from_variant(&encoded).expect("decode BotState");
        assert_eq!(value, decoded);
    }
}

#[test]
fn derived_enum_works_in_params_macro() {
    let params = perro_variant::params![BotState::Idle];
    let encoded = params.first().expect("param");
    let decoded = <BotState as DeriveVariant>::from_variant(encoded).expect("decode BotState");

    assert_eq!(decoded, BotState::Idle);
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

    let encoded = <BrainSnapshot as DeriveVariant>::to_variant(&value);
    let decoded =
        <BrainSnapshot as DeriveVariant>::from_variant(&encoded).expect("decode BrainSnapshot");
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
    let encoded = <BotState as DeriveVariant>::to_variant(&value);
    let obj = encoded.as_object().expect("enum encodes as object");

    assert_eq!(obj.get("__variant").and_then(|v| v.as_u16()), Some(1));
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

fn nested_get_by_hash(
    prefix: &str,
    value: &perro_variant::Variant,
    var: ScriptMemberID,
) -> Option<perro_variant::Variant> {
    let obj = value.as_object()?;
    for (key, child) in obj {
        let full = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}.{}", key.as_ref())
        };
        if ScriptMemberID::from_string(full.as_str()) == var {
            return Some(child.clone());
        }
        if let Some(found) = nested_get_by_hash(full.as_str(), child, var) {
            return Some(found);
        }
    }
    None
}

fn nested_set_by_hash(
    prefix: &str,
    value: &mut perro_variant::Variant,
    var: ScriptMemberID,
    new_value: &perro_variant::Variant,
) -> bool {
    let Some(obj) = value.as_object_mut() else {
        return false;
    };
    for (key, child) in obj {
        let full = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}.{}", key.as_ref())
        };
        if ScriptMemberID::from_string(full.as_str()) == var {
            *child = new_value.clone();
            return true;
        }
        if nested_set_by_hash(full.as_str(), child, var, new_value) {
            return true;
        }
    }
    false
}

fn generated_style_get_var(state: &DeepState, var: ScriptMemberID) -> perro_variant::Variant {
    const TOP_PLAYERS: ScriptMemberID = perro_ids::ScriptMemberID::from_string("players");
    const TOP_TOP: ScriptMemberID = perro_ids::ScriptMemberID::from_string("top");
    match var {
        TOP_PLAYERS => perro_variant::DeriveVariant::to_variant(&state.players),
        TOP_TOP => perro_variant::DeriveVariant::to_variant(&state.top),
        _ => {
            let nested_root = perro_variant::DeriveVariant::to_variant(&state.players);
            nested_get_by_hash("players", &nested_root, var).unwrap_or(perro_variant::Variant::Null)
        }
    }
}

fn generated_style_set_var(
    state: &mut DeepState,
    var: ScriptMemberID,
    value: &perro_variant::Variant,
) {
    const TOP_PLAYERS: ScriptMemberID = perro_ids::ScriptMemberID::from_string("players");
    const TOP_TOP: ScriptMemberID = perro_ids::ScriptMemberID::from_string("top");
    match var {
        TOP_PLAYERS => {
            if let Some(v) = <DeepMid as perro_variant::DeriveVariant>::from_variant(value) {
                state.players = v;
            }
        }
        TOP_TOP => {
            if let Some(v) = <i32 as perro_variant::DeriveVariant>::from_variant(value) {
                state.top = v;
            }
        }
        _ => {
            let mut nested_root = perro_variant::DeriveVariant::to_variant(&state.players);
            if nested_set_by_hash("players", &mut nested_root, var, value)
                && let Some(decoded) =
                    <DeepMid as perro_variant::DeriveVariant>::from_variant(&nested_root)
            {
                state.players = decoded;
            }
        }
    }
}

#[test]
fn generated_match_style_get_set_supports_deep_nested_paths() {
    let mut state = DeepState {
        players: DeepMid {
            roster: DeepLeaf { player_count: 2 },
        },
        top: 10,
    };

    let path = ScriptMemberID::from_string("players.roster.player_count");
    let before = generated_style_get_var(&state, path);
    assert_eq!(before.as_i32(), Some(2));

    generated_style_set_var(&mut state, path, &perro_variant::Variant::from(7_i32));
    assert_eq!(state.players.roster.player_count, 7);

    let after = generated_style_get_var(&state, path);
    assert_eq!(after.as_i32(), Some(7));
}

#[test]
fn compact_struct_array_mode_roundtrip_and_shape() {
    let value = CompactVec3 {
        x: 4.0,
        y: -2.0,
        z: 9.0,
    };
    let encoded = <CompactVec3 as DeriveVariant>::to_variant(&value);
    let arr = encoded.as_array().expect("array mode encodes as array");
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].as_f32(), Some(4.0));
    assert_eq!(arr[1].as_f32(), Some(-2.0));
    assert_eq!(arr[2].as_f32(), Some(9.0));
    let decoded = <CompactVec3 as DeriveVariant>::from_variant(&encoded).expect("decode compact");
    assert_eq!(decoded, value);
}

#[test]
fn compact_enum_u16_tag_roundtrip_and_shape() {
    let value = CompactBotState::Charging(
        0.8,
        CompactVec3 {
            x: 0.1,
            y: 0.2,
            z: -1.0,
        },
    );
    let encoded = <CompactBotState as DeriveVariant>::to_variant(&value);
    let obj = encoded.as_object().expect("enum object");
    assert_eq!(obj.get("__variant").and_then(|v| v.as_u16()), Some(1));
    assert!(obj.get("__data").and_then(|v| v.as_array()).is_some());
    let decoded =
        <CompactBotState as DeriveVariant>::from_variant(&encoded).expect("decode compact enum");
    assert_eq!(decoded, value);
}
