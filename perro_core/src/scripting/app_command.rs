use std::sync::mpsc::{Receiver, Sender, channel};

#[derive(Debug, Clone)]
pub enum AppCommand {
    SetWindowTitle(String),
    SetFpsCap(f32),
    SetCursorIcon(CursorIcon),
    Quit,
}

#[derive(Debug, Clone, Copy)]
pub enum CursorIcon {
    Default,
    Hand,
    Text,
    NotAllowed,
    Wait,
    Crosshair,
    Move,
    ResizeVertical,
    ResizeHorizontal,
    ResizeDiagonal1,
    ResizeDiagonal2,
}

pub fn create_command_channel() -> (Sender<AppCommand>, Receiver<AppCommand>) {
    channel()
}
