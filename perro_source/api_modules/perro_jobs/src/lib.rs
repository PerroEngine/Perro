//! Small parallel job API backed by Perro's shared Rayon pool.

use std::any::Any;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};

/// Error returned when worker code panics or its result channel closes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobError {
    /// Worker code panicked. The string contains a panic message when available.
    Panic(String),
    /// Worker stopped before it sent a result.
    Canceled,
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Panic(message) => write!(f, "job panicked: {message}"),
            Self::Canceled => f.write_str("job canceled"),
        }
    }
}

impl std::error::Error for JobError {}

/// Result handle for work submitted with [`spawn`].
#[derive(Debug)]
pub struct Job<T> {
    receiver: Receiver<Result<T, JobError>>,
}

impl<T> Job<T> {
    /// Return a finished result without blocking.
    pub fn try_take(&mut self) -> Result<Option<T>, JobError> {
        match self.receiver.try_recv() {
            Ok(Ok(value)) => Ok(Some(value)),
            Ok(Err(error)) => Err(error),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(JobError::Canceled),
        }
    }

    /// Wait for work and return its result.
    pub fn take(self) -> Result<T, JobError> {
        self.receiver.recv().unwrap_or(Err(JobError::Canceled))
    }
}

/// Submit owned CPU work to Perro's shared worker pool.
///
/// Stable web builds run the closure inline.
pub fn spawn<F, T>(work: F) -> Job<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = sync_channel(1);
        rayon::spawn(move || {
            let result = run_caught(work);
            let _ = sender.send(result);
        });
        Job { receiver }
    }

    #[cfg(target_arch = "wasm32")]
    {
        let (sender, receiver) = sync_channel(1);
        let _ = sender.send(run_caught(work));
        Job { receiver }
    }
}

/// Run two closures in parallel and return both results.
pub fn join<A, B, RA, RB>(left: A, right: B) -> (RA, RB)
where
    A: FnOnce() -> RA + Send,
    B: FnOnce() -> RB + Send,
    RA: Send,
    RB: Send,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        rayon::join(left, right)
    }

    #[cfg(target_arch = "wasm32")]
    {
        (left(), right())
    }
}

/// Map owned items in parallel while preserving input order.
pub fn par_map<T, R, F>(items: Vec<T>, map: F) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> R + Send + Sync,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        items.into_par_iter().map(map).collect()
    }

    #[cfg(target_arch = "wasm32")]
    {
        items.into_iter().map(map).collect()
    }
}

fn run_caught<F, T>(work: F) -> Result<T, JobError>
where
    F: FnOnce() -> T,
{
    catch_unwind(AssertUnwindSafe(work)).map_err(|payload| JobError::Panic(panic_message(payload)))
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_returns_typed_result() {
        let job = spawn(|| 6 * 7);
        assert_eq!(job.take(), Ok(42));
    }

    #[test]
    fn panic_returns_error() {
        let job = spawn(|| panic!("boom"));
        assert_eq!(job.take(), Err(JobError::Panic("boom".to_owned())));
    }

    #[test]
    fn join_returns_both_results() {
        assert_eq!(join(|| 20, || 22), (20, 22));
    }

    #[test]
    fn par_map_keeps_order() {
        assert_eq!(par_map(vec![3, 1, 2], |value| value * 2), vec![6, 2, 4]);
    }
}
