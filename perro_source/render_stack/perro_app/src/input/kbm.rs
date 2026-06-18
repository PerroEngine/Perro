use crate::App;
use perro_graphics::GraphicsBackend;
use winit::{
    event::{
        ElementState, Modifiers, MouseButton as WinitMouseButton, MouseScrollDelta, WindowEvent,
    },
    keyboard::{ModifiersState, PhysicalKey},
};

pub struct KbmInput {
    last_cursor_position: Option<winit::dpi::PhysicalPosition<f64>>,
    modifiers: ModifiersState,
}

impl KbmInput {
    pub fn new() -> Self {
        Self {
            last_cursor_position: None,
            modifiers: ModifiersState::empty(),
        }
    }

    pub fn handle_window_event<B: GraphicsBackend>(
        &mut self,
        app: &mut App<B>,
        event: &WindowEvent,
    ) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key
                    && let Some(key) = map_winit_key_code(code)
                {
                    app.set_key_state(key, event.state == ElementState::Pressed);
                }
                if event.state == ElementState::Pressed
                    && !self.text_input_suppressed()
                    && let Some(text) = event.text.as_ref()
                    && text.chars().any(|ch| !ch.is_control())
                {
                    app.push_text_input(text.to_string());
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::Focused(false) => {
                self.modifiers = Modifiers::default().state();
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                app.push_text_input(text.clone());
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(mapped) = map_winit_mouse_button(*button) {
                    app.set_mouse_button_state(mapped, *state == ElementState::Pressed);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(prev) = self.last_cursor_position {
                    let dx = (position.x - prev.x) as f32;
                    let dy = (prev.y - position.y) as f32;
                    app.add_mouse_delta(dx, dy);
                }
                app.set_mouse_position(position.x as f32, position.y as f32);
                self.last_cursor_position = Some(*position);
            }
            WindowEvent::CursorLeft { .. } => {
                self.last_cursor_position = None;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(pos) => {
                        ((pos.x as f32) / 40.0, (pos.y as f32) / 40.0)
                    }
                };
                app.add_mouse_wheel(dx, dy);
            }
            _ => {}
        }
    }

    pub fn handle_mouse_motion<B: GraphicsBackend>(
        &mut self,
        app: &mut App<B>,
        delta_x: f64,
        delta_y: f64,
    ) {
        let dx = delta_x as f32;
        let dy = -delta_y as f32;
        app.add_mouse_delta(dx, dy);
    }

    pub fn reset_cursor_position(&mut self) {
        self.last_cursor_position = None;
    }

    pub fn translate_cursor_position(&mut self, dx: f64, dy: f64) {
        if let Some(position) = &mut self.last_cursor_position {
            position.x += dx;
            position.y += dy;
        }
    }

    pub fn last_cursor_position(&self) -> Option<winit::dpi::PhysicalPosition<f64>> {
        self.last_cursor_position
    }

    fn text_input_suppressed(&self) -> bool {
        self.modifiers.control_key() || self.modifiers.alt_key() || self.modifiers.super_key()
    }
}

impl Default for KbmInput {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn map_winit_key_code(
    code: winit::keyboard::KeyCode,
) -> Option<perro_input_api::KeyCode> {
    match code {
        winit::keyboard::KeyCode::Backquote => Some(perro_input_api::KeyCode::Backquote),
        winit::keyboard::KeyCode::Backslash => Some(perro_input_api::KeyCode::Backslash),
        winit::keyboard::KeyCode::BracketLeft => Some(perro_input_api::KeyCode::BracketLeft),
        winit::keyboard::KeyCode::BracketRight => Some(perro_input_api::KeyCode::BracketRight),
        winit::keyboard::KeyCode::Comma => Some(perro_input_api::KeyCode::Comma),
        winit::keyboard::KeyCode::Digit0 => Some(perro_input_api::KeyCode::Digit0),
        winit::keyboard::KeyCode::Digit1 => Some(perro_input_api::KeyCode::Digit1),
        winit::keyboard::KeyCode::Digit2 => Some(perro_input_api::KeyCode::Digit2),
        winit::keyboard::KeyCode::Digit3 => Some(perro_input_api::KeyCode::Digit3),
        winit::keyboard::KeyCode::Digit4 => Some(perro_input_api::KeyCode::Digit4),
        winit::keyboard::KeyCode::Digit5 => Some(perro_input_api::KeyCode::Digit5),
        winit::keyboard::KeyCode::Digit6 => Some(perro_input_api::KeyCode::Digit6),
        winit::keyboard::KeyCode::Digit7 => Some(perro_input_api::KeyCode::Digit7),
        winit::keyboard::KeyCode::Digit8 => Some(perro_input_api::KeyCode::Digit8),
        winit::keyboard::KeyCode::Digit9 => Some(perro_input_api::KeyCode::Digit9),
        winit::keyboard::KeyCode::Equal => Some(perro_input_api::KeyCode::Equal),
        winit::keyboard::KeyCode::IntlBackslash => Some(perro_input_api::KeyCode::IntlBackslash),
        winit::keyboard::KeyCode::IntlRo => Some(perro_input_api::KeyCode::IntlRo),
        winit::keyboard::KeyCode::IntlYen => Some(perro_input_api::KeyCode::IntlYen),
        winit::keyboard::KeyCode::KeyA => Some(perro_input_api::KeyCode::KeyA),
        winit::keyboard::KeyCode::KeyB => Some(perro_input_api::KeyCode::KeyB),
        winit::keyboard::KeyCode::KeyC => Some(perro_input_api::KeyCode::KeyC),
        winit::keyboard::KeyCode::KeyD => Some(perro_input_api::KeyCode::KeyD),
        winit::keyboard::KeyCode::KeyE => Some(perro_input_api::KeyCode::KeyE),
        winit::keyboard::KeyCode::KeyF => Some(perro_input_api::KeyCode::KeyF),
        winit::keyboard::KeyCode::KeyG => Some(perro_input_api::KeyCode::KeyG),
        winit::keyboard::KeyCode::KeyH => Some(perro_input_api::KeyCode::KeyH),
        winit::keyboard::KeyCode::KeyI => Some(perro_input_api::KeyCode::KeyI),
        winit::keyboard::KeyCode::KeyJ => Some(perro_input_api::KeyCode::KeyJ),
        winit::keyboard::KeyCode::KeyK => Some(perro_input_api::KeyCode::KeyK),
        winit::keyboard::KeyCode::KeyL => Some(perro_input_api::KeyCode::KeyL),
        winit::keyboard::KeyCode::KeyM => Some(perro_input_api::KeyCode::KeyM),
        winit::keyboard::KeyCode::KeyN => Some(perro_input_api::KeyCode::KeyN),
        winit::keyboard::KeyCode::KeyO => Some(perro_input_api::KeyCode::KeyO),
        winit::keyboard::KeyCode::KeyP => Some(perro_input_api::KeyCode::KeyP),
        winit::keyboard::KeyCode::KeyQ => Some(perro_input_api::KeyCode::KeyQ),
        winit::keyboard::KeyCode::KeyR => Some(perro_input_api::KeyCode::KeyR),
        winit::keyboard::KeyCode::KeyS => Some(perro_input_api::KeyCode::KeyS),
        winit::keyboard::KeyCode::KeyT => Some(perro_input_api::KeyCode::KeyT),
        winit::keyboard::KeyCode::KeyU => Some(perro_input_api::KeyCode::KeyU),
        winit::keyboard::KeyCode::KeyV => Some(perro_input_api::KeyCode::KeyV),
        winit::keyboard::KeyCode::KeyW => Some(perro_input_api::KeyCode::KeyW),
        winit::keyboard::KeyCode::KeyX => Some(perro_input_api::KeyCode::KeyX),
        winit::keyboard::KeyCode::KeyY => Some(perro_input_api::KeyCode::KeyY),
        winit::keyboard::KeyCode::KeyZ => Some(perro_input_api::KeyCode::KeyZ),
        winit::keyboard::KeyCode::Minus => Some(perro_input_api::KeyCode::Minus),
        winit::keyboard::KeyCode::Period => Some(perro_input_api::KeyCode::Period),
        winit::keyboard::KeyCode::Quote => Some(perro_input_api::KeyCode::Quote),
        winit::keyboard::KeyCode::Semicolon => Some(perro_input_api::KeyCode::Semicolon),
        winit::keyboard::KeyCode::Slash => Some(perro_input_api::KeyCode::Slash),
        winit::keyboard::KeyCode::AltLeft => Some(perro_input_api::KeyCode::AltLeft),
        winit::keyboard::KeyCode::AltRight => Some(perro_input_api::KeyCode::AltRight),
        winit::keyboard::KeyCode::Backspace => Some(perro_input_api::KeyCode::Backspace),
        winit::keyboard::KeyCode::CapsLock => Some(perro_input_api::KeyCode::CapsLock),
        winit::keyboard::KeyCode::ContextMenu => Some(perro_input_api::KeyCode::ContextMenu),
        winit::keyboard::KeyCode::ControlLeft => Some(perro_input_api::KeyCode::ControlLeft),
        winit::keyboard::KeyCode::ControlRight => Some(perro_input_api::KeyCode::ControlRight),
        winit::keyboard::KeyCode::Enter => Some(perro_input_api::KeyCode::Enter),
        winit::keyboard::KeyCode::SuperLeft => Some(perro_input_api::KeyCode::SuperLeft),
        winit::keyboard::KeyCode::SuperRight => Some(perro_input_api::KeyCode::SuperRight),
        winit::keyboard::KeyCode::ShiftLeft => Some(perro_input_api::KeyCode::ShiftLeft),
        winit::keyboard::KeyCode::ShiftRight => Some(perro_input_api::KeyCode::ShiftRight),
        winit::keyboard::KeyCode::Space => Some(perro_input_api::KeyCode::Space),
        winit::keyboard::KeyCode::Tab => Some(perro_input_api::KeyCode::Tab),
        winit::keyboard::KeyCode::Convert => Some(perro_input_api::KeyCode::Convert),
        winit::keyboard::KeyCode::KanaMode => Some(perro_input_api::KeyCode::KanaMode),
        winit::keyboard::KeyCode::Lang1 => Some(perro_input_api::KeyCode::Lang1),
        winit::keyboard::KeyCode::Lang2 => Some(perro_input_api::KeyCode::Lang2),
        winit::keyboard::KeyCode::Lang3 => Some(perro_input_api::KeyCode::Lang3),
        winit::keyboard::KeyCode::Lang4 => Some(perro_input_api::KeyCode::Lang4),
        winit::keyboard::KeyCode::Lang5 => Some(perro_input_api::KeyCode::Lang5),
        winit::keyboard::KeyCode::NonConvert => Some(perro_input_api::KeyCode::NonConvert),
        winit::keyboard::KeyCode::Delete => Some(perro_input_api::KeyCode::Delete),
        winit::keyboard::KeyCode::End => Some(perro_input_api::KeyCode::End),
        winit::keyboard::KeyCode::Help => Some(perro_input_api::KeyCode::Help),
        winit::keyboard::KeyCode::Home => Some(perro_input_api::KeyCode::Home),
        winit::keyboard::KeyCode::Insert => Some(perro_input_api::KeyCode::Insert),
        winit::keyboard::KeyCode::PageDown => Some(perro_input_api::KeyCode::PageDown),
        winit::keyboard::KeyCode::PageUp => Some(perro_input_api::KeyCode::PageUp),
        winit::keyboard::KeyCode::ArrowDown => Some(perro_input_api::KeyCode::ArrowDown),
        winit::keyboard::KeyCode::ArrowLeft => Some(perro_input_api::KeyCode::ArrowLeft),
        winit::keyboard::KeyCode::ArrowRight => Some(perro_input_api::KeyCode::ArrowRight),
        winit::keyboard::KeyCode::ArrowUp => Some(perro_input_api::KeyCode::ArrowUp),
        winit::keyboard::KeyCode::NumLock => Some(perro_input_api::KeyCode::NumLock),
        winit::keyboard::KeyCode::Numpad0 => Some(perro_input_api::KeyCode::Numpad0),
        winit::keyboard::KeyCode::Numpad1 => Some(perro_input_api::KeyCode::Numpad1),
        winit::keyboard::KeyCode::Numpad2 => Some(perro_input_api::KeyCode::Numpad2),
        winit::keyboard::KeyCode::Numpad3 => Some(perro_input_api::KeyCode::Numpad3),
        winit::keyboard::KeyCode::Numpad4 => Some(perro_input_api::KeyCode::Numpad4),
        winit::keyboard::KeyCode::Numpad5 => Some(perro_input_api::KeyCode::Numpad5),
        winit::keyboard::KeyCode::Numpad6 => Some(perro_input_api::KeyCode::Numpad6),
        winit::keyboard::KeyCode::Numpad7 => Some(perro_input_api::KeyCode::Numpad7),
        winit::keyboard::KeyCode::Numpad8 => Some(perro_input_api::KeyCode::Numpad8),
        winit::keyboard::KeyCode::Numpad9 => Some(perro_input_api::KeyCode::Numpad9),
        winit::keyboard::KeyCode::NumpadAdd => Some(perro_input_api::KeyCode::NumpadAdd),
        winit::keyboard::KeyCode::NumpadBackspace => {
            Some(perro_input_api::KeyCode::NumpadBackspace)
        }
        winit::keyboard::KeyCode::NumpadClear => Some(perro_input_api::KeyCode::NumpadClear),
        winit::keyboard::KeyCode::NumpadClearEntry => {
            Some(perro_input_api::KeyCode::NumpadClearEntry)
        }
        winit::keyboard::KeyCode::NumpadComma => Some(perro_input_api::KeyCode::NumpadComma),
        winit::keyboard::KeyCode::NumpadDecimal => Some(perro_input_api::KeyCode::NumpadDecimal),
        winit::keyboard::KeyCode::NumpadDivide => Some(perro_input_api::KeyCode::NumpadDivide),
        winit::keyboard::KeyCode::NumpadEnter => Some(perro_input_api::KeyCode::NumpadEnter),
        winit::keyboard::KeyCode::NumpadEqual => Some(perro_input_api::KeyCode::NumpadEqual),
        winit::keyboard::KeyCode::NumpadHash => Some(perro_input_api::KeyCode::NumpadHash),
        winit::keyboard::KeyCode::NumpadMemoryAdd => {
            Some(perro_input_api::KeyCode::NumpadMemoryAdd)
        }
        winit::keyboard::KeyCode::NumpadMemoryClear => {
            Some(perro_input_api::KeyCode::NumpadMemoryClear)
        }
        winit::keyboard::KeyCode::NumpadMemoryRecall => {
            Some(perro_input_api::KeyCode::NumpadMemoryRecall)
        }
        winit::keyboard::KeyCode::NumpadMemoryStore => {
            Some(perro_input_api::KeyCode::NumpadMemoryStore)
        }
        winit::keyboard::KeyCode::NumpadMemorySubtract => {
            Some(perro_input_api::KeyCode::NumpadMemorySubtract)
        }
        winit::keyboard::KeyCode::NumpadMultiply => Some(perro_input_api::KeyCode::NumpadMultiply),
        winit::keyboard::KeyCode::NumpadParenLeft => {
            Some(perro_input_api::KeyCode::NumpadParenLeft)
        }
        winit::keyboard::KeyCode::NumpadParenRight => {
            Some(perro_input_api::KeyCode::NumpadParenRight)
        }
        winit::keyboard::KeyCode::NumpadStar => Some(perro_input_api::KeyCode::NumpadStar),
        winit::keyboard::KeyCode::NumpadSubtract => Some(perro_input_api::KeyCode::NumpadSubtract),
        winit::keyboard::KeyCode::Escape => Some(perro_input_api::KeyCode::Escape),
        winit::keyboard::KeyCode::Fn => Some(perro_input_api::KeyCode::Fn),
        winit::keyboard::KeyCode::FnLock => Some(perro_input_api::KeyCode::FnLock),
        winit::keyboard::KeyCode::PrintScreen => Some(perro_input_api::KeyCode::PrintScreen),
        winit::keyboard::KeyCode::ScrollLock => Some(perro_input_api::KeyCode::ScrollLock),
        winit::keyboard::KeyCode::Pause => Some(perro_input_api::KeyCode::Pause),
        winit::keyboard::KeyCode::BrowserBack => Some(perro_input_api::KeyCode::BrowserBack),
        winit::keyboard::KeyCode::BrowserFavorites => {
            Some(perro_input_api::KeyCode::BrowserFavorites)
        }
        winit::keyboard::KeyCode::BrowserForward => Some(perro_input_api::KeyCode::BrowserForward),
        winit::keyboard::KeyCode::BrowserHome => Some(perro_input_api::KeyCode::BrowserHome),
        winit::keyboard::KeyCode::BrowserRefresh => Some(perro_input_api::KeyCode::BrowserRefresh),
        winit::keyboard::KeyCode::BrowserSearch => Some(perro_input_api::KeyCode::BrowserSearch),
        winit::keyboard::KeyCode::BrowserStop => Some(perro_input_api::KeyCode::BrowserStop),
        winit::keyboard::KeyCode::Eject => Some(perro_input_api::KeyCode::Eject),
        winit::keyboard::KeyCode::LaunchApp1 => Some(perro_input_api::KeyCode::LaunchApp1),
        winit::keyboard::KeyCode::LaunchApp2 => Some(perro_input_api::KeyCode::LaunchApp2),
        winit::keyboard::KeyCode::LaunchMail => Some(perro_input_api::KeyCode::LaunchMail),
        winit::keyboard::KeyCode::MediaPlayPause => Some(perro_input_api::KeyCode::MediaPlayPause),
        winit::keyboard::KeyCode::MediaSelect => Some(perro_input_api::KeyCode::MediaSelect),
        winit::keyboard::KeyCode::MediaStop => Some(perro_input_api::KeyCode::MediaStop),
        winit::keyboard::KeyCode::MediaTrackNext => Some(perro_input_api::KeyCode::MediaTrackNext),
        winit::keyboard::KeyCode::MediaTrackPrevious => {
            Some(perro_input_api::KeyCode::MediaTrackPrevious)
        }
        winit::keyboard::KeyCode::Power => Some(perro_input_api::KeyCode::Power),
        winit::keyboard::KeyCode::Sleep => Some(perro_input_api::KeyCode::Sleep),
        winit::keyboard::KeyCode::AudioVolumeDown => {
            Some(perro_input_api::KeyCode::AudioVolumeDown)
        }
        winit::keyboard::KeyCode::AudioVolumeMute => {
            Some(perro_input_api::KeyCode::AudioVolumeMute)
        }
        winit::keyboard::KeyCode::AudioVolumeUp => Some(perro_input_api::KeyCode::AudioVolumeUp),
        winit::keyboard::KeyCode::WakeUp => Some(perro_input_api::KeyCode::WakeUp),
        winit::keyboard::KeyCode::Meta => Some(perro_input_api::KeyCode::Meta),
        winit::keyboard::KeyCode::Hyper => Some(perro_input_api::KeyCode::Hyper),
        winit::keyboard::KeyCode::Turbo => Some(perro_input_api::KeyCode::Turbo),
        winit::keyboard::KeyCode::Abort => Some(perro_input_api::KeyCode::Abort),
        winit::keyboard::KeyCode::Resume => Some(perro_input_api::KeyCode::Resume),
        winit::keyboard::KeyCode::Suspend => Some(perro_input_api::KeyCode::Suspend),
        winit::keyboard::KeyCode::Again => Some(perro_input_api::KeyCode::Again),
        winit::keyboard::KeyCode::Copy => Some(perro_input_api::KeyCode::Copy),
        winit::keyboard::KeyCode::Cut => Some(perro_input_api::KeyCode::Cut),
        winit::keyboard::KeyCode::Find => Some(perro_input_api::KeyCode::Find),
        winit::keyboard::KeyCode::Open => Some(perro_input_api::KeyCode::Open),
        winit::keyboard::KeyCode::Paste => Some(perro_input_api::KeyCode::Paste),
        winit::keyboard::KeyCode::Props => Some(perro_input_api::KeyCode::Props),
        winit::keyboard::KeyCode::Select => Some(perro_input_api::KeyCode::Select),
        winit::keyboard::KeyCode::Undo => Some(perro_input_api::KeyCode::Undo),
        winit::keyboard::KeyCode::Hiragana => Some(perro_input_api::KeyCode::Hiragana),
        winit::keyboard::KeyCode::Katakana => Some(perro_input_api::KeyCode::Katakana),
        winit::keyboard::KeyCode::F1 => Some(perro_input_api::KeyCode::F1),
        winit::keyboard::KeyCode::F2 => Some(perro_input_api::KeyCode::F2),
        winit::keyboard::KeyCode::F3 => Some(perro_input_api::KeyCode::F3),
        winit::keyboard::KeyCode::F4 => Some(perro_input_api::KeyCode::F4),
        winit::keyboard::KeyCode::F5 => Some(perro_input_api::KeyCode::F5),
        winit::keyboard::KeyCode::F6 => Some(perro_input_api::KeyCode::F6),
        winit::keyboard::KeyCode::F7 => Some(perro_input_api::KeyCode::F7),
        winit::keyboard::KeyCode::F8 => Some(perro_input_api::KeyCode::F8),
        winit::keyboard::KeyCode::F9 => Some(perro_input_api::KeyCode::F9),
        winit::keyboard::KeyCode::F10 => Some(perro_input_api::KeyCode::F10),
        winit::keyboard::KeyCode::F11 => Some(perro_input_api::KeyCode::F11),
        winit::keyboard::KeyCode::F12 => Some(perro_input_api::KeyCode::F12),
        winit::keyboard::KeyCode::F13 => Some(perro_input_api::KeyCode::F13),
        winit::keyboard::KeyCode::F14 => Some(perro_input_api::KeyCode::F14),
        winit::keyboard::KeyCode::F15 => Some(perro_input_api::KeyCode::F15),
        winit::keyboard::KeyCode::F16 => Some(perro_input_api::KeyCode::F16),
        winit::keyboard::KeyCode::F17 => Some(perro_input_api::KeyCode::F17),
        winit::keyboard::KeyCode::F18 => Some(perro_input_api::KeyCode::F18),
        winit::keyboard::KeyCode::F19 => Some(perro_input_api::KeyCode::F19),
        winit::keyboard::KeyCode::F20 => Some(perro_input_api::KeyCode::F20),
        winit::keyboard::KeyCode::F21 => Some(perro_input_api::KeyCode::F21),
        winit::keyboard::KeyCode::F22 => Some(perro_input_api::KeyCode::F22),
        winit::keyboard::KeyCode::F23 => Some(perro_input_api::KeyCode::F23),
        winit::keyboard::KeyCode::F24 => Some(perro_input_api::KeyCode::F24),
        winit::keyboard::KeyCode::F25 => Some(perro_input_api::KeyCode::F25),
        winit::keyboard::KeyCode::F26 => Some(perro_input_api::KeyCode::F26),
        winit::keyboard::KeyCode::F27 => Some(perro_input_api::KeyCode::F27),
        winit::keyboard::KeyCode::F28 => Some(perro_input_api::KeyCode::F28),
        winit::keyboard::KeyCode::F29 => Some(perro_input_api::KeyCode::F29),
        winit::keyboard::KeyCode::F30 => Some(perro_input_api::KeyCode::F30),
        winit::keyboard::KeyCode::F31 => Some(perro_input_api::KeyCode::F31),
        winit::keyboard::KeyCode::F32 => Some(perro_input_api::KeyCode::F32),
        winit::keyboard::KeyCode::F33 => Some(perro_input_api::KeyCode::F33),
        winit::keyboard::KeyCode::F34 => Some(perro_input_api::KeyCode::F34),
        winit::keyboard::KeyCode::F35 => Some(perro_input_api::KeyCode::F35),
        _ => None,
    }
}

pub(crate) fn map_winit_mouse_button(
    button: WinitMouseButton,
) -> Option<perro_input_api::MouseButton> {
    match button {
        WinitMouseButton::Left => Some(perro_input_api::MouseButton::Left),
        WinitMouseButton::Right => Some(perro_input_api::MouseButton::Right),
        WinitMouseButton::Middle => Some(perro_input_api::MouseButton::Middle),
        WinitMouseButton::Back => Some(perro_input_api::MouseButton::Back),
        WinitMouseButton::Forward => Some(perro_input_api::MouseButton::Forward),
        _ => None,
    }
}
