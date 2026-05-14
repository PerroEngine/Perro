use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProfilingSnapshot {
    pub simulation_time: Duration,
    pub graphics_time: Duration,
    pub frame_time: Duration,
    pub fps: f32,
}

pub trait TimeAPI {
    fn get_delta(&self) -> f32;
    fn get_fixed_delta(&self) -> f32;
    fn get_elapsed(&self) -> f32;
    fn get_simulation_time(&self) -> Duration;
    fn get_graphics_time(&self) -> Duration;
    fn get_frame_time(&self) -> Duration;
    fn get_fps(&self) -> f32;

    fn get_profiling(&self) -> ProfilingSnapshot {
        ProfilingSnapshot {
            simulation_time: self.get_simulation_time(),
            graphics_time: self.get_graphics_time(),
            frame_time: self.get_frame_time(),
            fps: self.get_fps(),
        }
    }
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

    pub fn get_simulation_time(&mut self) -> Duration {
        self.rt.get_simulation_time()
    }

    pub fn get_graphics_time(&mut self) -> Duration {
        self.rt.get_graphics_time()
    }

    pub fn get_frame_time(&mut self) -> Duration {
        self.rt.get_frame_time()
    }

    pub fn get_fps(&mut self) -> f32 {
        self.rt.get_fps()
    }

    pub fn get_profiling(&mut self) -> ProfilingSnapshot {
        self.rt.get_profiling()
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

/// Returns last measured simulation time.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
#[macro_export]
macro_rules! simulation_time {
    ($ctx:expr) => {
        $ctx.Time().get_simulation_time()
    };
}

/// Returns last measured graphics time.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
#[macro_export]
macro_rules! graphics_time {
    ($ctx:expr) => {
        $ctx.Time().get_graphics_time()
    };
}

/// Returns last measured frame time.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
#[macro_export]
macro_rules! frame_time {
    ($ctx:expr) => {
        $ctx.Time().get_frame_time()
    };
}

/// Returns last measured frames per second.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
#[macro_export]
macro_rules! fps {
    ($ctx:expr) => {
        $ctx.Time().get_fps()
    };
}

/// Returns last measured frame profiling snapshot.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
#[macro_export]
macro_rules! profiling {
    ($ctx:expr) => {
        $ctx.Time().get_profiling()
    };
}
