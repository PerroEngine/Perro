//! Runtime signal API.
//!
//! Connects script methods to named signals and emits signal payloads.

use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_variant::Variant;
use std::borrow::Borrow;

pub trait SignalAPI {
    fn signal_connect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
        params: &[Variant],
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
        params: &[Variant],
    ) -> bool {
        self.rt.signal_connect(script_id, signal, function, params)
    }

    pub fn signal_connect_many<S, F>(
        &mut self,
        script_id: NodeID,
        signals: S,
        functions: F,
        params: &[Variant],
    ) -> usize
    where
        S: IntoIterator,
        S::Item: Borrow<SignalID>,
        F: IntoIterator,
        F::Item: Borrow<ScriptMemberID>,
    {
        let functions: Vec<ScriptMemberID> = functions
            .into_iter()
            .map(|function| *function.borrow())
            .collect();
        if functions.is_empty() {
            return 0;
        }

        let mut connected = 0usize;
        for signal in signals {
            let signal = *signal.borrow();
            for function in functions.iter().copied() {
                connected += self.rt.signal_connect(script_id, signal, function, params) as usize;
            }
        }
        connected
    }

    pub fn signal_disconnect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool {
        self.rt.signal_disconnect(script_id, signal, function)
    }

    pub fn signal_disconnect_many<S, F>(
        &mut self,
        script_id: NodeID,
        signals: S,
        functions: F,
    ) -> usize
    where
        S: IntoIterator,
        S::Item: Borrow<SignalID>,
        F: IntoIterator,
        F::Item: Borrow<ScriptMemberID>,
    {
        let functions: Vec<ScriptMemberID> = functions
            .into_iter()
            .map(|function| *function.borrow())
            .collect();
        if functions.is_empty() {
            return 0;
        }

        let mut disconnected = 0usize;
        for signal in signals {
            let signal = *signal.borrow();
            for function in functions.iter().copied() {
                disconnected += self.rt.signal_disconnect(script_id, signal, function) as usize;
            }
        }
        disconnected
    }

    pub fn signal_emit(&mut self, signal: SignalID, params: &[Variant]) -> usize {
        self.rt.signal_emit(signal, params)
    }
}

/// Connects a signal to a script function handler.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `script`: script `NodeID`
/// - `signal`: `SignalID` (for example `signal!("on_hit")`)
/// - `function`: `ScriptMemberID` (for example `method!("handle_hit")`)
/// - `params` (optional): extra params appended after emitted params
#[macro_export]
macro_rules! signal_connect {
    ($ctx:expr, $script:expr, $signal:expr, $function:expr, $params:expr) => {
        $ctx.Signals()
            .signal_connect($script, $signal, $function, $params)
    };
    ($ctx:expr, $script:expr, $signal:expr, $function:expr) => {
        $ctx.Signals()
            .signal_connect($script, $signal, $function, &[])
    };
}

/// Connects many signals to many script function handlers.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `script`: script `NodeID`
/// - `signals`: slice, array, vec, or iterator of `SignalID`
/// - `functions`: slice, array, vec, or iterator of `ScriptMemberID`
/// - `params` (optional): extra params appended after emitted params
///
/// Returns number of new connections.
#[macro_export]
macro_rules! signal_connect_many {
    ($ctx:expr, $script:expr, $signals:expr, $functions:expr, $params:expr) => {
        $ctx.Signals()
            .signal_connect_many($script, $signals, $functions, $params)
    };
    ($ctx:expr, $script:expr, $signals:expr, $functions:expr) => {
        $ctx.Signals()
            .signal_connect_many($script, $signals, $functions, &[])
    };
}

/// Disconnects a signal-function connection.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
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

/// Disconnects many signal-function connections.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `script`: script `NodeID`
/// - `signals`: slice, array, vec, or iterator of `SignalID`
/// - `functions`: slice, array, vec, or iterator of `ScriptMemberID`
///
/// Returns number of removed connections.
#[macro_export]
macro_rules! signal_disconnect_many {
    ($ctx:expr, $script:expr, $signals:expr, $functions:expr) => {
        $ctx.Signals()
            .signal_disconnect_many($script, $signals, $functions)
    };
}

/// Emits a signal globally through the runtime signal bus.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
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
