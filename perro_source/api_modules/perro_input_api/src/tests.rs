use crate::{
    action_cancel_rebind, action_down, action_is_rebinding, action_pressed, action_rebind_result,
    action_released, action_start_rebind, mouse_mode, mouse_set_mode,
};

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

#[test]
fn clear_keyboard_mouse_state_releases_stale_inputs() {
    let mut input = InputSnapshot::new();
    input.set_input_map(InputMap::from_actions(vec![InputAction::new(
        "forward",
        vec![
            InputBinding::Key(KeyCode::KeyW),
            InputBinding::Mouse(MouseButton::Left),
        ],
    )]));
    input.set_mouse_mode_state(MouseMode::Captured);
    input.set_key_state(KeyCode::KeyW, true);
    input.set_mouse_button_state(MouseButton::Left, true);
    input.add_mouse_delta(6.0, -4.0);
    input.add_mouse_wheel(1.0, 2.0);

    input.clear_keyboard_mouse_state();

    let ctx = InputWindow::new(&input);
    assert!(!ctx.Keyboard().down(KeyCode::KeyW));
    assert!(!ctx.Mouse().down(MouseButton::Left));
    assert!(!ctx.Actions().down("forward"));
    assert!(!ctx.Actions().pressed("forward"));
    assert!(!ctx.Actions().released("forward"));
    assert_eq!(ctx.Mouse().delta().x, 0.0);
    assert_eq!(ctx.Mouse().delta().y, 0.0);
    assert_eq!(ctx.Mouse().wheel().x, 0.0);
    assert_eq!(ctx.Mouse().wheel().y, 0.0);
    assert_eq!(input.mouse_mode(), MouseMode::Captured);
}

#[test]
fn live_rebind_replaces_action_and_reports_result() {
    let mut input = InputSnapshot::new();
    input.set_input_map(InputMap::from_actions(vec![InputAction::new(
        "jump",
        vec![InputBinding::Key(KeyCode::Space)],
    )]));

    InputWindow::new(&input).Actions().start_rebind("jump");
    input.apply_queued_commands();
    assert!(InputWindow::new(&input).Actions().is_rebinding());

    input.set_mouse_button_state(MouseButton::Right, true);

    let window = InputWindow::new(&input);
    let actions = window.Actions();
    assert!(!actions.is_rebinding());
    assert_eq!(
        actions.rebind_result().map(|result| result.binding),
        Some(InputBinding::Mouse(MouseButton::Right))
    );
    assert_eq!(
        input.input_map().action("jump").unwrap().bindings,
        vec![InputBinding::Mouse(MouseButton::Right)]
    );
    assert!(actions.down("jump"));
}

#[test]
fn live_rebind_cancel_keeps_bindings() {
    let mut input = InputSnapshot::new();
    input.set_input_map(InputMap::from_actions(vec![InputAction::new(
        "jump",
        vec![InputBinding::Key(KeyCode::Space)],
    )]));

    let window = InputWindow::new(&input);
    let actions = window.Actions();
    actions.start_rebind("jump");
    actions.cancel_rebind();
    input.apply_queued_commands();
    input.set_key_state(KeyCode::Enter, true);

    assert_eq!(
        input.input_map().action("jump").unwrap().bindings,
        vec![InputBinding::Key(KeyCode::Space)]
    );
    assert!(input.rebind_result().is_none());
}

#[test]
fn live_rebind_macros_queue_query_and_report() {
    let mut input = InputSnapshot::new();
    input.set_input_map(InputMap::from_actions(vec![InputAction::new(
        "jump",
        vec![InputBinding::Key(KeyCode::Space)],
    )]));

    let window = InputWindow::new(&input);
    action_start_rebind!(&window, "jump");
    input.apply_queued_commands();
    let window = InputWindow::new(&input);
    assert!(action_is_rebinding!(&window));

    input.set_key_state(KeyCode::Enter, true);
    let window = InputWindow::new(&input);
    assert_eq!(
        action_rebind_result!(&window).map(|result| result.binding),
        Some(InputBinding::Key(KeyCode::Enter))
    );

    action_start_rebind!(&window, "jump");
    action_cancel_rebind!(&window);
    input.apply_queued_commands();
    assert!(!action_is_rebinding!(InputWindow::new(&input)));
}
