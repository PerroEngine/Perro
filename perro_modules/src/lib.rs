pub mod file;
pub mod json;
pub mod log;

pub mod prelude {
    pub use crate::file as FileMod;
    pub use crate::json as JSONMod;
    pub use crate::log as LogMod;
}
