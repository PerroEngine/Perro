use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_variant::Variant;

pub trait SignalAPI {
    fn connect_signal(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool;

    fn disconnect_signal(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool;

    fn emit_signal(&mut self, signal: SignalID, params: &[Variant]) -> usize;
}

pub struct SignalModule<'rt, R: SignalAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: SignalAPI + ?Sized> SignalModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn connect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool {
        self.rt.connect_signal(script_id, signal, function)
    }

    pub fn disconnect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool {
        self.rt.disconnect_signal(script_id, signal, function)
    }

    pub fn emit(&mut self, signal: SignalID, params: &[Variant]) -> usize {
        self.rt.emit_signal(signal, params)
    }
}

#[macro_export]
macro_rules! connect_signal {
    ($ctx:expr, $script:expr, $signal:expr, $function:expr) => {
        $ctx.Signals()
            .connect($script, $signal, $function)
    };
}

#[macro_export]
macro_rules! disconnect_signal {
    ($ctx:expr, $script:expr, $signal:expr, $function:expr) => {
        $ctx.Signals()
            .disconnect($script, $signal, $function)
    };
}

#[macro_export]
macro_rules! emit_signal {
    ($ctx:expr, $signal:expr, $params:expr) => {
        $ctx.Signals().emit($signal, $params)
    };
    ($ctx:expr, $signal:expr) => {
        $ctx.Signals().emit($signal, &[])
    };
}


