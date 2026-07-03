//! Raises the OS timer resolution for the lifetime of the app on Windows.
//!
//! Default Windows timer resolution is ~15.6ms, which makes `thread::sleep`
//! overshoot badly and forces frame-pacing code to fall back to busy-spinning
//! for high-rate caps. Requesting 1ms resolution (`timeBeginPeriod(1)`) lets
//! sleep-based waits land within ~1-2ms, so the sim/render loops can sleep
//! most of the remaining interval instead of spinning a full core.
//!
//! This is process-wide OS state, so it is acquired once near app startup
//! and released via `Drop` when the app exits.

#[cfg(windows)]
pub struct TimerResolutionGuard {
    active: bool,
}

#[cfg(windows)]
impl TimerResolutionGuard {
    /// Request 1ms system timer resolution. Best-effort: if the OS call
    /// fails, the guard is a no-op and callers fall back to prior behavior.
    pub fn acquire_1ms() -> Self {
        // SAFETY: timeBeginPeriod is safe to call with a valid period; winmm
        // is always available on Windows. Failure is a soft-fail (TIMERR_NOCANDO).
        let result = unsafe { windows_sys::Win32::Media::timeBeginPeriod(1) };
        Self {
            active: result == 0, // TIMERR_NOERROR == 0
        }
    }
}

#[cfg(windows)]
impl Drop for TimerResolutionGuard {
    fn drop(&mut self) {
        if self.active {
            // SAFETY: matches the successful timeBeginPeriod(1) call above.
            unsafe {
                windows_sys::Win32::Media::timeEndPeriod(1);
            }
        }
    }
}

#[cfg(not(windows))]
pub struct TimerResolutionGuard;

#[cfg(not(windows))]
impl TimerResolutionGuard {
    pub fn acquire_1ms() -> Self {
        Self
    }
}
