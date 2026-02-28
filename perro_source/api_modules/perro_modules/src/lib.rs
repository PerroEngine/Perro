pub mod file;
pub mod json;
pub mod log;
pub mod math;

pub mod prelude {
    pub use crate::file as FileMod;
    pub use crate::json as JSONMod;
    pub use crate::log as LogMod;
    pub use crate::math as MathMod;
    pub use crate::{deg_to_rad, log_error, log_info, log_print, log_warn, rad_to_deg};
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
