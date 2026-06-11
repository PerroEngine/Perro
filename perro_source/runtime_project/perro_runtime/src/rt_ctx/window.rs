use perro_runtime_api::sub_apis::{CursorIcon, FrameRateCap, WindowAPI, WindowMode, WindowRequest};

use crate::Runtime;

impl WindowAPI for Runtime {
    fn set_window_title(&mut self, title: impl Into<String>) {
        self.window_requests
            .push(WindowRequest::SetTitle(title.into()));
    }

    fn set_window_size(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.window_requests
            .push(WindowRequest::SetSize { width, height });
    }

    fn set_window_mode(&mut self, mode: WindowMode) {
        self.window_requests.push(WindowRequest::SetMode(mode));
    }

    fn set_frame_rate_cap(&mut self, cap: FrameRateCap) {
        self.window_requests
            .push(WindowRequest::SetFrameRateCap(cap));
    }

    fn set_cursor_icon(&mut self, icon: CursorIcon) {
        self.window_requests
            .push(WindowRequest::SetCursorIcon(icon));
    }

    fn get_active_refresh_rate(&mut self) -> Option<f32> {
        self.active_refresh_rate()
    }
}

impl Runtime {
    #[inline]
    pub fn set_active_refresh_rate(&mut self, refresh_rate: Option<f32>) {
        self.active_refresh_rate = refresh_rate.filter(|v| v.is_finite() && *v > 0.0);
    }

    #[inline]
    pub fn active_refresh_rate(&self) -> Option<f32> {
        self.active_refresh_rate
    }

    #[inline]
    pub fn drain_window_requests(&mut self, out: &mut Vec<WindowRequest>) {
        out.append(&mut self.window_requests);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_api_queues_requests_in_order() {
        let mut runtime = Runtime::new();
        runtime.set_window_title("Play");
        runtime.set_window_size(800, 600);
        runtime.set_window_mode(WindowMode::BorderlessFullscreen);
        runtime.set_frame_rate_cap(FrameRateCap::Fps(120.0));
        runtime.set_cursor_icon(CursorIcon::Move);
        runtime.set_active_refresh_rate(Some(144.0));

        let mut requests = Vec::new();
        runtime.drain_window_requests(&mut requests);

        assert_eq!(
            requests,
            vec![
                WindowRequest::SetTitle("Play".to_string()),
                WindowRequest::SetSize {
                    width: 800,
                    height: 600
                },
                WindowRequest::SetMode(WindowMode::BorderlessFullscreen),
                WindowRequest::SetFrameRateCap(FrameRateCap::Fps(120.0)),
                WindowRequest::SetCursorIcon(CursorIcon::Move),
            ]
        );
        assert_eq!(runtime.get_active_refresh_rate(), Some(144.0));
    }

    #[test]
    fn window_size_ignores_zero_dimensions() {
        let mut runtime = Runtime::new();
        runtime.set_window_size(0, 600);
        runtime.set_window_size(800, 0);

        let mut requests = Vec::new();
        runtime.drain_window_requests(&mut requests);

        assert!(requests.is_empty());
    }
}
