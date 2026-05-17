use crate::{action_down, action_pressed, action_released, mouse_mode, mouse_set_mode};

use super::{
    GamepadButton, InputAction, InputBinding, InputMap, InputSnapshot, InputWindow, JoyConButton,
    KeyCode, MouseButton, MouseMode, action_hash,
};

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

#[test]
fn action_queries_match_any_binding() {
    let mut input = InputSnapshot::new();
    input.set_input_map(InputMap::from_actions(vec![InputAction::new(
        "jump",
        vec![
            InputBinding::Key(KeyCode::Space),
            InputBinding::Mouse(MouseButton::Left),
            InputBinding::Gamepad(GamepadButton::Bottom),
            InputBinding::JoyCon(JoyConButton::Bottom),
        ],
    )]));

    input.set_key_state(KeyCode::Space, true);

    let ctx = InputWindow::new(&input);
    assert!(ctx.Actions().down("jump"));
    assert!(ctx.Actions().pressed("jump"));
    assert!(action_down!(&ctx, "jump"));
    assert!(action_pressed!(&ctx, "jump"));
    assert!(!ctx.Actions().released("jump"));

    input.begin_frame();
    input.set_key_state(KeyCode::Space, false);
    let ctx = InputWindow::new(&input);
    assert!(ctx.Actions().released("jump"));
    assert!(action_released!(&ctx, "jump"));

    input.begin_frame();
    input.set_gamepad_button_state(2, GamepadButton::Bottom, true);
    let ctx = InputWindow::new(&input);
    assert!(ctx.Actions().down("jump"));

    input.begin_frame();
    input.set_joycon_button_state(0, JoyConButton::Bottom, true);
    let ctx = InputWindow::new(&input);
    assert!(ctx.Actions().pressed_hash(action_hash("jump")));
}
