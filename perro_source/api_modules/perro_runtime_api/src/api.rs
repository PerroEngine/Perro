use crate::sub_apis::{
    AnimPlayerAPI, AnimPlayerModule, AnimTreeAPI, AnimTreeModule, MeshQueryModule, NavMeshAPI,
    NavMeshModule, NodeAPI, NodeModule, NodeQueryModule, PhysicsAPI, PhysicsModule,
    RuntimeAudioAPI, RuntimeAudioModule, SceneAPI, SceneModule, ScriptAPI, ScriptModule, SignalAPI,
    SignalModule, TimeAPI, TimeModule, TimerAPI, TimerModule, WindowAPI, WindowModule,
};

/// Full runtime contract used by script contexts.
///
/// Engine runtime types implement this by implementing every runtime sub-API.
/// Scripts normally do not name this trait directly; it exists to keep the
/// script context generic. Individual window accessors require only their domain trait.
pub trait RuntimeAPI:
    TimeAPI
    + TimerAPI
    + WindowAPI
    + NodeAPI
    + ScriptAPI
    + SignalAPI
    + PhysicsAPI
    + AnimPlayerAPI
    + AnimTreeAPI
    + SceneAPI
    + RuntimeAudioAPI
{
}
impl<T> RuntimeAPI for T where
    T: TimeAPI
        + TimerAPI
        + WindowAPI
        + NodeAPI
        + ScriptAPI
        + SignalAPI
        + PhysicsAPI
        + AnimPlayerAPI
        + AnimTreeAPI
        + SceneAPI
        + RuntimeAudioAPI
{
}

/// Script-facing runtime facade.
///
/// `RuntimeApiSurface` owns a temporary mutable borrow of the runtime for one
/// script callback. Domain accessors such as [`RuntimeApiSurface::Nodes`] and
/// [`RuntimeApiSurface::Physics`] return lightweight wrappers over the same borrow.
pub struct RuntimeApiSurface<'rt, RT: ?Sized> {
    rt: &'rt mut RT,
}

/// Backward-compatible facade name.
pub type RuntimeWindow<'rt, RT> = RuntimeApiSurface<'rt, RT>;

#[allow(non_snake_case)]
impl<'rt, RT: ?Sized> RuntimeApiSurface<'rt, RT> {
    // ---- Construction ----

    /// Create a runtime window around an existing runtime borrow.
    pub fn new(rt: &'rt mut RT) -> Self {
        Self { rt }
    }

    // ---- Frame/window state ----

    /// Access frame timing and profiling data.
    #[inline]
    pub fn Time(&mut self) -> TimeModule<'_, RT>
    where
        RT: TimeAPI,
    {
        TimeModule::new(self.rt)
    }

    /// Start, cancel, and inspect named one-shot timers.
    #[inline]
    pub fn Timers(&mut self) -> TimerModule<'_, RT>
    where
        RT: TimerAPI,
    {
        TimerModule::new(self.rt)
    }

    /// Queue window requests and read active refresh data.
    #[inline]
    pub fn Window(&mut self) -> WindowModule<'_, RT>
    where
        RT: WindowAPI,
    {
        WindowModule::new(self.rt)
    }

    // ---- Scene graph ----

    /// Access scene node creation, deletion, tags, transforms, and fields.
    #[inline]
    pub fn Nodes(&mut self) -> NodeModule<'_, RT>
    where
        RT: NodeAPI,
    {
        NodeModule::new(self.rt)
    }

    /// Build and run scene node queries.
    #[inline]
    pub fn NodeQuery(&mut self) -> NodeQueryModule<'_, RT>
    where
        RT: NodeAPI,
    {
        NodeQueryModule::new(self.rt)
    }

    /// Query mesh surfaces and material regions for 3D picking workflows.
    #[inline]
    pub fn MeshQuery(&mut self) -> MeshQueryModule<'_, RT>
    where
        RT: NodeAPI,
    {
        MeshQueryModule::new(self.rt)
    }

    /// Query 3D navigation meshes.
    #[inline]
    pub fn NavMesh(&mut self) -> NavMeshModule<'_, RT>
    where
        RT: NavMeshAPI,
    {
        NavMeshModule::new(self.rt)
    }

    // ---- Script and signals ----

    /// Access script attachment, variables, methods, and typed state helpers.
    #[inline]
    pub fn Scripts(&mut self) -> ScriptModule<'_, RT>
    where
        RT: ScriptAPI,
    {
        ScriptModule::new(self.rt)
    }

    /// Connect, disconnect, and emit runtime signals.
    #[inline]
    pub fn Signals(&mut self) -> SignalModule<'_, RT>
    where
        RT: SignalAPI,
    {
        SignalModule::new(self.rt)
    }

    // ---- Simulation ----

    /// Access physics state, forces, raycasts, prediction, and gravity.
    #[inline]
    pub fn Physics(&mut self) -> PhysicsModule<'_, RT>
    where
        RT: PhysicsAPI,
    {
        PhysicsModule::new(self.rt)
    }

    /// Control per-node animation players.
    #[inline]
    pub fn AnimPlayer(&mut self) -> AnimPlayerModule<'_, RT>
    where
        RT: AnimPlayerAPI,
    {
        AnimPlayerModule::new(self.rt)
    }

    /// Control animation tree slots and weights.
    #[inline]
    pub fn AnimTree(&mut self) -> AnimTreeModule<'_, RT>
    where
        RT: AnimTreeAPI,
    {
        AnimTreeModule::new(self.rt)
    }

    // ---- Loading and audio ----

    /// Load, preload, and release scenes.
    #[inline]
    pub fn Scene(&mut self) -> SceneModule<'_, RT>
    where
        RT: SceneAPI,
    {
        SceneModule::new(self.rt)
    }

    /// Play runtime audio attached to scene nodes.
    #[inline]
    pub fn Audio(&mut self) -> RuntimeAudioModule<'_, RT>
    where
        RT: RuntimeAudioAPI,
    {
        RuntimeAudioModule::new(self.rt)
    }

    // ---- Escape hatch ----

    /// Return the underlying runtime borrow for code that must call a raw API.
    #[inline]
    pub fn runtime_mut(&mut self) -> &mut RT {
        self.rt
    }
}

#[cfg(test)]
mod narrow_window_tests {
    use super::*;
    use std::time::Duration;
    struct ClockOnly;
    impl TimeAPI for ClockOnly {
        fn get_delta(&self) -> f32 {
            0.25
        }
        fn get_fixed_delta(&self) -> f32 {
            0.125
        }
        fn get_elapsed(&self) -> f32 {
            1.0
        }
        fn get_simulation_time(&self) -> Duration {
            Duration::ZERO
        }
        fn get_graphics_time(&self) -> Duration {
            Duration::ZERO
        }
        fn get_frame_time(&self) -> Duration {
            Duration::ZERO
        }
        fn get_fps(&self) -> f32 {
            60.0
        }
    }
    #[test]
    fn time_window_does_not_require_unrelated_services() {
        let mut clock = ClockOnly;
        let mut window = RuntimeWindow::new(&mut clock);
        assert_eq!(window.runtime_mut().get_delta(), 0.25);
        let _ = window.Time();
    }
}
