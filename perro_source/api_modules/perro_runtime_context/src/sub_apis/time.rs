pub trait TimeAPI {
    fn get_delta(&self) -> f32;
    fn get_fixed_delta(&self) -> f32;
    fn get_elapsed(&self) -> f32;
}

pub struct TimeModule<'rt, R: TimeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: TimeAPI + ?Sized> TimeModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn get_delta(&mut self) -> f32 {
        self.rt.get_delta()
    }

    pub fn get_fixed_delta(&mut self) -> f32 {
        self.rt.get_fixed_delta()
    }

    pub fn get_elapsed(&mut self) -> f32 {
        self.rt.get_elapsed()
    }
}

/// Returns frame delta time (seconds).
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
#[macro_export]
macro_rules! delta_time {
    ($ctx:expr) => {
        $ctx.Time().get_delta()
    };
}

/// Returns frame delta time clamped to `max` seconds.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `max`: maximum allowed seconds for returned delta
#[macro_export]
macro_rules! delta_time_capped {
    ($ctx:expr, $max:expr) => {{
        let dt = $ctx.Time().get_delta();
        dt.max(0.0).min($max)
    }};
}

/// Returns frame delta time clamped to `[min, max]` seconds.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `min`: minimum allowed seconds for returned delta
/// - `max`: maximum allowed seconds for returned delta
#[macro_export]
macro_rules! delta_time_clamped {
    ($ctx:expr, $min:expr, $max:expr) => {{
        let dt = $ctx.Time().get_delta();
        dt.max($min).min($max)
    }};
}

/// Returns fixed-step delta time (seconds).
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
#[macro_export]
macro_rules! fixed_delta_time {
    ($ctx:expr) => {
        $ctx.Time().get_fixed_delta()
    };
}

/// Returns elapsed runtime time (seconds).
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
#[macro_export]
macro_rules! elapsed_time {
    ($ctx:expr) => {
        $ctx.Time().get_elapsed()
    };
}
