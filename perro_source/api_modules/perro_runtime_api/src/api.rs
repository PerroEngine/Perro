use crate::sub_apis::{
    AnimPlayerAPI, AnimPlayerModule, AnimTreeAPI, AnimTreeModule, MeshQueryModule, NodeAPI,
    NodeModule, NodeQueryModule, PhysicsAPI, PhysicsModule, RuntimeAudioAPI, RuntimeAudioModule,
    SceneAPI, SceneModule, ScriptAPI, ScriptModule, SignalAPI, SignalModule, TimeAPI, TimeModule,
    WindowAPI, WindowModule,
};

/// Full runtime contract required by [`RuntimeWindow`].
///
/// Engine runtime types implement this by implementing every runtime sub-API.
/// Scripts normally do not name this trait directly; it exists to keep the
/// window facade generic while preserving one borrow of the runtime.
pub trait RuntimeAPI:
    TimeAPI
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
/// `RuntimeWindow` owns a temporary mutable borrow of the runtime for one script
/// callback. Domain accessors such as [`RuntimeWindow::Nodes`] and
/// [`RuntimeWindow::Physics`] return lightweight wrappers over the same borrow.
pub struct RuntimeWindow<'rt, RT: RuntimeAPI + ?Sized> {
    rt: &'rt mut RT,
}

#[allow(non_snake_case)]
impl<'rt, RT: RuntimeAPI + ?Sized> RuntimeWindow<'rt, RT> {
    // ---- Construction ----

    /// Create a runtime window around an existing runtime borrow.
    pub fn new(rt: &'rt mut RT) -> Self {
        Self { rt }
    }

    // ---- Frame/window state ----

    /// Access frame timing and profiling data.
    #[inline]
    pub fn Time(&mut self) -> TimeModule<'_, RT> {
        TimeModule::new(self.rt)
    }

    /// Queue window requests and read active refresh data.
    #[inline]
    pub fn Window(&mut self) -> WindowModule<'_, RT> {
        WindowModule::new(self.rt)
    }

    // ---- Scene graph ----

    /// Access scene node creation, deletion, tags, transforms, and fields.
    #[inline]
    pub fn Nodes(&mut self) -> NodeModule<'_, RT> {
        NodeModule::new(self.rt)
    }

    /// Build and run scene node queries.
    #[inline]
    pub fn NodeQuery(&mut self) -> NodeQueryModule<'_, RT> {
        NodeQueryModule::new(self.rt)
    }

    /// Query mesh surfaces and material regions for 3D picking workflows.
    #[inline]
    pub fn MeshQuery(&mut self) -> MeshQueryModule<'_, RT> {
        MeshQueryModule::new(self.rt)
    }

    // ---- Script and signals ----

    /// Access script attachment, variables, methods, and typed state helpers.
    #[inline]
    pub fn Scripts(&mut self) -> ScriptModule<'_, RT> {
        ScriptModule::new(self.rt)
    }

    /// Connect, disconnect, and emit runtime signals.
    #[inline]
    pub fn Signals(&mut self) -> SignalModule<'_, RT> {
        SignalModule::new(self.rt)
    }

    // ---- Simulation ----

    /// Access physics state, forces, raycasts, prediction, and gravity.
    #[inline]
    pub fn Physics(&mut self) -> PhysicsModule<'_, RT> {
        PhysicsModule::new(self.rt)
    }

    /// Control per-node animation players.
    #[inline]
    pub fn AnimPlayer(&mut self) -> AnimPlayerModule<'_, RT> {
        AnimPlayerModule::new(self.rt)
    }

    /// Control animation tree slots and weights.
    #[inline]
    pub fn AnimTree(&mut self) -> AnimTreeModule<'_, RT> {
        AnimTreeModule::new(self.rt)
    }

    // ---- Loading and audio ----

    /// Load, preload, and release scenes.
    #[inline]
    pub fn Scene(&mut self) -> SceneModule<'_, RT> {
        SceneModule::new(self.rt)
    }

    /// Play runtime audio attached to scene nodes.
    #[inline]
    pub fn Audio(&mut self) -> RuntimeAudioModule<'_, RT> {
        RuntimeAudioModule::new(self.rt)
    }

    // ---- Escape hatch ----

    /// Return the underlying runtime borrow for code that must call a raw API.
    #[inline]
    pub fn runtime_mut(&mut self) -> &mut RT {
        self.rt
    }
}
