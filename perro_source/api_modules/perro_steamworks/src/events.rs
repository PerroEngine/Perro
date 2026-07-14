use crate::{
    error::SteamError,
    event_queue::SteamEventQueue,
    types::{LobbyID, SteamEvent, SteamEventQueueStats},
};
use std::sync::{Mutex, OnceLock};

fn queue() -> &'static Mutex<SteamEventQueue> {
    static QUEUE: OnceLock<Mutex<SteamEventQueue>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(SteamEventQueue::new()))
}

pub(crate) fn push(event: SteamEvent) {
    if let Ok(mut queue) = queue().lock() {
        queue.push(event);
    }
}

pub fn poll_one() -> Result<Option<SteamEvent>, SteamError> {
    let _ = crate::app::run_callbacks();
    queue()
        .lock()
        .map(|mut queue| queue.pop_front())
        .map_err(|_| SteamError::NotReady)
}

pub fn drain() -> Result<Vec<SteamEvent>, SteamError> {
    let _ = crate::app::run_callbacks();
    queue()
        .lock()
        .map(|mut queue| queue.drain())
        .map_err(|_| SteamError::NotReady)
}

pub fn clear() -> Result<(), SteamError> {
    queue()
        .lock()
        .map(|mut queue| queue.clear())
        .map_err(|_| SteamError::NotReady)
}

pub fn queue_stats() -> Result<SteamEventQueueStats, SteamError> {
    queue()
        .lock()
        .map(|queue| queue.stats())
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
        steamworks::CallbackResult::AuthSessionTicketResponse(_) => {
            push(SteamEvent::Callback {
                name: "auth_session_ticket_response",
            });
        }
        steamworks::CallbackResult::TicketForWebApiResponse(_) => {
            push(SteamEvent::Callback {
                name: "ticket_for_webapi_response",
            });
        }
        steamworks::CallbackResult::ValidateAuthTicketResponse(response) => {
            push(SteamEvent::ServerAuthValidated {
                user: response.steam_id.into(),
                owner: response.owner_steam_id.into(),
                error: response.response.err().map(|err| err.to_string()),
            });
        }
        steamworks::CallbackResult::SteamServersConnected(_) => {
            push(SteamEvent::Callback {
                name: "steam_servers_connected",
            });
        }
        steamworks::CallbackResult::SteamServersDisconnected(_) => {
            push(SteamEvent::Callback {
                name: "steam_servers_disconnected",
            });
        }
        steamworks::CallbackResult::SteamServerConnectFailure(_) => {
            push(SteamEvent::Callback {
                name: "steam_server_connect_failure",
            });
        }
        steamworks::CallbackResult::MicroTxnAuthorizationResponse(_) => {
            push(SteamEvent::Callback {
                name: "microtxn_authorization_response",
            });
        }
        steamworks::CallbackResult::P2PSessionRequest(_) => {
            push(SteamEvent::Callback {
                name: "p2p_session_request",
            });
        }
        steamworks::CallbackResult::P2PSessionConnectFail(_) => {
            push(SteamEvent::Callback {
                name: "p2p_session_connect_fail",
            });
        }
        steamworks::CallbackResult::NetworkingMessagesSessionRequest(_) => {
            push(SteamEvent::Callback {
                name: "networking_messages_session_request",
            });
        }
        steamworks::CallbackResult::NetworkingMessagesSessionFailed(_) => {
            push(SteamEvent::Callback {
                name: "networking_messages_session_failed",
            });
        }
        steamworks::CallbackResult::NetConnectionStatusChanged(_) => {
            push(SteamEvent::Callback {
                name: "net_connection_status_changed",
            });
        }
        steamworks::CallbackResult::RelayNetworkStatusCallback(_) => {
            push(SteamEvent::Callback {
                name: "relay_network_status",
            });
        }
        steamworks::CallbackResult::DownloadItemResult(_) => {
            push(SteamEvent::Callback {
                name: "download_item_result",
            });
        }
        steamworks::CallbackResult::ScreenshotRequested(_) => {
            push(SteamEvent::Callback {
                name: "screenshot_requested",
            });
        }
        steamworks::CallbackResult::ScreenshotReady(_) => {
            push(SteamEvent::Callback {
                name: "screenshot_ready",
            });
        }
        steamworks::CallbackResult::RemotePlayConnected(_) => {
            push(SteamEvent::Callback {
                name: "remote_play_connected",
            });
        }
        steamworks::CallbackResult::RemotePlayDisconnected(_) => {
            push(SteamEvent::Callback {
                name: "remote_play_disconnected",
            });
        }
        steamworks::CallbackResult::UserStatsReceived(_) => {
            push(SteamEvent::Callback {
                name: "user_stats_received",
            });
        }
        steamworks::CallbackResult::UserStatsStored(_) => {
            push(SteamEvent::Callback {
                name: "user_stats_stored",
            });
        }
        steamworks::CallbackResult::UserAchievementStored(_) => {
            push(SteamEvent::Callback {
                name: "user_achievement_stored",
            });
        }
        steamworks::CallbackResult::UserAchievementIconFetched(_) => {
            push(SteamEvent::Callback {
                name: "user_achievement_icon_fetched",
            });
        }
        steamworks::CallbackResult::GamepadTextInputDismissed(_) => {
            push(SteamEvent::Callback {
                name: "gamepad_text_input_dismissed",
            });
        }
        steamworks::CallbackResult::FloatingGamepadTextInputDismissed(_) => {
            push(SteamEvent::Callback {
                name: "floating_gamepad_text_input_dismissed",
            });
        }
        steamworks::CallbackResult::GSClientApprove(_) => {
            push(SteamEvent::Callback {
                name: "gs_client_approve",
            });
        }
        steamworks::CallbackResult::GSClientDeny(_) => {
            push(SteamEvent::Callback {
                name: "gs_client_deny",
            });
        }
        steamworks::CallbackResult::GSClientKick(_) => {
            push(SteamEvent::Callback {
                name: "gs_client_kick",
            });
        }
        steamworks::CallbackResult::GSClientGroupStatus(_) => {
            push(SteamEvent::Callback {
                name: "gs_client_group_status",
            });
        }
        steamworks::CallbackResult::NewUrlLaunchParameters(_) => {
            push(SteamEvent::Callback {
                name: "new_url_launch_parameters",
            });
        }
        steamworks::CallbackResult::LobbyCreated(_) | steamworks::CallbackResult::LobbyEnter(_) => {
        }
    }
}

pub(crate) fn push_lobby_list(result: Result<Vec<steamworks::LobbyId>, steamworks::SteamError>) {
    match result {
        Ok(lobbies) => push(SteamEvent::LobbyList {
            lobbies: lobbies.into_iter().map(LobbyID::from).collect(),
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

pub(crate) fn push_lobby_join(target: LobbyID, result: Result<steamworks::LobbyId, ()>) {
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
    use std::sync::{Mutex, OnceLock};

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn queue_drains_in_order() {
        let _guard = test_lock();
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
        let _guard = test_lock();
        push(SteamEvent::OverlayChanged { active: false });
        clear().expect("clear");
        assert_eq!(drain(), Ok(Vec::new()));
    }
}
