use crate::modules::{
    JSONModule, NodeAPI, NodeModule, ScriptAPI, ScriptModule, TimeAPI, TimeModule,
};

pub trait RuntimeAPI: TimeAPI + NodeAPI + ScriptAPI {}
impl<T> RuntimeAPI for T where T: TimeAPI + NodeAPI + ScriptAPI {}

pub struct API<'rt, R: RuntimeAPI + ?Sized> {
    rt: &'rt R,
}

#[allow(non_snake_case)]
impl<'rt, R: RuntimeAPI + ?Sized> API<'rt, R> {
    pub fn new(rt: &'rt R) -> Self {
        Self { rt }
    }

    #[inline]
    pub fn Time(&self) -> TimeModule<'_, R> {
        TimeModule::new(self.rt)
    }

    #[inline]
    pub fn Nodes(&self) -> NodeModule<'_, R> {
        NodeModule::new(self.rt)
    }

    #[inline]
    pub fn Scripts(&self) -> ScriptModule<'_, R> {
        ScriptModule::new(self.rt)
    }

    #[inline]
    pub fn Json(&self) -> JSONModule {
        JSONModule::new()
    }
}
