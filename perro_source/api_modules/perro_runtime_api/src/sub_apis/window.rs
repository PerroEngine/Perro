//! Runtime window API.
//!
//! Queues window title, size, mode, close, and frame-rate requests for the app layer.

pub use perro_ui::CursorIcon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowMode {
    Windowed,
    BorderlessFullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameRateCap {
    Unlimited,
    Fps(f32),
    RefreshRate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WindowRequest {
    SetTitle(String),
    SetSize { width: u32, height: u32 },
    SetMode(WindowMode),
    SetFrameRateCap(FrameRateCap),
    SetCursorIcon(CursorIcon),
    CloseApp,
}

pub trait WindowAPI {
    fn set_window_title(&mut self, title: impl Into<String>);
    fn set_window_size(&mut self, width: u32, height: u32);
    fn set_window_mode(&mut self, mode: WindowMode);
    fn set_frame_rate_cap(&mut self, cap: FrameRateCap);
    fn set_cursor_icon(&mut self, icon: CursorIcon);
    fn close_app(&mut self);
    fn get_active_refresh_rate(&mut self) -> Option<f32>;
}

pub struct WindowModule<'rt, R: WindowAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: WindowAPI + ?Sized> WindowModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.rt.set_window_title(title);
    }

    pub fn set_size(&mut self, width: u32, height: u32) {
        self.rt.set_window_size(width, height);
    }

    pub fn set_mode(&mut self, mode: WindowMode) {
        self.rt.set_window_mode(mode);
    }

    pub fn set_windowed(&mut self) {
        self.set_mode(WindowMode::Windowed);
    }

    pub fn set_borderless_fullscreen(&mut self) {
        self.set_mode(WindowMode::BorderlessFullscreen);
    }

    pub fn set_frame_rate_cap(&mut self, cap: FrameRateCap) {
        self.rt.set_frame_rate_cap(cap);
    }

    pub fn set_frame_rate_limit(&mut self, fps: f32) {
        self.set_frame_rate_cap(FrameRateCap::Fps(fps));
    }

    pub fn set_refresh_rate_cap(&mut self) {
        self.set_frame_rate_cap(FrameRateCap::RefreshRate);
    }

    pub fn set_unlimited_frame_rate(&mut self) {
        self.set_frame_rate_cap(FrameRateCap::Unlimited);
    }

    pub fn set_cursor_icon(&mut self, icon: CursorIcon) {
        self.rt.set_cursor_icon(icon);
    }

    pub fn close_app(&mut self) {
        self.rt.close_app();
    }

    pub fn get_active_refresh_rate(&mut self) -> Option<f32> {
        self.rt.get_active_refresh_rate()
    }
}

#[macro_export]
macro_rules! window_set_title {
    ($ctx:expr, $title:expr) => {
        $ctx.Window().set_title($title)
    };
}

#[macro_export]
macro_rules! window_set_size {
    ($ctx:expr, $width:expr, $height:expr) => {
        $ctx.Window().set_size($width, $height)
    };
}

#[macro_export]
macro_rules! window_set_mode {
    ($ctx:expr, $mode:expr) => {
        $ctx.Window().set_mode($mode)
    };
}

#[macro_export]
macro_rules! window_set_frame_rate_cap {
    ($ctx:expr, $cap:expr) => {
        $ctx.Window().set_frame_rate_cap($cap)
    };
}

#[macro_export]
macro_rules! window_set_frame_rate_limit {
    ($ctx:expr, $fps:expr) => {
        $ctx.Window().set_frame_rate_limit($fps)
    };
}

#[macro_export]
macro_rules! window_set_cursor_icon {
    ($ctx:expr, $icon:expr) => {
        $ctx.Window().set_cursor_icon($icon)
    };
}

#[macro_export]
macro_rules! close_app {
    ($ctx:expr) => {
        $ctx.Window().close_app()
    };
}

#[macro_export]
macro_rules! window_get_active_refresh_rate {
    ($ctx:expr) => {
        $ctx.Window().get_active_refresh_rate()
    };
}
