//! Capture session control exposed to runtime hosts and QA tooling.

use perro_capture::{
    CaptureConfig, CaptureProgress, CaptureSessionState, CaptureSource, CaptureSourceKind,
    FinalizedCapture, Framing, OutputSize, OutputSpec,
};
use perro_ids::NodeID;
use perro_nodes::NodeType;
use std::time::Duration;

/// Typed camera source helpers for normal script/API use.
pub trait CaptureSourceNodeExt {
    /// Build a 2D camera source from an exact live [`NodeID`].
    fn camera_2d_node(node: NodeID) -> CaptureSource;
    /// Build a 3D camera source from an exact live [`NodeID`].
    fn camera_3d_node(node: NodeID) -> CaptureSource;
}

impl CaptureSourceNodeExt for CaptureSource {
    fn camera_2d_node(node: NodeID) -> CaptureSource {
        CaptureSource::camera_2d_node_id(node.as_u64())
    }

    fn camera_3d_node(node: NodeID) -> CaptureSource {
        CaptureSource::camera_3d_node_id(node.as_u64())
    }
}

/// Renderer route after runtime source lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureSourceRoute {
    /// Source kind requested by capture config.
    pub kind: CaptureSourceKind,
    /// Resolved scene node; `None` for the main window.
    pub node: Option<NodeID>,
    /// Render stream node that owns the selected output texture.
    pub render_node: Option<NodeID>,
    /// Concrete node type when `node` resolves to a scene node.
    pub node_type: Option<NodeType>,
    /// Pixel size exposed by source route before output framing.
    pub source_size: OutputSize,
    /// Aspect policy passed to renderer extraction.
    pub framing: Framing,
}

/// Runtime implementation contract for capture lifecycle control.
pub trait CaptureAPI {
    /// Start from public config, deriving source size and staging paths.
    fn capture_start_config(&mut self, config: CaptureConfig) -> Result<(), String>;
    /// Start a capture session for a resolved source size.
    fn capture_start(
        &mut self,
        config: CaptureConfig,
        source_size: OutputSize,
        staging_root: &str,
    ) -> Result<(), String>;
    /// Read current capture state.
    fn capture_state(&self) -> Option<CaptureSessionState>;
    /// Read bounded queue progress.
    fn capture_progress(&self) -> Option<CaptureProgress>;
    /// Return the last committed output and metadata paths.
    fn capture_last_output(&self) -> Option<FinalizedCapture>;
    /// Resolve active source and render dimensions.
    fn capture_source(&self) -> Option<CaptureSource>;
    /// Return resolved renderer route for active capture.
    fn capture_source_route(&self) -> Option<CaptureSourceRoute>;
    /// Resolve active render dimensions.
    fn capture_render_size(&self) -> Option<OutputSize>;
    /// Submit one renderer RGBA frame.
    fn capture_submit_rgba(
        &mut self,
        frame_index: u64,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<u64, String>;
    /// Wait for all submitted frames.
    fn capture_drain(&self) -> Result<(), String>;
    /// Number of fully encoded frames.
    fn capture_completed_frames(&self) -> u64;
    /// Request stop at a safe boundary.
    fn capture_request_stop(&mut self, output: OutputSpec);
    /// Request stop with the output stored in the active config.
    fn capture_request_configured_stop(&mut self) -> Result<(), String>;
    /// Take a pending stop request.
    fn capture_take_stop_request(&mut self) -> bool;
    /// Take output spec attached to a pending stop request.
    fn capture_take_stop_output(&mut self) -> Option<OutputSpec>;
    /// Append a replay/director action.
    fn capture_record_action(
        &mut self,
        timestamp: Duration,
        action: String,
        payload: Option<String>,
    ) -> Result<(), String>;
    /// Stop capture and atomically commit output.
    fn capture_stop(&mut self, output: OutputSpec) -> Result<FinalizedCapture, String>;
}

/// Script/QA capture module over a runtime borrow.
pub struct CaptureModule<'rt, RT: ?Sized> {
    rt: &'rt mut RT,
}

impl<'rt, RT: ?Sized> CaptureModule<'rt, RT> {
    pub(crate) fn new(rt: &'rt mut RT) -> Self {
        Self { rt }
    }
}

impl<'rt, RT: CaptureAPI + ?Sized> CaptureModule<'rt, RT> {
    /// Start capture through the normal renderer bridge.
    ///
    /// Set [`CaptureConfig::output`] before calling this method.
    pub fn start(&mut self, config: CaptureConfig) -> Result<(), String> {
        self.rt.capture_start_config(config)
    }

    /// Start with explicit integration dimensions and staging path.
    pub fn start_raw(
        &mut self,
        config: CaptureConfig,
        source_size: OutputSize,
        staging_root: &str,
    ) -> Result<(), String> {
        self.rt.capture_start(config, source_size, staging_root)
    }

    /// Read current state.
    pub fn state(&self) -> Option<CaptureSessionState> {
        self.rt.capture_state()
    }

    /// Read queue progress.
    pub fn progress(&self) -> Option<CaptureProgress> {
        self.rt.capture_progress()
    }

    /// Return last committed output and metadata paths.
    pub fn last_output(&self) -> Option<FinalizedCapture> {
        self.rt.capture_last_output()
    }

    /// Resolve active source route.
    pub fn source(&self) -> Option<CaptureSource> {
        self.rt.capture_source()
    }

    /// Return resolved renderer route for active capture.
    pub fn source_route(&self) -> Option<CaptureSourceRoute> {
        self.rt.capture_source_route()
    }

    /// Resolve renderer dimensions before downsample.
    pub fn render_size(&self) -> Option<OutputSize> {
        self.rt.capture_render_size()
    }

    /// Submit renderer RGBA bytes.
    pub fn submit_rgba(
        &mut self,
        frame_index: u64,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<u64, String> {
        self.rt
            .capture_submit_rgba(frame_index, width, height, rgba)
    }

    /// Wait until all submitted frames encode.
    pub fn drain(&self) -> Result<(), String> {
        self.rt.capture_drain()
    }

    /// Read fully encoded frame count.
    pub fn completed_frames(&self) -> u64 {
        self.rt.capture_completed_frames()
    }

    /// Request stop at next app boundary with the active config output.
    pub fn stop(&mut self) -> Result<(), String> {
        self.rt.capture_request_configured_stop()
    }

    /// Request stop at next app boundary with an output override.
    pub fn request_stop(&mut self, output: OutputSpec) {
        self.rt.capture_request_stop(output);
    }

    /// Read and clear pending stop request.
    pub fn take_stop_request(&mut self) -> bool {
        self.rt.capture_take_stop_request()
    }

    /// Take output spec attached to a pending stop request.
    pub fn take_stop_output(&mut self) -> Option<OutputSpec> {
        self.rt.capture_take_stop_output()
    }

    /// Append replay/director action.
    pub fn record_action(
        &mut self,
        timestamp: Duration,
        action: impl Into<String>,
        payload: Option<String>,
    ) -> Result<(), String> {
        self.rt
            .capture_record_action(timestamp, action.into(), payload)
    }

    /// Drain and commit directly for renderer hosts.
    pub fn finish_raw(&mut self, output: OutputSpec) -> Result<FinalizedCapture, String> {
        self.rt.capture_stop(output)
    }
}
