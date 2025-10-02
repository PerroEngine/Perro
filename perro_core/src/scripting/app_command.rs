use std::sync::mpsc::{Sender, Receiver, channel};

#[derive(Debug, Clone)]
pub enum AppCommand {
    SetWindowTitle(String),
    SetTargetFPS(f32),
    Quit,
}

pub fn create_command_channel() -> (Sender<AppCommand>, Receiver<AppCommand>) {
    channel()
}