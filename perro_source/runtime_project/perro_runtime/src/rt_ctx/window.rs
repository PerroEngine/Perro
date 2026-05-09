use perro_runtime_context::sub_apis::{WindowAPI, WindowMode, WindowRequest};

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
}

impl Runtime {
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
            ]
        );
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
