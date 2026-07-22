//! Display HDR state + control.

use crate::api::ViewportAPI;
use perro_render_bridge::{HdrMode, HdrStatus};

pub struct DisplayModule<'a, R: ViewportAPI + ?Sized> {
    api: &'a R,
}

impl<'a, R: ViewportAPI + ?Sized> DisplayModule<'a, R> {
    #[inline]
    pub const fn new(api: &'a R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn set_hdr_mode(&self, mode: HdrMode) {
        self.api.set_hdr_mode(mode);
    }

    #[inline]
    pub fn hdr_status(&self) -> HdrStatus {
        self.api.hdr_status()
    }

    #[inline]
    pub fn hdr_supported(&self) -> bool {
        self.hdr_status().supported
    }

    #[inline]
    pub fn hdr_active(&self) -> bool {
        self.hdr_status().active
    }
}

#[macro_export]
macro_rules! hdr_set {
    ($res:expr, $mode:expr) => {
        $res.set_hdr_mode($mode)
    };
}

#[macro_export]
macro_rules! hdr_status {
    ($res:expr) => {
        $res.hdr_status()
    };
}

#[macro_export]
macro_rules! hdr_supported {
    ($res:expr) => {
        $res.hdr_status().supported
    };
}

#[macro_export]
macro_rules! hdr_active {
    ($res:expr) => {
        $res.hdr_status().active
    };
}
