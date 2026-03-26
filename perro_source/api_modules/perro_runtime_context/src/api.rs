use crate::sub_apis::{
    AnimPlayerAPI, AnimPlayerModule, NodeAPI, NodeModule, PhysicsAPI, PhysicsModule, SceneAPI,
    SceneModule, ScriptAPI, ScriptModule, SignalAPI, SignalModule, TimeAPI, TimeModule,
};

pub trait RuntimeAPI:
    TimeAPI + NodeAPI + ScriptAPI + SignalAPI + PhysicsAPI + AnimPlayerAPI + SceneAPI
{
}
impl<T> RuntimeAPI for T where
    T: TimeAPI + NodeAPI + ScriptAPI + SignalAPI + PhysicsAPI + AnimPlayerAPI + SceneAPI
{
}

pub struct RuntimeContext<'rt, RT: RuntimeAPI + ?Sized> {
    rt: &'rt mut RT,
}

#[allow(non_snake_case)]
impl<'rt, RT: RuntimeAPI + ?Sized> RuntimeContext<'rt, RT> {
    pub fn new(rt: &'rt mut RT) -> Self {
        Self { rt }
    }

    #[inline]
    pub fn Time(&mut self) -> TimeModule<'_, RT> {
        TimeModule::new(self.rt)
    }

    #[inline]
    pub fn Nodes(&mut self) -> NodeModule<'_, RT> {
        NodeModule::new(self.rt)
    }

    #[inline]
    pub fn Scripts(&mut self) -> ScriptModule<'_, RT> {
        ScriptModule::new(self.rt)
    }

    #[inline]
    pub fn Signals(&mut self) -> SignalModule<'_, RT> {
        SignalModule::new(self.rt)
    }

    #[inline]
    pub fn Physics(&mut self) -> PhysicsModule<'_, RT> {
        PhysicsModule::new(self.rt)
    }

    #[inline]
    pub fn AnimPlayer(&mut self) -> AnimPlayerModule<'_, RT> {
        AnimPlayerModule::new(self.rt)
    }

    #[inline]
    pub fn Scene(&mut self) -> SceneModule<'_, RT> {
        SceneModule::new(self.rt)
    }

    #[inline]
    pub fn runtime_mut(&mut self) -> &mut RT {
        self.rt
    }
}
