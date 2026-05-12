#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowMode {
    Windowed,
    BorderlessFullscreen,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowRequest {
    SetTitle(String),
    SetSize { width: u32, height: u32 },
    SetMode(WindowMode),
}

pub trait WindowAPI {
    fn set_window_title(&mut self, title: impl Into<String>);
    fn set_window_size(&mut self, width: u32, height: u32);
    fn set_window_mode(&mut self, mode: WindowMode);
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
