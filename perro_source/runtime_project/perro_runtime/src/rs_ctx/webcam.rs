use super::core::RuntimeResourceApi;
use super::core::WebcamErrorMessage;
use super::core::WebcamFrameMessage;
use perro_ids::{TextureID, WebcamID, string_to_u64};
use perro_render_bridge::{RenderCommand, ResourceCommand};
use perro_resource_api::sub_apis::{WebcamAPI, WebcamConfig, WebcamDevice, WebcamFrame};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos", test))]
use std::sync::Arc;

fn clamp_size(width: u32, height: u32) -> (u32, u32) {
    (width.clamp(1, 8192), height.clamp(1, 8192))
}

#[cfg(all(
    any(target_os = "windows", target_os = "linux", target_os = "macos"),
    not(test)
))]
fn webcam_log_enabled() -> bool {
    std::env::var("PERRO_WEBCAM_LOG")
        .ok()
        .is_some_and(|raw| matches!(raw.as_str(), "1" | "true" | "yes" | "on"))
}

impl RuntimeResourceApi {
    pub(crate) fn poll_webcam_messages(&self) {
        // coalesce to the newest frame per webcam id: only the last frame is ever
        // displayed, so drop stale queued frames instead of paying a full copy +
        // GPU upload for each. webcams are few, so a linear scan is fine.
        let mut latest: Vec<WebcamFrameMessage> = Vec::new();
        if let Ok(rx) = self.webcam_frame_rx.lock() {
            while let Ok(message) = rx.try_recv() {
                match latest.iter_mut().find(|queued| queued.id == message.id) {
                    Some(slot) => *slot = message,
                    None => latest.push(message),
                }
            }
        }
        for message in latest {
            let _ = self.queue_webcam_frame(message.id, message.frame);
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
                let prev = state
                    .webcam_last_error_by_id
                    .insert(error.id, error.error.clone());
                if prev.as_deref() != Some(error.error.as_str()) {
                    eprintln!(
                        "[perro][webcam] error id={} err={}",
                        error.id.as_u64(),
                        error.error
                    );
                }
            }
        }
    }

    #[cfg(all(
        any(target_os = "windows", target_os = "linux", target_os = "macos"),
        not(test)
    ))]
    fn start_webcam_capture(&self, id: WebcamID, config: WebcamConfig) {
        use nokhwa::pixel_format::RgbAFormat;
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
            let log_enabled = webcam_log_enabled();
            let index = webcam_camera_index_for_device(config.device.as_ref());
            if log_enabled {
                eprintln!(
                    "[perro][webcam] start id={} device={} {}x{}@{} mirror={}",
                    id.as_u64(),
                    index.as_string(),
                    config.width.max(1),
                    config.height.max(1),
                    config.fps.max(1),
                    config.mirror
                );
            }
            let mut camera = match open_webcam_camera(index, &config) {
                Ok(camera) => {
                    if log_enabled {
                        let format = camera.camera_format();
                        eprintln!(
                            "[perro][webcam] open id={} fmt={} {}x{}@{}",
                            id.as_u64(),
                            format.format(),
                            format.width(),
                            format.height(),
                            format.frame_rate()
                        );
                    }
                    camera
                }
                Err(err) => {
                    if log_enabled {
                        eprintln!("[perro][webcam] open fail id={} err={err}", id.as_u64());
                    }
                    let _ = error_tx.send(WebcamErrorMessage {
                        id,
                        error: err.to_string(),
                    });
                    return;
                }
            };
            let frame_delay = Duration::from_millis((1000 / config.fps.max(1) as u64).max(1));
            let mut logged_first_frame = false;
            let mut frame_count = 0u32;
            let mut fps_window_start = Instant::now();
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
                        if log_enabled && !logged_first_frame {
                            eprintln!(
                                "[perro][webcam] first frame id={} {}x{} len={}",
                                id.as_u64(),
                                width,
                                height,
                                rgba.len()
                            );
                            logged_first_frame = true;
                        }
                        frame_count += 1;
                        if log_enabled && frame_count.is_multiple_of(120) {
                            let window = fps_window_start.elapsed().as_secs_f64();
                            if window > 0.0 {
                                eprintln!(
                                    "[perro][webcam] id={} measured_fps={:.1}",
                                    id.as_u64(),
                                    120.0 / window
                                );
                            }
                            fps_window_start = Instant::now();
                        }
                        if config.mirror {
                            mirror_rgba_rows(width, height, &mut rgba);
                        }
                        // drop this frame when the runtime lags (channel full):
                        // only the newest frame is ever displayed anyway.
                        match frame_tx.try_send(WebcamFrameMessage {
                            id,
                            frame: WebcamFrame {
                                width,
                                height,
                                rgba,
                            },
                        }) {
                            Ok(()) => {}
                            Err(std::sync::mpsc::TrySendError::Full(_)) => {}
                            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => break,
                        }
                    }
                    Err(err) => {
                        if log_enabled {
                            eprintln!("[perro][webcam] frame fail id={} err={err}", id.as_u64());
                        }
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
        state
            .webcam_resolution_by_id
            .insert(id, [frame.width, frame.height]);
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
                // move the decoded Vec straight into the command: no Vec->Arc->Vec
                // round trip; the backend copies it once into the resident buffer.
                rgba: frame.rgba.into(),
            }));
        true
    }

    pub(crate) fn ensure_webcam_node_slot(
        &self,
        node: perro_ids::NodeID,
        config: WebcamConfig,
    ) -> WebcamID {
        let mut config = self.resolve_webcam_config(config);
        // Clamp before the same-config compare below; the stored config is
        // post-clamp, so an unclamped compare would thrash close/reopen.
        let (width, height) = clamp_size(config.width, config.height);
        config.width = width;
        config.height = height;
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
    #[cfg(all(
        any(target_os = "windows", target_os = "linux", target_os = "macos"),
        not(test)
    ))]
    fn webcam_devices(&self) -> Result<Vec<WebcamDevice>, String> {
        use nokhwa::utils::{ApiBackend, CameraIndex};

        let devices: Vec<_> = nokhwa::query(ApiBackend::Auto)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|info| {
                let extra = info.misc();
                let index = match info.index() {
                    CameraIndex::Index(index) => Some(*index),
                    CameraIndex::String(index) => index.parse::<u32>().ok(),
                };
                let slot = webcam_device_slot(&info, &extra);
                WebcamDevice {
                    slot,
                    index,
                    name: info.human_name(),
                    description: info.description().to_string(),
                    extra,
                }
            })
            .collect();
        if devices.is_empty() {
            #[cfg(target_os = "windows")]
            if let Some(fallback) = windows_pnp_webcam_devices() {
                return Ok(fallback);
            }
        }
        Ok(devices)
    }

    #[cfg(any(test, target_arch = "wasm32", target_os = "android"))]
    fn webcam_devices(&self) -> Result<Vec<WebcamDevice>, String> {
        Err("webcam device query backend unavailable for this build".to_string())
    }

    fn webcam_open(&self, mut config: WebcamConfig) -> Result<WebcamID, String> {
        config = self.resolve_webcam_config(config);
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

    fn webcam_resolution(&self, id: WebcamID) -> Option<[u32; 2]> {
        self.state
            .lock()
            .ok()?
            .webcam_resolution_by_id
            .get(&id)
            .copied()
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
        state.webcam_resolution_by_id.remove(&id);
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

#[cfg(all(
    any(target_os = "windows", target_os = "linux", target_os = "macos"),
    not(test)
))]
fn open_webcam_camera(
    index: nokhwa::utils::CameraIndex,
    config: &WebcamConfig,
) -> Result<nokhwa::Camera, nokhwa::NokhwaError> {
    use nokhwa::pixel_format::RgbAFormat;
    use nokhwa::utils::{CameraFormat, FrameFormat, RequestedFormat, RequestedFormatType};

    let width = config.width.max(1);
    let height = config.height.max(1);
    let fps = config.fps.max(1);
    let formats = [
        FrameFormat::MJPEG,
        FrameFormat::YUYV,
        FrameFormat::NV12,
        FrameFormat::RAWRGB,
        FrameFormat::RAWBGR,
    ];
    let mut last_err = None;

    for format in formats {
        let requested = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::Exact(
            CameraFormat::new_from(width, height, format, fps),
        ));
        match nokhwa::Camera::new(index.clone(), requested) {
            Ok(mut camera) => match camera.open_stream() {
                Ok(()) => match camera
                    .frame()
                    .and_then(|frame| frame.decode_image::<RgbAFormat>())
                {
                    Ok(_) => return Ok(camera),
                    Err(err) => last_err = Some(err),
                },
                Err(err) => last_err = Some(err),
            },
            Err(err) => last_err = Some(err),
        }
    }

    for requested in [
        RequestedFormat::new::<RgbAFormat>(RequestedFormatType::HighestFrameRate(fps)),
        RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate),
        RequestedFormat::new::<RgbAFormat>(RequestedFormatType::None),
    ] {
        match nokhwa::Camera::new(index.clone(), requested) {
            Ok(mut camera) => match camera.open_stream() {
                Ok(()) => match camera
                    .frame()
                    .and_then(|frame| frame.decode_image::<RgbAFormat>())
                {
                    Ok(_) => return Ok(camera),
                    Err(err) => last_err = Some(err),
                },
                Err(err) => last_err = Some(err),
            },
            Err(err) => last_err = Some(err),
        }
    }

    Err(last_err.unwrap_or_else(|| {
        nokhwa::NokhwaError::GeneralError("no supported webcam format".to_string())
    }))
}

#[cfg(all(
    any(target_os = "windows", target_os = "linux", target_os = "macos"),
    not(test)
))]
fn default_webcam_camera_index() -> nokhwa::utils::CameraIndex {
    use nokhwa::utils::{ApiBackend, CameraIndex};

    nokhwa::query(ApiBackend::Auto)
        .ok()
        .and_then(|devices| devices.into_iter().next().map(|info| info.index().clone()))
        .unwrap_or(CameraIndex::Index(0))
}

impl RuntimeResourceApi {
    #[cfg(all(
        any(target_os = "windows", target_os = "linux", target_os = "macos"),
        not(test)
    ))]
    fn resolve_webcam_config(&self, config: WebcamConfig) -> WebcamConfig {
        use nokhwa::utils::ApiBackend;

        if !config.device.trim().is_empty() {
            return config;
        }
        if let Ok(cache) = self.webcam_default_slot.lock()
            && let Some(slot) = cache.as_ref()
        {
            return WebcamConfig {
                device: slot.clone().into(),
                ..config
            };
        }
        let Ok(devices) = nokhwa::query(ApiBackend::Auto) else {
            return config;
        };
        let Some(info) = devices.into_iter().next() else {
            return config;
        };
        let extra = info.misc();
        let slot = webcam_device_slot(&info, &extra);
        if slot.trim().is_empty() {
            return config;
        }
        if let Ok(mut cache) = self.webcam_default_slot.lock() {
            *cache = Some(slot.clone());
        }
        WebcamConfig {
            device: slot.into(),
            ..config
        }
    }

    #[cfg(any(test, target_arch = "wasm32", target_os = "android"))]
    fn resolve_webcam_config(&self, config: WebcamConfig) -> WebcamConfig {
        config
    }
}

#[cfg(all(
    any(target_os = "windows", target_os = "linux", target_os = "macos"),
    not(test)
))]
fn webcam_camera_index_for_device(device: &str) -> nokhwa::utils::CameraIndex {
    use nokhwa::utils::{ApiBackend, CameraIndex};

    let device = device.trim();
    if device.is_empty() {
        return default_webcam_camera_index();
    }
    if let Ok(index) = device.parse::<u32>() {
        return CameraIndex::Index(index);
    }
    if let Ok(devices) = nokhwa::query(ApiBackend::Auto)
        && let Some(index) = devices.into_iter().find_map(|info| {
            let extra = info.misc();
            let slot = webcam_device_slot(&info, &extra);
            let matches = slot == device
                || extra == device
                || info.human_name() == device
                || info.description() == device;
            if !matches {
                return None;
            }
            match info.index() {
                CameraIndex::Index(index) => Some(CameraIndex::Index(*index)),
                CameraIndex::String(index) => index
                    .parse::<u32>()
                    .ok()
                    .map(CameraIndex::Index)
                    .or_else(|| Some(CameraIndex::String(index.clone()))),
            }
        })
    {
        return index;
    }
    CameraIndex::String(device.to_string())
}

#[cfg(all(
    any(target_os = "windows", target_os = "linux", target_os = "macos"),
    not(test)
))]
fn webcam_device_slot(info: &nokhwa::utils::CameraInfo, extra: &str) -> String {
    let _ = extra;

    info.index().as_string()
}

#[cfg(all(
    target_os = "windows",
    any(target_os = "windows", target_os = "linux", target_os = "macos"),
    not(test)
))]
fn windows_pnp_webcam_devices() -> Option<Vec<WebcamDevice>> {
    use std::process::Command;

    let script = concat!(
        "Get-CimInstance Win32_PnPEntity | ",
        "Where-Object { ",
        "($_.PNPClass -in @('Camera','Image','MEDIA')) -and ",
        "($_.Name -match '(?i)camera|webcam|video|logitech|c922') ",
        "} | ForEach-Object { ",
        "\"$($_.Name)`t$($_.PNPClass)`t$($_.Status)`t$($_.DeviceID)\" ",
        "}"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let devices: Vec<_> = text
        .lines()
        .filter_map(parse_windows_pnp_webcam_line)
        .enumerate()
        .map(|(i, mut device)| {
            device.slot = i.to_string();
            device.index = Some(i as u32);
            device
        })
        .collect();
    (!devices.is_empty()).then_some(devices)
}

#[cfg(all(
    target_os = "windows",
    any(target_os = "windows", target_os = "linux", target_os = "macos"),
    not(test)
))]
fn parse_windows_pnp_webcam_line(line: &str) -> Option<WebcamDevice> {
    let mut parts = line.splitn(4, '\t');
    let name = parts.next()?.trim();
    let class = parts.next()?.trim();
    let status = parts.next()?.trim();
    let device_id = parts.next()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(WebcamDevice {
        slot: String::new(),
        index: None,
        name: name.to_string(),
        description: format!("Windows PnP fallback; class={class}; status={status}"),
        extra: device_id.to_string(),
    })
}

#[cfg(all(
    any(target_os = "windows", target_os = "linux", target_os = "macos"),
    not(test)
))]
fn mirror_rgba_rows(width: u32, height: u32, rgba: &mut [u8]) {
    let width = width as usize;
    let height = height as usize;
    if width < 2 || height == 0 {
        return;
    }
    let row_bytes = width * 4;
    // swap whole 4-byte pixels from both ends instead of 4 per-channel swaps.
    for y in 0..height {
        let Some(row) = rgba.get_mut(y * row_bytes..(y + 1) * row_bytes) else {
            break;
        };
        let (mut lo, mut hi) = (0usize, width - 1);
        while lo < hi {
            let l = lo * 4;
            let h = hi * 4;
            let mut pixel = [0u8; 4];
            pixel.copy_from_slice(&row[l..l + 4]);
            row.copy_within(h..h + 4, l);
            row[h..h + 4].copy_from_slice(&pixel);
            lo += 1;
            hi -= 1;
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
        assert_eq!(api.webcam_resolution(no_cpu), Some([1, 1]));

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
