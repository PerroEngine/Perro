pub mod ids;
mod macros;

pub use ids::*;

pub mod prelude {
    pub use crate::ids::*;
    pub use crate::{func, method, sid, signal, smid, tag, tags, var};
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
