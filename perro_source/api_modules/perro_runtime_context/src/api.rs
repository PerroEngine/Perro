use crate::sub_apis::{
    NodeAPI, NodeModule, PhysicsAPI, PhysicsModule, ScriptAPI, ScriptModule, SignalAPI,
    SignalModule, TimeAPI, TimeModule,
};

pub trait RuntimeAPI: TimeAPI + NodeAPI + ScriptAPI + SignalAPI + PhysicsAPI {}
impl<T> RuntimeAPI for T where T: TimeAPI + NodeAPI + ScriptAPI + SignalAPI + PhysicsAPI {}

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
    pub fn runtime_mut(&mut self) -> &mut RT {
        self.rt
    }
}
