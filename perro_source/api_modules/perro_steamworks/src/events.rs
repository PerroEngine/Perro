use crate::{
    error::SteamError,
    types::{LobbyId, SteamEvent},
};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

fn queue() -> &'static Mutex<VecDeque<SteamEvent>> {
    static QUEUE: OnceLock<Mutex<VecDeque<SteamEvent>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub(crate) fn push(event: SteamEvent) {
    if let Ok(mut queue) = queue().lock() {
        queue.push_back(event);
    }
}

pub fn poll_one() -> Result<Option<SteamEvent>, SteamError> {
    queue()
        .lock()
        .map(|mut queue| queue.pop_front())
        .map_err(|_| SteamError::NotReady)
}

pub fn drain() -> Result<Vec<SteamEvent>, SteamError> {
    queue()
        .lock()
        .map(|mut queue| queue.drain(..).collect())
        .map_err(|_| SteamError::NotReady)
}

pub fn clear() -> Result<(), SteamError> {
    queue()
        .lock()
        .map(|mut queue| queue.clear())
        .map_err(|_| SteamError::NotReady)
}

pub(crate) fn enqueue_callback(callback: steamworks::CallbackResult) {
    match callback {
        steamworks::CallbackResult::GameLobbyJoinRequested(event) => {
            push(SteamEvent::LobbyJoinRequested {
                lobby: event.lobby_steam_id.into(),
                friend: event.friend_steam_id.into(),
            });
        }
        steamworks::CallbackResult::GameRichPresenceJoinRequested(event) => {
            push(SteamEvent::RichPresenceJoinRequested {
                friend: event.friend_steam_id.into(),
                connect: event.connect,
            });
        }
        steamworks::CallbackResult::GameOverlayActivated(event) => {
            push(SteamEvent::OverlayChanged {
                active: event.active,
            });
        }
        steamworks::CallbackResult::PersonaStateChange(event) => {
            push(SteamEvent::PersonaChanged {
                user: event.steam_id.into(),
            });
        }
        steamworks::CallbackResult::LobbyChatMsg(event) => {
            push(SteamEvent::LobbyChat {
                lobby: event.lobby.into(),
                user: event.user.into(),
                chat_id: event.chat_id,
            });
        }
        steamworks::CallbackResult::LobbyChatUpdate(event) => {
            push(SteamEvent::LobbyMemberChanged {
                lobby: event.lobby.into(),
                user: event.user_changed.into(),
            });
        }
        steamworks::CallbackResult::LobbyDataUpdate(event) => {
            push(SteamEvent::LobbyDataUpdated {
                lobby: event.lobby.into(),
                member: event.member.into(),
            });
        }
        _ => {}
    }
}

pub(crate) fn push_lobby_list(result: Result<Vec<steamworks::LobbyId>, steamworks::SteamError>) {
    match result {
        Ok(lobbies) => push(SteamEvent::LobbyList {
            lobbies: lobbies.into_iter().map(LobbyId::from).collect(),
        }),
        Err(_) => push(SteamEvent::LobbyListFailed),
    }
}

pub(crate) fn push_lobby_create(result: Result<steamworks::LobbyId, steamworks::SteamError>) {
    match result {
        Ok(lobby) => push(SteamEvent::LobbyCreated {
            lobby: lobby.into(),
        }),
        Err(_) => push(SteamEvent::LobbyCreateFailed),
    }
}

pub(crate) fn push_lobby_join(target: LobbyId, result: Result<steamworks::LobbyId, ()>) {
    match result {
        Ok(lobby) => push(SteamEvent::LobbyJoined {
            lobby: lobby.into(),
        }),
        Err(_) => push(SteamEvent::LobbyJoinFailed { lobby: target }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SteamID;

    #[test]
    fn queue_drains_in_order() {
        clear().expect("clear");
        push(SteamEvent::OverlayChanged { active: true });
        push(SteamEvent::PersonaChanged {
            user: SteamID::from_id(7),
        });

        assert_eq!(
            poll_one(),
            Ok(Some(SteamEvent::OverlayChanged { active: true }))
        );
        assert_eq!(
            drain(),
            Ok(vec![SteamEvent::PersonaChanged {
                user: SteamID::from_id(7)
            }])
        );
        assert_eq!(poll_one(), Ok(None));
    }

    #[test]
    fn clear_removes_events() {
        push(SteamEvent::OverlayChanged { active: false });
        clear().expect("clear");
        assert_eq!(drain(), Ok(Vec::new()));
    }
}
