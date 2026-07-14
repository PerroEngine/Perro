use ahash::AHashMap;
use perro_ids::{SignalID, TimerID};
use std::{cmp::Ordering, collections::BinaryHeap, time::Duration};

#[derive(Clone, Copy)]
struct ActiveTimer {
    deadline: f64,
    generation: u64,
    finished: SignalID,
}

#[derive(Clone, Copy)]
struct Deadline {
    at: f64,
    sequence: u64,
    timer: TimerID,
    generation: u64,
}

impl PartialEq for Deadline {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at && self.sequence == other.sequence
    }
}

impl Eq for Deadline {}

impl PartialOrd for Deadline {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Deadline {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .at
            .total_cmp(&self.at)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

pub(crate) struct TimerRuntimeState {
    clock: f64,
    next_generation: u64,
    deadlines: BinaryHeap<Deadline>,
    active: AHashMap<TimerID, ActiveTimer>,
    stale_deadlines: usize,
    due_scratch: Vec<SignalID>,
}

impl TimerRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            clock: 0.0,
            next_generation: 1,
            deadlines: BinaryHeap::new(),
            active: AHashMap::new(),
            stale_deadlines: 0,
            due_scratch: Vec::new(),
        }
    }

    pub(crate) fn start(&mut self, timer: TimerID, finished: SignalID, duration: Duration) -> bool {
        if duration.is_zero() {
            self.stale_deadlines += self.active.remove(&timer).is_some() as usize;
            self.maybe_compact();
            return true;
        }

        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let deadline = self.clock + duration.as_secs_f64();
        let replaced = self.active.insert(
            timer,
            ActiveTimer {
                deadline,
                generation,
                finished,
            },
        );
        self.stale_deadlines += replaced.is_some() as usize;
        self.deadlines.push(Deadline {
            at: deadline,
            sequence: generation,
            timer,
            generation,
        });
        self.maybe_compact();
        false
    }

    pub(crate) fn cancel(&mut self, timer: TimerID) -> bool {
        let removed = self.active.remove(&timer).is_some();
        self.stale_deadlines += removed as usize;
        self.maybe_compact();
        removed
    }

    pub(crate) fn is_active(&self, timer: TimerID) -> bool {
        self.active.contains_key(&timer)
    }

    pub(crate) fn remaining(&self, timer: TimerID) -> Option<Duration> {
        self.active
            .get(&timer)
            .map(|entry| Duration::from_secs_f64((entry.deadline - self.clock).max(0.0)))
    }

    pub(crate) fn advance(&mut self, delta_seconds: f32) -> Vec<SignalID> {
        if delta_seconds.is_finite() {
            self.clock += delta_seconds.max(0.0) as f64;
        }
        let mut due = std::mem::take(&mut self.due_scratch);
        due.clear();

        while self
            .deadlines
            .peek()
            .is_some_and(|entry| entry.at <= self.clock)
        {
            let entry = self.deadlines.pop().expect("peeked timer deadline");
            let Some(active) = self.active.get(&entry.timer).copied() else {
                self.stale_deadlines = self.stale_deadlines.saturating_sub(1);
                continue;
            };
            if active.generation != entry.generation {
                self.stale_deadlines = self.stale_deadlines.saturating_sub(1);
                continue;
            }
            self.active.remove(&entry.timer);
            due.push(active.finished);
        }
        due
    }

    fn maybe_compact(&mut self) {
        const MIN_STALE_TO_COMPACT: usize = 64;
        if self.stale_deadlines < MIN_STALE_TO_COMPACT || self.stale_deadlines <= self.active.len()
        {
            return;
        }

        let active = &self.active;
        self.deadlines.retain(|entry| {
            active
                .get(&entry.timer)
                .is_some_and(|timer| timer.generation == entry.generation)
        });
        self.stale_deadlines = 0;
    }

    #[cfg(any(test, feature = "bench"))]
    pub(crate) fn counts(&self) -> (usize, usize, usize) {
        (
            self.active.len(),
            self.deadlines.len(),
            self.stale_deadlines,
        )
    }

    pub(crate) fn reuse_due(&mut self, mut due: Vec<SignalID>) {
        due.clear();
        self.due_scratch = due;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_skips_old_deadline() {
        let timer = TimerID::from_string("wait");
        let finished = SignalID::from_string("wait_finished");
        let mut state = TimerRuntimeState::new();
        assert!(!state.start(timer, finished, Duration::from_secs(1)));
        let due = state.advance(0.5);
        assert!(due.is_empty());
        state.reuse_due(due);
        assert!(!state.start(timer, finished, Duration::from_secs(1)));
        let due = state.advance(0.5);
        assert!(due.is_empty());
        state.reuse_due(due);
        assert_eq!(state.advance(0.5), vec![finished]);
    }

    #[test]
    fn cancel_and_zero_duration() {
        let timer = TimerID::from_string("wait");
        let finished = SignalID::from_string("wait_finished");
        let mut state = TimerRuntimeState::new();
        assert!(!state.start(timer, finished, Duration::from_secs(2)));
        assert!(state.cancel(timer));
        assert!(!state.is_active(timer));
        assert!(state.start(timer, finished, Duration::ZERO));
    }

    #[test]
    fn reset_storm_compacts_stale_deadlines() {
        let timer = TimerID::from_string("wait");
        let finished = SignalID::from_string("wait_finished");
        let mut state = TimerRuntimeState::new();
        for millis in 1..=10_000 {
            state.start(timer, finished, Duration::from_millis(millis));
        }
        let (active, deadlines, stale) = state.counts();
        assert_eq!(active, 1);
        assert!(deadlines < 64, "deadline heap grew to {deadlines}");
        assert!(stale < 64, "stale count grew to {stale}");
    }
}
