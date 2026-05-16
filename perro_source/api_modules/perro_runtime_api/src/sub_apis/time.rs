use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProfilingSnapshot {
    pub simulation_time: Duration,
    pub graphics_time: Duration,
    pub frame_time: Duration,
    pub fps: f32,
    pub draw_gpu_prepare_3d: Duration,
    pub draw_gpu_prepare_3d_frustum: Duration,
    pub draw_gpu_prepare_3d_hiz: Duration,
    pub draw_gpu_prepare_3d_indirect: Duration,
    pub draw_gpu_prepare_3d_cull_inputs: Duration,
    pub draw_calls_2d: u32,
    pub draw_calls_3d: u32,
    pub draw_calls_total: u32,
    pub draw_instances_3d: u32,
    pub draw_material_refs_3d: u32,
    pub skip_prepare_3d: u32,
    pub skip_prepare_3d_frustum: u32,
    pub skip_prepare_3d_hiz: u32,
    pub skip_prepare_3d_indirect: u32,
    pub skip_prepare_3d_cull_inputs: u32,
}

pub trait TimeAPI {
    fn get_delta(&self) -> f32;
    fn get_fixed_delta(&self) -> f32;
    fn get_elapsed(&self) -> f32;
    fn get_simulation_time(&self) -> Duration;
    fn get_graphics_time(&self) -> Duration;
    fn get_frame_time(&self) -> Duration;
    fn get_fps(&self) -> f32;
    fn get_draw_gpu_prepare_3d(&self) -> Duration {
        Duration::ZERO
    }
    fn get_draw_gpu_prepare_3d_frustum(&self) -> Duration {
        Duration::ZERO
    }
    fn get_draw_gpu_prepare_3d_hiz(&self) -> Duration {
        Duration::ZERO
    }
    fn get_draw_gpu_prepare_3d_indirect(&self) -> Duration {
        Duration::ZERO
    }
    fn get_draw_gpu_prepare_3d_cull_inputs(&self) -> Duration {
        Duration::ZERO
    }
    fn get_draw_calls_2d(&self) -> u32 {
        0
    }
    fn get_draw_calls_3d(&self) -> u32 {
        0
    }
    fn get_draw_calls_total(&self) -> u32 {
        0
    }
    fn get_draw_instances_3d(&self) -> u32 {
        0
    }
    fn get_draw_material_refs_3d(&self) -> u32 {
        0
    }
    fn get_skip_prepare_3d(&self) -> u32 {
        0
    }
    fn get_skip_prepare_3d_frustum(&self) -> u32 {
        0
    }
    fn get_skip_prepare_3d_hiz(&self) -> u32 {
        0
    }
    fn get_skip_prepare_3d_indirect(&self) -> u32 {
        0
    }
    fn get_skip_prepare_3d_cull_inputs(&self) -> u32 {
        0
    }

    fn get_profiling(&self) -> ProfilingSnapshot {
        ProfilingSnapshot {
            simulation_time: self.get_simulation_time(),
            graphics_time: self.get_graphics_time(),
            frame_time: self.get_frame_time(),
            fps: self.get_fps(),
            draw_gpu_prepare_3d: self.get_draw_gpu_prepare_3d(),
            draw_gpu_prepare_3d_frustum: self.get_draw_gpu_prepare_3d_frustum(),
            draw_gpu_prepare_3d_hiz: self.get_draw_gpu_prepare_3d_hiz(),
            draw_gpu_prepare_3d_indirect: self.get_draw_gpu_prepare_3d_indirect(),
            draw_gpu_prepare_3d_cull_inputs: self.get_draw_gpu_prepare_3d_cull_inputs(),
            draw_calls_2d: self.get_draw_calls_2d(),
            draw_calls_3d: self.get_draw_calls_3d(),
            draw_calls_total: self.get_draw_calls_total(),
            draw_instances_3d: self.get_draw_instances_3d(),
            draw_material_refs_3d: self.get_draw_material_refs_3d(),
            skip_prepare_3d: self.get_skip_prepare_3d(),
            skip_prepare_3d_frustum: self.get_skip_prepare_3d_frustum(),
            skip_prepare_3d_hiz: self.get_skip_prepare_3d_hiz(),
            skip_prepare_3d_indirect: self.get_skip_prepare_3d_indirect(),
            skip_prepare_3d_cull_inputs: self.get_skip_prepare_3d_cull_inputs(),
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
