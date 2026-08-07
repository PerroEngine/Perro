//! Re-export of the dependency-free [`perro_builtin_assets`] leaf crate.
//!
//! Build scripts should depend on `perro_builtin_assets` directly; depending on
//! `perro_api` just for these consts pulls its entire graph into the host build.
pub use perro_builtin_assets::{PERRO_LOGO_SVG, PERRO_LOGO_SVG_SOURCE};
