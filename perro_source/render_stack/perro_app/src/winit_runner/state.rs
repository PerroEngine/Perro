use super::*;

impl<B: GraphicsBackend> RunnerState<B> {
    pub(super) fn new(
        app: App<B>,
        title: &str,
        fixed_timestep: Option<f32>,
        #[cfg(not(target_arch = "wasm32"))] preloaded_images: Option<PreloadedProjectImages>,
    ) -> Self {
        let now = Instant::now();
        #[cfg(not(target_arch = "wasm32"))]
        let preloaded_images =
            preloaded_images.unwrap_or_else(|| preload_project_images(app.runtime.project()));
        #[cfg(not(target_arch = "wasm32"))]
        let startup_splash =
            StartupSplashState::from_preloaded(preloaded_images.startup_splash.clone(), now);
        #[cfg(target_arch = "wasm32")]
        let startup_splash = StartupSplashState::from_preloaded(now);
        let normalized_fixed_timestep = normalize_fixed_timestep_seconds(fixed_timestep);
        let frame_rate_cap = app
            .runtime
            .project()
            .map(|project| project_frame_rate_cap(project.config.frame_rate_cap))
            .unwrap_or(RuntimeFrameRateCap::Unlimited);
        let vsync_enabled = app
            .runtime
            .project()
            .map(|project| project.config.vsync)
            .unwrap_or(false);
        eprintln!("[perro][runtime] frame_rate_cap=({frame_rate_cap:?})");
        Self {
            app,
            title: title.to_owned(),
            window: None,
            fixed_timestep: normalized_fixed_timestep,
            fixed_accumulator: 0.0,
            pacer: FramePacer::new(frame_rate_cap, vsync_enabled),
            last_frame_start: now,
            last_frame_end: now,
            run_start: now,
            timing_csv: TimingCsvWriter::from_env(),
            #[cfg(feature = "profile_heavy")]
            profile_csv: ProfileCsvWriter::from_env(),
            #[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
            mem_profile_enabled: std::env::var("PERRO_MEM_PROFILE").ok().is_some_and(|raw| {
                let normalized = raw.trim().to_ascii_lowercase();
                matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
            }),
            #[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
            mem_profile_csv: MemProfileCsvWriter::from_env(),
            timing_warmup_frames_left: TIMING_WARMUP_FRAMES,
            batch_start: now,
            batch: BatchCoreStats::default(),
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui: BatchUiStats::default(),
            #[cfg(feature = "profile_heavy")]
            batch_heavy: BatchHeavyStats::default(),
            frame_index: 0,
            fps_window_start: now,
            fps_window_frames: 0,
            kbm_input: crate::input::KbmInput::new(),
            gamepad_input: crate::input::GamepadInput::new(),
            joycon_input: crate::input::JoyConInput::new(),
            mouse_mode: MouseMode::Visible,
            mouse_uses_raw_motion: false,
            cursor_icon: perro_ui::CursorIcon::Default,
            window_requests: Vec::new(),
            cursor_inside_window: false,
            #[cfg(not(target_arch = "wasm32"))]
            last_window_position: None,
            #[cfg(not(target_arch = "wasm32"))]
            preloaded_images,
            startup_splash,
            exit_result: None,
        }
    }

    pub(super) fn apply_mouse_mode(window: &Window, mode: MouseMode) -> (MouseMode, bool) {
        match mode {
            MouseMode::Visible => {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(true);
                (MouseMode::Visible, false)
            }
            MouseMode::Hidden => {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(false);
                (MouseMode::Hidden, false)
            }
            MouseMode::Captured => match window.set_cursor_grab(CursorGrabMode::Locked) {
                Ok(_) => {
                    window.set_cursor_visible(false);
                    (MouseMode::Captured, true)
                }
                Err(_locked_err) => match window.set_cursor_grab(CursorGrabMode::Confined) {
                    Ok(_) => {
                        window.set_cursor_visible(false);
                        (MouseMode::Captured, false)
                    }
                    Err(_confined_err) => {
                        window.set_cursor_visible(true);
                        (MouseMode::Visible, false)
                    }
                },
            },
            MouseMode::Confined => match window.set_cursor_grab(CursorGrabMode::Confined) {
                Ok(_) => {
                    window.set_cursor_visible(true);
                    (MouseMode::Confined, false)
                }
                Err(_err) => {
                    window.set_cursor_visible(true);
                    (MouseMode::Visible, false)
                }
            },
            MouseMode::ConfinedHidden => match window.set_cursor_grab(CursorGrabMode::Confined) {
                Ok(_) => {
                    window.set_cursor_visible(false);
                    (MouseMode::ConfinedHidden, false)
                }
                Err(_err) => {
                    window.set_cursor_visible(false);
                    (MouseMode::Hidden, false)
                }
            },
        }
    }

    pub(super) fn set_mouse_mode(&mut self, mode: MouseMode) {
        if self.mouse_mode == mode {
            return;
        }
        if let Some(window) = &self.window {
            let (applied_mode, uses_raw_motion) = Self::apply_mouse_mode(window.as_ref(), mode);
            if matches!(
                applied_mode,
                MouseMode::Captured | MouseMode::Confined | MouseMode::ConfinedHidden
            ) {
                center_cursor(window.as_ref());
            }
            self.mouse_mode = applied_mode;
            self.mouse_uses_raw_motion = uses_raw_motion;
            self.app.set_mouse_mode_state(applied_mode);
            self.app.clear_mouse_delta();
            self.kbm_input.reset_cursor_position();
        } else {
            self.mouse_mode = MouseMode::Visible;
            self.mouse_uses_raw_motion = false;
            self.app.set_mouse_mode_state(MouseMode::Visible);
        }
    }

    pub(super) fn reset_mouse_mode_for_exit(&mut self) {
        if let Some(window) = &self.window {
            release_mouse(window.as_ref());
            self.mouse_mode = MouseMode::Visible;
            self.mouse_uses_raw_motion = false;
            self.app.set_mouse_mode_state(MouseMode::Visible);
            self.app.clear_mouse_delta();
            self.kbm_input.reset_cursor_position();
        } else {
            self.mouse_mode = MouseMode::Visible;
            self.mouse_uses_raw_motion = false;
            self.app.set_mouse_mode_state(MouseMode::Visible);
            self.app.clear_mouse_delta();
            self.kbm_input.reset_cursor_position();
        }
    }

    pub(super) fn clear_keyboard_mouse_focus_state(&mut self) {
        self.cursor_inside_window = false;
        self.app.clear_keyboard_mouse_state();
        self.kbm_input.clear_focus_state();
    }

    pub(super) fn apply_mouse_mode_request(&mut self) {
        self.app.apply_input_commands();
        if let Some(mode) = self.app.take_mouse_mode_request() {
            self.set_mouse_mode(mode);
        }
    }

    pub(super) fn set_cursor_icon(&mut self, icon: perro_ui::CursorIcon) {
        if self.cursor_icon == icon {
            return;
        }
        if let Some(window) = &self.window {
            window.set_cursor(map_cursor_icon(icon));
        }
        self.cursor_icon = icon;
    }

    pub(super) fn apply_cursor_icon_request(&mut self) {
        if let Some(icon) = self.app.take_cursor_icon_request() {
            self.set_cursor_icon(icon);
        }
    }

    pub(super) fn apply_window_requests(&mut self, event_loop: &ActiveEventLoop) {
        self.app.drain_window_requests(&mut self.window_requests);
        if self.window_requests.is_empty() {
            return;
        }

        if self
            .window_requests
            .iter()
            .any(|request| matches!(request, WindowRequest::CloseApp))
        {
            self.window_requests.clear();
            self.request_exit(event_loop, AppExitResult::event_loop_exit());
            return;
        }

        let Some(window) = self.window.as_ref().cloned() else {
            self.window_requests.clear();
            return;
        };

        let requests = std::mem::take(&mut self.window_requests);
        for request in requests {
            match request {
                WindowRequest::SetTitle(title) => window.set_title(&title),
                WindowRequest::SetSize { width, height } => {
                    let _ = window.request_inner_size(PhysicalSize::new(width, height));
                }
                WindowRequest::SetMode(WindowMode::Windowed) => {
                    window.set_fullscreen(None);
                }
                WindowRequest::SetMode(WindowMode::BorderlessFullscreen) => {
                    let monitor = window
                        .current_monitor()
                        .or_else(|| pick_monitor(event_loop));
                    window.set_fullscreen(Some(Fullscreen::Borderless(monitor)));
                }
                WindowRequest::SetFrameRateCap(cap) => {
                    // No-op when unchanged: per-frame script calls must not
                    // spam stderr or re-anchor the deadline.
                    if self.pacer.set_cap(cap) {
                        eprintln!("[perro][runtime] frame_rate_cap=({:?})", self.pacer.cap());
                    }
                }
                WindowRequest::SetCursorIcon(icon) => {
                    self.set_cursor_icon(icon);
                }
                WindowRequest::CloseApp => {}
            }
        }
    }

    pub(super) fn request_exit(&mut self, event_loop: &ActiveEventLoop, result: AppExitResult) {
        if self.exit_result.is_some() {
            return;
        }
        self.exit_result = Some(result);
        self.reset_mouse_mode_for_exit();
        if let Some(window) = self.window.take() {
            window.set_visible(false);
        }
        event_loop.exit();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn sync_window_position(&mut self, position: PhysicalPosition<i32>) {
        if let Some(prev) = self.last_window_position
            && self.cursor_inside_window
        {
            let dx = f64::from(prev.x - position.x);
            let dy = f64::from(prev.y - position.y);
            self.kbm_input.translate_cursor_position(dx, dy);
            if let Some(cursor) = self.kbm_input.last_cursor_position() {
                self.app
                    .set_mouse_position(cursor.x as f32, cursor.y as f32);
            }
        }
        self.last_window_position = Some(position);
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn sync_window_position(&mut self, _position: PhysicalPosition<i32>) {}

    /// Refresh the cached monitor rate and mirror it into the runtime.
    /// Hits OS display queries; call on resume/move/scale change only.
    pub(super) fn sync_refresh_rate(&mut self) {
        let refresh_hz = self.pacer.update_refresh_rate(self.window.as_deref());
        self.app.runtime.set_active_refresh_rate(refresh_hz);
    }

    pub(super) fn apply_frame_control_flow(&self, event_loop: &ActiveEventLoop, now: Instant) {
        if let Some(deadline) = self.pacer.deadline()
            && deadline > now
        {
            // OS timers overshoot by a few ms; wake early and poll the rest.
            // With 1ms-resolution system timers (see timer_resolution.rs),
            // WaitUntil is accurate enough to use for high-rate caps too, so
            // only intervals too small to leave any wake headroom fall back
            // to Poll (busy event loop).
            let wake_at = deadline.checked_sub(FRAME_WAKE_HEADROOM);
            if wake_at.is_none_or(|wake_at| wake_at <= now) {
                event_loop.set_control_flow(ControlFlow::Poll);
            } else if let Some(wake_at) = wake_at {
                event_loop.set_control_flow(ControlFlow::WaitUntil(wake_at));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Poll);
        }
    }

    #[inline]
    pub(super) fn should_sample_timing(&self) -> bool {
        #[cfg(any(feature = "profile_heavy", feature = "ui_profile", feature = "fps"))]
        {
            true
        }
        #[cfg(not(any(feature = "profile_heavy", feature = "ui_profile", feature = "fps")))]
        {
            self.frame_index == 1
                || self
                    .frame_index
                    .is_multiple_of(LOG_TIMING_SAMPLE_STRIDE as u64)
        }
    }
}
