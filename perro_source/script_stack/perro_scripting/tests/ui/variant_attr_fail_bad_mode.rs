extern crate self as perro_api;

pub mod variant {
    pub use perro_variant::{DeriveVariant, SceneVariantResolver, Variant, VariantSchema};
}

use perro_scripting::Variant;

#[derive(Variant)]
#[variant(mode = "bad")]
struct BadMode {
    x: i32,
}

fn main() {}
