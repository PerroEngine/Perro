#[macro_export]
macro_rules! emit_net_event {
    ($ctx:expr, $event:expr) => {{
        let _ = &$ctx;
        let _ = &$event;
        Err::<(), &'static str>("perro networking unsupported on web target")
    }};
}
