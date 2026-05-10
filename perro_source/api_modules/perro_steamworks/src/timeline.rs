use crate::{app, error::SteamError};
use std::time::Duration;

pub type TimelineGameMode = steamworks::timeline::TimelineGameMode;
pub type TimelineEventClipPriority = steamworks::timeline::TimelineEventClipPriority;

pub fn set_game_mode(mode: TimelineGameMode) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.timeline().set_timeline_game_mode(mode);
        Ok(())
    })
}

pub fn set_state_description(description: &str, duration: Duration) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .timeline()
            .set_timeline_state_description(description, duration);
        Ok(())
    })
}

pub fn clear_state_description(duration: Duration) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.timeline().clear_timeline_state_description(duration);
        Ok(())
    })
}

pub fn add_event(
    icon: &str,
    title: &str,
    description: &str,
    priority: u32,
    start_offset_seconds: f32,
    duration: Duration,
    clip_priority: TimelineEventClipPriority,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.timeline().add_timeline_event(
            icon,
            title,
            description,
            priority,
            start_offset_seconds,
            duration,
            clip_priority,
        );
        Ok(())
    })
}
