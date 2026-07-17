extern crate self as perro_api;

pub mod variant {
    pub use perro_variant::{DeriveVariant, Variant, VariantSchema};
}

use perro_ids::ScriptMemberID;
use perro_scripting::Variant;
use perro_variant::{DeriveVariant, Variant as VariantValue, VariantSchema};
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

#[derive(Debug, Clone, Default, PartialEq, Variant)]
enum DefaultBotState {
    #[default]
    Idle,
    Charging,
}

#[derive(Debug, Clone, PartialEq, Variant)]
struct TuplePair(i32, String);

#[derive(Debug, Clone, PartialEq, Variant)]
#[variant(mode = "object")]
struct TupleObject(i32, bool);

#[derive(Debug, Clone, PartialEq, Variant)]
struct GenericBox<T> {
    value: T,
    items: Vec<T>,
}

#[derive(Debug, Clone, PartialEq, Variant)]
struct GenericTuple<T>(T, Vec<T>);

#[derive(Debug, Clone, PartialEq, Variant)]
enum GenericEnum<T> {
    Empty,
    One(T),
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
fn tuple_struct_roundtrip_variant_codec() {
    let value = TuplePair(7, "seven".to_string());
    let encoded = VariantValue::from(value.clone());
    let arr = encoded.as_array().expect("tuple struct uses array mode");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0].as_i32(), Some(7));
    assert_eq!(arr[1].as_str(), Some("seven"));
    assert_eq!(encoded.parse::<TuplePair>(), Ok(value));

    let value = TupleObject(5, true);
    let encoded = VariantValue::from(value.clone());
    let obj = encoded.as_object().expect("object mode tuple struct");
    assert_eq!(obj.get("0").and_then(|v| v.as_i32()), Some(5));
    assert_eq!(obj.get("1").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(encoded.parse::<TupleObject>(), Ok(value));
}

#[test]
fn generic_struct_and_enum_roundtrip_variant_codec() {
    let named = GenericBox {
        value: 3_i32,
        items: vec![1, 2, 3],
    };
    let encoded = VariantValue::from(named.clone());
    assert_eq!(encoded.parse::<GenericBox<i32>>(), Ok(named));

    let tuple = GenericTuple("root".to_string(), vec!["a".to_string(), "b".to_string()]);
    let encoded = VariantValue::from(tuple.clone());
    assert_eq!(encoded.into_parse::<GenericTuple<String>>(), Ok(tuple));

    let item = GenericEnum::One(TuplePair(9, "nine".to_string()));
    let encoded = VariantValue::from(item.clone());
    assert_eq!(encoded.parse::<GenericEnum<TuplePair>>(), Ok(item));
}

#[test]
fn derived_enum_decodes_unit_variant_from_string() {
    let value = VariantValue::from("Idle");
    assert_eq!(value.parse::<BotState>(), Ok(BotState::Idle));
    assert_eq!(
        VariantValue::from("Idle").into_parse::<CompactBotState>(),
        Ok(CompactBotState::Idle)
    );
    assert!(VariantValue::from("Charging").parse::<BotState>().is_err());
}

#[test]
fn custom_struct_roundtrip_variant_codec() {
    let value = sample_profile();
    let encoded = <BotProfile as DeriveVariant>::to_variant(&value);
    let decoded = <BotProfile as DeriveVariant>::from_variant(&encoded).expect("decode BotProfile");
    assert_eq!(value, decoded);
}

#[test]
fn variant_from_and_parse_decodes_custom_struct_and_enum() {
    let profile = sample_profile();
    let encoded = VariantValue::from(profile.clone());
    let decoded = encoded.parse::<BotProfile>().expect("parse BotProfile");
    assert_eq!(decoded, profile);

    let state = BotState::Fired {
        power: 0.91,
        direction: Vec3Like {
            x: 0.05,
            y: 0.15,
            z: -0.98,
        },
    };
    let encoded = VariantValue::from(state.clone());
    let decoded = encoded.parse::<BotState>().expect("parse BotState");
    assert_eq!(decoded, state);
}

#[test]
fn variant_from_and_into_parse_decodes_custom_struct_and_enum() {
    let profile = sample_profile();
    let decoded = VariantValue::from(profile.clone())
        .into_parse::<BotProfile>()
        .expect("into_parse BotProfile");
    assert_eq!(decoded, profile);

    let state = BotState::Charging(
        0.75,
        Vec3Like {
            x: 0.0,
            y: 0.2,
            z: -1.0,
        },
    );
    let decoded = VariantValue::from(state.clone())
        .into_parse::<BotState>()
        .expect("into_parse BotState");
    assert_eq!(decoded, state);
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
fn enum_encoding_shape_is_compact_tag_array() {
    let value = BotState::Charging(
        0.42,
        Vec3Like {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
    );
    let encoded = <BotState as DeriveVariant>::to_variant(&value);
    let arr = encoded.as_array().expect("fielded enum encodes as array");
    assert_eq!(arr[0].as_u16(), Some(1), "slot 0 carries the tag");
    assert_eq!(arr.len(), 3, "tag + one slot per field");

    // Unit variants collapse to the bare tag: no container at all.
    let unit = <BotState as DeriveVariant>::to_variant(&BotState::Idle);
    assert_eq!(unit.as_u16(), Some(0));
}

#[test]
fn enum_decodes_legacy_variant_data_object_form() {
    // Shape emitted by pre-compact engine versions and scene tooling.
    let mut obj = BTreeMap::<Arc<str>, VariantValue>::new();
    obj.insert(Arc::from("__variant"), VariantValue::from(1u16));
    obj.insert(
        Arc::from("__data"),
        VariantValue::Array(vec![
            VariantValue::from(0.42f32),
            <Vec3Like as DeriveVariant>::to_variant(&Vec3Like {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            }),
        ]),
    );
    let legacy = VariantValue::Object(obj);

    let expected = BotState::Charging(
        0.42,
        Vec3Like {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
    );
    assert_eq!(legacy.parse::<BotState>(), Ok(expected.clone()));
    assert_eq!(legacy.into_parse::<BotState>(), Ok(expected));

    // Legacy unit form: object with only a tag.
    let mut obj = BTreeMap::<Arc<str>, VariantValue>::new();
    obj.insert(Arc::from("__variant"), VariantValue::from(0u16));
    let legacy_unit = VariantValue::Object(obj);
    assert_eq!(legacy_unit.parse::<BotState>(), Ok(BotState::Idle));
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
    let arr = encoded.as_array().expect("enum array");
    assert_eq!(arr[0].as_u16(), Some(1));
    assert_eq!(arr.len(), 3);
    let decoded =
        <CompactBotState as DeriveVariant>::from_variant(&encoded).expect("decode compact enum");
    assert_eq!(decoded, value);
}

#[test]
fn named_enum_variant_encodes_positional_fields() {
    let value = BotState::Fired {
        power: 0.5,
        direction: Vec3Like {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        },
    };
    let encoded = <BotState as DeriveVariant>::to_variant(&value);
    let arr = encoded.as_array().expect("named variant encodes as array");
    assert_eq!(arr[0].as_u16(), Some(2));
    assert_eq!(
        arr[1].as_f32(),
        Some(0.5),
        "fields follow declaration order"
    );
    assert_eq!(encoded.parse::<BotState>(), Ok(value));
}

#[test]
fn default_enum_decodes_from_null() {
    let decoded = <DefaultBotState as DeriveVariant>::from_variant(&VariantValue::Null)
        .expect("null -> default");
    assert_eq!(decoded, DefaultBotState::Idle);

    let decoded = <DefaultBotState as DeriveVariant>::from_owned_variant(VariantValue::Null)
        .expect("owned null -> default");
    assert_eq!(decoded, DefaultBotState::Idle);
}
