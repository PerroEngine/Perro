pub mod file;
pub mod json;
pub mod log;

pub mod prelude {
    pub use crate::file as FileMod;
    pub use crate::json as JSONMod;
    pub use crate::log as LogMod;
    pub use crate::{log_error, log_info, log_print, log_warn};
}

#[cfg(test)]
mod tests {
    #[test]
    fn log_macros_typecheck_and_forward() {
        let v = 42;
        crate::log_print!("print {v}");
        crate::log_info!("info {v}");
        crate::log_warn!("warn {v}");
        crate::log_error!("error {v}");
    }
}
