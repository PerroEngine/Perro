use perro_ids::{SignalID, TimerID};
use perro_runtime_api::sub_apis::{SignalAPI, TimerAPI};
use std::time::Duration;

use crate::Runtime;

impl TimerAPI for Runtime {
    fn timer_start(
        &mut self,
        duration: Duration,
        timer: TimerID,
        started: SignalID,
        finished: SignalID,
    ) {
        let finish_at_once = self.timer_runtime.start(timer, finished, duration);
        self.signal_emit(started, &[]);
        if finish_at_once {
            self.signal_emit(finished, &[]);
        }
    }

    fn timer_cancel(&mut self, timer: TimerID) -> bool {
        self.timer_runtime.cancel(timer)
    }

    fn timer_is_active(&self, timer: TimerID) -> bool {
        self.timer_runtime.is_active(timer)
    }

    fn timer_remaining(&self, timer: TimerID) -> Option<Duration> {
        self.timer_runtime.remaining(timer)
    }
}
