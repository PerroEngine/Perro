//! `PERRO_SIM=<preset>` dev perf simulation: force low-end GPU quality + fewer
//! CPU worker threads than the dev machine really has.
//!
//! Point is profiling, not emulation. Two axes, both exact rather than faked:
//!
//! * GPU: flip the same `constrained_adapter` policy an integrated adapter
//!   already trips (1080p scene cap, msaa off + fxaa swap, ssao low, small
//!   shadow atlas, `MemoryHints::MemoryUsage`) on a discrete card, and
//!   optionally request the `LowPower` adapter so a laptop actually runs on
//!   its iGPU. No shader-level throttle: a big card stays a big card, only
//!   the quality tier moves.
//! * CPU: cap the shared rayon pool + every worker-count decision to N
//!   threads. That is a real core count cut, not a stall injection.
//!
//! Off unless `PERRO_SIM` is set. Parsed once at startup.

use std::sync::OnceLock;

/// Resolved simulation settings for this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimProfile {
    /// Force the low-end GPU quality policy on any adapter.
    pub gpu_constrained: bool,
    /// Ask wgpu for the `LowPower` adapter (real iGPU on hybrid machines).
    pub prefer_low_power: bool,
    /// Extra scene-resolution ceiling, on top of the constrained 1080p cap.
    pub max_render_pixels: Option<u64>,
    /// Worker-thread ceiling for the shared pool + parallel dispatch.
    pub cpu_cores: Option<usize>,
}

impl SimProfile {
    /// No simulation: run at full machine capability.
    pub const OFF: Self = Self {
        gpu_constrained: false,
        prefer_low_power: false,
        max_render_pixels: None,
        cpu_cores: None,
    };

    /// True when any knob deviates from the host machine.
    pub const fn active(&self) -> bool {
        self.gpu_constrained
            || self.prefer_low_power
            || self.max_render_pixels.is_some()
            || self.cpu_cores.is_some()
    }
}

impl Default for SimProfile {
    fn default() -> Self {
        Self::OFF
    }
}

static PROFILE: OnceLock<SimProfile> = OnceLock::new();

/// Parse `PERRO_SIM`, install the capped worker pool, and log the result.
///
/// Call as early in `main` as the entry point allows: the thread-pool cap only
/// applies to a pool that has not been built yet. Later calls are ignored.
pub fn init() {
    if PROFILE.get().is_some() {
        return;
    }
    let profile = PROFILE.get_or_init(parse_env);
    if !profile.active() {
        return;
    }
    install_thread_pool(profile.cpu_cores);
    log_profile(profile);
}

/// Resolved profile. Parses on first use when [`init`] never ran, so a caller
/// that skipped init still sees the right settings (minus the pool cap).
pub fn profile() -> &'static SimProfile {
    PROFILE.get_or_init(parse_env)
}

/// True when any simulation knob is on.
pub fn active() -> bool {
    profile().active()
}

/// Worker count for parallel dispatch, capped by `PERRO_SIM` when set.
///
/// Every site that used to read `available_parallelism` directly goes through
/// here, so a core-capped run does not silently keep splitting work N ways.
pub fn worker_count() -> usize {
    let machine = machine_parallelism();
    match profile().cpu_cores {
        Some(cores) => cores.clamp(1, machine),
        None => machine,
    }
}

fn machine_parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
}

#[cfg(target_arch = "wasm32")]
fn parse_env() -> SimProfile {
    SimProfile::OFF
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_env() -> SimProfile {
    match std::env::var("PERRO_SIM") {
        Ok(spec) => parse_spec(&spec, machine_parallelism()),
        Err(_) => SimProfile::OFF,
    }
}

/// Parse a `PERRO_SIM` spec against a machine core count.
///
/// Comma-separated tokens, later tokens win: preset names (`off`, `igpu`,
/// `low_end`, `half`, `potato`) plus explicit overrides `cores=N`,
/// `gpu=off|constrained|igpu`, and `pixels=WxH`.
pub fn parse_spec(spec: &str, machine_cores: usize) -> SimProfile {
    let machine_cores = machine_cores.max(1);
    let mut profile = SimProfile::OFF;
    for token in spec.split(',') {
        if let Err(err) = apply_token(&mut profile, token, machine_cores) {
            eprintln!("[perro][sim] {err}; ignored");
        }
    }
    profile
}

/// Check a spec without applying it, so a CLI flag can reject a typo up front
/// instead of launching a run that silently simulates nothing.
pub fn validate_spec(spec: &str) -> Result<(), String> {
    let mut profile = SimProfile::OFF;
    for token in spec.split(',') {
        apply_token(&mut profile, token, machine_parallelism())?;
    }
    Ok(())
}

/// Preset names accepted by a spec, for help text.
pub const PRESETS: &[&str] = &["off", "igpu", "low_end", "half", "potato"];

fn apply_token(profile: &mut SimProfile, token: &str, machine_cores: usize) -> Result<(), String> {
    let token = token.trim().to_ascii_lowercase();
    if token.is_empty() {
        return Ok(());
    }
    if let Some(preset) = preset(&token, machine_cores) {
        *profile = preset;
        return Ok(());
    }
    let Some((key, value)) = token.split_once('=') else {
        return Err(format!(
            "unknown token `{token}` (presets: {})",
            PRESETS.join(", ")
        ));
    };
    match key {
        "cores" | "cpu" | "threads" => match value.parse::<usize>() {
            Ok(cores) if cores >= 1 => profile.cpu_cores = Some(cores.min(machine_cores)),
            _ => return Err(format!("bad core count `{value}`")),
        },
        "gpu" => match value {
            "off" | "full" => {
                profile.gpu_constrained = false;
                profile.prefer_low_power = false;
            }
            "constrained" | "low" | "low_end" | "lowend" => profile.gpu_constrained = true,
            "igpu" => {
                profile.gpu_constrained = true;
                profile.prefer_low_power = true;
            }
            _ => return Err(format!("unknown gpu mode `{value}`")),
        },
        "pixels" | "res" | "resolution" => match parse_pixels(value) {
            Some(pixels) => profile.max_render_pixels = Some(pixels),
            None => return Err(format!("bad resolution `{value}`")),
        },
        _ => {
            return Err(format!(
                "unknown token `{token}` (presets: {})",
                PRESETS.join(", ")
            ));
        }
    }
    Ok(())
}

fn preset(name: &str, machine_cores: usize) -> Option<SimProfile> {
    match name {
        "off" | "none" => Some(SimProfile::OFF),
        // Quality tier of an integrated adapter, and the real iGPU when the
        // machine has one. Core count left alone: that is a separate axis.
        "igpu" => Some(SimProfile {
            gpu_constrained: true,
            prefer_low_power: true,
            ..SimProfile::OFF
        }),
        // Budget desktop: low-end GPU tier on whatever card is present, 4 cores.
        "low_end" | "lowend" | "low" => Some(SimProfile {
            gpu_constrained: true,
            cpu_cores: Some(4.min(machine_cores)),
            ..SimProfile::OFF
        }),
        // "half as capable as this machine": half the cores, low-end GPU tier.
        "half" => Some(SimProfile {
            gpu_constrained: true,
            cpu_cores: Some(machine_cores.div_ceil(2)),
            ..SimProfile::OFF
        }),
        // Floor: iGPU tier at 720p on 2 cores.
        "potato" => Some(SimProfile {
            gpu_constrained: true,
            prefer_low_power: true,
            max_render_pixels: Some(1280 * 720),
            cpu_cores: Some(2.min(machine_cores)),
        }),
        _ => None,
    }
}

fn parse_pixels(value: &str) -> Option<u64> {
    let (width, height) = value.split_once(['x', 'X', '*'])?;
    let width = width.trim().parse::<u64>().ok()?;
    let height = height.trim().parse::<u64>().ok()?;
    (width > 0 && height > 0).then_some(width * height)
}

#[cfg(target_arch = "wasm32")]
fn install_thread_pool(_cores: Option<usize>) {}

#[cfg(not(target_arch = "wasm32"))]
fn install_thread_pool(cores: Option<usize>) {
    let Some(cores) = cores else {
        return;
    };
    let cores = cores.clamp(1, machine_parallelism());
    if let Err(err) = rayon::ThreadPoolBuilder::new()
        .num_threads(cores)
        .build_global()
    {
        eprintln!("[perro][sim] worker pool cap to {cores} fail: {err}");
    }
}

fn log_profile(profile: &SimProfile) {
    let gpu = match (profile.gpu_constrained, profile.prefer_low_power) {
        (true, true) => "igpu",
        (true, false) => "constrained",
        _ => "full",
    };
    let cores = match profile.cpu_cores {
        Some(cores) => cores.to_string(),
        None => "all".to_owned(),
    };
    let pixels = match profile.max_render_pixels {
        Some(pixels) => pixels.to_string(),
        None => "default".to_owned(),
    };
    eprintln!(
        "[perro][sim] PERF SIM ON gpu=({gpu}) cores=({cores}/{}) max_scene_pixels=({pixels}) -- timings are NOT this machine",
        machine_parallelism()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_spec_is_off() {
        assert_eq!(parse_spec("", 16), SimProfile::OFF);
        assert!(!SimProfile::OFF.active());
    }

    #[test]
    fn igpu_preset_forces_low_power_tier() {
        let profile = parse_spec("igpu", 16);
        assert!(profile.gpu_constrained);
        assert!(profile.prefer_low_power);
        assert_eq!(profile.cpu_cores, None);
        assert!(profile.active());
    }

    #[test]
    fn half_preset_halves_cores() {
        assert_eq!(parse_spec("half", 16).cpu_cores, Some(8));
        assert_eq!(parse_spec("half", 9).cpu_cores, Some(5));
        assert_eq!(parse_spec("half", 1).cpu_cores, Some(1));
    }

    #[test]
    fn potato_preset_caps_scene_pixels() {
        let profile = parse_spec("potato", 16);
        assert_eq!(profile.max_render_pixels, Some(1280 * 720));
        assert_eq!(profile.cpu_cores, Some(2));
    }

    #[test]
    fn later_tokens_override_preset() {
        let profile = parse_spec("igpu,cores=3", 16);
        assert!(profile.prefer_low_power);
        assert_eq!(profile.cpu_cores, Some(3));

        let profile = parse_spec("potato,gpu=off", 16);
        assert!(!profile.gpu_constrained);
        assert!(!profile.prefer_low_power);
        assert_eq!(profile.cpu_cores, Some(2));
    }

    #[test]
    fn core_request_clamps_to_machine() {
        assert_eq!(parse_spec("cores=64", 8).cpu_cores, Some(8));
        assert_eq!(parse_spec("cores=0", 8).cpu_cores, None);
        assert_eq!(parse_spec("cores=abc", 8).cpu_cores, None);
    }

    #[test]
    fn pixels_token_parses_dimensions() {
        assert_eq!(
            parse_spec("pixels=1280x720", 8).max_render_pixels,
            Some(921_600)
        );
        assert_eq!(
            parse_spec("res=640X480", 8).max_render_pixels,
            Some(307_200)
        );
        assert_eq!(parse_spec("pixels=0x720", 8).max_render_pixels, None);
        assert_eq!(parse_spec("pixels=nope", 8).max_render_pixels, None);
    }

    #[test]
    fn unknown_tokens_do_not_change_profile() {
        assert_eq!(parse_spec("bogus,gpu=weird", 8), SimProfile::OFF);
    }

    #[test]
    fn validate_spec_reports_first_bad_token() {
        assert!(validate_spec("igpu,cores=2").is_ok());
        assert!(validate_spec("half").is_ok());
        assert!(validate_spec("").is_ok());
        assert!(validate_spec("igpu,bogus").is_err());
        assert!(validate_spec("cores=zero").is_err());
        assert!(validate_spec("gpu=turbo").is_err());
    }

    #[test]
    fn every_listed_preset_parses() {
        for name in PRESETS {
            assert!(validate_spec(name).is_ok(), "preset {name} rejected");
        }
    }

    #[test]
    fn whitespace_and_case_tolerated() {
        assert_eq!(
            parse_spec(" IGPU , Cores=2 ", 16),
            parse_spec("igpu,cores=2", 16)
        );
    }
}
