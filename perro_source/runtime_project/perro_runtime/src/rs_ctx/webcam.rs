use super::core::RuntimeResourceApi;
use super::core::WebcamErrorMessage;
#[cfg(all(
    any(target_os = "windows", target_os = "linux", target_os = "macos"),
    not(test)
))]
use super::core::WebcamFrameMessage;
use perro_ids::{TextureID, WebcamID, string_to_u64};
use perro_render_bridge::{RenderCommand, ResourceCommand};
use perro_resource_api::sub_apis::{WebcamAPI, WebcamConfig, WebcamFrame};
use std::sync::Arc;

fn clamp_size(width: u32, height: u32) -> (u32, u32) {
    (width.clamp(1, 8192), height.clamp(1, 8192))
}

impl RuntimeResourceApi {
    pub(crate) fn poll_webcam_messages(&self) {
        let mut frames = Vec::new();
        if let Ok(rx) = self.webcam_frame_rx.lock() {
            while let Ok(frame) = rx.try_recv() {
                frames.push(frame);
            }
        }
        for frame in frames {
            let _ = self.queue_webcam_frame(frame.id, frame.frame);
        }

        let mut errors = Vec::new();
        if let Ok(rx) = self.webcam_error_rx.lock() {
            while let Ok(error) = rx.try_recv() {
                errors.push(error);
            }
        }
        if !errors.is_empty() {
            let mut state = self.state.lock().expect("resource api mutex poisoned");
            for error in errors {
                state.webcam_last_error_by_id.insert(error.id, error.error);
            }
        }
    }

    #[cfg(all(
        any(target_os = "windows", target_os = "linux", target_os = "macos"),
        not(test)
    ))]
    fn start_webcam_capture(&self, id: WebcamID, config: WebcamConfig) {
        use nokhwa::pixel_format::RgbAFormat;
        use nokhwa::utils::{
            CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType,
        };
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, Instant};

        let Ok(mut stops) = self.webcam_stop_by_id.lock() else {
            return;
        };
        if stops.contains_key(&id) {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        stops.insert(id, stop.clone());
        drop(stops);

        let frame_tx = self.webcam_frame_tx.clone();
        let error_tx = self.webcam_error_tx.clone();
        std::thread::spawn(move || {
            let index = if config.device.trim().is_empty() {
                CameraIndex::Index(0)
            } else if let Ok(index) = config.device.parse::<u32>() {
                CameraIndex::Index(index)
            } else {
                CameraIndex::String(config.device.to_string())
            };
            let requested = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::Closest(
                CameraFormat::new_from(
                    config.width.max(1),
                    config.height.max(1),
                    FrameFormat::MJPEG,
                    config.fps.max(1),
                ),
            ));
            let mut camera = match nokhwa::Camera::new(index, requested) {
                Ok(camera) => camera,
                Err(err) => {
                    let _ = error_tx.send(WebcamErrorMessage {
                        id,
                        error: err.to_string(),
                    });
                    return;
                }
            };
            if let Err(err) = camera.open_stream() {
                let _ = error_tx.send(WebcamErrorMessage {
                    id,
                    error: err.to_string(),
                });
                return;
            }
            let frame_delay = Duration::from_millis((1000 / config.fps.max(1) as u64).max(1));
            while !stop.load(Ordering::Relaxed) {
                let start = Instant::now();
                match camera
                    .frame()
                    .and_then(|frame| frame.decode_image::<RgbAFormat>())
                {
                    Ok(image) => {
                        let width = image.width();
                        let height = image.height();
                        let mut rgba = image.into_raw();
                        if config.mirror {
                            mirror_rgba_rows(width, height, &mut rgba);
                        }
                        let _ = frame_tx.send(WebcamFrameMessage {
                            id,
                            frame: WebcamFrame {
                                width,
                                height,
                                rgba,
                            },
                        });
                    }
                    Err(err) => {
                        let _ = error_tx.send(WebcamErrorMessage {
                            id,
                            error: err.to_string(),
                        });
                    }
                }
                let elapsed = start.elapsed();
                if elapsed < frame_delay {
                    std::thread::sleep(frame_delay - elapsed);
                }
            }
        });
    }

    #[cfg(any(test, target_arch = "wasm32", target_os = "android"))]
    fn start_webcam_capture(&self, id: WebcamID, _config: WebcamConfig) {
        let _ = self.webcam_error_tx.send(WebcamErrorMessage {
            id,
            error: "webcam capture backend unavailable for this build".to_string(),
        });
    }

    pub(crate) fn queue_webcam_frame(&self, id: WebcamID, frame: WebcamFrame) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let Some(texture) = state.webcam_texture_by_id.get(&id).copied() else {
            return false;
        };
        let expected_len = (frame.width as usize)
            .checked_mul(frame.height as usize)
            .and_then(|pixels| pixels.checked_mul(4));
        if frame.width == 0 || frame.height == 0 || expected_len != Some(frame.rgba.len()) {
            state
                .webcam_last_error_by_id
                .insert(id, "invalid webcam frame rgba len".to_string());
            return false;
        }
        if state
            .webcam_config_by_id
            .get(&id)
            .is_some_and(|config| config.cpu_frames)
        {
            state.webcam_frame_by_id.insert(id, frame.clone());
        }
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::WriteTextureRgba {
                id: texture,
                width: frame.width,
                height: frame.height,
                rgba: Arc::from(frame.rgba),
            }));
        true
    }

    pub(crate) fn ensure_webcam_node_slot(
        &self,
        node: perro_ids::NodeID,
        config: WebcamConfig,
    ) -> WebcamID {
        let mut config = config;
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.webcam_node_by_node.get(&node).copied() {
            let same_config = state
                .webcam_config_by_id
                .get(&id)
                .is_some_and(|current| current == &config);
            if state.webcam_open_by_id.contains(&id) && same_config {
                return id;
            }
            drop(state);
            let _ = perro_resource_api::sub_apis::WebcamAPI::webcam_close(self, id);
            return self.ensure_webcam_node_slot(node, config);
        }
        let id = state.allocate_webcam_id();
        let texture = state.allocate_texture_id();
        let source = format!("webcam://node/{}", node.as_u64());
        let source_hash = string_to_u64(&source);
        let request = state.allocate_request();
        let (width, height) = clamp_size(config.width, config.height);
        config.width = width;
        config.height = height;
        state.webcam_texture_by_id.insert(id, texture);
        let start_config = config.clone();
        state.webcam_config_by_id.insert(id, config);
        state.webcam_open_by_id.insert(id);
        state.webcam_node_by_node.insert(node, id);
        state.texture_by_source.insert(source_hash, texture);
        state.texture_pending_by_source.insert(source_hash, request);
        state
            .texture_pending_source_by_request
            .insert(request, source.clone());
        state.texture_pending_id_by_request.insert(request, texture);
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::CreateExternalTexture {
                request,
                id: texture,
                source,
                reserved: true,
                width,
                height,
            },
        ));
        drop(state);
        self.start_webcam_capture(id, start_config);
        id
    }

    pub(crate) fn release_webcam_node_slot(&self, node: perro_ids::NodeID) -> bool {
        let id = self
            .state
            .lock()
            .ok()
            .and_then(|state| state.webcam_node_by_node.get(&node).copied());
        let Some(id) = id else {
            return false;
        };
        perro_resource_api::sub_apis::WebcamAPI::webcam_close(self, id)
    }
}

impl WebcamAPI for RuntimeResourceApi {
    fn webcam_open(&self, mut config: WebcamConfig) -> Result<WebcamID, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "webcam mutex poisoned".to_string())?;
        let id = state.allocate_webcam_id();
        let texture = state.allocate_texture_id();
        let source = format!("webcam://{}", id.as_u64());
        let source_hash = string_to_u64(&source);
        let request = state.allocate_request();
        let (width, height) = clamp_size(config.width, config.height);
        config.width = width;
        config.height = height;
        state.webcam_texture_by_id.insert(id, texture);
        let start_config = config.clone();
        state.webcam_config_by_id.insert(id, config);
        state.webcam_open_by_id.insert(id);
        state.texture_by_source.insert(source_hash, texture);
        state.texture_pending_by_source.insert(source_hash, request);
        state
            .texture_pending_source_by_request
            .insert(request, source.clone());
        state.texture_pending_id_by_request.insert(request, texture);
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::CreateExternalTexture {
                request,
                id: texture,
                source,
                reserved: true,
                width,
                height,
            },
        ));
        drop(state);
        self.start_webcam_capture(id, start_config);
        Ok(id)
    }

    fn webcam_default(&self) -> Result<WebcamID, String> {
        let current = self
            .state
            .lock()
            .map_err(|_| "webcam mutex poisoned".to_string())?
            .webcam_default_id;
        if let Some(id) = current
            && self.webcam_is_open(id)
        {
            return Ok(id);
        }
        let id = self.webcam_open(WebcamConfig::default())?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "webcam mutex poisoned".to_string())?;
        state.webcam_default_id = Some(id);
        Ok(id)
    }

    fn webcam_texture(&self, id: WebcamID) -> TextureID {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.webcam_texture_by_id.get(&id).copied())
            .unwrap_or_else(TextureID::nil)
    }

    fn webcam_frame_rgba(&self, id: WebcamID) -> Option<WebcamFrame> {
        self.state.lock().ok()?.webcam_frame_by_id.get(&id).cloned()
    }

    fn webcam_is_open(&self, id: WebcamID) -> bool {
        self.state
            .lock()
            .map(|state| state.webcam_open_by_id.contains(&id))
            .unwrap_or(false)
    }

    fn webcam_last_error(&self, id: WebcamID) -> Option<String> {
        self.state
            .lock()
            .ok()?
            .webcam_last_error_by_id
            .get(&id)
            .cloned()
    }

    fn webcam_close(&self, id: WebcamID) -> bool {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return false,
        };
        if !state.webcam_open_by_id.remove(&id) {
            return false;
        }
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        if let Ok(mut stops) = self.webcam_stop_by_id.lock()
            && let Some(stop) = stops.remove(&id)
        {
            use std::sync::atomic::Ordering;
            stop.store(true, Ordering::Relaxed);
        }
        if state.webcam_default_id == Some(id) {
            state.webcam_default_id = None;
        }
        let node_sources: Vec<_> = state
            .webcam_node_by_node
            .iter()
            .filter_map(|(node, webcam)| (*webcam == id).then_some(node.as_u64()))
            .collect();
        state.webcam_node_by_node.retain(|_, webcam| *webcam != id);
        state.webcam_config_by_id.remove(&id);
        state.webcam_frame_by_id.remove(&id);
        state.webcam_last_error_by_id.remove(&id);
        if let Some(texture) = state.webcam_texture_by_id.remove(&id) {
            state.texture_loaded_by_id.remove(&texture);
            let mut sources = vec![format!("webcam://{}", id.as_u64())];
            sources.extend(
                node_sources
                    .into_iter()
                    .map(|node| format!("webcam://node/{node}")),
            );
            for source in sources {
                let source_hash = string_to_u64(&source);
                state.texture_by_source.remove(&source_hash);
                state.texture_pending_by_source.remove(&source_hash);
            }
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::DropTexture {
                    id: texture,
                }));
            let _ = state.free_texture_id(texture);
        }
        let _ = state.free_webcam_id(id);
        true
    }
}

#[cfg_attr(test, allow(dead_code))]
fn mirror_rgba_rows(width: u32, height: u32, rgba: &mut [u8]) {
    let width = width as usize;
    let height = height as usize;
    if width < 2 || height == 0 {
        return;
    }
    for y in 0..height {
        let row = y * width * 4;
        for x in 0..(width / 2) {
            let left = row + x * 4;
            let right = row + (width - 1 - x) * 4;
            for c in 0..4 {
                rgba.swap(left + c, right + c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_ids::NodeID;

    fn api() -> Arc<RuntimeResourceApi> {
        RuntimeResourceApi::new(None, None, None, None, None, None, None, None)
    }

    #[test]
    fn default_webcam_allocs_stable_id_and_texture() {
        let api = api();
        let id = api.webcam_default().expect("default webcam id");
        let same = api.webcam_default().expect("same default webcam id");
        let texture = api.webcam_texture(id);

        assert_eq!(id, same);
        assert!(api.webcam_is_open(id));
        assert!(!texture.is_nil());

        let mut commands = Vec::new();
        api.drain_commands(&mut commands);

        assert!(api.webcam_last_error(id).is_some());
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Resource(ResourceCommand::CreateExternalTexture { id, .. })
                if *id == texture
        )));
    }

    #[test]
    fn cpu_frames_flag_controls_rgba_retention() {
        let api = api();
        let no_cpu = api
            .webcam_open(WebcamConfig {
                cpu_frames: false,
                ..WebcamConfig::default()
            })
            .expect("webcam id");
        let no_cpu_texture = api.webcam_texture(no_cpu);

        assert!(api.queue_webcam_frame(
            no_cpu,
            WebcamFrame {
                width: 1,
                height: 1,
                rgba: vec![1, 2, 3, 4],
            },
        ));
        assert!(api.webcam_frame_rgba(no_cpu).is_none());

        let with_cpu = api
            .webcam_open(WebcamConfig {
                cpu_frames: true,
                ..WebcamConfig::default()
            })
            .expect("webcam id");
        assert!(api.queue_webcam_frame(
            with_cpu,
            WebcamFrame {
                width: 1,
                height: 1,
                rgba: vec![5, 6, 7, 8],
            },
        ));
        assert_eq!(
            api.webcam_frame_rgba(with_cpu).map(|frame| frame.rgba),
            Some(vec![5, 6, 7, 8])
        );

        let mut commands = Vec::new();
        api.drain_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Resource(ResourceCommand::WriteTextureRgba { id, rgba, .. })
                if *id == no_cpu_texture && rgba.as_ref() == [1, 2, 3, 4]
        )));
    }

    #[test]
    fn node_slot_opens_and_releases_automatic_webcam() {
        let api = api();
        let node = NodeID::from_parts(42, 7);
        let id = api.ensure_webcam_node_slot(node, WebcamConfig::default());
        let same = api.ensure_webcam_node_slot(node, WebcamConfig::default());

        assert_eq!(id, same);
        assert!(api.webcam_is_open(id));
        assert!(!api.webcam_texture(id).is_nil());

        assert!(api.release_webcam_node_slot(node));
        assert!(!api.webcam_is_open(id));
        assert!(api.webcam_texture(id).is_nil());
    }
}
