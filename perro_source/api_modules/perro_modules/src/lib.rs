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
mod tests {
    #[test]
    fn log_macros_typecheck_and_forward() {
        let v = 42;
        crate::log_print!("print {v}");
        crate::log_info!("info {v}");
        crate::log_warn!("warn {v}");
        crate::log_error!("error {v}");
    }

    #[test]
    fn math_macros_typecheck_and_forward() {
        let degrees = 180.0;
        let radians = std::f32::consts::PI;
        let _ = crate::deg_to_rad!(degrees);
        let _ = crate::rad_to_deg!(radians);
    }
}
