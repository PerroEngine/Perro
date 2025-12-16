//! Input Manager - handles keyboard, mouse, and input action mapping

use crate::structs2d::vector2::Vector2;
use std::collections::{HashMap, HashSet};
use winit::keyboard::KeyCode;

/// Represents a single input source (keyboard key, mouse button, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputSource {
    Key(KeyCode),
    MouseButton(MouseButton),
    MouseWheelUp,
    MouseWheelDown,
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

/// Current input state
#[derive(Default)]
pub struct InputState {
    /// Currently pressed keys
    pub keys_pressed: HashSet<KeyCode>,
    /// Currently pressed mouse buttons
    pub mouse_buttons_pressed: HashSet<MouseButton>,
    /// Mouse position in screen coordinates (pixels)
    pub mouse_position: Vector2,
    /// Mouse position in world coordinates (if camera is set up)
    pub mouse_position_world: Option<Vector2>,
    /// Scroll wheel delta (accumulated this frame)
    pub scroll_delta: f32,
    /// Whether mouse wheel scrolled up this frame
    pub mouse_wheel_up: bool,
    /// Whether mouse wheel scrolled down this frame
    pub mouse_wheel_down: bool,
    /// Text input buffer (for text input events)
    pub text_input: String,
}

/// Input action mapping - maps action names to input sources
pub type InputMap = HashMap<String, Vec<InputSource>>;

/// Input Manager - tracks input state and action mappings
pub struct InputManager {
    state: InputState,
    action_map: InputMap,
}

impl InputManager {
    pub fn new() -> Self {
        Self {
            state: InputState::default(),
            action_map: HashMap::new(),
        }
    }

    /// Load input mappings from a map
    pub fn load_action_map(&mut self, map: InputMap) {
        self.action_map = map;
    }

    /// Get the current input state
    pub fn state(&self) -> &InputState {
        &self.state
    }

    /// Get mutable input state
    pub fn state_mut(&mut self) -> &mut InputState {
        &mut self.state
    }

    /// Check if an action is currently pressed
    pub fn is_action_pressed(&self, action: &str) -> bool {
        if let Some(sources) = self.action_map.get(action) {
            sources.iter().any(|source| match source {
                InputSource::Key(key) => self.state.keys_pressed.contains(key),
                InputSource::MouseButton(btn) => self.state.mouse_buttons_pressed.contains(btn),
                InputSource::MouseWheelUp => self.state.mouse_wheel_up,
                InputSource::MouseWheelDown => self.state.mouse_wheel_down,
            })
        } else {
            false
        }
    }

    /// Check if a key is pressed (raw key access)
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.state.keys_pressed.contains(&key)
    }

    /// Check if a mouse button is pressed (raw button access)
    pub fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.state.mouse_buttons_pressed.contains(&button)
    }

    /// Get mouse position in screen space
    pub fn get_mouse_position(&self) -> Vector2 {
        self.state.mouse_position
    }

    /// Get mouse position in world space (if available)
    pub fn get_mouse_position_world(&self) -> Option<Vector2> {
        self.state.mouse_position_world
    }

    /// Get scroll wheel delta
    pub fn get_scroll_delta(&self) -> f32 {
        self.state.scroll_delta
    }

    /// Get text input buffer
    pub fn get_text_input(&self) -> &str {
        &self.state.text_input
    }

    /// Clear text input buffer (call after processing)
    pub fn clear_text_input(&mut self) {
        self.state.text_input.clear();
    }

    /// Check if mouse wheel scrolled up this frame
    pub fn is_mouse_wheel_up(&self) -> bool {
        self.state.mouse_wheel_up
    }

    /// Check if mouse wheel scrolled down this frame
    pub fn is_mouse_wheel_down(&self) -> bool {
        self.state.mouse_wheel_down
    }

    /// Update scroll delta (call at end of frame to reset)
    pub fn reset_scroll_delta(&mut self) {
        self.state.scroll_delta = 0.0;
        self.state.mouse_wheel_up = false;
        self.state.mouse_wheel_down = false;
    }

    /// Convert screen coordinates to world coordinates using camera transform
    /// For 2D cameras: takes camera position, rotation, zoom, and virtual screen size
    pub fn screen_to_world_2d(
        &self,
        screen_pos: Vector2,
        camera_pos: Vector2,
        camera_rotation: f32,
        camera_zoom: f32,
        virtual_width: f32,
        virtual_height: f32,
        window_width: f32,
        window_height: f32,
    ) -> Vector2 {
        // Convert screen pixel coordinates to virtual coordinates
        let virtual_aspect = virtual_width / virtual_height;
        let window_aspect = window_width / window_height;

        let (scale_x, scale_y) = if window_aspect > virtual_aspect {
            (virtual_aspect / window_aspect, 1.0)
        } else {
            (1.0, window_aspect / virtual_aspect)
        };

        // Normalize screen position to [0, 1]
        let normalized_x = screen_pos.x / window_width;
        let normalized_y = screen_pos.y / window_height;

        // Convert to virtual space coordinates
        let virtual_x = (normalized_x - 0.5) * virtual_width * scale_x;
        let virtual_y = (normalized_y - 0.5) * virtual_height * scale_y;

        // Apply camera zoom
        let zoomed_x = virtual_x / camera_zoom;
        let zoomed_y = virtual_y / camera_zoom;

        // Rotate around origin
        let cos_r = camera_rotation.cos();
        let sin_r = camera_rotation.sin();
        let rotated_x = zoomed_x * cos_r - zoomed_y * sin_r;
        let rotated_y = zoomed_x * sin_r + zoomed_y * cos_r;

        // Translate by camera position
        Vector2::new(rotated_x + camera_pos.x, rotated_y + camera_pos.y)
    }

    /// Handle key press
    pub fn handle_key_press(&mut self, key: KeyCode) {
        self.state.keys_pressed.insert(key);
    }

    /// Handle key release
    pub fn handle_key_release(&mut self, key: KeyCode) {
        self.state.keys_pressed.remove(&key);
    }

    /// Handle mouse button press
    pub fn handle_mouse_button_press(&mut self, button: MouseButton) {
        self.state.mouse_buttons_pressed.insert(button);
    }

    /// Handle mouse button release
    pub fn handle_mouse_button_release(&mut self, button: MouseButton) {
        self.state.mouse_buttons_pressed.remove(&button);
    }

    /// Handle mouse movement
    pub fn handle_mouse_move(&mut self, position: Vector2) {
        self.state.mouse_position = position;
    }

    /// Handle scroll wheel
    pub fn handle_scroll(&mut self, delta: f32) {
        self.state.scroll_delta += delta;
        if delta > 0.0 {
            self.state.mouse_wheel_up = true;
        } else if delta < 0.0 {
            self.state.mouse_wheel_down = true;
        }
    }

    /// Handle text input
    pub fn handle_text_input(&mut self, text: String) {
        self.state.text_input.push_str(&text);
    }
}

impl Default for InputManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse an input source string from project.toml
/// Examples: "Space", "KeyW", "MouseLeft", "MouseWheelUp"
pub fn parse_input_source(s: &str) -> Option<InputSource> {
    // Mouse buttons
    if s.starts_with("Mouse") {
        return match s {
            "MouseLeft" => Some(InputSource::MouseButton(MouseButton::Left)),
            "MouseRight" => Some(InputSource::MouseButton(MouseButton::Right)),
            "MouseMiddle" => Some(InputSource::MouseButton(MouseButton::Middle)),
            "MouseWheelUp" => Some(InputSource::MouseWheelUp),
            "MouseWheelDown" => Some(InputSource::MouseWheelDown),
            _ => None,
        };
    }

    // Keyboard keys - try to parse as KeyCode
    // Common key names
    let key = match s {
        "Space" => KeyCode::Space,
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Escape,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Up" => KeyCode::ArrowUp,
        "Down" => KeyCode::ArrowDown,
        "Left" => KeyCode::ArrowLeft,
        "Right" => KeyCode::ArrowRight,
        "Shift" => KeyCode::ShiftLeft, // Default to left shift
        "Control" => KeyCode::ControlLeft,
        "Alt" => KeyCode::AltLeft,
        "Meta" => KeyCode::SuperLeft,
        _ => {
            // Try parsing as "KeyX" format
            if s.starts_with("Key") && s.len() == 4 {
                let ch = s.chars().nth(3)?;
                if ch.is_ascii_alphabetic() {
                    match ch.to_ascii_uppercase() {
                        'A' => KeyCode::KeyA,
                        'B' => KeyCode::KeyB,
                        'C' => KeyCode::KeyC,
                        'D' => KeyCode::KeyD,
                        'E' => KeyCode::KeyE,
                        'F' => KeyCode::KeyF,
                        'G' => KeyCode::KeyG,
                        'H' => KeyCode::KeyH,
                        'I' => KeyCode::KeyI,
                        'J' => KeyCode::KeyJ,
                        'K' => KeyCode::KeyK,
                        'L' => KeyCode::KeyL,
                        'M' => KeyCode::KeyM,
                        'N' => KeyCode::KeyN,
                        'O' => KeyCode::KeyO,
                        'P' => KeyCode::KeyP,
                        'Q' => KeyCode::KeyQ,
                        'R' => KeyCode::KeyR,
                        'S' => KeyCode::KeyS,
                        'T' => KeyCode::KeyT,
                        'U' => KeyCode::KeyU,
                        'V' => KeyCode::KeyV,
                        'W' => KeyCode::KeyW,
                        'X' => KeyCode::KeyX,
                        'Y' => KeyCode::KeyY,
                        'Z' => KeyCode::KeyZ,
                        _ => return None,
                    }
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    };

    Some(InputSource::Key(key))
}
