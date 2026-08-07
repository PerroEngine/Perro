//! Engine-shipped asset bytes, with no dependencies.
//!
//! Kept as its own leaf crate so build scripts (window icon embedding) can pull
//! the fallback logo without dragging the whole `perro_api` graph into the host
//! build. Re-exported as `perro_api::builtin_assets`.

pub const PERRO_LOGO_SVG: &[u8] = include_bytes!("perro.svg");
pub const PERRO_LOGO_SVG_SOURCE: &str = "__perro_builtin_logo_svg__";
