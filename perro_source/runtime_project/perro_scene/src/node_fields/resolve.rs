#[path = "resolve/scene_fields.rs"]
mod scene_fields;
pub(super) use scene_fields::*;
#[path = "resolve/string_fields.rs"]
mod string_fields;
pub(super) use string_fields::*;
#[path = "resolve/shared.rs"]
mod shared;
pub use shared::water_body_removed_field;
pub(super) use shared::*;
