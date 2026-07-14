//! Named one-shot runtime timers.

use perro_ids::{SignalID, TimerID};
use std::time::Duration;

#[doc(hidden)]
pub fn timer_signal_ids(name: &str) -> (TimerID, SignalID, SignalID) {
    let mut started = String::with_capacity(name.len() + "_started".len());
    started.push_str(name);
    started.push_str("_started");
    let mut finished = String::with_capacity(name.len() + "_finished".len());
    finished.push_str(name);
    finished.push_str("_finished");
    (
        TimerID::from_string(name),
        SignalID::from_string(&started),
        SignalID::from_string(&finished),
    )
}

pub trait TimerAPI {
    fn timer_start(
        &mut self,
        duration: Duration,
        timer: TimerID,
        started: SignalID,
        finished: SignalID,
    );
    fn timer_cancel(&mut self, timer: TimerID) -> bool;
    fn timer_is_active(&self, timer: TimerID) -> bool;
    fn timer_remaining(&self, timer: TimerID) -> Option<Duration>;
}

pub struct TimerModule<'rt, R: TimerAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: TimerAPI + ?Sized> TimerModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn start(
        &mut self,
        duration: Duration,
        timer: TimerID,
        started: SignalID,
        finished: SignalID,
    ) {
        self.rt.timer_start(duration, timer, started, finished);
    }

    pub fn cancel(&mut self, timer: TimerID) -> bool {
        self.rt.timer_cancel(timer)
    }

    pub fn is_active(&self, timer: TimerID) -> bool {
        self.rt.timer_is_active(timer)
    }

    pub fn remaining(&self, timer: TimerID) -> Option<Duration> {
        self.rt.timer_remaining(timer)
    }
}

/// Start or reset a named one-shot timer.
///
/// Emits `<name>_started` at once and `<name>_finished` at expiry.
#[macro_export]
macro_rules! timer_start {
    ($ctx:expr, $duration:expr, $name:literal) => {{
        const __TIMER: $crate::perro_ids::TimerID = $crate::perro_ids::TimerID::from_string($name);
        const __STARTED: $crate::perro_ids::SignalID =
            $crate::perro_ids::SignalID::from_string(concat!($name, "_started"));
        const __FINISHED: $crate::perro_ids::SignalID =
            $crate::perro_ids::SignalID::from_string(concat!($name, "_finished"));
        $ctx.Timers()
            .start($duration, __TIMER, __STARTED, __FINISHED)
    }};
    ($ctx:expr, $duration:expr, $name:expr) => {{
        let __name = &$name;
        let (__timer, __started, __finished) =
            $crate::sub_apis::timer_signal_ids(::core::convert::AsRef::<str>::as_ref(__name));
        $ctx.Timers()
            .start($duration, __timer, __started, __finished)
    }};
}

#[macro_export]
macro_rules! timer_cancel {
    ($ctx:expr, $name:literal) => {{
        const __TIMER: $crate::perro_ids::TimerID = $crate::perro_ids::TimerID::from_string($name);
        $ctx.Timers().cancel(__TIMER)
    }};
    ($ctx:expr, $name:expr) => {{
        let __name = &$name;
        let __timer =
            $crate::perro_ids::TimerID::from_string(::core::convert::AsRef::<str>::as_ref(__name));
        $ctx.Timers().cancel(__timer)
    }};
}

#[macro_export]
macro_rules! timer_is_active {
    ($ctx:expr, $name:literal) => {{
        const __TIMER: $crate::perro_ids::TimerID = $crate::perro_ids::TimerID::from_string($name);
        $ctx.Timers().is_active(__TIMER)
    }};
    ($ctx:expr, $name:expr) => {{
        let __name = &$name;
        let __timer =
            $crate::perro_ids::TimerID::from_string(::core::convert::AsRef::<str>::as_ref(__name));
        $ctx.Timers().is_active(__timer)
    }};
}

#[macro_export]
macro_rules! timer_remaining {
    ($ctx:expr, $name:literal) => {{
        const __TIMER: $crate::perro_ids::TimerID = $crate::perro_ids::TimerID::from_string($name);
        $ctx.Timers().remaining(__TIMER)
    }};
    ($ctx:expr, $name:expr) => {{
        let __name = &$name;
        let __timer =
            $crate::perro_ids::TimerID::from_string(::core::convert::AsRef::<str>::as_ref(__name));
        $ctx.Timers().remaining(__timer)
    }};
}

#[macro_export]
macro_rules! timer_started {
    ($name:literal) => {{
        const __SIGNAL: $crate::perro_ids::SignalID =
            $crate::perro_ids::SignalID::from_string(concat!($name, "_started"));
        __SIGNAL
    }};
    ($name:expr) => {{
        let __name = &$name;
        let (_, __started, _) =
            $crate::sub_apis::timer_signal_ids(::core::convert::AsRef::<str>::as_ref(__name));
        __started
    }};
}

#[macro_export]
macro_rules! timer_finished {
    ($name:literal) => {{
        const __SIGNAL: $crate::perro_ids::SignalID =
            $crate::perro_ids::SignalID::from_string(concat!($name, "_finished"));
        __SIGNAL
    }};
    ($name:expr) => {{
        let __name = &$name;
        let (_, _, __finished) =
            $crate::sub_apis::timer_signal_ids(::core::convert::AsRef::<str>::as_ref(__name));
        __finished
    }};
}
