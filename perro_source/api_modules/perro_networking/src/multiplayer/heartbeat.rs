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
    pub fn from_secs_f32(interval: f32, timeout: f32) -> Result<Self, String> {
        if !interval.is_finite() || interval < 0.0 {
            return Err("heartbeat interval must be finite and nonnegative".into());
        }
        if !timeout.is_finite() || timeout < 0.0 {
            return Err("heartbeat timeout must be finite and nonnegative".into());
        }
        let interval = Duration::try_from_secs_f32(interval)
            .map_err(|_| "heartbeat interval is too large".to_string())?;
        let timeout = Duration::try_from_secs_f32(timeout)
            .map_err(|_| "heartbeat timeout is too large".to_string())?;
        Ok(Self::new(interval, timeout))
    }
}

impl Default for HeartbeatConfig {
    /// 1s between beats, 5s of silence before a peer is dropped (~4 missed
    /// beats of tolerance, so a stutter or brief stall won't false-trip).
    fn default() -> Self {
        Self::new(Duration::from_secs(1), Duration::from_secs(5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_constructor_rejects_invalid_durations() {
        for value in [-1.0, f32::NAN, f32::INFINITY, f32::MAX] {
            assert!(HeartbeatConfig::from_secs_f32(value, 1.0).is_err());
            assert!(HeartbeatConfig::from_secs_f32(1.0, value).is_err());
        }
        assert_eq!(
            HeartbeatConfig::from_secs_f32(1.0, 5.0)
                .expect("valid heartbeat")
                .timeout,
            Duration::from_secs(5)
        );
    }
}
