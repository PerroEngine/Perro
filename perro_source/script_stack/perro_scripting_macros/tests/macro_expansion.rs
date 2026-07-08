extern crate self as perro_api;

pub mod variant {
    pub use perro_variant::{DeriveVariant, Variant, VariantSchema};
}

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
