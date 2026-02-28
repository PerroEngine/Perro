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
