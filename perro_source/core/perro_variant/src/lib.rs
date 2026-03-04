mod macros;
pub mod variant;
pub use variant::*;

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
