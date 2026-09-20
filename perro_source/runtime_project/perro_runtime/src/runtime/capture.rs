//! Runtime-owned capture lifecycle host.

use super::Runtime;
use perro_capture::{
    ActionEvent, CaptureConfig, CaptureProgress, CaptureSession, CaptureSessionState,
    CaptureSource, CaptureSourceKind, FinalizedCapture, Framing, OutputSize, OutputSpec,
};
use perro_ids::NodeID;
use perro_input_api::{KeyCode, MouseButton};
use perro_nodes::{NodeType, SceneNodeData, SubView};
use perro_render_bridge::{
    CameraProjectionState, CameraStreamCommand, CameraStreamSourceState, CameraStreamState,
    RenderCommand,
};
use perro_runtime_api::sub_apis::{CaptureAPI, CaptureSourceRoute, SignalAPI};
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct CaptureFinish {
    worker: Option<std::thread::JoinHandle<Result<FinalizedCapture, String>>>,
}

impl Drop for CaptureFinish {
    fn drop(&mut self) {
        // Runtime shutdown must wait for output commit and staging cleanup.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Runtime {
    pub(crate) fn apply_capture_actions_before_fixed_tick(&mut self, delta: f32) {
        let timestamp = self.capture_replay_time;
        let events = self
            .capture_session
            .as_ref()
            .map(|session| {
                session
                    .actions()
                    .events()
                    .iter()
                    .skip(self.capture_replay_cursor)
                    .take_while(|event| event.timestamp <= timestamp)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.capture_replay_cursor = self.capture_replay_cursor.saturating_add(events.len());
        for event in &events {
            self.apply_capture_action(event);
        }
        if delta.is_finite() && delta > 0.0 {
            self.capture_replay_time = self
                .capture_replay_time
                .saturating_add(Duration::from_secs_f32(delta));
        }
    }

    fn apply_capture_action(&mut self, event: &ActionEvent) {
        let action = event
            .action
            .strip_prefix("input:")
            .unwrap_or(event.action.as_str());
        let (kind, inline_arg) = action
            .split_once(':')
            .map_or((action, None), |(kind, arg)| (kind, Some(arg)));
        let arg = inline_arg.or(event.payload.as_deref()).unwrap_or_default();
        match kind {
            "key_down" => {
                if let Some(key) = KeyCode::from_name(arg) {
                    self.set_key_state(key, true);
                }
            }
            "key_up" => {
                if let Some(key) = KeyCode::from_name(arg) {
                    self.set_key_state(key, false);
                }
            }
            "mouse_down" => {
                if let Some(button) = MouseButton::from_name(arg) {
                    self.set_mouse_button_state(button, true);
                }
            }
            "mouse_up" => {
                if let Some(button) = MouseButton::from_name(arg) {
                    self.set_mouse_button_state(button, false);
                }
            }
            "text" => self.push_text_input(arg),
            "signal" | "emit_signal" if !arg.trim().is_empty() => {
                self.signal_emit(perro_ids::SignalID::from_string(arg), &[]);
            }
            _ => {}
        }
    }

    /// Start a renderer-owned capture session.
    pub fn capture_start(
        &mut self,
        config: CaptureConfig,
        source_size: OutputSize,
        staging_root: &str,
    ) -> Result<(), String> {
        if self.capture_session.is_some() || self.capture_finish.is_some() {
            return Err("capture session already active".to_owned());
        }
        let typed_camera = matches!(
            &config.source,
            CaptureSource::Camera2DNode(_) | CaptureSource::Camera3DNode(_)
        );
        let mut route = self.capture_resolve_source(&config, source_size)?;
        if typed_camera {
            // Typed sources own a fresh stream so SS, alpha, + framing never
            // inherit limits from an authored camera stream.
            route.render_node = None;
        }
        if route.kind == CaptureSourceKind::RenderTarget && config.framing == Framing::Expand {
            return Err(
                "capture framing expand needs a camera or UI sub-view source; render target pixels cannot reveal extra view"
                    .to_owned(),
            );
        }
        let one_shot = if matches!(
            route.kind,
            CaptureSourceKind::Camera2D | CaptureSourceKind::Camera3D
        ) && route.render_node.is_none()
        {
            let camera = route
                .node
                .ok_or_else(|| "capture camera route has no node".to_owned())?;
            let render_size = config
                .render_size(route.source_size)
                .map_err(|error| error.to_string())?;
            let stream_size = capture_stream_size(route.source_size, render_size, config.framing)?;
            let output_texture = self.resource_api.camera_capture_texture(camera);
            let Some(mut state) = self.camera_capture_state(
                camera,
                output_texture,
                [stream_size.width, stream_size.height],
            ) else {
                self.resource_api.release_camera_capture_texture(camera);
                return Err(format!(
                    "capture camera {camera} cannot create render stream"
                ));
            };
            state.transparent_background = config.transparent;
            apply_capture_framing(&mut state, route.source_size, render_size, config.framing);
            route.render_node = Some(camera);
            Some((camera, Arc::new(state)))
        } else {
            None
        };
        let session = match CaptureSession::start(config, route.source_size, staging_root) {
            Ok(session) => session,
            Err(error) => {
                if let Some((camera, _)) = one_shot {
                    self.resource_api.release_camera_capture_texture(camera);
                }
                return Err(error.to_string());
            }
        };
        if let Some((camera, state)) = one_shot {
            self.render
                .queue_command(RenderCommand::CameraStream(CameraStreamCommand::Upsert {
                    node: camera,
                    state,
                }));
            self.capture_owned_render_node = Some(camera);
        } else {
            self.capture_owned_render_node = None;
        }
        self.capture_last_output = None;
        self.capture_source_route = Some(route);
        self.capture_stop_requested = false;
        self.capture_stop_output = None;
        self.capture_replay_cursor = 0;
        self.capture_replay_time = Duration::ZERO;
        self.capture_session = Some(session);
        Ok(())
    }

    /// Return active capture state, if a session exists.
    pub fn capture_state(&self) -> Option<CaptureSessionState> {
        self.capture_session
            .as_ref()
            .map(CaptureSession::state)
            .or_else(|| {
                self.capture_finish
                    .as_ref()
                    .map(|_| CaptureSessionState::Draining)
            })
    }

    /// Return active queue progress, if a session exists.
    pub fn capture_progress(&self) -> Option<CaptureProgress> {
        self.capture_session.as_ref().map(CaptureSession::progress)
    }

    /// Borrow active config for render bridge setup.
    pub fn capture_config(&self) -> Option<&CaptureConfig> {
        self.capture_session.as_ref().map(CaptureSession::config)
    }

    /// Resolve active source route for render bridge setup.
    pub fn capture_source(&self) -> Option<&CaptureSource> {
        self.capture_session.as_ref().map(CaptureSession::source)
    }

    /// Resolve and validate active capture source against live scene nodes.
    pub fn capture_source_route(&self) -> Option<CaptureSourceRoute> {
        self.capture_source_route
    }

    /// Resolve a capture config source before session setup.
    pub fn capture_resolve_source(
        &self,
        config: &CaptureConfig,
        fallback_size: OutputSize,
    ) -> Result<CaptureSourceRoute, String> {
        config
            .source
            .validate()
            .map_err(|error| error.to_string())?;
        let route = match &config.source {
            CaptureSource::MainWindow | CaptureSource::Window => CaptureSourceRoute {
                kind: CaptureSourceKind::MainWindow,
                node: None,
                render_node: None,
                node_type: None,
                source_size: fallback_size,
                framing: config.framing,
            },
            CaptureSource::Camera2D(name) => self.resolve_capture_node(
                name,
                CaptureSourceKind::Camera2D,
                &[NodeType::Camera2D],
                fallback_size,
                config.framing,
            )?,
            CaptureSource::Camera2DNode(node) => self.resolve_capture_node_id(
                NodeID::from_u64(*node),
                CaptureSourceKind::Camera2D,
                &[NodeType::Camera2D],
                fallback_size,
                config.framing,
            )?,
            CaptureSource::Camera3D(name) => self.resolve_capture_node(
                name,
                CaptureSourceKind::Camera3D,
                &[NodeType::Camera3D],
                fallback_size,
                config.framing,
            )?,
            CaptureSource::Camera3DNode(node) => self.resolve_capture_node_id(
                NodeID::from_u64(*node),
                CaptureSourceKind::Camera3D,
                &[NodeType::Camera3D],
                fallback_size,
                config.framing,
            )?,
            CaptureSource::UISubView(name) => self.resolve_capture_node(
                name,
                CaptureSourceKind::UISubView,
                &[NodeType::UiSubView],
                fallback_size,
                config.framing,
            )?,
            CaptureSource::RenderTarget(name) => self.resolve_capture_node(
                name,
                CaptureSourceKind::RenderTarget,
                &[
                    NodeType::SubView2D,
                    NodeType::SubView3D,
                    NodeType::UiSubView,
                    NodeType::CameraStream2D,
                    NodeType::CameraStream3D,
                    NodeType::UiCameraStream,
                ],
                fallback_size,
                config.framing,
            )?,
            CaptureSource::Named(name) => self.resolve_capture_node(
                name,
                CaptureSourceKind::Named,
                NodeType::ALL,
                fallback_size,
                config.framing,
            )?,
        };
        Ok(route)
    }

    fn resolve_capture_node(
        &self,
        name: &str,
        kind: CaptureSourceKind,
        accepted: &[NodeType],
        fallback_size: OutputSize,
        framing: perro_capture::Framing,
    ) -> Result<CaptureSourceRoute, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("capture source name is empty".to_owned());
        }
        let parsed = name.parse::<NodeID>().ok();
        let candidates = parsed.into_iter().chain(self.nodes.named_ids(name));
        let mut found_type = None;
        for node_id in candidates {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            let node_type = node.node_type();
            found_type.get_or_insert(node_type);
            if !accepted.contains(&node_type) {
                continue;
            }
            return Ok(CaptureSourceRoute {
                kind,
                node: Some(node_id),
                render_node: self.capture_render_node(node_id, node_type),
                node_type: Some(node_type),
                source_size: capture_node_size(&node.data, fallback_size),
                framing,
            });
        }
        if let Some(node_type) = found_type {
            return Err(format!(
                "capture source `{name}` has type {node_type}, expected {:?}",
                accepted
            ));
        }
        Err(format!("capture source `{name}` not found"))
    }

    fn resolve_capture_node_id(
        &self,
        node_id: NodeID,
        kind: CaptureSourceKind,
        accepted: &[NodeType],
        fallback_size: OutputSize,
        framing: perro_capture::Framing,
    ) -> Result<CaptureSourceRoute, String> {
        let Some(node) = self.nodes.get(node_id) else {
            return Err(format!("capture source node {node_id} not found"));
        };
        let node_type = node.node_type();
        if !accepted.contains(&node_type) {
            return Err(format!(
                "capture source node {node_id} has type {node_type}, expected {:?}",
                accepted
            ));
        }
        Ok(CaptureSourceRoute {
            kind,
            node: Some(node_id),
            render_node: self.capture_render_node(node_id, node_type),
            node_type: Some(node_type),
            source_size: capture_node_size(&node.data, fallback_size),
            framing,
        })
    }

    fn capture_render_node(&self, node_id: NodeID, node_type: NodeType) -> Option<NodeID> {
        match node_type {
            NodeType::Camera2D | NodeType::Camera3D => self.nodes.iter().find_map(|(id, node)| {
                let matches = match (&node.data, node_type) {
                    (SceneNodeData::CameraStream2D(stream), NodeType::Camera2D) => {
                        stream.stream.camera == node_id
                    }
                    (SceneNodeData::CameraStream3D(stream), NodeType::Camera3D) => {
                        stream.stream.camera == node_id
                    }
                    (SceneNodeData::UiCameraStream(stream), NodeType::Camera2D)
                    | (SceneNodeData::UiCameraStream(stream), NodeType::Camera3D) => {
                        stream.stream.camera == node_id
                    }
                    _ => false,
                };
                matches.then_some(id)
            }),
            NodeType::SubView2D
            | NodeType::SubView3D
            | NodeType::UiSubView
            | NodeType::CameraStream2D
            | NodeType::CameraStream3D
            | NodeType::UiCameraStream => Some(node_id),
            _ => None,
        }
    }

    /// Resolve renderer dimensions before final downsample.
    pub fn capture_render_size(&self) -> Option<OutputSize> {
        self.capture_session
            .as_ref()
            .map(CaptureSession::render_size)
    }

    /// Resolve final output dimensions.
    pub fn capture_output_size(&self) -> Option<OutputSize> {
        self.capture_session
            .as_ref()
            .map(CaptureSession::output_size)
    }

    /// Refresh one-shot camera extraction after scene transforms update.
    pub(crate) fn refresh_capture_source_stream(&mut self) {
        let Some(render_size) = self.capture_render_size() else {
            return;
        };
        let capture_transparent = self
            .capture_config()
            .is_some_and(|config| config.transparent);
        let capture_framing = self
            .capture_config()
            .map(|config| config.framing)
            .unwrap_or(Framing::Fit);
        if let Some(camera) = self.capture_owned_render_node {
            let output_texture = self.resource_api.camera_capture_texture(camera);
            let source_size = self
                .capture_source_route
                .map(|route| route.source_size)
                .unwrap_or(render_size);
            let stream_size = match capture_stream_size(source_size, render_size, capture_framing) {
                Ok(size) => size,
                Err(_) => {
                    self.capture_stop_requested = true;
                    return;
                }
            };
            let Some(mut state) = self.camera_capture_state(
                camera,
                output_texture,
                [stream_size.width, stream_size.height],
            ) else {
                self.capture_stop_requested = true;
                return;
            };
            state.transparent_background = capture_transparent;
            apply_capture_framing(&mut state, source_size, render_size, capture_framing);
            self.render
                .queue_command(RenderCommand::CameraStream(CameraStreamCommand::Upsert {
                    node: camera,
                    state: Arc::new(state),
                }));
            return;
        }
        let Some(route) = self.capture_source_route else {
            return;
        };
        if route.kind != CaptureSourceKind::UISubView {
            return;
        }
        let Some(node) = route.node else {
            return;
        };
        let source_size = route.source_size;
        let stream_size = match capture_stream_size(source_size, render_size, capture_framing) {
            Ok(size) => size,
            Err(_) => {
                self.capture_stop_requested = true;
                return;
            }
        };
        let Some(SceneNodeData::UiSubView(view)) = self.nodes.get(node).map(|node| &node.data)
        else {
            self.capture_stop_requested = true;
            return;
        };
        let mut view = SubView::from(view.as_ref());
        view.resolution.x = stream_size.width;
        view.resolution.y = stream_size.height;
        let Some(mut state) = self.sub_view_state(node, &view, None) else {
            self.capture_stop_requested = true;
            return;
        };
        state.transparent_background = capture_transparent;
        apply_capture_framing(&mut state, source_size, render_size, capture_framing);
        self.render
            .queue_command(RenderCommand::CameraStream(CameraStreamCommand::Upsert {
                node,
                state: Arc::new(state),
            }));
    }

    /// Submit a rendered RGBA frame to the bounded PNG queue.
    pub fn capture_submit_rgba(
        &mut self,
        frame_index: u64,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<u64, String> {
        let session = self
            .capture_session
            .as_mut()
            .ok_or_else(|| "capture session not active".to_owned())?;
        let submitted = session.progress().submitted;
        if frame_index != submitted {
            return Err(format!(
                "capture frame index mismatch: expected {submitted}, got {frame_index}"
            ));
        }
        session
            .submit_rgba(width, height, rgba)
            .map_err(|error| error.to_string())
    }

    /// Wait for all queued PNG frames to finish.
    pub fn capture_drain(&self) -> Result<(), String> {
        self.capture_session
            .as_ref()
            .ok_or_else(|| "capture session not active".to_owned())?
            .drain()
            .map_err(|error| error.to_string())
    }

    /// Number of fully encoded PNG frames.
    pub fn capture_completed_frames(&self) -> u64 {
        self.capture_session
            .as_ref()
            .map(CaptureSession::completed_frames)
            .unwrap_or(0)
    }

    /// Record realtime output slots filled from a prior rendered frame.
    pub fn capture_note_realtime_duplicates(&mut self, count: u64) -> Result<(), String> {
        self.capture_session
            .as_mut()
            .ok_or_else(|| "capture session not active".to_owned())?
            .note_realtime_duplicates(count)
            .map_err(|error| error.to_string())
    }

    /// Request stop at the next safe app boundary.
    pub fn capture_request_stop(&mut self, output: perro_capture::OutputSpec) {
        self.capture_stop_requested = true;
        self.capture_stop_output = Some(output);
    }

    /// Read and clear a pending stop request.
    pub fn capture_take_stop_request(&mut self) -> bool {
        std::mem::take(&mut self.capture_stop_requested)
    }

    /// Take output spec attached to a pending stop request.
    pub fn capture_take_stop_output(&mut self) -> Option<OutputSpec> {
        self.capture_stop_output.take()
    }

    /// Record a replay/director action in the active session.
    pub fn capture_record_action(
        &mut self,
        timestamp: Duration,
        action: String,
        payload: Option<String>,
    ) -> Result<(), String> {
        self.capture_session
            .as_mut()
            .ok_or_else(|| "capture session not active".to_owned())?
            .record_action(timestamp, action, payload)
            .map_err(|error| error.to_string())
    }

    /// Stop capture, drain workers, and commit output.
    pub fn capture_stop(&mut self, output: OutputSpec) -> Result<FinalizedCapture, String> {
        let session = self.capture_take_session()?;
        let result = session.stop_to(output).map_err(|error| error.to_string())?;
        self.capture_last_output = Some(result.clone());
        Ok(result)
    }

    /// Drain and pack off the app thread; poll until commit completes.
    pub fn capture_stop_async(&mut self, output: OutputSpec) -> Result<(), String> {
        let session = self.capture_take_session()?;
        let worker = std::thread::Builder::new()
            .name("perro-capture-pack".to_owned())
            .spawn(move || session.stop_to(output).map_err(|error| error.to_string()))
            .map_err(|error| error.to_string())?;
        self.capture_finish = Some(CaptureFinish {
            worker: Some(worker),
        });
        Ok(())
    }

    /// Return None while background packing remains active.
    pub fn capture_poll_finish(&mut self) -> Result<Option<FinalizedCapture>, String> {
        let Some(finish) = self.capture_finish.as_ref() else {
            return Ok(None);
        };
        if !finish
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            return Ok(None);
        }
        let mut finish = self.capture_finish.take().expect("capture finish present");
        let result = finish
            .worker
            .take()
            .expect("capture worker present")
            .join()
            .map_err(|_| "capture pack worker panicked".to_owned())??;
        self.capture_last_output = Some(result.clone());
        Ok(Some(result))
    }

    /// Release a failed recording and its temporary frames.
    pub fn capture_cancel(&mut self) {
        if self.capture_session.is_some() {
            let _ = self.capture_take_session();
        }
    }

    fn capture_take_session(&mut self) -> Result<CaptureSession, String> {
        let session = self
            .capture_session
            .take()
            .ok_or_else(|| "capture session not active".to_owned())?;
        let restore_route = self.capture_source_route;
        self.capture_source_route = None;
        let owned_render_node = self.capture_owned_render_node.take();
        if let Some(route) = restore_route {
            self.restore_capture_source_stream(route);
        }
        if let Some(node) = owned_render_node {
            self.render.queue_command(RenderCommand::CameraStream(
                CameraStreamCommand::RemoveNode { node },
            ));
            self.resource_api.release_camera_capture_texture(node);
        }
        self.capture_stop_requested = false;
        self.capture_stop_output = None;
        Ok(session)
    }

    fn restore_capture_source_stream(&mut self, route: CaptureSourceRoute) {
        if route.kind != CaptureSourceKind::UISubView {
            return;
        }
        let Some(node) = route.node else {
            return;
        };
        let Some(view) = self.nodes.get(node).and_then(|scene_node| {
            let SceneNodeData::UiSubView(view) = &scene_node.data else {
                return None;
            };
            Some(SubView::from(view.as_ref()))
        }) else {
            return;
        };
        let Some(state) = self.sub_view_state(node, &view, None) else {
            return;
        };
        self.render
            .queue_command(RenderCommand::CameraStream(CameraStreamCommand::Upsert {
                node,
                state: Arc::new(state),
            }));
    }

    /// Return last committed output metadata.
    pub fn capture_last_output(&self) -> Option<&FinalizedCapture> {
        self.capture_last_output.as_ref()
    }
}

impl CaptureAPI for Runtime {
    fn capture_start_config(&mut self, config: CaptureConfig) -> Result<(), String> {
        let project = self
            .project()
            .ok_or_else(|| "capture requires a loaded project".to_owned())?;
        let source_size = OutputSize::new(
            project.config.virtual_width.max(1),
            project.config.virtual_height.max(1),
        )
        .map_err(|error| error.to_string())?;
        let staging_root = config
            .output
            .as_ref()
            .and_then(|output| output.path.parent())
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(&project.root)
            .join(".perro-capture");
        let staging_root = staging_root.to_string_lossy().into_owned();
        Runtime::capture_start(self, config, source_size, &staging_root)
    }

    fn capture_start(
        &mut self,
        config: CaptureConfig,
        source_size: OutputSize,
        staging_root: &str,
    ) -> Result<(), String> {
        Runtime::capture_start(self, config, source_size, staging_root)
    }

    fn capture_state(&self) -> Option<CaptureSessionState> {
        Runtime::capture_state(self)
    }

    fn capture_progress(&self) -> Option<CaptureProgress> {
        Runtime::capture_progress(self)
    }

    fn capture_last_output(&self) -> Option<FinalizedCapture> {
        Runtime::capture_last_output(self).cloned()
    }

    fn capture_source(&self) -> Option<CaptureSource> {
        Runtime::capture_source(self).cloned()
    }

    fn capture_source_route(&self) -> Option<CaptureSourceRoute> {
        Runtime::capture_source_route(self)
    }

    fn capture_render_size(&self) -> Option<OutputSize> {
        Runtime::capture_render_size(self)
    }

    fn capture_submit_rgba(
        &mut self,
        frame_index: u64,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<u64, String> {
        Runtime::capture_submit_rgba(self, frame_index, width, height, rgba)
    }

    fn capture_drain(&self) -> Result<(), String> {
        Runtime::capture_drain(self)
    }

    fn capture_completed_frames(&self) -> u64 {
        Runtime::capture_completed_frames(self)
    }

    fn capture_request_stop(&mut self, output: OutputSpec) {
        Runtime::capture_request_stop(self, output)
    }

    fn capture_request_configured_stop(&mut self) -> Result<(), String> {
        let output = self
            .capture_config()
            .and_then(|config| config.output.clone())
            .ok_or_else(|| "capture config output missing".to_owned())?;
        Runtime::capture_request_stop(self, output);
        Ok(())
    }

    fn capture_take_stop_request(&mut self) -> bool {
        Runtime::capture_take_stop_request(self)
    }

    fn capture_take_stop_output(&mut self) -> Option<OutputSpec> {
        Runtime::capture_take_stop_output(self)
    }

    fn capture_record_action(
        &mut self,
        timestamp: Duration,
        action: String,
        payload: Option<String>,
    ) -> Result<(), String> {
        Runtime::capture_record_action(self, timestamp, action, payload)
    }

    fn capture_stop(&mut self, output: OutputSpec) -> Result<FinalizedCapture, String> {
        Runtime::capture_stop(self, output)
    }
}

fn apply_capture_framing(
    state: &mut CameraStreamState,
    source: OutputSize,
    target: OutputSize,
    framing: Framing,
) {
    if framing != Framing::Expand {
        return;
    }
    let source_aspect = aspect_ratio(source);
    let target_aspect = aspect_ratio(target);
    if !(source_aspect.is_finite() && target_aspect.is_finite()) {
        return;
    }
    if let CameraStreamSourceState::ThreeD(camera) = &mut state.source {
        camera.projection = expand_projection(camera.projection, source_aspect, target_aspect);
    }
}

fn capture_stream_size(
    source: OutputSize,
    target: OutputSize,
    framing: Framing,
) -> Result<OutputSize, String> {
    if framing == Framing::Expand {
        return Ok(target);
    }
    let source_ratio = u64::from(source.width) * u64::from(target.height);
    let target_ratio = u64::from(target.width) * u64::from(source.height);
    let cover = framing == Framing::Crop;
    let (width, height) = if source_ratio > target_ratio {
        if cover {
            (
                scale_dimension(target.height, source.width, source.height, true)?,
                target.height,
            )
        } else {
            (
                target.width,
                scale_dimension(target.width, source.height, source.width, false)?,
            )
        }
    } else if cover {
        (
            target.width,
            scale_dimension(target.width, source.height, source.width, true)?,
        )
    } else {
        (
            scale_dimension(target.height, source.width, source.height, false)?,
            target.height,
        )
    };
    OutputSize::new(width.max(1), height.max(1)).map_err(|error| error.to_string())
}

fn scale_dimension(
    value: u32,
    numerator: u32,
    denominator: u32,
    round_up: bool,
) -> Result<u32, String> {
    let scaled = u64::from(value)
        .checked_mul(u64::from(numerator))
        .ok_or_else(|| "capture framing dimension overflow".to_owned())?;
    let denominator = u64::from(denominator.max(1));
    let rounded = if round_up {
        scaled
            .checked_add(denominator - 1)
            .ok_or_else(|| "capture framing dimension overflow".to_owned())?
            / denominator
    } else {
        scaled / denominator
    };
    u32::try_from(rounded.max(1)).map_err(|_| "capture framing dimension overflow".to_owned())
}

fn aspect_ratio(size: OutputSize) -> f32 {
    size.width.max(1) as f32 / size.height.max(1) as f32
}

fn expand_projection(
    projection: CameraProjectionState,
    source_aspect: f32,
    target_aspect: f32,
) -> CameraProjectionState {
    let source_aspect = source_aspect.max(f32::EPSILON);
    let target_aspect = target_aspect.max(f32::EPSILON);
    let vertical_factor = (source_aspect / target_aspect).max(1.0);
    let horizontal_factor = (target_aspect / source_aspect).max(1.0);
    match projection {
        CameraProjectionState::Perspective {
            fov_y_degrees,
            near,
            far,
        } => {
            let fov = if fov_y_degrees.is_finite() {
                fov_y_degrees
                    .to_radians()
                    .clamp(10.0_f32.to_radians(), 120.0_f32.to_radians())
            } else {
                60.0_f32.to_radians()
            };
            let expanded = (fov * 0.5).tan() * vertical_factor;
            CameraProjectionState::Perspective {
                fov_y_degrees: (expanded.atan() * 2.0).to_degrees().clamp(10.0, 120.0),
                near,
                far,
            }
        }
        CameraProjectionState::Orthographic { size, near, far } => {
            let size = if size.is_finite() { size } else { 10.0 };
            CameraProjectionState::Orthographic {
                size: size * vertical_factor,
                near,
                far,
            }
        }
        CameraProjectionState::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => CameraProjectionState::Frustum {
            left: scale_frustum_axis(left, right, horizontal_factor).0,
            right: scale_frustum_axis(left, right, horizontal_factor).1,
            bottom: scale_frustum_axis(bottom, top, vertical_factor).0,
            top: scale_frustum_axis(bottom, top, vertical_factor).1,
            near,
            far,
        },
    }
}

fn scale_frustum_axis(min: f32, max: f32, factor: f32) -> (f32, f32) {
    let center = (min + max) * 0.5;
    let half = (max - min).abs() * factor * 0.5;
    (center - half, center + half)
}

fn capture_node_size(data: &SceneNodeData, fallback: OutputSize) -> OutputSize {
    let (width, height) = match data {
        SceneNodeData::SubView2D(view) => (view.sub_view.resolution.x, view.sub_view.resolution.y),
        SceneNodeData::SubView3D(view) => (view.sub_view.resolution.x, view.sub_view.resolution.y),
        SceneNodeData::UiSubView(view) => (view.resolution.x, view.resolution.y),
        SceneNodeData::CameraStream2D(stream) => {
            (stream.stream.resolution.x, stream.stream.resolution.y)
        }
        SceneNodeData::CameraStream3D(stream) => {
            (stream.stream.resolution.x, stream.stream.resolution.y)
        }
        SceneNodeData::UiCameraStream(stream) => {
            (stream.stream.resolution.x, stream.stream.resolution.y)
        }
        _ => return fallback,
    };
    if width == 0 || height == 0 {
        fallback
    } else {
        OutputSize { width, height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_nodes::{Camera2D, Camera3D, CameraStream2D, UiSubView};
    use perro_runtime_api::sub_apis::{CaptureSourceNodeExt, NodeAPI};
    use perro_structs::Color;
    use std::borrow::Cow;
    use std::fs;

    #[test]
    fn async_finish_polls_without_waiting_and_clears_failed_worker() {
        let mut runtime = Runtime::new();
        let (release, wait) = std::sync::mpsc::channel();
        runtime.capture_finish = Some(CaptureFinish {
            worker: Some(std::thread::spawn(move || {
                wait.recv().unwrap();
                Err("pack failed".to_owned())
            })),
        });
        assert_eq!(runtime.capture_state(), Some(CaptureSessionState::Draining));
        assert_eq!(runtime.capture_poll_finish().unwrap(), None);
        assert!(
            runtime
                .capture_start(
                    CaptureConfig::default(),
                    OutputSize {
                        width: 2,
                        height: 2
                    },
                    "."
                )
                .is_err()
        );
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            match runtime.capture_poll_finish() {
                Err(error) => {
                    assert_eq!(error, "pack failed");
                    break;
                }
                Ok(None) => {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                }
                Ok(Some(_)) => panic!("unexpected output"),
            }
        }
        assert_eq!(runtime.capture_state(), None);
        assert!(runtime.capture_last_output().is_none());
    }

    #[test]
    fn async_stop_commits_output_and_allows_restart_then_cancel() {
        let root = std::env::temp_dir().join(format!(
            "perro-runtime-async-capture-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let mut runtime = Runtime::new();
        let config = CaptureConfig {
            supersample: 1,
            ..CaptureConfig::default()
        };
        let size = OutputSize {
            width: 2,
            height: 2,
        };
        runtime
            .capture_start(config.clone(), size, root.to_str().unwrap())
            .unwrap();
        runtime.capture_submit_rgba(0, 2, 2, &[255; 16]).unwrap();
        let stage = runtime
            .capture_session
            .as_ref()
            .unwrap()
            .staging_dir()
            .to_owned();
        let output = root.join("clip.gif");
        runtime
            .capture_stop_async(OutputSpec::new(&output, perro_capture::OutputFormat::Gif))
            .unwrap();
        assert_eq!(runtime.capture_state(), Some(CaptureSessionState::Draining));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Some(result) = runtime.capture_poll_finish().unwrap() {
                break result;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(result.frame_count, 1);
        assert_eq!(runtime.capture_last_output(), Some(&result));
        assert!(output.is_file());
        assert!(!stage.exists());
        runtime
            .capture_start(config, size, root.to_str().unwrap())
            .unwrap();
        let stage = runtime
            .capture_session
            .as_ref()
            .unwrap()
            .staging_dir()
            .to_owned();
        runtime.capture_cancel();
        assert_eq!(runtime.capture_state(), None);
        assert!(!stage.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn host_exposes_start_state_stop_and_metadata() {
        let root =
            std::env::temp_dir().join(format!("perro-runtime-capture-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap_or_else(|error| panic!("capture root: {error}"));
        let mut runtime = Runtime::new();
        let config = CaptureConfig {
            width: Some(2),
            height: Some(2),
            duration: None,
            ..CaptureConfig::default()
        };
        Runtime::capture_start(
            &mut runtime,
            config,
            OutputSize {
                width: 2,
                height: 2,
            },
            root.to_str().unwrap_or("."),
        )
        .unwrap_or_else(|error| panic!("capture start: {error}"));
        assert_eq!(
            runtime.capture_state(),
            Some(CaptureSessionState::Recording)
        );
        let output = root.join("frames");
        let result = Runtime::capture_stop(
            &mut runtime,
            OutputSpec::new(&output, perro_capture::OutputFormat::PngSequence),
        )
        .unwrap_or_else(|error| panic!("capture stop: {error}"));
        assert_eq!(result.frame_count, 0);
        assert!(result.metadata_path.is_file());
        assert_eq!(runtime.capture_last_output(), Some(&result));
        assert_eq!(runtime.capture_state(), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stop_request_keeps_output_until_app_boundary_and_allows_restart() {
        let root =
            std::env::temp_dir().join(format!("perro-runtime-capture-stop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap_or_else(|error| panic!("capture root: {error}"));
        let mut runtime = Runtime::new();
        let config = CaptureConfig {
            duration: None,
            ..CaptureConfig::default()
        };
        runtime
            .capture_start(
                config.clone(),
                OutputSize {
                    width: 2,
                    height: 2,
                },
                root.to_str().unwrap_or("."),
            )
            .unwrap_or_else(|error| panic!("capture start: {error}"));
        let output = OutputSpec::new(root.join("first"), perro_capture::OutputFormat::PngSequence);
        runtime.capture_request_stop(output.clone());
        assert!(runtime.capture_take_stop_request());
        assert_eq!(runtime.capture_take_stop_output(), Some(output.clone()));
        assert!(!runtime.capture_take_stop_request());
        runtime
            .capture_stop(output)
            .unwrap_or_else(|error| panic!("capture stop: {error}"));
        runtime
            .capture_start(
                config,
                OutputSize {
                    width: 2,
                    height: 2,
                },
                root.to_str().unwrap_or("."),
            )
            .unwrap_or_else(|error| panic!("capture restart: {error}"));
        assert_eq!(
            runtime.capture_state(),
            Some(CaptureSessionState::Recording)
        );
        runtime
            .capture_stop(OutputSpec::new(
                root.join("second"),
                perro_capture::OutputFormat::PngSequence,
            ))
            .unwrap_or_else(|error| panic!("capture restart stop: {error}"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn replay_input_action_applies_before_fixed_script_tick() {
        let root =
            std::env::temp_dir().join(format!("perro-runtime-replay-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap_or_else(|error| panic!("capture root: {error}"));
        let mut runtime = Runtime::new();
        Runtime::capture_start(
            &mut runtime,
            CaptureConfig {
                duration: None,
                ..CaptureConfig::default()
            },
            OutputSize {
                width: 2,
                height: 2,
            },
            root.to_str().unwrap_or("."),
        )
        .unwrap_or_else(|error| panic!("capture start: {error}"));
        Runtime::capture_record_action(
            &mut runtime,
            Duration::ZERO,
            "input:key_down".to_owned(),
            Some("KeyW".to_owned()),
        )
        .unwrap_or_else(|error| panic!("record action: {error}"));
        runtime.fixed_update(1.0 / 60.0);
        assert!(runtime.input.is_key_down(KeyCode::KeyW));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn source_route_resolves_named_camera_and_rejects_wrong_type() {
        let mut runtime = Runtime::new();
        let camera = NodeAPI::create::<Camera2D>(&mut runtime);
        runtime.nodes.rename(camera, Cow::Borrowed("hero-camera"));
        let stream = NodeAPI::create::<CameraStream2D>(&mut runtime);
        NodeAPI::with_node_mut::<CameraStream2D, _, _>(&mut runtime, stream, |stream| {
            stream.stream.camera = camera;
        })
        .expect("camera stream");
        let config = CaptureConfig {
            source: CaptureSource::camera_2d("hero-camera"),
            framing: perro_capture::Framing::Crop,
            ..CaptureConfig::default()
        };
        let route = runtime
            .capture_resolve_source(
                &config,
                OutputSize {
                    width: 320,
                    height: 180,
                },
            )
            .unwrap_or_else(|error| panic!("resolve camera: {error}"));
        assert_eq!(route.node, Some(camera));
        assert_eq!(route.render_node, Some(stream));
        assert_eq!(route.node_type, Some(perro_nodes::NodeType::Camera2D));
        assert_eq!(route.source_size.width, 320);
        assert_eq!(route.framing, perro_capture::Framing::Crop);

        let wrong = CaptureConfig {
            source: CaptureSource::camera_3d("hero-camera"),
            ..CaptureConfig::default()
        };
        let error = runtime
            .capture_resolve_source(
                &wrong,
                OutputSize {
                    width: 320,
                    height: 180,
                },
            )
            .expect_err("wrong source type must fail");
        assert!(error.contains("expected"));
    }

    #[test]
    fn source_route_resolves_exact_typed_camera_ids_only() {
        let mut runtime = Runtime::new();
        let camera_2d = NodeAPI::create::<Camera2D>(&mut runtime);
        let camera_3d = NodeAPI::create::<Camera3D>(&mut runtime);
        let size = OutputSize {
            width: 320,
            height: 180,
        };

        let route_2d = runtime
            .capture_resolve_source(
                &CaptureConfig {
                    source: CaptureSource::camera_2d_node(camera_2d),
                    ..CaptureConfig::default()
                },
                size,
            )
            .unwrap_or_else(|error| panic!("resolve typed 2D camera: {error}"));
        assert_eq!(route_2d.node, Some(camera_2d));
        assert_eq!(route_2d.kind, CaptureSourceKind::Camera2D);

        let route_3d = runtime
            .capture_resolve_source(
                &CaptureConfig {
                    source: CaptureSource::camera_3d_node(camera_3d),
                    ..CaptureConfig::default()
                },
                size,
            )
            .unwrap_or_else(|error| panic!("resolve typed 3D camera: {error}"));
        assert_eq!(route_3d.node, Some(camera_3d));
        assert_eq!(route_3d.kind, CaptureSourceKind::Camera3D);

        let wrong_type = runtime
            .capture_resolve_source(
                &CaptureConfig {
                    source: CaptureSource::camera_3d_node(camera_2d),
                    ..CaptureConfig::default()
                },
                size,
            )
            .expect_err("typed wrong node kind must fail");
        assert!(wrong_type.contains("expected"));

        let missing = runtime
            .capture_resolve_source(
                &CaptureConfig {
                    source: CaptureSource::camera_2d_node(NodeID::from_parts(999_999, 3)),
                    ..CaptureConfig::default()
                },
                size,
            )
            .expect_err("typed stale node must fail");
        assert!(missing.contains("not found"));
    }

    #[test]
    fn typed_camera_owns_capture_stream_and_keeps_authored_stream() {
        let root = std::env::temp_dir().join(format!(
            "perro-runtime-typed-camera-stream-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap_or_else(|error| panic!("capture root: {error}"));
        let mut runtime = Runtime::new();
        let camera = NodeAPI::create::<Camera2D>(&mut runtime);
        let authored_stream = NodeAPI::create::<CameraStream2D>(&mut runtime);
        NodeAPI::with_node_mut::<CameraStream2D, _, _>(&mut runtime, authored_stream, |stream| {
            stream.stream.camera = camera
        })
        .expect("authored camera stream");
        runtime
            .capture_start(
                CaptureConfig {
                    source: CaptureSource::camera_2d_node(camera),
                    width: Some(2),
                    height: Some(2),
                    duration: None,
                    ..CaptureConfig::default()
                },
                OutputSize {
                    width: 2,
                    height: 2,
                },
                root.to_str().unwrap_or("."),
            )
            .unwrap_or_else(|error| panic!("typed capture start: {error}"));
        assert_eq!(
            runtime
                .capture_source_route()
                .and_then(|route| route.render_node),
            Some(camera)
        );
        assert_eq!(runtime.capture_owned_render_node, Some(camera));
        assert!(runtime.nodes.get(authored_stream).is_some());

        runtime
            .capture_stop(OutputSpec::new(
                root.join("typed-camera"),
                perro_capture::OutputFormat::PngSequence,
            ))
            .unwrap_or_else(|error| panic!("typed capture stop: {error}"));
        assert_eq!(runtime.capture_owned_render_node, None);
        assert!(runtime.nodes.get(authored_stream).is_some());
        let mut commands = Vec::new();
        runtime.render.drain_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::RemoveNode { node })
                if *node == camera
        )));
        assert!(!commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::RemoveNode { node })
                if *node == authored_stream
        )));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn raw_camera_route_queues_one_shot_stream_and_removes_it_on_stop() {
        let root =
            std::env::temp_dir().join(format!("perro-runtime-raw-camera-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap_or_else(|error| panic!("capture root: {error}"));
        let mut runtime = Runtime::new();
        let camera = NodeAPI::create::<Camera2D>(&mut runtime);
        runtime.nodes.rename(camera, Cow::Borrowed("raw-camera"));
        let config = CaptureConfig {
            source: CaptureSource::camera_2d("raw-camera"),
            transparent: true,
            duration: None,
            ..CaptureConfig::default()
        };
        runtime
            .capture_start(
                config,
                OutputSize {
                    width: 2,
                    height: 2,
                },
                root.to_str().unwrap_or("."),
            )
            .unwrap_or_else(|error| panic!("capture start: {error}"));
        let route = runtime.capture_source_route().expect("raw camera route");
        assert_eq!(route.node, Some(camera));
        assert_eq!(route.render_node, Some(camera));
        let mut commands = Vec::new();
        runtime.render.drain_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == camera && state.transparent_background
        )));

        NodeAPI::with_node_mut::<Camera2D, _, _>(&mut runtime, camera, |camera| {
            camera.transform.position.x = 7.0;
            camera.zoom = 2.0;
        })
        .expect("camera update");
        runtime.refresh_capture_source_stream();
        commands.clear();
        runtime.render.drain_commands(&mut commands);
        let refreshed = commands.iter().find_map(|command| match command {
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == camera =>
            {
                match &state.source {
                    perro_render_bridge::CameraStreamSourceState::TwoD(camera) => Some(camera),
                    _ => None,
                }
            }
            _ => None,
        });
        let refreshed = refreshed.expect("refreshed camera state");
        assert_eq!(refreshed.position[0], 7.0);
        assert_eq!(refreshed.zoom, 2.0);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == camera && state.transparent_background
        )));

        runtime
            .capture_stop(OutputSpec::new(
                root.join("frames"),
                perro_capture::OutputFormat::PngSequence,
            ))
            .unwrap_or_else(|error| panic!("capture stop: {error}"));
        assert_eq!(runtime.capture_owned_render_node, None);
        runtime.render.drain_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::RemoveNode { node }) if *node == camera
        )));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn capture_stream_size_keeps_source_aspect_per_framing() {
        let source = OutputSize {
            width: 16,
            height: 9,
        };
        let target = OutputSize {
            width: 100,
            height: 100,
        };
        assert_eq!(
            capture_stream_size(source, target, Framing::Fit).expect("fit size"),
            OutputSize {
                width: 100,
                height: 56
            }
        );
        assert_eq!(
            capture_stream_size(source, target, Framing::Stretch).expect("stretch size"),
            OutputSize {
                width: 100,
                height: 56
            }
        );
        assert_eq!(
            capture_stream_size(source, target, Framing::Crop).expect("crop size"),
            OutputSize {
                width: 178,
                height: 100
            }
        );
        assert_eq!(
            capture_stream_size(source, target, Framing::Expand).expect("expand size"),
            target
        );
    }

    #[test]
    fn ui_sub_view_capture_restores_authored_alpha_state() {
        let root =
            std::env::temp_dir().join(format!("perro-runtime-ui-capture-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap_or_else(|error| panic!("capture root: {error}"));
        let mut runtime = Runtime::new();
        let view = NodeAPI::create::<UiSubView>(&mut runtime);
        runtime.nodes.rename(view, Cow::Borrowed("capture-view"));
        NodeAPI::with_node_mut::<UiSubView, _, _>(&mut runtime, view, |view| {
            view.background = Color::WHITE;
        })
        .expect("ui sub-view");
        runtime
            .capture_start(
                CaptureConfig {
                    source: CaptureSource::ui_sub_view("capture-view"),
                    transparent: true,
                    duration: None,
                    ..CaptureConfig::default()
                },
                OutputSize {
                    width: 2,
                    height: 2,
                },
                root.to_str().unwrap_or("."),
            )
            .unwrap_or_else(|error| panic!("capture start: {error}"));
        runtime.refresh_capture_source_stream();
        let mut commands = Vec::new();
        runtime.render.drain_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == view && state.transparent_background
        )));

        runtime
            .capture_stop(OutputSpec::new(
                root.join("frames"),
                perro_capture::OutputFormat::PngSequence,
            ))
            .unwrap_or_else(|error| panic!("capture stop: {error}"));
        commands.clear();
        runtime.render.drain_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == view && !state.transparent_background
        )));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_projection_reveals_tall_axis_and_keeps_wide_vertical_view() {
        let projection = CameraProjectionState::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 100.0,
        };
        let wide = expand_projection(projection, 16.0 / 9.0, 32.0 / 9.0);
        let tall = expand_projection(projection, 16.0 / 9.0, 9.0 / 16.0);
        let CameraProjectionState::Perspective {
            fov_y_degrees: wide_fov,
            ..
        } = wide
        else {
            panic!("wide projection mode");
        };
        let CameraProjectionState::Perspective {
            fov_y_degrees: tall_fov,
            ..
        } = tall
        else {
            panic!("tall projection mode");
        };
        assert!((wide_fov - 60.0).abs() < 1.0e-5);
        assert!(tall_fov > wide_fov);
    }

    #[test]
    fn expand_projection_supports_ortho_frustum_and_equal_aspect() {
        let ortho = expand_projection(
            CameraProjectionState::Orthographic {
                size: 10.0,
                near: 0.1,
                far: 100.0,
            },
            16.0 / 9.0,
            9.0 / 16.0,
        );
        assert!(matches!(
            ortho,
            CameraProjectionState::Orthographic { size, .. } if (size - 31.6049).abs() < 0.01
        ));

        let frustum = expand_projection(
            CameraProjectionState::Frustum {
                left: -1.0,
                right: 1.0,
                bottom: -1.0,
                top: 1.0,
                near: 0.1,
                far: 100.0,
            },
            16.0 / 9.0,
            32.0 / 9.0,
        );
        assert!(matches!(
            frustum,
            CameraProjectionState::Frustum { left, right, bottom, top, .. }
                if (left + 2.0).abs() < 0.01
                    && (right - 2.0).abs() < 0.01
                    && (bottom + 1.0).abs() < 0.01
                    && (top - 1.0).abs() < 0.01
        ));

        let equal = expand_projection(
            CameraProjectionState::Orthographic {
                size: 10.0,
                near: 0.1,
                far: 100.0,
            },
            16.0 / 9.0,
            16.0 / 9.0,
        );
        assert_eq!(
            equal,
            CameraProjectionState::Orthographic {
                size: 10.0,
                near: 0.1,
                far: 100.0,
            }
        );
    }

    #[test]
    fn source_route_uses_stream_resolution_and_validates_before_start() {
        let root = std::env::temp_dir().join(format!("perro-runtime-route-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap_or_else(|error| panic!("capture root: {error}"));
        let mut runtime = Runtime::new();
        let stream = NodeAPI::create::<CameraStream2D>(&mut runtime);
        runtime.nodes.rename(stream, Cow::Borrowed("stream"));
        let expand_error = runtime
            .capture_start(
                CaptureConfig {
                    source: CaptureSource::render_target("stream"),
                    framing: Framing::Expand,
                    duration: None,
                    ..CaptureConfig::default()
                },
                OutputSize {
                    width: 2,
                    height: 2,
                },
                root.to_str().unwrap_or("."),
            )
            .expect_err("render target expand must fail preflight");
        assert!(expand_error.contains("render target pixels"));
        let config = CaptureConfig {
            source: CaptureSource::render_target("stream"),
            duration: None,
            ..CaptureConfig::default()
        };
        runtime
            .capture_start(
                config,
                OutputSize {
                    width: 2,
                    height: 2,
                },
                root.to_str().unwrap_or("."),
            )
            .unwrap_or_else(|error| panic!("capture start: {error}"));
        let route = runtime.capture_source_route().expect("route after start");
        assert_eq!(route.node, Some(stream));
        assert_eq!(
            route.source_size,
            OutputSize {
                width: 512,
                height: 512
            }
        );
        assert_eq!(
            runtime.capture_render_size(),
            Some(OutputSize {
                width: 1024,
                height: 1024
            })
        );
        let output = root.join("frames");
        runtime
            .capture_stop(OutputSpec::new(
                &output,
                perro_capture::OutputFormat::PngSequence,
            ))
            .unwrap_or_else(|error| panic!("capture stop: {error}"));
        let _ = fs::remove_dir_all(root);

        let mut missing = Runtime::new();
        let error = missing
            .capture_start(
                CaptureConfig {
                    source: CaptureSource::ui_sub_view("missing"),
                    ..CaptureConfig::default()
                },
                OutputSize {
                    width: 2,
                    height: 2,
                },
                ".",
            )
            .expect_err("missing source must fail before session");
        assert!(error.contains("not found"));
        assert_eq!(missing.capture_state(), None);
    }
}
