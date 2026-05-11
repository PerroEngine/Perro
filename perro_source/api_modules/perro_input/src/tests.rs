use crate::{mouse_mode, mouse_set_mode};

use super::{InputSnapshot, InputWindow, MouseMode};

#[test]
fn mouse_mode_defaults_visible() {
    let input = InputSnapshot::new();

    assert_eq!(input.mouse_mode(), MouseMode::Visible);
}

#[test]
fn mouse_mode_command_sets_state_and_request() {
    let mut input = InputSnapshot::new();
    {
        let ctx = InputWindow::new(&input);
        ctx.Mouse().capture();
    }

    input.apply_queued_commands();

    assert_eq!(input.mouse_mode(), MouseMode::Captured);
    assert_eq!(input.take_mouse_mode_request(), Some(MouseMode::Captured));
    assert_eq!(input.take_mouse_mode_request(), None);
}

#[test]
fn mouse_mode_macro_queues_request() {
    let mut input = InputSnapshot::new();
    {
        let ctx = InputWindow::new(&input);
        mouse_set_mode!(&ctx, MouseMode::Confined);
    }

    input.apply_queued_commands();

    assert_eq!(mouse_mode!(InputWindow::new(&input)), MouseMode::Confined);
    assert_eq!(input.take_mouse_mode_request(), Some(MouseMode::Confined));
}
