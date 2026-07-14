use std::time::Duration;

/// Session-level liveness policy. The session sends a heartbeat frame whenever
/// it has been silent for `interval` (real traffic counts as a heartbeat), and
/// treats a peer as gone once nothing has arrived from it within `timeout`.
///
/// Because liveness lives in the session — not the transport — local and Steam
/// get identical disconnect behavior. Games should rely on this instead of
/// rolling their own heartbeat; disable it here if a game wants to opt out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeartbeatConfig {
    pub enabled: bool,
    pub interval: Duration,
    pub timeout: Duration,
}

impl HeartbeatConfig {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::new(Duration::from_secs(1), Duration::from_secs(5))
        }
    }

    pub const fn new(interval: Duration, timeout: Duration) -> Self {
        Self {
            enabled: true,
            interval,
            timeout,
        }
    }

    /// Convenience for game code that thinks in float seconds.
    pub fn from_secs_f32(interval: f32, timeout: f32) -> Self {
        Self::new(
            Duration::from_secs_f32(interval),
            Duration::from_secs_f32(timeout),
        )
    }
}

impl Default for HeartbeatConfig {
    /// 1s between beats, 5s of silence before a peer is dropped (~4 missed
    /// beats of tolerance, so a stutter or brief stall won't false-trip).
    fn default() -> Self {
        Self::new(Duration::from_secs(1), Duration::from_secs(5))
    }
}
