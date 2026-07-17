use super::*;

#[derive(Clone, Copy, Debug)]
struct UiFocusCandidate {
    node: NodeID,
    rect: ComputedUiRect,
}

struct UiButtonLikeHitData<'a> {
    disabled: bool,
    input_enabled: bool,
    mouse_filter: perro_ui::UiMouseFilter,
    input_mask: &'a perro_ui::UiInputMask,
    corner_radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UiInputSource {
    Kbm,
    Gamepad(usize),
    JoyCon(usize),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct UiDirectionalNav {
    source: UiInputSource,
    dir: [i8; 2],
}

#[path = "events/controls.rs"]
mod controls;
#[path = "events/focus.rs"]
mod focus;
#[path = "events/navigation.rs"]
mod navigation;
#[path = "events/pointer.rs"]
mod pointer;
#[path = "events/scroll.rs"]
mod scroll;

fn apply_scroll_delta(current: f32, delta: f32, max_scroll: f32) -> f32 {
    if delta.is_infinite() && delta.is_sign_negative() {
        0.0
    } else if delta.is_infinite() {
        max_scroll
    } else {
        (current + delta).clamp(0.0, max_scroll)
    }
}

fn compare_focus_visual_order(a: &UiFocusCandidate, b: &UiFocusCandidate) -> std::cmp::Ordering {
    b.rect
        .center
        .y
        .partial_cmp(&a.rect.center.y)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| {
            a.rect
                .center
                .x
                .partial_cmp(&b.rect.center.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| a.node.as_u64().cmp(&b.node.as_u64()))
}

fn ui_button_like_inactive(data: &SceneNodeData) -> Option<bool> {
    match data {
        SceneNodeData::UiButton(button) => Some(button_inactive(button)),
        SceneNodeData::UiDropdown(dropdown) => Some(button_inactive(&dropdown.button)),
        SceneNodeData::UiCheckbox(checkbox) => Some(checkbox_inactive(checkbox)),
        SceneNodeData::UiImageButton(button) => Some(image_button_inactive(button)),
        SceneNodeData::UiNineSliceButton(button) => Some(nine_slice_button_inactive(button)),
        _ => None,
    }
}

fn ui_button_like_custom_event_signals<'a>(
    data: &'a SceneNodeData,
    event: &str,
) -> Option<&'a [SignalID]> {
    match data {
        SceneNodeData::UiButton(button) => {
            (!button_inactive(button)).then_some(button_custom_event_signals(button, event))
        }
        SceneNodeData::UiDropdown(dropdown) => (!button_inactive(&dropdown.button))
            .then_some(button_custom_event_signals(&dropdown.button, event)),
        SceneNodeData::UiCheckbox(checkbox) => (!checkbox_inactive(checkbox))
            .then_some(button_custom_event_signals(&checkbox.button, event)),
        SceneNodeData::UiImageButton(button) => (!image_button_inactive(button))
            .then_some(image_button_custom_event_signals(button, event)),
        SceneNodeData::UiNineSliceButton(button) => (!nine_slice_button_inactive(button))
            .then_some(nine_slice_button_custom_event_signals(button, event)),
        _ => None,
    }
}

fn button_named_event(event: &str) -> &str {
    match event {
        "click" => "clicked",
        other => other,
    }
}

fn ui_button_like_hit_data(
    data: &SceneNodeData,
    state: UiButtonVisualState,
) -> Option<UiButtonLikeHitData<'_>> {
    match data {
        SceneNodeData::UiButton(button) => Some(UiButtonLikeHitData {
            disabled: button.disabled,
            input_enabled: button.input_enabled,
            mouse_filter: button.mouse_filter,
            input_mask: &button.input_mask,
            corner_radius: button_style(button, state).corner_radius(),
        }),
        SceneNodeData::UiDropdown(dropdown) => Some(UiButtonLikeHitData {
            disabled: dropdown.disabled,
            input_enabled: dropdown.input_enabled,
            mouse_filter: dropdown.mouse_filter,
            input_mask: &dropdown.input_mask,
            corner_radius: button_style(&dropdown.button, state).corner_radius(),
        }),
        SceneNodeData::UiCheckbox(checkbox) => Some(UiButtonLikeHitData {
            disabled: checkbox.disabled,
            input_enabled: checkbox.input_enabled,
            mouse_filter: checkbox.mouse_filter,
            input_mask: &checkbox.input_mask,
            corner_radius: checkbox_style(checkbox, state).corner_radius(),
        }),
        SceneNodeData::UiImageButton(button) => Some(UiButtonLikeHitData {
            disabled: button.disabled,
            input_enabled: button.input_enabled,
            mouse_filter: button.mouse_filter,
            input_mask: &button.input_mask,
            corner_radius: 0.0,
        }),
        SceneNodeData::UiNineSliceButton(button) => Some(UiButtonLikeHitData {
            disabled: button.disabled,
            input_enabled: button.input_enabled,
            mouse_filter: button.mouse_filter,
            input_mask: &button.input_mask,
            corner_radius: 0.0,
        }),
        _ => None,
    }
}

fn color_picker_color_at_point(
    wheel_rect: ComputedUiRect,
    mode: perro_ui::UiColorPickerMode,
    alpha: f32,
    point: Vector2,
) -> Option<Color> {
    let delta = point - wheel_rect.center;
    let radius = wheel_rect.size.x.min(wheel_rect.size.y).abs() * 0.5;
    if matches!(mode, perro_ui::UiColorPickerMode::Swatches) {
        let min = wheel_rect.min();
        let local_x = point.x - min.x;
        let local_y = point.y - min.y;
        if local_x < 0.0
            || local_y < 0.0
            || local_x >= wheel_rect.size.x
            || local_y >= wheel_rect.size.y
        {
            return None;
        }
        let col = (local_x / (wheel_rect.size.x / 6.0)).floor() as usize;
        let row_from_bottom = (local_y / (wheel_rect.size.y / 4.0)).floor() as usize;
        let row = 3usize.saturating_sub(row_from_bottom.min(3));
        let mut color = perro_ui::ui_color_picker_swatches()[row * 6 + col.min(5)];
        color = color.with_alpha(alpha);
        return Some(color);
    }
    let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
    if distance > radius {
        return None;
    }
    let mut hue = delta.y.atan2(delta.x).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    let mut saturation = (distance / radius).clamp(0.0, 1.0);
    if matches!(mode, perro_ui::UiColorPickerMode::BlockWheel) {
        hue = ((hue * 12.0).floor() + 0.5) / 12.0;
        saturation = ((saturation * 4.0).floor() + 0.5).min(4.0) / 4.0;
    }
    Some(hsv_color(hue, saturation, 1.0, alpha))
}

fn hsv_color(h: f32, s: f32, v: f32, a: f32) -> Color {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::new(r, g, b, a)
}

fn stick_nav_direction(stick: Vector2, threshold: f32) -> Option<[i8; 2]> {
    let ax = stick.x.abs();
    let ay = stick.y.abs();
    if ax < threshold && ay < threshold {
        return None;
    }
    if ax > ay {
        Some(if stick.x < 0.0 { [-1, 0] } else { [1, 0] })
    } else {
        Some(if stick.y < 0.0 { [0, -1] } else { [0, 1] })
    }
}

fn ui_scroll_keys() -> &'static [KeyCode] {
    &[
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Home,
        KeyCode::End,
    ]
}
