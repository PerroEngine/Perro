//! Capture timing, sizing, and RGBA frame preparation.

use image::codecs::png::PngEncoder;
use image::{ImageEncoder, RgbaImage, imageops};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const GIF_ALPHA_THRESHOLD: u8 = 128;
const GIF_MAX_OPAQUE_COLORS: usize = 255;
const GIF_MAX_PALETTE_SAMPLES: usize = 1_000_000;

/// Capture source selected by a renderer adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureSource {
    /// Full game/window output.
    MainWindow,
    /// Alias for full game/window output.
    Window,
    /// A named 2D camera.
    Camera2D(String),
    /// An exact 2D camera node handle.
    #[doc(hidden)]
    Camera2DNode(u64),
    /// A named 3D camera.
    Camera3D(String),
    /// An exact 3D camera node handle.
    #[doc(hidden)]
    Camera3DNode(u64),
    /// A named UI sub-view.
    UISubView(String),
    /// A named render target or viewport.
    RenderTarget(String),
    /// A renderer-defined source name.
    Named(String),
}

/// Renderer route selected by [`CaptureSource`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureSourceKind {
    /// Main window or game output.
    MainWindow,
    /// Camera2D output.
    Camera2D,
    /// Camera3D output.
    Camera3D,
    /// UISubView output.
    UISubView,
    /// Explicit render target or viewport.
    RenderTarget,
    /// Renderer-defined source.
    Named,
}

impl CaptureSource {
    /// Create a main-window source.
    pub const fn main_window() -> Self {
        Self::MainWindow
    }

    /// Validate a source route before renderer setup.
    pub fn validate(&self) -> Result<(), CaptureError> {
        match self {
            Self::MainWindow | Self::Window => Ok(()),
            Self::Camera2D(name)
            | Self::Camera3D(name)
            | Self::UISubView(name)
            | Self::RenderTarget(name)
            | Self::Named(name)
                if name.trim().is_empty() =>
            {
                Err(CaptureError::InvalidConfig("capture source name is empty"))
            }
            Self::Camera2DNode(node) | Self::Camera3DNode(node) if *node == 0 => {
                Err(CaptureError::InvalidConfig("capture source node id is nil"))
            }
            _ => Ok(()),
        }
    }

    /// Return source routing kind for renderer integration.
    pub const fn kind(&self) -> CaptureSourceKind {
        match self {
            Self::MainWindow | Self::Window => CaptureSourceKind::MainWindow,
            Self::Camera2D(_) | Self::Camera2DNode(_) => CaptureSourceKind::Camera2D,
            Self::Camera3D(_) | Self::Camera3DNode(_) => CaptureSourceKind::Camera3D,
            Self::UISubView(_) => CaptureSourceKind::UISubView,
            Self::RenderTarget(_) => CaptureSourceKind::RenderTarget,
            Self::Named(_) => CaptureSourceKind::Named,
        }
    }

    /// Create a named 2D-camera source.
    pub fn camera_2d(name: impl Into<String>) -> Self {
        Self::Camera2D(name.into())
    }

    /// Create an exact 2D-camera source from its encoded node handle.
    #[doc(hidden)]
    pub const fn camera_2d_node_id(node: u64) -> Self {
        Self::Camera2DNode(node)
    }

    /// Create a named 3D-camera source.
    pub fn camera_3d(name: impl Into<String>) -> Self {
        Self::Camera3D(name.into())
    }

    /// Create an exact 3D-camera source from its encoded node handle.
    #[doc(hidden)]
    pub const fn camera_3d_node_id(node: u64) -> Self {
        Self::Camera3DNode(node)
    }

    /// Create a named UI-sub-view source.
    pub fn ui_sub_view(name: impl Into<String>) -> Self {
        Self::UISubView(name.into())
    }

    /// Create a named render-target source.
    pub fn render_target(name: impl Into<String>) -> Self {
        Self::RenderTarget(name.into())
    }

    fn label(&self) -> String {
        match self {
            Self::MainWindow | Self::Window => "main_window".to_owned(),
            Self::Camera2D(name) => format!("camera2d:{name}"),
            Self::Camera2DNode(node) => format!("camera2d:node:{node}"),
            Self::Camera3D(name) => format!("camera3d:{name}"),
            Self::Camera3DNode(node) => format!("camera3d:node:{node}"),
            Self::UISubView(name) => format!("ui_sub_view:{name}"),
            Self::RenderTarget(name) => format!("render_target:{name}"),
            Self::Named(name) => name.clone(),
        }
    }
}

/// Source or explicit output aspect ratio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AspectRatio {
    /// Keep source width/height ratio.
    Preserve,
    /// Use an explicit width:height ratio.
    Ratio { width: u32, height: u32 },
}

impl AspectRatio {
    /// Create and validate an explicit ratio.
    pub const fn new(width: u32, height: u32) -> Result<Self, CaptureError> {
        if width == 0 || height == 0 {
            return Err(CaptureError::InvalidConfig(
                "aspect ratio needs nonzero sides",
            ));
        }
        Ok(Self::Ratio { width, height })
    }
}

/// Framing behavior when source and output ratios differ.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Framing {
    /// Keep all source pixels and letterbox as needed.
    #[default]
    Fit,
    /// Fill output and crop excess source pixels.
    Crop,
    /// Expand camera view to fill output.
    Expand,
    /// Scale each axis independently.
    Stretch,
}

/// Runtime capture or timeline-controlled offline render.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaptureMode {
    /// Sample frames from live game time.
    #[default]
    Realtime,
    /// Sample a deterministic frame schedule.
    Offline,
}

/// Exact rational frame rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRate {
    numerator: u32,
    denominator: u32,
}

impl FrameRate {
    /// Build a positive rate and reduce its fraction.
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, CaptureError> {
        if numerator == 0 || denominator == 0 {
            return Err(CaptureError::InvalidConfig(
                "frame rate needs nonzero numerator and denominator",
            ));
        }
        let divisor = gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    /// Build an integer rate.
    pub const fn integer(fps: u32) -> Self {
        Self {
            numerator: fps,
            denominator: 1,
        }
    }

    /// Parse an exact decimal or `num/den` rate.
    pub fn parse(raw: &str) -> Result<Self, CaptureError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(CaptureError::InvalidConfig("frame rate is empty"));
        }
        if let Some((numerator, denominator)) = raw.split_once('/') {
            let numerator = numerator
                .parse::<u32>()
                .map_err(|_| CaptureError::InvalidConfig("invalid frame rate numerator"))?;
            let denominator = denominator
                .parse::<u32>()
                .map_err(|_| CaptureError::InvalidConfig("invalid frame rate denominator"))?;
            return Self::new(numerator, denominator);
        }
        let (whole, fraction) = raw.split_once('.').map_or((raw, ""), |parts| parts);
        if whole.is_empty() && fraction.is_empty()
            || !whole.bytes().all(|byte| byte.is_ascii_digit())
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.len() > 9
        {
            return Err(CaptureError::InvalidConfig("invalid decimal frame rate"));
        }
        let whole = if whole.is_empty() {
            0
        } else {
            whole
                .parse::<u64>()
                .map_err(|_| CaptureError::InvalidConfig("frame rate overflow"))?
        };
        let scale = 10_u64.pow(fraction.len() as u32);
        let fractional = if fraction.is_empty() {
            0
        } else {
            fraction
                .parse::<u64>()
                .map_err(|_| CaptureError::InvalidConfig("frame rate overflow"))?
        };
        let numerator = whole
            .checked_mul(scale)
            .and_then(|value| value.checked_add(fractional))
            .ok_or(CaptureError::InvalidConfig("frame rate overflow"))?;
        let numerator = u32::try_from(numerator)
            .map_err(|_| CaptureError::InvalidConfig("frame rate exceeds u32"))?;
        let denominator = u32::try_from(scale)
            .map_err(|_| CaptureError::InvalidConfig("frame rate denominator overflow"))?;
        Self::new(numerator, denominator)
    }

    /// Rate numerator.
    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    /// Rate denominator.
    pub const fn denominator(self) -> u32 {
        self.denominator
    }

    /// Return decimal text with bounded precision.
    pub fn decimal_string(self) -> String {
        let value = f64::from(self.numerator) / f64::from(self.denominator);
        format!("{value:.9}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    }

    /// Return ffmpeg rational text.
    pub fn ffmpeg_string(self) -> String {
        format!("{}/{}", self.numerator, self.denominator)
    }
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

/// Exact frame schedule independent of render throughput.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameSchedule {
    rate: FrameRate,
    frame_count: u64,
}

impl FrameSchedule {
    /// Create a schedule with an exact frame count.
    pub const fn new(fps: u32, frame_count: u64) -> Result<Self, CaptureError> {
        if fps == 0 {
            return Err(CaptureError::InvalidConfig("fps must be > 0"));
        }
        Ok(Self {
            rate: FrameRate::integer(fps),
            frame_count,
        })
    }

    /// Create a schedule with an exact rational rate and frame count.
    pub const fn new_rate(rate: FrameRate, frame_count: u64) -> Self {
        Self { rate, frame_count }
    }

    /// Create a schedule whose count equals floor(duration * fps).
    pub fn from_duration(fps: u32, duration: Duration) -> Result<Self, CaptureError> {
        Self::from_rate(FrameRate::new(fps, 1)?, duration)
    }

    /// Create a schedule whose count equals floor(duration * rate).
    pub fn from_rate(rate: FrameRate, duration: Duration) -> Result<Self, CaptureError> {
        let count = duration
            .as_nanos()
            .checked_mul(u128::from(rate.numerator))
            .ok_or(CaptureError::InvalidConfig("duration * fps overflow"))?
            .checked_div(
                u128::from(rate.denominator)
                    .checked_mul(1_000_000_000)
                    .ok_or(CaptureError::InvalidConfig("frame duration overflow"))?,
            )
            .ok_or(CaptureError::InvalidConfig(
                "frame rate denominator is zero",
            ))?;
        let frame_count = u64::try_from(count)
            .map_err(|_| CaptureError::InvalidConfig("frame count overflow"))?;
        Ok(Self { rate, frame_count })
    }

    /// Requested frames per second.
    pub const fn fps(self) -> u32 {
        self.rate.numerator / self.rate.denominator
    }

    /// Exact rational rate.
    pub const fn rate(self) -> FrameRate {
        self.rate
    }

    /// Exact number of output frames.
    pub const fn frame_count(self) -> u64 {
        self.frame_count
    }

    /// Timestamp for frame index, measured from zero.
    pub fn timestamp(self, frame: u64) -> Option<Duration> {
        if frame >= self.frame_count {
            return None;
        }
        let nanos = u128::from(frame)
            .checked_mul(1_000_000_000)?
            .checked_mul(u128::from(self.rate.denominator))?
            / u128::from(self.rate.numerator);
        Some(Duration::from_nanos(u64::try_from(nanos).ok()?))
    }

    /// Return every output timestamp in order.
    pub fn timestamps(self) -> Result<Vec<Duration>, CaptureError> {
        let count = usize::try_from(self.frame_count)
            .map_err(|_| CaptureError::InvalidConfig("frame count exceeds platform size"))?;
        let mut out = Vec::with_capacity(count);
        for index in 0..self.frame_count {
            let timestamp = self
                .timestamp(index)
                .ok_or(CaptureError::InvalidConfig("schedule timestamp overflow"))?;
            out.push(timestamp);
        }
        Ok(out)
    }
}

/// Exact output dimensions in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputSize {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Canonical sequence or encoded animation output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Keep canonical numbered PNG files in an output directory.
    PngSequence,
    /// Palette animation encoded by the native image encoder.
    Gif,
    /// VP9 animation encoded by an external ffmpeg binary.
    WebM,
    /// H.264 animation encoded by an external ffmpeg binary.
    Mp4,
    /// Animated WebP encoded by an external ffmpeg binary.
    AnimatedWebP,
}

/// Final output path and encoder policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputSpec {
    /// Final file path, or directory path for [`OutputFormat::PngSequence`].
    pub path: PathBuf,
    /// Output encoder.
    pub format: OutputFormat,
    /// Optional ffmpeg executable path for external formats.
    pub ffmpeg: Option<PathBuf>,
}

impl OutputSpec {
    /// Create output spec using PATH lookup for ffmpeg formats.
    pub fn new(path: impl Into<PathBuf>, format: OutputFormat) -> Self {
        Self {
            path: path.into(),
            format,
            ffmpeg: None,
        }
    }

    /// Set an explicit ffmpeg executable.
    pub fn with_ffmpeg(mut self, path: impl Into<PathBuf>) -> Self {
        self.ffmpeg = Some(path.into());
        self
    }
}

impl OutputSize {
    /// Create validated dimensions.
    pub const fn new(width: u32, height: u32) -> Result<Self, CaptureError> {
        if width == 0 || height == 0 {
            return Err(CaptureError::InvalidConfig("output dimensions must be > 0"));
        }
        Ok(Self { width, height })
    }
}

/// Output and timing policy for a capture.
#[derive(Clone, Debug)]
pub struct CaptureConfig {
    /// Source selected by renderer integration.
    pub source: CaptureSource,
    /// Optional exact output width.
    pub width: Option<u32>,
    /// Optional exact output height.
    pub height: Option<u32>,
    /// Output aspect policy.
    pub aspect_ratio: AspectRatio,
    /// Source/output framing policy.
    pub framing: Framing,
    /// Preserve renderer alpha when true.
    pub transparent: bool,
    /// Render scale before final downsample. Default: 2.
    pub supersample: u32,
    /// Requested output FPS. Default: 60.
    pub fps: u32,
    /// Optional exact rational FPS; overrides [`Self::fps`] when set.
    pub frame_rate: Option<FrameRate>,
    /// Realtime or offline timeline.
    pub mode: CaptureMode,
    /// Optional duration used to derive exact offline frame count.
    pub duration: Option<Duration>,
    /// Optional exact frame count; overrides duration schedule when set.
    pub frame_count: Option<u64>,
    /// Optional default output used by [`CaptureSession::finalize`].
    pub output: Option<OutputSpec>,
    /// Maximum worker count for bounded RGBA preparation.
    pub parallel_workers: usize,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            source: CaptureSource::MainWindow,
            width: None,
            height: None,
            aspect_ratio: AspectRatio::Preserve,
            framing: Framing::Fit,
            transparent: false,
            supersample: 2,
            fps: 60,
            frame_rate: None,
            mode: CaptureMode::Realtime,
            duration: None,
            frame_count: None,
            output: None,
            parallel_workers: 2,
        }
    }
}

impl CaptureConfig {
    /// Validate config fields and source dimensions.
    pub fn validate(&self, source: OutputSize) -> Result<(), CaptureError> {
        self.source.validate()?;
        if self.supersample == 0 {
            return Err(CaptureError::InvalidConfig("supersample must be > 0"));
        }
        let _ = self.effective_frame_rate()?;
        if let AspectRatio::Ratio { width, height } = self.aspect_ratio
            && (width == 0 || height == 0)
        {
            return Err(CaptureError::InvalidConfig(
                "aspect ratio needs nonzero sides",
            ));
        }
        let _ = self.resolve_output_size(source)?;
        if let Some(duration) = self.duration
            && self.frame_count.is_none()
        {
            let _ = FrameSchedule::from_rate(self.effective_frame_rate()?, duration)?;
        }
        if let Some(frame_count) = self.frame_count {
            let _ = FrameSchedule::new_rate(self.effective_frame_rate()?, frame_count);
        }
        Ok(())
    }

    /// Resolve final output size without applying supersample.
    pub fn resolve_output_size(&self, source: OutputSize) -> Result<OutputSize, CaptureError> {
        self.validate_dimensions_only(source)?;
        let ratio = match self.aspect_ratio {
            AspectRatio::Preserve => (u64::from(source.width), u64::from(source.height)),
            AspectRatio::Ratio { width, height } => (u64::from(width), u64::from(height)),
        };
        let (width, height) = match (self.width, self.height) {
            (Some(width), Some(height)) => (width, height),
            (Some(width), None) => {
                let height = rounded_ratio(width, ratio.1, ratio.0)?;
                (width, height)
            }
            (None, Some(height)) => {
                let width = rounded_ratio(height, ratio.0, ratio.1)?;
                (width, height)
            }
            (None, None) => (source.width, source.height),
        };
        OutputSize::new(width, height)
    }

    /// Dimensions the renderer must produce before downsampling.
    pub fn render_size(&self, source: OutputSize) -> Result<OutputSize, CaptureError> {
        let output = self.resolve_output_size(source)?;
        let width = output
            .width
            .checked_mul(self.supersample)
            .ok_or(CaptureError::InvalidConfig("render width overflow"))?;
        let height = output
            .height
            .checked_mul(self.supersample)
            .ok_or(CaptureError::InvalidConfig("render height overflow"))?;
        OutputSize::new(width, height)
    }

    /// Create the deterministic schedule when duration exists.
    pub fn schedule(&self) -> Result<Option<FrameSchedule>, CaptureError> {
        if let Some(frame_count) = self.frame_count {
            return Ok(Some(FrameSchedule::new_rate(
                self.effective_frame_rate()?,
                frame_count,
            )));
        }
        self.duration
            .map(|duration| FrameSchedule::from_rate(self.effective_frame_rate()?, duration))
            .transpose()
    }

    /// Resolve exact rate while keeping integer `fps` source compatibility.
    pub fn effective_frame_rate(&self) -> Result<FrameRate, CaptureError> {
        self.frame_rate
            .map_or_else(|| FrameRate::new(self.fps, 1), Ok)
    }

    fn validate_dimensions_only(&self, source: OutputSize) -> Result<(), CaptureError> {
        if source.width == 0 || source.height == 0 {
            return Err(CaptureError::InvalidConfig("source dimensions must be > 0"));
        }
        if self.width == Some(0) || self.height == Some(0) {
            return Err(CaptureError::InvalidConfig(
                "requested dimensions must be > 0",
            ));
        }
        Ok(())
    }
}

fn rounded_ratio(value: u32, numerator: u64, denominator: u64) -> Result<u32, CaptureError> {
    let scaled = u64::from(value)
        .checked_mul(numerator)
        .ok_or(CaptureError::InvalidConfig("aspect dimensions overflow"))?;
    let rounded = scaled
        .checked_add(denominator / 2)
        .ok_or(CaptureError::InvalidConfig("aspect dimensions overflow"))?
        / denominator;
    let out = u32::try_from(rounded)
        .map_err(|_| CaptureError::InvalidConfig("aspect dimensions exceed u32"))?;
    if out == 0 {
        return Err(CaptureError::InvalidConfig("aspect ratio rounds to zero"));
    }
    Ok(out)
}

/// Errors from capture config and frame preparation.
#[derive(Clone, Debug)]
pub enum CaptureError {
    /// Invalid dimensions or timing policy.
    InvalidConfig(&'static str),
    /// Input frame dimensions or bytes do not match the requested render size.
    InvalidFrame(&'static str),
    /// Filesystem failure while staging or packing.
    Io(String),
    /// PNG/GIF image codec failure.
    Image(String),
    /// Requested external encoder cannot run.
    EncoderUnavailable(String),
    /// External encoder returned failure.
    EncoderFailed(String),
    /// Output already exists; final rename stays atomic.
    OutputExists(PathBuf),
    /// Submitted frame count differs from deterministic schedule.
    FrameCountMismatch { expected: u64, actual: u64 },
    /// Session no longer accepts frames.
    SessionFinalized,
    /// Staging cleanup failed after successful output.
    CleanupFailed(String),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(msg) => write!(f, "invalid capture config: {msg}"),
            Self::InvalidFrame(msg) => write!(f, "invalid capture frame: {msg}"),
            Self::Io(msg) => write!(f, "capture I/O error: {msg}"),
            Self::Image(msg) => write!(f, "capture image error: {msg}"),
            Self::EncoderUnavailable(msg) => write!(f, "capture encoder unavailable: {msg}"),
            Self::EncoderFailed(msg) => write!(f, "capture encoder failed: {msg}"),
            Self::OutputExists(path) => write!(f, "capture output exists: {}", path.display()),
            Self::FrameCountMismatch { expected, actual } => {
                write!(
                    f,
                    "capture frame count mismatch: expected {expected}, got {actual}"
                )
            }
            Self::SessionFinalized => write!(f, "capture session finalized"),
            Self::CleanupFailed(msg) => write!(f, "capture cleanup failed: {msg}"),
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<io::Error> for CaptureError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<image::ImageError> for CaptureError {
    fn from(error: image::ImageError) -> Self {
        Self::Image(error.to_string())
    }
}

/// Resize RGBA bytes with alpha-correct premultiplied filtering.
///
/// The returned buffer always has exactly `target` dimensions. RGB values are
/// premultiplied before filtering and restored after filtering to avoid dark
/// fringes around transparent content.
pub fn downsample_rgba(
    rgba: &[u8],
    source: OutputSize,
    target: OutputSize,
    parallel_workers: usize,
) -> Result<Vec<u8>, CaptureError> {
    validate_downsample(rgba.len(), source, target)?;
    downsample_rgba_owned(rgba.to_vec(), source, target, parallel_workers)
}

fn validate_downsample(
    byte_count: usize,
    source: OutputSize,
    target: OutputSize,
) -> Result<(), CaptureError> {
    let pixel_count = usize::try_from(u64::from(source.width) * u64::from(source.height))
        .map_err(|_| CaptureError::InvalidFrame("source pixel count overflow"))?;
    let expected = pixel_count
        .checked_mul(4)
        .ok_or(CaptureError::InvalidFrame("source byte count overflow"))?;
    if byte_count != expected {
        return Err(CaptureError::InvalidFrame("RGBA byte count mismatch"));
    }
    OutputSize::new(target.width, target.height)?;
    Ok(())
}

fn downsample_rgba_owned(
    rgba: Vec<u8>,
    source: OutputSize,
    target: OutputSize,
    parallel_workers: usize,
) -> Result<Vec<u8>, CaptureError> {
    validate_downsample(rgba.len(), source, target)?;
    if source == target {
        return Ok(rgba);
    }

    let worker_count = parallel_workers.max(1);
    if worker_count > 1 {
        if let Ok(pool) = rayon::ThreadPoolBuilder::new()
            .num_threads(worker_count)
            .build()
        {
            pool.install(|| resize_rgba_owned(rgba, source, target, true))
        } else {
            resize_rgba_owned(rgba, source, target, false)
        }
    } else {
        resize_rgba_owned(rgba, source, target, false)
    }
}

fn resize_rgba_owned(
    mut premultiplied: Vec<u8>,
    source: OutputSize,
    target: OutputSize,
    parallel: bool,
) -> Result<Vec<u8>, CaptureError> {
    if parallel {
        premultiply(premultiplied.as_mut_slice());
    } else {
        premultiply_sequential(premultiplied.as_mut_slice());
    }
    let source_image = RgbaImage::from_raw(source.width, source.height, premultiplied)
        .ok_or(CaptureError::InvalidFrame("RGBA image allocation failed"))?;
    let mut resized = imageops::resize(
        &source_image,
        target.width,
        target.height,
        imageops::FilterType::Lanczos3,
    );
    if parallel {
        unpremultiply(resized.as_mut());
    } else {
        unpremultiply_sequential(resized.as_mut());
    }
    Ok(resized.into_raw())
}

fn premultiply(image: &mut [u8]) {
    image.par_chunks_mut(4).for_each(premultiply_pixel);
}

fn premultiply_sequential(image: &mut [u8]) {
    image.chunks_exact_mut(4).for_each(premultiply_pixel);
}

fn premultiply_pixel(pixel: &mut [u8]) {
    let alpha = u16::from(pixel[3]);
    pixel[0] = ((u16::from(pixel[0]) * alpha + 127) / 255) as u8;
    pixel[1] = ((u16::from(pixel[1]) * alpha + 127) / 255) as u8;
    pixel[2] = ((u16::from(pixel[2]) * alpha + 127) / 255) as u8;
}

fn unpremultiply(image: &mut [u8]) {
    image.par_chunks_mut(4).for_each(unpremultiply_pixel);
}

fn unpremultiply_sequential(image: &mut [u8]) {
    image.chunks_exact_mut(4).for_each(unpremultiply_pixel);
}

fn unpremultiply_pixel(pixel: &mut [u8]) {
    let alpha = u16::from(pixel[3]);
    if alpha == 0 {
        pixel[0] = 0;
        pixel[1] = 0;
        pixel[2] = 0;
        return;
    }
    pixel[0] = ((u16::from(pixel[0]) * 255 + alpha / 2) / alpha).min(255) as u8;
    pixel[1] = ((u16::from(pixel[1]) * 255 + alpha / 2) / alpha).min(255) as u8;
    pixel[2] = ((u16::from(pixel[2]) * 255 + alpha / 2) / alpha).min(255) as u8;
}

/// Progress snapshot for a bounded capture session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureProgress {
    /// Jobs accepted by the session.
    pub submitted: u64,
    /// PNG jobs fully written and closed.
    pub completed: u64,
    /// Deterministic target, when duration supplied.
    pub expected: Option<u64>,
}

/// Timeline event retained in capture metadata for replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionEvent {
    /// Simulation time at which action occurs.
    pub timestamp: Duration,
    /// Stable action name.
    pub action: String,
    /// Optional JSON or user payload.
    pub payload: Option<String>,
}

/// Ordered director/QA action timeline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActionTimeline {
    events: Vec<ActionEvent>,
}

impl ActionTimeline {
    /// Add an action in timeline order.
    pub fn record(
        &mut self,
        timestamp: Duration,
        action: impl Into<String>,
        payload: Option<String>,
    ) {
        self.events.push(ActionEvent {
            timestamp,
            action: action.into(),
            payload,
        });
        self.events.sort_by_key(|event| event.timestamp);
    }

    /// Borrow recorded events.
    pub fn events(&self) -> &[ActionEvent] {
        &self.events
    }

    /// Apply each not-yet-applied action up to an offline simulation time.
    pub fn apply_until<F>(&self, timestamp: Duration, cursor: &mut usize, mut apply: F)
    where
        F: FnMut(&ActionEvent),
    {
        while *cursor < self.events.len() && self.events[*cursor].timestamp <= timestamp {
            apply(&self.events[*cursor]);
            *cursor = (*cursor).saturating_add(1);
        }
    }
}

/// Capture lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureSessionState {
    /// Accepting frames and actions.
    Recording,
    /// Waiting for submitted PNG jobs.
    Draining,
    /// Output commit complete or session consumed.
    Finalized,
}

/// Completed capture output and resolved dimensions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalizedCapture {
    /// Final file or PNG-sequence directory.
    pub output_path: PathBuf,
    /// Sidecar metadata path, or sequence metadata path.
    pub metadata_path: PathBuf,
    /// Number of canonical frames.
    pub frame_count: u64,
    /// Exact final output dimensions.
    pub output_size: OutputSize,
    /// Exact render dimensions before downsample.
    pub render_size: OutputSize,
}

struct WorkerState {
    completed: u64,
    error: Option<CaptureError>,
}

struct EncodeJob {
    index: u64,
    rgba: Vec<u8>,
    source: OutputSize,
    target: OutputSize,
    workers: usize,
    stage_dir: Arc<PathBuf>,
}

/// Capture lifecycle with bounded staging workers and deterministic frame names.
pub struct CaptureSession {
    config: CaptureConfig,
    source_size: OutputSize,
    output_size: OutputSize,
    render_size: OutputSize,
    schedule: Option<FrameSchedule>,
    stage_dir: PathBuf,
    stage_dir_shared: Arc<PathBuf>,
    sender: Option<SyncSender<EncodeJob>>,
    workers: Vec<JoinHandle<()>>,
    shared: Arc<(Mutex<WorkerState>, Condvar)>,
    submitted: u64,
    realtime_duplicates: u64,
    actions: ActionTimeline,
    finished: bool,
}

impl CaptureSession {
    /// Start a session under `staging_root`; staged data is removed on finish or drop.
    pub fn start(
        config: CaptureConfig,
        source_size: OutputSize,
        staging_root: impl AsRef<Path>,
    ) -> Result<Self, CaptureError> {
        config.validate(source_size)?;
        let output_size = config.resolve_output_size(source_size)?;
        let render_size = config.render_size(source_size)?;
        let schedule = config.schedule()?;
        let stage_dir = create_stage_dir(staging_root.as_ref())?;
        let stage_dir_shared = Arc::new(stage_dir.clone());
        let worker_count = config.parallel_workers.clamp(1, 32);
        let (sender, receiver) = mpsc::sync_channel(worker_count.saturating_mul(2).max(1));
        let receiver = Arc::new(Mutex::new(receiver));
        let shared = Arc::new((
            Mutex::new(WorkerState {
                completed: 0,
                error: None,
            }),
            Condvar::new(),
        ));
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let receiver = Arc::clone(&receiver);
            let shared = Arc::clone(&shared);
            workers.push(thread::spawn(move || worker_loop(receiver, shared)));
        }
        Ok(Self {
            config,
            source_size,
            output_size,
            render_size,
            schedule,
            stage_dir,
            stage_dir_shared,
            sender: Some(sender),
            workers,
            shared,
            submitted: 0,
            realtime_duplicates: 0,
            actions: ActionTimeline::default(),
            finished: false,
        })
    }

    /// Alias for [`CaptureSession::start`].
    pub fn new(
        config: CaptureConfig,
        source_size: OutputSize,
        staging_root: impl AsRef<Path>,
    ) -> Result<Self, CaptureError> {
        Self::start(config, source_size, staging_root)
    }

    /// Resolved source dimensions.
    pub const fn source_size(&self) -> OutputSize {
        self.source_size
    }

    /// Borrow immutable capture config for renderer setup and replay.
    pub fn config(&self) -> &CaptureConfig {
        &self.config
    }

    /// Selected source route.
    pub fn source(&self) -> &CaptureSource {
        &self.config.source
    }

    /// Current lifecycle state.
    pub fn state(&self) -> CaptureSessionState {
        if self.finished {
            return CaptureSessionState::Finalized;
        }
        if self.completed_frames() < self.submitted {
            CaptureSessionState::Draining
        } else {
            CaptureSessionState::Recording
        }
    }

    /// Resolved final dimensions.
    pub const fn output_size(&self) -> OutputSize {
        self.output_size
    }

    /// Resolved renderer dimensions before downsample.
    pub const fn render_size(&self) -> OutputSize {
        self.render_size
    }

    /// Deterministic schedule, when duration configured.
    pub const fn schedule(&self) -> Option<FrameSchedule> {
        self.schedule
    }

    /// Directory containing canonical PNG intermediates.
    pub fn staging_dir(&self) -> &Path {
        &self.stage_dir
    }

    /// Add an action for replay metadata.
    pub fn record_action(
        &mut self,
        timestamp: Duration,
        action: impl Into<String>,
        payload: Option<String>,
    ) -> Result<(), CaptureError> {
        if self.finished {
            return Err(CaptureError::SessionFinalized);
        }
        self.actions.record(timestamp, action, payload);
        Ok(())
    }

    /// Record an action at an exact output frame timestamp.
    pub fn record_action_at_frame(
        &mut self,
        frame: u64,
        action: impl Into<String>,
        payload: Option<String>,
    ) -> Result<(), CaptureError> {
        let schedule = self
            .schedule
            .ok_or(CaptureError::InvalidConfig("frame action needs a schedule"))?;
        let timestamp = schedule
            .timestamp(frame)
            .ok_or(CaptureError::InvalidConfig("action frame outside schedule"))?;
        self.record_action(timestamp, action, payload)
    }

    /// Borrow action timeline for deterministic replay.
    pub fn actions(&self) -> &ActionTimeline {
        &self.actions
    }

    /// Record realtime output frames filled from a prior rendered frame.
    pub fn note_realtime_duplicates(&mut self, count: u64) -> Result<(), CaptureError> {
        if self.finished {
            return Err(CaptureError::SessionFinalized);
        }
        self.realtime_duplicates = self.realtime_duplicates.saturating_add(count);
        Ok(())
    }

    /// Apply recorded actions before a simulation tick.
    pub fn apply_actions_until<F>(&self, timestamp: Duration, cursor: &mut usize, apply: F)
    where
        F: FnMut(&ActionEvent),
    {
        self.actions.apply_until(timestamp, cursor, apply);
    }

    /// Submit one renderer RGBA frame without blocking on PNG encode.
    pub fn submit_rgba(
        &mut self,
        render_width: u32,
        render_height: u32,
        rgba: &[u8],
    ) -> Result<u64, CaptureError> {
        if self.finished {
            return Err(CaptureError::SessionFinalized);
        }
        self.drain_error()?;
        if (OutputSize {
            width: render_width,
            height: render_height,
        }) != self.render_size
        {
            return Err(CaptureError::InvalidFrame("render dimensions mismatch"));
        }
        let expected_bytes = usize::try_from(
            u64::from(render_width)
                .checked_mul(u64::from(render_height))
                .and_then(|value| value.checked_mul(4))
                .ok_or(CaptureError::InvalidFrame("frame byte count overflow"))?,
        )
        .map_err(|_| CaptureError::InvalidFrame("frame byte count exceeds platform size"))?;
        if rgba.len() != expected_bytes {
            return Err(CaptureError::InvalidFrame("RGBA byte count mismatch"));
        }
        if let Some(expected) = self.schedule.map(FrameSchedule::frame_count)
            && self.submitted >= expected
        {
            return Err(CaptureError::FrameCountMismatch {
                expected,
                actual: self.submitted.saturating_add(1),
            });
        }
        let index = self.submitted;
        self.submitted = self.submitted.saturating_add(1);
        let job = EncodeJob {
            index,
            rgba: rgba.to_vec(),
            source: self.render_size,
            target: self.output_size,
            workers: 1,
            stage_dir: Arc::clone(&self.stage_dir_shared),
        };
        self.sender
            .as_ref()
            .ok_or(CaptureError::SessionFinalized)?
            .send(job)
            .map_err(|_| CaptureError::EncoderFailed("PNG worker stopped".to_owned()))?;
        Ok(index)
    }

    /// Submit one frame and wait until its PNG reaches disk.
    pub fn ingest_rgba(
        &mut self,
        render_width: u32,
        render_height: u32,
        rgba: &[u8],
    ) -> Result<u64, CaptureError> {
        let index = self.submit_rgba(render_width, render_height, rgba)?;
        self.drain()?;
        Ok(index)
    }

    /// Wait until all submitted frames finish encoding.
    pub fn drain(&self) -> Result<(), CaptureError> {
        let (lock, cv) = &*self.shared;
        let mut state = lock
            .lock()
            .map_err(|_| CaptureError::EncoderFailed("worker state poisoned".to_owned()))?;
        while state.completed < self.submitted && state.error.is_none() {
            state = cv
                .wait(state)
                .map_err(|_| CaptureError::EncoderFailed("worker state poisoned".to_owned()))?;
        }
        state.error.clone().map_or(Ok(()), Err)
    }

    /// Number of fully encoded canonical PNG frames.
    pub fn completed_frames(&self) -> u64 {
        self.shared
            .0
            .lock()
            .map(|state| state.completed)
            .unwrap_or(0)
    }

    /// Current submit/encode progress.
    pub fn progress(&self) -> CaptureProgress {
        CaptureProgress {
            submitted: self.submitted,
            completed: self.completed_frames(),
            expected: self.schedule.map(FrameSchedule::frame_count),
        }
    }

    /// Finalize using config output.
    pub fn finalize(self) -> Result<FinalizedCapture, CaptureError> {
        let spec = match self.config.output.clone() {
            Some(spec) => spec,
            None => {
                return self.finalize_error(CaptureError::InvalidConfig("output spec required"));
            }
        };
        self.finalize_to(spec)
    }

    /// Stop and commit using config output.
    pub fn stop(self) -> Result<FinalizedCapture, CaptureError> {
        self.finalize()
    }

    /// Stop and commit to explicit output.
    pub fn stop_to(self, spec: OutputSpec) -> Result<FinalizedCapture, CaptureError> {
        self.finalize_to(spec)
    }

    /// Finalize to output, atomically renaming only after encode success.
    pub fn finalize_to(mut self, spec: OutputSpec) -> Result<FinalizedCapture, CaptureError> {
        let result = self.finalize_to_inner(spec);
        match result {
            Ok(result) => Ok(result),
            Err(error) => self.finalize_error(error),
        }
    }

    fn finalize_to_inner(&mut self, spec: OutputSpec) -> Result<FinalizedCapture, CaptureError> {
        self.drain()?;
        self.shutdown_workers()?;
        if let Some(expected) = self.schedule.map(FrameSchedule::frame_count)
            && self.submitted != expected
        {
            return Err(CaptureError::FrameCountMismatch {
                expected,
                actual: self.submitted,
            });
        }
        let metadata = self.metadata_json();
        let stage_metadata = self.stage_dir.join("metadata.json");
        write_atomic(&stage_metadata, metadata.as_bytes())?;
        if spec.path.exists() {
            return Err(CaptureError::OutputExists(spec.path));
        }
        if let Some(parent) = spec.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        if spec.format == OutputFormat::PngSequence {
            fs::rename(&self.stage_dir, &spec.path)?;
            self.finished = true;
            return Ok(FinalizedCapture {
                metadata_path: spec.path.join("metadata.json"),
                output_path: spec.path,
                frame_count: self.submitted,
                output_size: self.output_size,
                render_size: self.render_size,
            });
        }
        let temporary = temp_path(&spec.path);
        let pack_result = match spec.format {
            OutputFormat::Gif => self.pack_gif(&temporary),
            OutputFormat::WebM | OutputFormat::Mp4 | OutputFormat::AnimatedWebP => {
                self.pack_ffmpeg(&spec, &temporary)
            }
            OutputFormat::PngSequence => Ok(()),
        };
        if let Err(error) = pack_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if let Err(error) = fs::rename(&temporary, &spec.path) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        let metadata_path = spec.path.with_extension("json");
        if let Err(error) = write_atomic(&metadata_path, metadata.as_bytes()) {
            return match fs::remove_file(&spec.path) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(CaptureError::CleanupFailed(format!(
                    "{error}; output rollback: {cleanup}"
                ))),
            };
        }
        self.cleanup_stage()?;
        self.finished = true;
        Ok(FinalizedCapture {
            output_path: spec.path,
            metadata_path,
            frame_count: self.submitted,
            output_size: self.output_size,
            render_size: self.render_size,
        })
    }

    fn cleanup_stage(&self) -> Result<(), CaptureError> {
        if self.stage_dir.exists() {
            fs::remove_dir_all(&self.stage_dir)
                .map_err(|error| CaptureError::CleanupFailed(error.to_string()))?;
        }
        Ok(())
    }

    fn finalize_error(mut self, primary: CaptureError) -> Result<FinalizedCapture, CaptureError> {
        let _ = self.shutdown_workers();
        match self.cleanup_stage() {
            Ok(()) => Err(primary),
            Err(error) => Err(CaptureError::CleanupFailed(format!(
                "{primary}; cleanup: {error}"
            ))),
        }
    }

    fn drain_error(&self) -> Result<(), CaptureError> {
        self.shared
            .0
            .lock()
            .map_err(|_| CaptureError::EncoderFailed("worker state poisoned".to_owned()))?
            .error
            .clone()
            .map_or(Ok(()), Err)
    }

    fn shutdown_workers(&mut self) -> Result<(), CaptureError> {
        self.sender.take();
        let mut panic_error = None;
        for worker in self.workers.drain(..) {
            if worker.join().is_err() && panic_error.is_none() {
                panic_error = Some(CaptureError::EncoderFailed(
                    "PNG worker panicked".to_owned(),
                ));
            }
        }
        panic_error.map_or(Ok(()), Err)
    }

    fn metadata_json(&self) -> String {
        let schedule = self.schedule;
        let rate = self
            .config
            .frame_rate
            .unwrap_or_else(|| FrameRate::integer(self.config.fps));
        let frame_count = schedule.map_or(self.submitted, FrameSchedule::frame_count);
        let duration = self.config.duration.map_or_else(
            || {
                u128::from(frame_count)
                    .saturating_mul(1_000_000_000)
                    .saturating_mul(u128::from(rate.denominator()))
                    / u128::from(rate.numerator())
            },
            |value| value.as_nanos(),
        );
        let timestamp_step_numerator_ns =
            1_000_000_000_u128.saturating_mul(u128::from(rate.denominator()));
        let actions = self
            .actions
            .events
            .iter()
            .map(|event| {
                let payload = event
                    .payload
                    .as_deref()
                    .map(|value| format!(",\"payload\":\"{}\"", json_escape(value)))
                    .unwrap_or_default();
                format!(
                    "{{\"timestamp_ns\":{},\"action\":\"{}\"{payload}}}",
                    event.timestamp.as_nanos(),
                    json_escape(&event.action)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"source\":\"{}\",\"mode\":\"{}\",\"fps\":{},\"fps_num\":{},\"fps_den\":{},\"frame_count\":{},\"duration_ns\":{},\"first_timestamp_ns\":0,\"timestamp_step_num_ns\":{},\"timestamp_step_den\":{},\"realtime_duplicate_frames\":{},\"transparent\":{},\"supersample\":{},\"source_width\":{},\"source_height\":{},\"render_width\":{},\"render_height\":{},\"output_width\":{},\"output_height\":{},\"framing\":\"{}\",\"actions\":[{}]}}\n",
            json_escape(&self.config.source.label()),
            match self.config.mode {
                CaptureMode::Realtime => "realtime",
                CaptureMode::Offline => "offline",
            },
            rate.decimal_string(),
            rate.numerator(),
            rate.denominator(),
            frame_count,
            duration,
            timestamp_step_numerator_ns,
            rate.numerator(),
            self.realtime_duplicates,
            self.config.transparent,
            self.config.supersample,
            self.source_size.width,
            self.source_size.height,
            self.render_size.width,
            self.render_size.height,
            self.output_size.width,
            self.output_size.height,
            match self.config.framing {
                Framing::Fit => "fit",
                Framing::Crop => "crop",
                Framing::Expand => "expand",
                Framing::Stretch => "stretch",
            },
            actions
        )
    }

    fn frame_path(&self, index: u64) -> PathBuf {
        self.stage_dir.join(format!("frame_{index:06}.png"))
    }

    fn pack_gif(&self, output: &Path) -> Result<(), CaptureError> {
        let width = u16::try_from(self.output_size.width)
            .map_err(|_| CaptureError::InvalidConfig("GIF width exceeds 65535"))?;
        let height = u16::try_from(self.output_size.height)
            .map_err(|_| CaptureError::InvalidConfig("GIF height exceeds 65535"))?;
        let palette = self.build_gif_palette()?;
        let file = BufWriter::new(File::create(output)?);
        let mut encoder = gif::Encoder::new(file, width, height, palette.colors())
            .map_err(|error| CaptureError::Image(error.to_string()))?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|error| CaptureError::Image(error.to_string()))?;
        let rate = self.config.effective_frame_rate()?;
        for index in 0..self.submitted {
            let frame = image::open(self.frame_path(index))?.into_rgba8();
            let mut frame = gif::Frame::from_indexed_pixels(
                width,
                height,
                palette.index_pixels(frame.as_raw()),
                Some(0),
            );
            frame.delay = gif_frame_delay_centiseconds(rate, index)?;
            frame.dispose = gif::DisposalMethod::Background;
            encoder
                .write_frame(&frame)
                .map_err(|error| CaptureError::Image(error.to_string()))?;
        }
        Ok(())
    }

    fn build_gif_palette(&self) -> Result<GifPalette, CaptureError> {
        let frames = usize::try_from(self.submitted)
            .map_err(|_| CaptureError::InvalidFrame("GIF frame count exceeds platform size"))?;
        let frame_sample_budget = GIF_MAX_PALETTE_SAMPLES
            .checked_div(frames.max(1))
            .unwrap_or(1)
            .max(1);
        let mut exact = BTreeSet::new();
        let mut exact_overflow = false;
        let mut samples = Vec::new();

        for index in 0..self.submitted {
            let frame = image::open(self.frame_path(index))?.into_rgba8();
            let opaque_count = frame
                .as_raw()
                .chunks_exact(4)
                .filter(|pixel| pixel[3] >= GIF_ALPHA_THRESHOLD)
                .count();
            let sample_step = opaque_count.div_ceil(frame_sample_budget).max(1);
            let mut opaque_index = 0_usize;
            for pixel in frame.as_raw().chunks_exact(4) {
                if pixel[3] < GIF_ALPHA_THRESHOLD {
                    continue;
                }
                let rgb = [pixel[0], pixel[1], pixel[2]];
                if !exact_overflow {
                    exact.insert(rgb);
                    if exact.len() > GIF_MAX_OPAQUE_COLORS {
                        exact.clear();
                        exact_overflow = true;
                    }
                }
                if opaque_index.is_multiple_of(sample_step)
                    && samples.len() / 4 < GIF_MAX_PALETTE_SAMPLES
                {
                    samples.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
                opaque_index += 1;
            }
        }

        if !exact_overflow {
            return Ok(GifPalette::exact(exact));
        }
        Ok(GifPalette::quantized(samples))
    }

    fn pack_ffmpeg(&self, spec: &OutputSpec, output: &Path) -> Result<(), CaptureError> {
        let program = spec
            .ffmpeg
            .as_deref()
            .unwrap_or_else(|| Path::new("ffmpeg"));
        let probe = Command::new(program)
            .arg("-version")
            .output()
            .map_err(|error| {
                CaptureError::EncoderUnavailable(format!("{}: {error}", program.display()))
            })?;
        if !probe.status.success() {
            return Err(CaptureError::EncoderUnavailable(
                program.display().to_string(),
            ));
        }
        let pattern = self.stage_dir.join("frame_%06d.png");
        let mut command = Command::new(program);
        command
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args([
                "-framerate",
                &self.config.effective_frame_rate()?.ffmpeg_string(),
            ])
            .arg("-i")
            .arg(pattern);
        match spec.format {
            OutputFormat::WebM => {
                command.args(["-c:v", "libvpx-vp9", "-pix_fmt"]);
                command.arg(if self.config.transparent {
                    "yuva420p"
                } else {
                    "yuv420p"
                });
            }
            OutputFormat::Mp4 => {
                command.args(["-c:v", "libx264", "-pix_fmt", "yuv420p"]);
            }
            OutputFormat::AnimatedWebP => {
                command.args([
                    "-f",
                    "webp",
                    "-c:v",
                    "libwebp_anim",
                    "-lossless",
                    "1",
                    "-compression_level",
                    "6",
                    "-pix_fmt",
                    "bgra",
                    "-loop",
                    "0",
                ]);
            }
            OutputFormat::PngSequence | OutputFormat::Gif => {}
        }
        let status = command.arg(output).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(CaptureError::EncoderFailed(format!(
                "{} exited with {status}",
                program.display()
            )))
        }
    }
}

impl Drop for CaptureSession {
    fn drop(&mut self) {
        self.sender.take();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        if !self.finished {
            let _ = fs::remove_dir_all(&self.stage_dir);
        }
    }
}

enum GifPalette {
    Exact {
        colors: Vec<u8>,
        indexes: BTreeMap<[u8; 3], u8>,
    },
    Quantized {
        colors: Vec<u8>,
        quantizer: color_quant::NeuQuant,
    },
}

impl GifPalette {
    fn exact(colors: BTreeSet<[u8; 3]>) -> Self {
        let mut palette = vec![0, 0, 0];
        let mut indexes = BTreeMap::new();
        for (offset, color) in colors.into_iter().enumerate() {
            let index = u8::try_from(offset + 1).unwrap_or(u8::MAX);
            palette.extend_from_slice(&color);
            indexes.insert(color, index);
        }
        if palette.len() == 3 {
            palette.extend_from_slice(&[0, 0, 0]);
        }
        Self::Exact {
            colors: palette,
            indexes,
        }
    }

    fn quantized(samples: Vec<u8>) -> Self {
        let quantizer = color_quant::NeuQuant::new(1, GIF_MAX_OPAQUE_COLORS, &samples);
        let mut colors = vec![0, 0, 0];
        colors.extend_from_slice(&quantizer.color_map_rgb());
        Self::Quantized { colors, quantizer }
    }

    fn colors(&self) -> &[u8] {
        match self {
            Self::Exact { colors, .. } | Self::Quantized { colors, .. } => colors,
        }
    }

    fn index_pixels(&self, rgba: &[u8]) -> Vec<u8> {
        rgba.chunks_exact(4)
            .map(|pixel| {
                if pixel[3] < GIF_ALPHA_THRESHOLD {
                    return 0;
                }
                match self {
                    Self::Exact { indexes, .. } => indexes
                        .get(&[pixel[0], pixel[1], pixel[2]])
                        .copied()
                        .unwrap_or(0),
                    Self::Quantized { quantizer, .. } => {
                        let opaque = [pixel[0], pixel[1], pixel[2], 255];
                        u8::try_from(quantizer.index_of(&opaque) + 1).unwrap_or(u8::MAX)
                    }
                }
            })
            .collect()
    }
}

fn gif_frame_delay_centiseconds(rate: FrameRate, index: u64) -> Result<u16, CaptureError> {
    let scale = u128::from(rate.denominator()) * 100;
    let denominator = u128::from(rate.numerator());
    let start = u128::from(index)
        .checked_mul(scale)
        .ok_or(CaptureError::InvalidConfig("GIF frame delay overflow"))?;
    let end = u128::from(index.saturating_add(1))
        .checked_mul(scale)
        .ok_or(CaptureError::InvalidConfig("GIF frame delay overflow"))?;
    let start = (start + denominator / 2) / denominator;
    let end = (end + denominator / 2) / denominator;
    Ok(u16::try_from(end.saturating_sub(start).max(1)).unwrap_or(u16::MAX))
}

fn worker_loop(
    receiver: Arc<Mutex<Receiver<EncodeJob>>>,
    shared: Arc<(Mutex<WorkerState>, Condvar)>,
) {
    loop {
        let job = match receiver.lock() {
            Ok(lock) => lock.recv(),
            Err(_) => return,
        };
        let Ok(job) = job else { return };
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| encode_job(job)))
            .map_err(|_| CaptureError::EncoderFailed("PNG worker panicked".to_owned()))
            .and_then(|result| result);
        let (lock, cv) = &*shared;
        if let Ok(mut state) = lock.lock() {
            if let Err(error) = result
                && state.error.is_none()
            {
                state.error = Some(error);
            }
            state.completed = state.completed.saturating_add(1);
            cv.notify_all();
        }
    }
}

fn encode_job(job: EncodeJob) -> Result<(), CaptureError> {
    let resized = downsample_rgba_owned(job.rgba, job.source, job.target, job.workers)?;
    let path = job.stage_dir.join(format!("frame_{:06}.png", job.index));
    let temporary = path.with_extension("png.partial");
    let file = File::create(&temporary)?;
    let mut writer = BufWriter::new(file);
    PngEncoder::new(&mut writer).write_image(
        &resized,
        job.target.width,
        job.target.height,
        image::ExtendedColorType::Rgba8,
    )?;
    writer.flush()?;
    let file = writer
        .into_inner()
        .map_err(|error| CaptureError::Io(error.to_string()))?;
    drop(file);
    fs::rename(temporary, path)?;
    Ok(())
}

fn create_stage_dir(root: &Path) -> Result<PathBuf, CaptureError> {
    static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);
    let parent = root.join(".perro_capture_staging");
    fs::create_dir_all(&parent)?;
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    for _ in 0..64 {
        let id = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("capture-{}-{tick}-{id}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(CaptureError::Io(
        "could not allocate staging directory".to_owned(),
    ))
}

fn temp_path(path: &Path) -> PathBuf {
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("capture-output");
    path.with_file_name(format!(".{name}.{id}.partial"))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), CaptureError> {
    let temporary = temp_path(path);
    let result = (|| {
        let mut file = File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok::<(), CaptureError>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            character if character.is_control() => format!("\\u{:04x}", character as u32)
                .chars()
                .collect::<Vec<_>>(),
            character => vec![character],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::AnimationDecoder;

    #[test]
    fn offline_schedule_keeps_exact_count_and_timestamps() {
        let schedule = FrameSchedule::from_duration(300, Duration::from_secs(10))
            .unwrap_or_else(|err| panic!("schedule: {err}"));
        assert_eq!(schedule.frame_count(), 3_000);
        assert_eq!(schedule.timestamp(0), Some(Duration::ZERO));
        assert_eq!(schedule.timestamp(1), Some(Duration::from_nanos(3_333_333)));
        assert_eq!(
            schedule.timestamp(2_999),
            Some(Duration::from_nanos(9_996_666_666))
        );
        assert_eq!(schedule.timestamp(3_000), None);
    }

    #[test]
    fn fractional_rate_uses_exact_rational_schedule() {
        let rate = FrameRate::parse("23.976").unwrap_or_else(|err| panic!("rate: {err}"));
        assert_eq!(rate, FrameRate::new(2997, 125).expect("reduced rate"));
        assert_eq!(rate.ffmpeg_string(), "2997/125");
        let schedule = FrameSchedule::from_rate(
            FrameRate::parse("30000/1001").unwrap_or_else(|err| panic!("rate: {err}")),
            Duration::from_millis(1_001),
        )
        .unwrap_or_else(|err| panic!("schedule: {err}"));
        assert_eq!(schedule.frame_count(), 30);
        assert_eq!(
            schedule.timestamp(1),
            Some(Duration::from_nanos(33_366_666))
        );
    }

    #[test]
    fn gif_delay_distributes_fractional_centiseconds() {
        let rate = FrameRate::integer(60);
        let delays = (0..6)
            .map(|index| gif_frame_delay_centiseconds(rate, index).expect("GIF delay"))
            .collect::<Vec<_>>();
        assert_eq!(delays, [2, 1, 2, 2, 1, 2]);
        assert_eq!(delays.into_iter().sum::<u16>(), 10);

        let ntsc = FrameRate::new(30_000, 1_001).expect("NTSC rate");
        let delays = (0..3)
            .map(|index| gif_frame_delay_centiseconds(ntsc, index).expect("GIF delay"))
            .collect::<Vec<_>>();
        assert_eq!(delays, [3, 4, 3]);
    }

    #[test]
    fn gif_palette_keeps_exact_colors_and_cuts_partial_alpha() {
        let palette = GifPalette::exact(BTreeSet::from([[10, 20, 30], [40, 50, 60]]));
        let indexed = palette.index_pixels(&[10, 20, 30, 255, 40, 50, 60, 128, 10, 20, 30, 127]);
        assert_ne!(indexed[0], 0);
        assert_ne!(indexed[1], 0);
        assert_ne!(indexed[0], indexed[1]);
        assert_eq!(indexed[2], 0);
        assert_eq!(palette.colors().len(), 9);
    }

    #[test]
    fn config_frame_rate_overrides_integer_fps() {
        let config = CaptureConfig {
            fps: 60,
            frame_rate: Some(FrameRate::parse("29.97").unwrap_or_else(|err| panic!("rate: {err}"))),
            duration: Some(Duration::from_secs(1)),
            frame_count: None,
            ..CaptureConfig::default()
        };
        let schedule = config
            .schedule()
            .unwrap_or_else(|err| panic!("schedule: {err}"))
            .expect("duration schedule");
        assert_eq!(schedule.frame_count(), 29);
        assert_eq!(schedule.rate(), FrameRate::new(2997, 100).expect("rate"));
    }

    #[test]
    fn width_only_preserves_source_ratio() {
        let mut config = CaptureConfig {
            width: Some(1_170),
            ..CaptureConfig::default()
        };
        config.aspect_ratio = AspectRatio::Preserve;
        let output = config
            .resolve_output_size(OutputSize::new(16, 9).unwrap_or_else(|err| panic!("size: {err}")))
            .unwrap_or_else(|err| panic!("output: {err}"));
        assert_eq!(
            output,
            OutputSize {
                width: 1_170,
                height: 658
            }
        );
        assert_eq!(
            config
                .render_size(OutputSize {
                    width: 16,
                    height: 9
                })
                .unwrap_or_else(|err| panic!("render: {err}")),
            OutputSize {
                width: 2_340,
                height: 1_316
            }
        );
    }

    #[test]
    fn explicit_aspect_ratio_does_not_change_exact_width_height() {
        let config = CaptureConfig {
            width: Some(1_920),
            aspect_ratio: AspectRatio::new(16, 9).unwrap_or_else(|err| panic!("ratio: {err}")),
            ..CaptureConfig::default()
        };
        let output = config
            .resolve_output_size(OutputSize {
                width: 4,
                height: 3,
            })
            .unwrap_or_else(|err| panic!("output: {err}"));
        assert_eq!(
            output,
            OutputSize {
                width: 1_920,
                height: 1_080
            }
        );
    }

    #[test]
    fn downsample_preserves_transparent_rgb_without_dark_fringe() {
        let source = OutputSize {
            width: 2,
            height: 1,
        };
        let target = OutputSize {
            width: 1,
            height: 1,
        };
        let rgba = [255, 0, 0, 128, 0, 0, 0, 0];
        let resized =
            downsample_rgba(&rgba, source, target, 2).unwrap_or_else(|err| panic!("resize: {err}"));
        assert_eq!(resized.len(), 4);
        assert!(resized[0] >= 240);
        assert!(resized[3] > 0);
    }

    #[test]
    fn downsample_rejects_bad_buffer() {
        let result = downsample_rgba(
            &[0; 3],
            OutputSize {
                width: 1,
                height: 1,
            },
            OutputSize {
                width: 1,
                height: 1,
            },
            1,
        );
        assert!(matches!(result, Err(CaptureError::InvalidFrame(_))));
    }

    #[test]
    fn owned_downsample_reuses_same_size_buffer() {
        let source = OutputSize {
            width: 2,
            height: 2,
        };
        let rgba = vec![17_u8; 16];
        let ptr = rgba.as_ptr();
        let resized = downsample_rgba_owned(rgba, source, source, 1)
            .unwrap_or_else(|err| panic!("resize: {err}"));
        assert_eq!(resized.as_ptr(), ptr);
    }

    fn test_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("perro-capture-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap_or_else(|err| panic!("test root: {err}"));
        root
    }

    fn test_config() -> CaptureConfig {
        CaptureConfig {
            width: Some(2),
            height: Some(2),
            fps: 2,
            mode: CaptureMode::Offline,
            duration: Some(Duration::from_secs(1)),
            parallel_workers: 2,
            ..CaptureConfig::default()
        }
    }

    fn test_frame() -> Vec<u8> {
        let mut frame = vec![0_u8; 4 * 4 * 4];
        for pixel in frame.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[255, 64, 16, 255]);
        }
        frame
    }

    #[test]
    fn session_drains_workers_and_writes_exact_png_dimensions() {
        let root = test_root("sequence");
        let mut session = CaptureSession::start(
            test_config(),
            OutputSize {
                width: 2,
                height: 2,
            },
            &root,
        )
        .unwrap_or_else(|err| panic!("start: {err}"));
        let frame = test_frame();
        session
            .submit_rgba(4, 4, &frame)
            .unwrap_or_else(|err| panic!("submit: {err}"));
        session
            .submit_rgba(4, 4, &frame)
            .unwrap_or_else(|err| panic!("submit: {err}"));
        session.drain().unwrap_or_else(|err| panic!("drain: {err}"));
        assert_eq!(session.completed_frames(), 2);
        let output = root.join("frames");
        let result = session
            .finalize_to(OutputSpec::new(&output, OutputFormat::PngSequence))
            .unwrap_or_else(|err| panic!("finalize: {err}"));
        assert_eq!(result.frame_count, 2);
        assert_eq!(
            result.output_size,
            OutputSize {
                width: 2,
                height: 2
            }
        );
        let image = image::open(output.join("frame_000000.png"))
            .unwrap_or_else(|err| panic!("read frame: {err}"));
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 2);
        let metadata = fs::read_to_string(output.join("metadata.json"))
            .unwrap_or_else(|err| panic!("read metadata: {err}"));
        assert!(metadata.contains("\"duration_ns\":1000000000"));
        assert!(metadata.contains("\"first_timestamp_ns\":0"));
        assert!(metadata.contains("\"timestamp_step_num_ns\":1000000000"));
        assert!(metadata.contains("\"timestamp_step_den\":2"));
        assert!(metadata.contains("\"realtime_duplicate_frames\":0"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn packed_success_removes_staging_after_output_commit() {
        let root = test_root("gif");
        let mut session = CaptureSession::start(
            test_config(),
            OutputSize {
                width: 2,
                height: 2,
            },
            &root,
        )
        .unwrap_or_else(|err| panic!("start: {err}"));
        let frame = test_frame();
        session
            .ingest_rgba(4, 4, &frame)
            .unwrap_or_else(|err| panic!("ingest: {err}"));
        session
            .ingest_rgba(4, 4, &frame)
            .unwrap_or_else(|err| panic!("ingest: {err}"));
        let stage = session.staging_dir().to_path_buf();
        let output = root.join("clip.gif");
        let result = session
            .finalize_to(OutputSpec::new(&output, OutputFormat::Gif))
            .unwrap_or_else(|err| panic!("finalize: {err}"));
        assert_eq!(result.frame_count, 2);
        assert!(output.is_file());
        assert!(result.metadata_path.is_file());
        assert!(!stage.exists());
        let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
            File::open(&output).unwrap_or_else(|err| panic!("open GIF: {err}")),
        ))
        .unwrap_or_else(|err| panic!("decode GIF: {err}"));
        let frames = decoder
            .into_frames()
            .collect_frames()
            .unwrap_or_else(|err| panic!("read GIF frames: {err}"));
        assert_eq!(frames.len(), 2);
        assert!(
            frames
                .iter()
                .all(|frame| frame.buffer().dimensions() == (2, 2))
        );
        assert_eq!(frames[0].delay().numer_denom_ms(), (500, 1));
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::Indexed);
        let mut decoder = options
            .read_info(std::io::BufReader::new(
                File::open(&output).unwrap_or_else(|err| panic!("open indexed GIF: {err}")),
            ))
            .unwrap_or_else(|err| panic!("decode indexed GIF: {err}"));
        assert!(decoder.global_palette().is_some());
        let mut indexed_frames = 0;
        while let Some(frame) = decoder
            .read_next_frame()
            .unwrap_or_else(|err| panic!("read indexed GIF frame: {err}"))
        {
            assert!(frame.palette.is_none());
            assert_eq!(frame.dispose, gif::DisposalMethod::Background);
            indexed_frames += 1;
        }
        assert_eq!(indexed_frames, 2);
        let metadata = fs::read_to_string(&result.metadata_path)
            .unwrap_or_else(|err| panic!("read GIF metadata: {err}"));
        assert!(metadata.contains("\"frame_count\":2"));
        assert!(metadata.contains("\"duration_ns\":1000000000"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_external_pack_removes_staging_frames() {
        let root = test_root("failed-pack");
        let mut session = CaptureSession::start(
            test_config(),
            OutputSize {
                width: 2,
                height: 2,
            },
            &root,
        )
        .unwrap_or_else(|err| panic!("start: {err}"));
        let frame = test_frame();
        session
            .ingest_rgba(4, 4, &frame)
            .unwrap_or_else(|err| panic!("ingest: {err}"));
        session
            .ingest_rgba(4, 4, &frame)
            .unwrap_or_else(|err| panic!("ingest: {err}"));
        let stage = session.staging_dir().to_path_buf();
        let result = session.finalize_to(
            OutputSpec::new(root.join("clip.webm"), OutputFormat::WebM)
                .with_ffmpeg(root.join("missing-ffmpeg")),
        );
        match result {
            Err(CaptureError::EncoderUnavailable(_)) => {}
            Err(error) => panic!("unexpected error: {error:?}"),
            Ok(_) => panic!("missing encoder unexpectedly succeeded"),
        }
        assert!(!stage.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn finalize_rejects_missing_scheduled_frames_and_removes_staging() {
        let root = test_root("count");
        let mut session = CaptureSession::start(
            test_config(),
            OutputSize {
                width: 2,
                height: 2,
            },
            &root,
        )
        .unwrap_or_else(|err| panic!("start: {err}"));
        let frame = test_frame();
        session
            .ingest_rgba(4, 4, &frame)
            .unwrap_or_else(|err| panic!("ingest: {err}"));
        let stage = session.staging_dir().to_path_buf();
        let result = session.finalize_to(OutputSpec::new(
            root.join("frames"),
            OutputFormat::PngSequence,
        ));
        assert!(matches!(
            result,
            Err(CaptureError::FrameCountMismatch {
                expected: 2,
                actual: 1
            })
        ));
        assert!(!stage.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dropping_session_removes_staging_frames() {
        let root = test_root("drop-cleanup");
        let mut session = CaptureSession::start(
            test_config(),
            OutputSize {
                width: 2,
                height: 2,
            },
            &root,
        )
        .unwrap_or_else(|err| panic!("start: {err}"));
        session
            .ingest_rgba(4, 4, &test_frame())
            .unwrap_or_else(|err| panic!("ingest: {err}"));
        let stage = session.staging_dir().to_path_buf();
        assert!(stage.exists());
        drop(session);
        assert!(!stage.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn metadata_failure_removes_media_staging_and_partial_files() {
        let root = test_root("metadata-rollback");
        let mut session = CaptureSession::start(
            test_config(),
            OutputSize {
                width: 2,
                height: 2,
            },
            &root,
        )
        .unwrap();
        session.ingest_rgba(4, 4, &test_frame()).unwrap();
        session.ingest_rgba(4, 4, &test_frame()).unwrap();
        let stage = session.staging_dir().to_path_buf();
        let output = root.join("clip.gif");
        let metadata = output.with_extension("json");
        fs::create_dir(&metadata).unwrap();
        fs::write(metadata.join("keep"), b"keep").unwrap();
        assert!(
            session
                .finalize_to(OutputSpec::new(&output, OutputFormat::Gif))
                .is_err()
        );
        assert!(!stage.exists());
        assert!(!output.exists());
        assert!(metadata.join("keep").is_file());
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            entry
                .unwrap()
                .path()
                .extension()
                .is_none_or(|extension| extension != "partial")
        }));
        assert_eq!(fs::read_dir(stage.parent().unwrap()).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_routes_and_action_replay_follow_schedule() {
        let source = CaptureSource::ui_sub_view("gameplay");
        assert_eq!(source.kind(), CaptureSourceKind::UISubView);
        assert!(source.validate().is_ok());
        assert!(CaptureSource::camera_3d("").validate().is_err());

        let root = test_root("actions");
        let mut config = test_config();
        config.source = source;
        let mut session = CaptureSession::start(
            config,
            OutputSize {
                width: 2,
                height: 2,
            },
            &root,
        )
        .unwrap_or_else(|err| panic!("start: {err}"));
        session
            .record_action_at_frame(1, "roll", Some("{\"count\":2}".to_owned()))
            .unwrap_or_else(|err| panic!("action: {err}"));
        let mut cursor = 0;
        let mut applied = Vec::new();
        session.apply_actions_until(Duration::from_millis(500), &mut cursor, |event| {
            applied.push(event.action.clone());
        });
        assert_eq!(applied, vec!["roll"]);
        assert_eq!(cursor, 1);
        drop(session);
        let _ = fs::remove_dir_all(root);
    }
}
