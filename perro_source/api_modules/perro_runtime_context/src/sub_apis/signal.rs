use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_variant::Variant;

pub trait SignalAPI {
    fn signal_connect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool;

    fn signal_disconnect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool;

    fn signal_emit(&mut self, signal: SignalID, params: &[Variant]) -> usize;
}

pub struct SignalModule<'rt, R: SignalAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: SignalAPI + ?Sized> SignalModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn signal_connect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool {
        self.rt.signal_connect(script_id, signal, function)
    }

    pub fn signal_disconnect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool {
        self.rt.signal_disconnect(script_id, signal, function)
    }

    pub fn signal_emit(&mut self, signal: SignalID, params: &[Variant]) -> usize {
        self.rt.signal_emit(signal, params)
    }
}

/// Connects a signal to a script function handler.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `script`: script `NodeID`
/// - `signal`: `SignalID` (for example `signal!("on_hit")`)
/// - `function`: `ScriptMemberID` (for example `method!("handle_hit")`)
#[macro_export]
macro_rules! signal_connect {
    ($ctx:expr, $script:expr, $signal:expr, $function:expr) => {
        $ctx.Signals().signal_connect($script, $signal, $function)
    };
}

/// Disconnects a signal-function connection.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `script`: script `NodeID`
/// - `signal`: `SignalID`
/// - `function`: `ScriptMemberID`
#[macro_export]
macro_rules! signal_disconnect {
    ($ctx:expr, $script:expr, $signal:expr, $function:expr) => {
        $ctx.Signals()
            .signal_disconnect($script, $signal, $function)
    };
}

/// Emits a signal globally through the runtime signal bus.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `signal`: `SignalID`
/// - `params` (optional): `&[Variant]`
#[macro_export]
macro_rules! signal_emit {
    ($ctx:expr, $signal:expr, $params:expr) => {
        $ctx.Signals().signal_emit($signal, $params)
    };
    ($ctx:expr, $signal:expr) => {
        $ctx.Signals().signal_emit($signal, &[])
    };
}
