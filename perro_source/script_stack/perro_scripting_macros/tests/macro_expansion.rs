extern crate self as perro_api;

pub mod variant {
    pub use perro_variant::{
        DeriveVariant, SceneAssetKind, SceneVariantResolver, Variant, VariantSchema,
    };
}

use perro_ids::TextureID;
use perro_scripting_macros::{State, Variant};
use perro_variant::{DeriveVariant, Variant as VariantValue, VariantSchema};

#[State]
#[derive(Debug, PartialEq)]
struct Defaults {
    #[default = 7]
    count: i32,
    #[default("ready".to_owned())]
    label: String,
    flag: bool,
}

#[derive(Debug, PartialEq, Variant)]
#[variant(mode = "object")]
struct Stats {
    hp: i32,
    name: String,
}

#[derive(Debug, PartialEq, Variant)]
#[variant(mode = "object")]
struct SceneStats {
    icon: TextureID,
    nested: Vec<Option<TextureID>>,
}

#[derive(Debug, PartialEq, Variant)]
enum SceneChoice {
    Icon(TextureID),
}

struct Resolver;

impl perro_variant::SceneVariantResolver for Resolver {
    fn resolve_asset(
        &mut self,
        kind: perro_variant::SceneAssetKind,
        path: &str,
    ) -> Option<VariantValue> {
        (kind == perro_variant::SceneAssetKind::Texture && path == "res://icon.png")
            .then(|| TextureID::from_u64(99).into())
    }
}

#[test]
fn state_macro_builds_default_from_field_attrs() {
    assert_eq!(
        Defaults::default(),
        Defaults {
            count: 7,
            label: "ready".to_owned(),
            flag: false,
        }
    );
}

#[test]
fn variant_macro_round_trips_object_mode_and_schema() {
    let stats = Stats {
        hp: 42,
        name: "pup".to_owned(),
    };

    let encoded = stats.to_variant();
    let VariantValue::Object(fields) = &encoded else {
        panic!("expected object variant");
    };
    assert!(fields.contains_key("hp"));
    assert!(fields.contains_key("name"));

    assert_eq!(Stats::from_variant(&encoded), Some(stats));
    assert_eq!(Stats::field_names(), &["hp", "name"]);
}

#[test]
fn variant_macro_scene_decode_recurses_into_fields() {
    let path = VariantValue::from("res://icon.png");
    let encoded = VariantValue::Object(std::collections::BTreeMap::from([
        ("icon".into(), path.clone()),
        (
            "nested".into(),
            VariantValue::Array(vec![path, VariantValue::Null]),
        ),
    ]));
    let mut resolver = Resolver;
    assert_eq!(
        SceneStats::from_scene_variant(&encoded, &mut resolver),
        Some(SceneStats {
            icon: TextureID::from_u64(99),
            nested: vec![Some(TextureID::from_u64(99)), None],
        })
    );
    assert!(SceneStats::from_variant(&encoded).is_none());

    let choice = VariantValue::Array(vec![
        VariantValue::from(0_u16),
        VariantValue::from("res://icon.png"),
    ]);
    assert_eq!(
        SceneChoice::from_scene_variant(&choice, &mut resolver),
        Some(SceneChoice::Icon(TextureID::from_u64(99)))
    );
}
