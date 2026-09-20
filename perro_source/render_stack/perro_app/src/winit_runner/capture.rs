use super::{AppExitResult, RunnerState};
use perro_capture::{
    AspectRatio, CaptureConfig, CaptureMode, CaptureSource, FrameRate, Framing, OutputFormat,
    OutputSize, OutputSpec,
};
use perro_graphics::{
    CaptureFrameCallback, CaptureRenderFraming, CapturedRgbaFrame, GraphicsBackend, frame_rgba,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use winit::event_loop::ActiveEventLoop;

const MAX_REALTIME_SUBMITS_PER_PRESENT: u64 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CadenceRange {
    first: u64,
    last: u64,
    reaches_due: bool,
}

#[derive(Clone, Copy, Debug)]
struct RealtimeCadence {
    rate: FrameRate,
    duration: Option<Duration>,
    frame_limit: Option<u64>,
    next_slot: u64,
}

impl RealtimeCadence {
    fn new(rate: FrameRate, duration: Option<Duration>, frame_limit: Option<u64>) -> Self {
        Self {
            rate,
            duration,
            frame_limit,
            next_slot: 0,
        }
    }

    fn frame_count(self) -> Option<u64> {
        self.frame_limit.or_else(|| {
            self.duration.map(|duration| {
                let numerator = duration
                    .as_nanos()
                    .saturating_mul(u128::from(self.rate.numerator()));
                let denominator =
                    1_000_000_000u128.saturating_mul(u128::from(self.rate.denominator()));
                u64::try_from(numerator / denominator).unwrap_or(u64::MAX)
            })
        })
    }

    fn due_through(self, elapsed: Duration) -> Option<u64> {
        let numerator = elapsed
            .as_nanos()
            .saturating_mul(u128::from(self.rate.numerator()));
        let denominator = 1_000_000_000u128.saturating_mul(u128::from(self.rate.denominator()));
        let mut last = u64::try_from(numerator / denominator).unwrap_or(u64::MAX);
        if let Some(count) = self.frame_count() {
            if count == 0 {
                return None;
            }
            last = last.min(count.saturating_sub(1));
        }
        Some(last)
    }

    fn take_due(
        &mut self,
        elapsed: Duration,
        has_frame: bool,
        max_submits: u64,
    ) -> Option<CadenceRange> {
        let due_last = self.due_through(elapsed)?;
        if due_last < self.next_slot || !has_frame || max_submits == 0 {
            return None;
        }
        let last = self
            .next_slot
            .saturating_add(max_submits.saturating_sub(1))
            .min(due_last);
        let range = CadenceRange {
            first: self.next_slot,
            last,
            reaches_due: last == due_last,
        };
        self.next_slot = last.saturating_add(1);
        Some(range)
    }

    fn complete(self) -> bool {
        self.frame_count()
            .is_some_and(|count| self.next_slot >= count)
    }

    fn needs_readback(self, elapsed: Duration, has_frame: bool) -> bool {
        !has_frame
            || self
                .due_through(elapsed)
                .is_some_and(|last| last >= self.next_slot)
    }
}

#[derive(Debug)]
struct PreparedFrame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

pub(super) struct RunnerCapture {
    frames: mpsc::Receiver<CapturedRgbaFrame>,
    callback: CaptureFrameCallback,
    render_size: OutputSize,
    preserve_alpha: bool,
    output: Option<OutputSpec>,
    expected_frames: Option<u64>,
    next_frame: u64,
    cadence: Option<RealtimeCadence>,
    started_at: Instant,
    last_frame: Option<PreparedFrame>,
    pending_frame: Option<PreparedFrame>,
    pre_roll_ready_streak: u8,
    pre_roll_attempts: u16,
    pre_roll_started_at: Instant,
    pre_roll_complete: bool,
    callback_enabled: bool,
    armed: bool,
    exit_when_done: bool,
}

impl RunnerCapture {
    pub(super) fn from_env<B: GraphicsBackend>(
        app: &mut crate::App<B>,
    ) -> Result<Option<Self>, String> {
        let Some(output) = env_nonempty("PERRO_CAPTURE_OUTPUT") else {
            return Ok(None);
        };
        let project = app
            .runtime
            .project()
            .ok_or_else(|| "capture requires a loaded project".to_owned())?;
        let source_size = OutputSize::new(
            project.config.virtual_width.max(1),
            project.config.virtual_height.max(1),
        )
        .map_err(|error| error.to_string())?;
        let frame_rate = env_frame_rate()?;
        let duration = env_duration("PERRO_CAPTURE_DURATION")?;
        let frames = env_parse::<u64>("PERRO_CAPTURE_FRAMES")?.filter(|frames| *frames > 0);
        if frames.is_none() && duration.is_none() {
            return Err("PERRO_CAPTURE_FRAMES or PERRO_CAPTURE_DURATION must be > 0".to_owned());
        }
        let mode = if env_nonempty("PERRO_CAPTURE_MODE").as_deref() == Some("offline") {
            CaptureMode::Offline
        } else {
            CaptureMode::Realtime
        };
        let config = CaptureConfig {
            source: parse_source(
                env_nonempty("PERRO_CAPTURE_SOURCE")
                    .as_deref()
                    .unwrap_or("main"),
            )?,
            width: env_parse("PERRO_CAPTURE_WIDTH")?,
            height: env_parse("PERRO_CAPTURE_HEIGHT")?,
            aspect_ratio: parse_aspect(
                env_nonempty("PERRO_CAPTURE_ASPECT")
                    .as_deref()
                    .unwrap_or("preserve"),
            )?,
            framing: parse_framing(
                env_nonempty("PERRO_CAPTURE_FRAMING")
                    .as_deref()
                    .unwrap_or("fit"),
            )?,
            transparent: env_nonempty("PERRO_CAPTURE_TRANSPARENT").as_deref() == Some("1"),
            supersample: env_parse::<u32>("PERRO_CAPTURE_SUPERSAMPLE")?.unwrap_or(2),
            fps: frame_rate.numerator() / frame_rate.denominator(),
            frame_rate: Some(frame_rate),
            mode,
            duration,
            frame_count: frames,
            output: None,
            parallel_workers: std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(2)
                .clamp(1, 8),
        };
        let output_spec = OutputSpec::new(PathBuf::from(output), parse_format()?);
        let preserve_alpha = config.transparent;
        let expected_frames = config
            .schedule()
            .map_err(|error| error.to_string())?
            .map(|schedule| schedule.frame_count());
        let stage_root = output_spec
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .join(".perro-capture");
        app.runtime
            .capture_start(config, source_size, stage_root.to_string_lossy().as_ref())?;
        let render_size = app
            .runtime
            .capture_render_size()
            .ok_or_else(|| "capture session did not resolve render size".to_owned())?;
        Ok(Some(Self::new_bridge(
            render_size,
            preserve_alpha,
            Some(output_spec),
            expected_frames,
            true,
            (mode == CaptureMode::Realtime)
                .then(|| RealtimeCadence::new(frame_rate, duration, frames)),
        )))
    }

    pub(super) fn from_runtime<B: GraphicsBackend>(
        app: &mut crate::App<B>,
    ) -> Result<Option<Self>, String> {
        let Some(config) = app.runtime.capture_config() else {
            return Ok(None);
        };
        let render_size = app
            .runtime
            .capture_render_size()
            .ok_or_else(|| "capture session did not resolve render size".to_owned())?;
        let expected_frames = config
            .schedule()
            .map_err(|error| error.to_string())?
            .map(|schedule| schedule.frame_count());
        let mode = config.mode;
        let frame_rate = config
            .effective_frame_rate()
            .map_err(|error| error.to_string())?;
        Ok(Some(Self::new_bridge(
            render_size,
            config.transparent,
            config.output.clone(),
            expected_frames,
            false,
            (mode == CaptureMode::Realtime)
                .then(|| RealtimeCadence::new(frame_rate, config.duration, config.frame_count)),
        )))
    }

    fn new_bridge(
        render_size: OutputSize,
        preserve_alpha: bool,
        output: Option<OutputSpec>,
        expected_frames: Option<u64>,
        exit_when_done: bool,
        cadence: Option<RealtimeCadence>,
    ) -> Self {
        let (sender, frames) = mpsc::sync_channel(4);
        let callback: CaptureFrameCallback = Arc::new(move |frame| {
            sender
                .send(frame)
                .map_err(|_| "capture frame receiver closed".to_owned())
        });
        Self {
            frames,
            callback,
            render_size,
            preserve_alpha,
            output,
            expected_frames,
            next_frame: 0,
            cadence,
            started_at: Instant::now(),
            last_frame: None,
            pending_frame: None,
            pre_roll_ready_streak: 0,
            pre_roll_attempts: 0,
            pre_roll_started_at: Instant::now(),
            pre_roll_complete: false,
            callback_enabled: false,
            armed: false,
            exit_when_done,
        }
    }
}

impl<B: GraphicsBackend> RunnerState<B> {
    pub(super) fn sync_capture_bridge(&mut self) {
        if self.capture.is_some() || self.app.runtime.capture_state().is_none() {
            return;
        }
        match RunnerCapture::from_runtime(&mut self.app) {
            Ok(Some(capture)) => self.capture = Some(capture),
            Ok(None) => {}
            Err(error) => self.capture_error = Some(error),
        }
    }

    /// Warm renderer assets and capture targets without advancing simulation.
    /// Returns true while the caller must skip the scheduled output frame.
    pub(super) fn pre_roll_offline_capture(&mut self) -> bool {
        if self.offline_fps.is_none() {
            return false;
        }
        self.sync_capture_bridge();
        let Some(capture) = self.capture.as_mut() else {
            return false;
        };
        if capture.pre_roll_complete {
            return false;
        }
        capture.pre_roll_attempts = capture.pre_roll_attempts.saturating_add(1);
        if capture.pre_roll_attempts > 600
            || capture.pre_roll_started_at.elapsed() > Duration::from_secs(30)
        {
            self.app.graphics.set_startup_warm_boost(false);
            self.capture_error = Some(
                "offline capture renderer did not become ready within 600 presents or 30 seconds"
                    .to_owned(),
            );
            return false;
        }
        if !capture.armed {
            if let Err(error) = self
                .app
                .graphics
                .set_capture_target(capture.render_size.width, capture.render_size.height)
            {
                self.capture_error = Some(error);
                return false;
            }
            let source_node = self
                .app
                .runtime
                .capture_source_route()
                .and_then(|route| route.render_node);
            self.app.graphics.set_capture_source_node(source_node);
            self.app.graphics.set_capture_alpha(capture.preserve_alpha);
            self.app.graphics.set_capture_callback(None);
            capture.callback_enabled = false;
            capture.armed = true;
            self.app.graphics.set_startup_warm_boost(true);
        }

        self.app.present();
        let mut inflight = Vec::new();
        self.app
            .runtime
            .copy_inflight_render_requests(&mut inflight);
        let ready = inflight.is_empty() && self.app.graphics.pipeline_warm_idle();
        capture.pre_roll_ready_streak = if ready {
            capture.pre_roll_ready_streak.saturating_add(1)
        } else {
            0
        };
        if capture.pre_roll_ready_streak >= 2 {
            capture.pre_roll_complete = true;
            capture.started_at = Instant::now();
            self.app.graphics.set_startup_warm_boost(false);
        }
        true
    }

    pub(super) fn arm_capture(&mut self) {
        let Some(capture) = self.capture.as_mut() else {
            return;
        };
        if !capture.armed {
            if let Err(error) = self
                .app
                .graphics
                .set_capture_target(capture.render_size.width, capture.render_size.height)
            {
                self.capture_error = Some(error);
                return;
            }
            let source_node = self
                .app
                .runtime
                .capture_source_route()
                .and_then(|route| route.render_node);
            self.app.graphics.set_capture_source_node(source_node);
            self.app.graphics.set_capture_alpha(capture.preserve_alpha);
            capture.armed = true;
        }
        let callback_enabled = capture.cadence.as_ref().is_none_or(|cadence| {
            cadence.needs_readback(
                capture.started_at.elapsed(),
                capture.last_frame.is_some() || capture.pending_frame.is_some(),
            )
        });
        if capture.callback_enabled != callback_enabled {
            self.app.graphics.set_capture_callback(if callback_enabled {
                Some(Arc::clone(&capture.callback))
            } else {
                None
            });
            capture.callback_enabled = callback_enabled;
        }
    }

    pub(super) fn poll_capture_after_present(&mut self, event_loop: &ActiveEventLoop) {
        let Some(mut capture) = self.capture.take() else {
            return;
        };
        let result = (|| -> Result<bool, String> {
            let realtime = capture.cadence.is_some();
            if !realtime {
                self.app.graphics.drain_capture()?;
            }
            if let Some(error) = self.app.graphics.take_capture_error() {
                return Err(error);
            }
            let mut stop_requested = self.app.runtime.capture_take_stop_request();
            if stop_requested && let Some(output) = self.app.runtime.capture_take_stop_output() {
                capture.output = Some(output);
            }
            if !realtime {
                self.app.graphics.drain_capture()?;
            }
            if realtime {
                let mut latest = None;
                loop {
                    match capture.frames.try_recv() {
                        Ok(frame) => latest = Some(frame),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            return Err("capture frame channel disconnected".to_owned());
                        }
                    }
                }
                if let Some(frame) = latest {
                    capture.pending_frame = Some(self.prepare_frame(&capture, frame)?);
                }
                let elapsed = capture.started_at.elapsed();
                let has_frame = capture.last_frame.is_some() || capture.pending_frame.is_some();
                let due = capture.cadence.as_mut().and_then(|cadence| {
                    cadence.take_due(elapsed, has_frame, MAX_REALTIME_SUBMITS_PER_PRESENT)
                });
                if let Some(range) = due {
                    let use_pending_for_latest =
                        range.reaches_due && capture.pending_frame.is_some();
                    let duplicate_count = range
                        .last
                        .saturating_sub(range.first)
                        .saturating_add(1)
                        .saturating_sub(u64::from(use_pending_for_latest));
                    for slot in range.first..=range.last {
                        let prepared = if use_pending_for_latest && slot == range.last {
                            capture.pending_frame.as_ref()
                        } else {
                            capture
                                .last_frame
                                .as_ref()
                                .or(capture.pending_frame.as_ref())
                        }
                        .ok_or_else(|| "capture cadence frame missing".to_owned())?;
                        self.submit_prepared(capture.next_frame, prepared)?;
                        capture.next_frame = capture.next_frame.saturating_add(1);
                    }
                    if use_pending_for_latest && let Some(frame) = capture.pending_frame.take() {
                        capture.last_frame = Some(frame);
                    }
                    if duplicate_count > 0 {
                        self.app
                            .runtime
                            .capture_note_realtime_duplicates(duplicate_count)?;
                    }
                }
                if capture.cadence.is_some_and(|cadence| cadence.complete()) {
                    stop_requested = true;
                }
            } else {
                while capture
                    .expected_frames
                    .is_none_or(|expected| capture.next_frame < expected)
                {
                    let frame = match capture.frames.try_recv() {
                        Ok(frame) => frame,
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            return Err("capture frame channel disconnected".to_owned());
                        }
                    };
                    let prepared = self.prepare_frame(&capture, frame)?;
                    self.submit_prepared(capture.next_frame, &prepared)?;
                    capture.next_frame = capture.next_frame.saturating_add(1);
                }
            }
            if capture
                .expected_frames
                .is_some_and(|expected| capture.next_frame < expected)
                && !stop_requested
            {
                return Ok(false);
            }
            if capture.expected_frames.is_none() && !stop_requested {
                return Ok(false);
            }
            if stop_requested && !realtime {
                self.app.graphics.drain_capture()?;
                while let Ok(frame) = capture.frames.try_recv() {
                    let prepared = self.prepare_frame(&capture, frame)?;
                    self.submit_prepared(capture.next_frame, &prepared)?;
                    capture.next_frame = capture.next_frame.saturating_add(1);
                }
            }
            self.app.graphics.drain_capture()?;
            self.app.runtime.capture_drain()?;
            let output = capture
                .output
                .clone()
                .ok_or_else(|| "capture stop output missing".to_owned())?;
            let result = self.app.runtime.capture_stop(output)?;
            if let Some(expected) = capture.expected_frames
                && result.frame_count != expected
            {
                return Err(format!(
                    "capture finalized {} frames; expected {}",
                    result.frame_count, expected
                ));
            }
            self.app.graphics.set_capture_callback(None);
            self.app.graphics.clear_capture_target();
            eprintln!("[perro][capture] output={}", result.output_path.display());
            Ok(true)
        })();
        match result {
            Ok(true) if capture.exit_when_done => {
                self.request_exit(event_loop, AppExitResult::event_loop_exit())
            }
            Ok(true) => {}
            Ok(false) => self.capture = Some(capture),
            Err(error) => {
                eprintln!("[perro][capture] error: {error}");
                if capture.exit_when_done {
                    self.capture_error = Some(error);
                }
                self.app.graphics.set_capture_callback(None);
                self.app.graphics.clear_capture_target();
                if capture.exit_when_done {
                    self.request_exit(event_loop, AppExitResult::event_loop_exit());
                }
            }
        }
    }

    fn prepare_frame(
        &self,
        capture: &RunnerCapture,
        frame: CapturedRgbaFrame,
    ) -> Result<PreparedFrame, String> {
        let (width, height, rgba) = if [frame.width, frame.height]
            == [capture.render_size.width, capture.render_size.height]
        {
            (frame.width, frame.height, frame.rgba)
        } else {
            let framing = self
                .app
                .runtime
                .capture_source_route()
                .map(|route| match route.framing {
                    perro_capture::Framing::Fit => CaptureRenderFraming::Fit,
                    perro_capture::Framing::Crop => CaptureRenderFraming::Crop,
                    perro_capture::Framing::Expand => CaptureRenderFraming::Expand,
                    perro_capture::Framing::Stretch => CaptureRenderFraming::Stretch,
                })
                .ok_or_else(|| "capture source route unavailable".to_owned())?;
            let rgba = frame_rgba(
                &frame.rgba,
                [frame.width, frame.height],
                [capture.render_size.width, capture.render_size.height],
                framing,
            )?;
            (capture.render_size.width, capture.render_size.height, rgba)
        };
        Ok(PreparedFrame {
            width,
            height,
            rgba,
        })
    }

    fn submit_prepared(&mut self, frame_index: u64, frame: &PreparedFrame) -> Result<(), String> {
        self.app.runtime.capture_submit_rgba(
            frame_index,
            frame.width,
            frame.height,
            &frame.rgba,
        )?;
        Ok(())
    }
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn env_parse<T: std::str::FromStr>(name: &str) -> Result<Option<T>, String> {
    env_nonempty(name)
        .map(|value| {
            value
                .parse()
                .map_err(|_| format!("invalid {name} `{value}`"))
        })
        .transpose()
}

fn env_duration(name: &str) -> Result<Option<Duration>, String> {
    let Some(raw) = env_nonempty(name) else {
        return Ok(None);
    };
    parse_duration(name, &raw).map(Some)
}

fn parse_duration(name: &str, raw: &str) -> Result<Duration, String> {
    let seconds = raw
        .parse::<f64>()
        .map_err(|_| format!("invalid {name} `{raw}`"))?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(format!("{name} must be finite and > 0"));
    }
    Duration::try_from_secs_f64(seconds).map_err(|_| format!("{name} exceeds duration range"))
}

fn env_frame_rate() -> Result<FrameRate, String> {
    let numerator = env_parse::<u32>("PERRO_CAPTURE_FPS_NUM")?;
    let denominator = env_parse::<u32>("PERRO_CAPTURE_FPS_DEN")?;
    resolve_frame_rate(
        env_nonempty("PERRO_CAPTURE_FPS").as_deref(),
        numerator,
        denominator,
    )
}

fn resolve_frame_rate(
    raw: Option<&str>,
    numerator: Option<u32>,
    denominator: Option<u32>,
) -> Result<FrameRate, String> {
    if let (Some(numerator), Some(denominator)) = (numerator, denominator) {
        return FrameRate::new(numerator, denominator)
            .map_err(|error| format!("invalid capture frame rate: {error}"));
    }
    let raw = raw.unwrap_or("60");
    FrameRate::parse(raw).map_err(|error| format!("invalid PERRO_CAPTURE_FPS: {error}"))
}

fn parse_source(raw: &str) -> Result<CaptureSource, String> {
    if matches!(raw, "main" | "window" | "main_window") {
        return Ok(CaptureSource::MainWindow);
    }
    let (kind, name) = raw
        .split_once(':')
        .ok_or_else(|| "capture source must be main or kind:name".to_owned())?;
    let source = match kind {
        "camera2d" => CaptureSource::Camera2D(name.to_owned()),
        "camera3d" => CaptureSource::Camera3D(name.to_owned()),
        "ui" | "uisubview" | "ui_sub_view" => CaptureSource::UISubView(name.to_owned()),
        "target" | "render_target" => CaptureSource::RenderTarget(name.to_owned()),
        _ => return Err(format!("unsupported capture source kind `{kind}`")),
    };
    source.validate().map_err(|error| error.to_string())?;
    Ok(source)
}

fn parse_aspect(raw: &str) -> Result<AspectRatio, String> {
    if raw == "preserve" {
        return Ok(AspectRatio::Preserve);
    }
    let (width, height) = raw
        .split_once(':')
        .ok_or_else(|| format!("invalid capture aspect `{raw}`"))?;
    AspectRatio::new(
        width
            .parse()
            .map_err(|_| format!("invalid capture aspect `{raw}`"))?,
        height
            .parse()
            .map_err(|_| format!("invalid capture aspect `{raw}`"))?,
    )
    .map_err(|error| error.to_string())
}

fn parse_framing(raw: &str) -> Result<Framing, String> {
    match raw {
        "fit" => Ok(Framing::Fit),
        "crop" => Ok(Framing::Crop),
        "expand" => Ok(Framing::Expand),
        "stretch" => Ok(Framing::Stretch),
        _ => Err(format!("invalid capture framing `{raw}`")),
    }
}

fn parse_format() -> Result<OutputFormat, String> {
    match env_nonempty("PERRO_CAPTURE_FORMAT")
        .as_deref()
        .unwrap_or("png")
    {
        "png" => Ok(OutputFormat::PngSequence),
        "gif" => Ok(OutputFormat::Gif),
        "webm" => Ok(OutputFormat::WebM),
        "mp4" => Ok(OutputFormat::Mp4),
        "webp" => Ok(OutputFormat::AnimatedWebP),
        value => Err(format!("invalid capture format `{value}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::{CadenceRange, RealtimeCadence, parse_duration, resolve_frame_rate};
    use perro_capture::FrameRate;
    use std::time::Duration;

    #[test]
    fn parse_capture_rates_exact() {
        assert_eq!(
            resolve_frame_rate(Some("29.97"), None, None).expect("decimal rate"),
            FrameRate::new(2997, 100).expect("rate")
        );
        assert_eq!(
            resolve_frame_rate(None, Some(30000), Some(1001)).expect("rational rate"),
            FrameRate::new(30000, 1001).expect("rate")
        );
    }

    #[test]
    fn parse_capture_duration_finite() {
        assert_eq!(
            parse_duration("PERRO_CAPTURE_DURATION", "1.25").expect("duration"),
            Duration::from_millis(1_250)
        );
        assert!(parse_duration("PERRO_CAPTURE_DURATION", "0").is_err());
        assert!(parse_duration("PERRO_CAPTURE_DURATION", "NaN").is_err());
    }

    #[test]
    fn cadence_waits_b4_first_frame_and_emits_t0() {
        let rate = FrameRate::integer(10);
        let mut cadence = RealtimeCadence::new(rate, None, Some(3));
        assert_eq!(cadence.take_due(Duration::from_millis(1), false, 8), None);
        assert_eq!(
            cadence.take_due(Duration::from_millis(1), true, 8),
            Some(CadenceRange {
                first: 0,
                last: 0,
                reaches_due: true,
            })
        );
        assert_eq!(cadence.take_due(Duration::from_millis(20), true, 8), None);
    }

    #[test]
    fn cadence_gates_readback_between_due_slots() {
        let rate = FrameRate::integer(10);
        let mut cadence = RealtimeCadence::new(rate, None, Some(3));
        assert!(cadence.needs_readback(Duration::ZERO, false));
        let _ = cadence.take_due(Duration::ZERO, true, 8);
        assert!(!cadence.needs_readback(Duration::from_millis(20), true));
        assert!(cadence.needs_readback(Duration::from_millis(100), true));
    }

    #[test]
    fn cadence_jump_fills_missed_slots_and_caps_exact_count() {
        let rate = FrameRate::integer(10);
        let mut cadence = RealtimeCadence::new(rate, None, Some(4));
        assert_eq!(
            cadence.take_due(Duration::ZERO, true, 8),
            Some(CadenceRange {
                first: 0,
                last: 0,
                reaches_due: true,
            })
        );
        assert_eq!(
            cadence.take_due(Duration::from_millis(350), true, 8),
            Some(CadenceRange {
                first: 1,
                last: 3,
                reaches_due: true,
            })
        );
        assert!(cadence.complete());
        assert_eq!(cadence.take_due(Duration::from_secs(2), true, 8), None);
    }

    #[test]
    fn cadence_bounds_backlog_per_present() {
        let rate = FrameRate::integer(60);
        let mut cadence = RealtimeCadence::new(rate, None, Some(20));
        assert_eq!(
            cadence.take_due(Duration::ZERO, true, 8),
            Some(CadenceRange {
                first: 0,
                last: 0,
                reaches_due: true,
            })
        );
        assert_eq!(
            cadence.take_due(Duration::from_secs(1), true, 8),
            Some(CadenceRange {
                first: 1,
                last: 8,
                reaches_due: false,
            })
        );
        assert_eq!(
            cadence.take_due(Duration::from_secs(1), true, 8),
            Some(CadenceRange {
                first: 9,
                last: 16,
                reaches_due: false,
            })
        );
        assert_eq!(
            cadence.take_due(Duration::from_secs(1), true, 8),
            Some(CadenceRange {
                first: 17,
                last: 19,
                reaches_due: true,
            })
        );
        assert!(cadence.complete());
    }

    #[test]
    fn cadence_duration_reaches_exact_deadline() {
        let rate = FrameRate::integer(10);
        let mut cadence = RealtimeCadence::new(rate, Some(Duration::from_millis(300)), None);
        assert_eq!(cadence.frame_count(), Some(3));
        assert_eq!(
            cadence.take_due(Duration::ZERO, true, 8),
            Some(CadenceRange {
                first: 0,
                last: 0,
                reaches_due: true,
            })
        );
        assert_eq!(
            cadence.take_due(Duration::from_millis(100), true, 8),
            Some(CadenceRange {
                first: 1,
                last: 1,
                reaches_due: true,
            })
        );
        assert_eq!(
            cadence.take_due(Duration::from_millis(300), true, 8),
            Some(CadenceRange {
                first: 2,
                last: 2,
                reaches_due: true,
            })
        );
        assert!(cadence.complete());
    }
}
