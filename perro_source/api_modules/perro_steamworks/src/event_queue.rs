use crate::types::{SteamEvent, SteamEventQueueStats};
use std::collections::VecDeque;

pub(crate) const STEAM_EVENT_QUEUE_CAPACITY: usize = 1024;

pub(crate) struct SteamEventQueue {
    events: VecDeque<SteamEvent>,
    dropped: u64,
    coalesced: u64,
}

impl SteamEventQueue {
    pub(crate) fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(STEAM_EVENT_QUEUE_CAPACITY),
            dropped: 0,
            coalesced: 0,
        }
    }

    #[cfg(any(feature = "steamworks-runtime", test))]
    pub(crate) fn push(&mut self, event: SteamEvent) {
        if let Some(index) = self
            .events
            .iter()
            .position(|queued| coalesces(queued, &event))
        {
            self.events.remove(index);
            self.events.push_back(event);
            self.coalesced = self.coalesced.saturating_add(1);
            return;
        }

        if self.events.len() == STEAM_EVENT_QUEUE_CAPACITY {
            let drop_index = self.events.iter().position(is_coalescible).unwrap_or(0);
            self.events.remove(drop_index);
            self.dropped = self.dropped.saturating_add(1);
        }
        self.events.push_back(event);
    }

    pub(crate) fn pop_front(&mut self) -> Option<SteamEvent> {
        self.events.pop_front()
    }

    pub(crate) fn drain(&mut self) -> Vec<SteamEvent> {
        self.events.drain(..).collect()
    }

    pub(crate) fn clear(&mut self) {
        self.events.clear();
    }

    pub(crate) fn stats(&self) -> SteamEventQueueStats {
        SteamEventQueueStats {
            capacity: STEAM_EVENT_QUEUE_CAPACITY,
            len: self.events.len(),
            dropped: self.dropped,
            coalesced: self.coalesced,
        }
    }
}

#[cfg(any(feature = "steamworks-runtime", test))]
fn is_coalescible(event: &SteamEvent) -> bool {
    matches!(
        event,
        SteamEvent::LobbyDataUpdated { .. }
            | SteamEvent::LobbyMemberChanged { .. }
            | SteamEvent::PersonaChanged { .. }
            | SteamEvent::OverlayChanged { .. }
    )
}

#[cfg(any(feature = "steamworks-runtime", test))]
fn coalesces(queued: &SteamEvent, incoming: &SteamEvent) -> bool {
    match (queued, incoming) {
        (
            SteamEvent::LobbyDataUpdated {
                lobby: queued_lobby,
                member: queued_member,
            },
            SteamEvent::LobbyDataUpdated { lobby, member },
        ) => queued_lobby == lobby && queued_member == member,
        (
            SteamEvent::LobbyMemberChanged {
                lobby: queued_lobby,
                user: queued_user,
            },
            SteamEvent::LobbyMemberChanged { lobby, user },
        ) => queued_lobby == lobby && queued_user == user,
        (SteamEvent::PersonaChanged { user: queued_user }, SteamEvent::PersonaChanged { user }) => {
            queued_user == user
        }
        (SteamEvent::OverlayChanged { .. }, SteamEvent::OverlayChanged { .. }) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SteamID;

    #[test]
    fn coalesces_latest_state_event() {
        let mut queue = SteamEventQueue::new();
        queue.push(SteamEvent::OverlayChanged { active: false });
        queue.push(SteamEvent::Callback { name: "edge" });
        queue.push(SteamEvent::OverlayChanged { active: true });

        assert_eq!(queue.stats().len, 2);
        assert_eq!(queue.stats().coalesced, 1);
        assert_eq!(queue.stats().dropped, 0);
        assert_eq!(
            queue.drain(),
            vec![
                SteamEvent::Callback { name: "edge" },
                SteamEvent::OverlayChanged { active: true }
            ]
        );
    }

    #[test]
    fn saturation_drops_state_before_edge_and_counts() {
        let mut queue = SteamEventQueue::new();
        queue.push(SteamEvent::PersonaChanged {
            user: SteamID::from_id(1),
        });
        for _ in 1..STEAM_EVENT_QUEUE_CAPACITY {
            queue.push(SteamEvent::Callback { name: "edge" });
        }
        queue.push(SteamEvent::LobbyListFailed);

        let stats = queue.stats();
        assert_eq!(stats.capacity, STEAM_EVENT_QUEUE_CAPACITY);
        assert_eq!(stats.len, STEAM_EVENT_QUEUE_CAPACITY);
        assert_eq!(stats.dropped, 1);
        assert!(
            !queue
                .events
                .iter()
                .any(|event| matches!(event, SteamEvent::PersonaChanged { .. }))
        );
        assert_eq!(queue.events.back(), Some(&SteamEvent::LobbyListFailed));
    }
}
