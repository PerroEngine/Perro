use crate::sub_apis::{NodeAPI, NodeModule, ScriptAPI, ScriptModule, TimeAPI, TimeModule};

pub trait RuntimeAPI: TimeAPI + NodeAPI + ScriptAPI {}
impl<T> RuntimeAPI for T where T: TimeAPI + NodeAPI + ScriptAPI {}

pub struct RuntimeContext<'rt, R: RuntimeAPI + ?Sized> {
    rt: &'rt mut R,
}

#[allow(non_snake_case)]
impl<'rt, R: RuntimeAPI + ?Sized> RuntimeContext<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    #[inline]
    pub fn Time(&mut self) -> TimeModule<'_, R> {
        TimeModule::new(self.rt)
    }

    #[inline]
    pub fn Nodes(&mut self) -> NodeModule<'_, R> {
        NodeModule::new(self.rt)
    }

    #[inline]
    pub fn Scripts(&mut self) -> ScriptModule<'_, R> {
        ScriptModule::new(self.rt)
    }
}
