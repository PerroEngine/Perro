use crate::types::{
    LobbyDataKey, LobbyId, LobbyInfo, LobbyJoinability, LobbySearch, LobbyType, SteamID,
};
use crate::{app, error::SteamError, events};

pub fn create(kind: LobbyType, max_members: u32) -> Result<(), SteamError> {
    if max_members > 250 {
        return Err(SteamError::CallFailed(
            "matchmaking.create_lobby.max_members",
        ));
    }

    app::with_client(|client| {
        client
            .matchmaking()
            .create_lobby(kind.into(), max_members, events::push_lobby_create);
        Ok(())
    })
}

pub fn request_list(search: LobbySearch<'_>) -> Result<(), SteamError> {
    app::with_client(|client| {
        let matchmaking = client.matchmaking();
        if let Some(count) = search.max_results {
            matchmaking.set_request_lobby_list_result_count_filter(count);
        }
        if let Some(open_slots) = search.open_slots {
            matchmaking.set_request_lobby_list_slots_available_filter(open_slots);
        }
        if let Some(distance) = search.distance {
            matchmaking.set_request_lobby_list_distance_filter(distance.into());
        }
        for filter in &search.string_filters {
            matchmaking.add_request_lobby_list_string_filter(steamworks::StringFilter(
                steamworks::LobbyKey::new(filter.key.as_str()),
                &filter.value,
                filter.kind.into(),
            ));
        }
        for filter in &search.number_filters {
            matchmaking.add_request_lobby_list_numerical_filter(steamworks::NumberFilter(
                steamworks::LobbyKey::new(filter.key.as_str()),
                filter.value,
                filter.comparison.into(),
            ));
        }
        for filter in &search.near_value_filters {
            matchmaking.add_request_lobby_list_near_value_filter(steamworks::NearFilter(
                steamworks::LobbyKey::new(filter.key.as_str()),
                filter.value,
            ));
        }
        matchmaking.request_lobby_list(events::push_lobby_list);
        Ok(())
    })
}

pub fn join(lobby: LobbyId) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .matchmaking()
            .join_lobby(lobby.into(), move |result| {
                events::push_lobby_join(lobby, result)
            });
        Ok(())
    })
}

pub fn leave(lobby: LobbyId) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.matchmaking().leave_lobby(lobby.into());
        Ok(())
    })
}

pub fn set_data<'a>(
    lobby: LobbyId,
    key: impl Into<LobbyDataKey<'a>>,
    value: &str,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        let key = key.into();
        if client
            .matchmaking()
            .set_lobby_data(lobby.into(), key.as_str(), value)
        {
            Ok(())
        } else {
            Err(SteamError::CallFailed("matchmaking.set_lobby_data"))
        }
    })
}

pub fn get_data<'a>(
    lobby: LobbyId,
    key: impl Into<LobbyDataKey<'a>>,
) -> Result<Option<String>, SteamError> {
    app::with_client(|client| {
        let key = key.into();
        Ok(client.matchmaking().lobby_data(lobby.into(), key.as_str()))
    })
}

pub fn all_data(lobby: LobbyId) -> Result<Vec<(String, String)>, SteamError> {
    app::with_client(|client| {
        let matchmaking = client.matchmaking();
        let lobby = lobby.into();
        let count = matchmaking.lobby_data_count(lobby);
        let mut data = Vec::with_capacity(count as usize);
        for idx in 0..count {
            if let Some(entry) = matchmaking.lobby_data_by_index(lobby, idx) {
                data.push(entry);
            }
        }
        Ok(data)
    })
}

pub fn members(lobby: LobbyId) -> Result<Vec<SteamID>, SteamError> {
    app::with_client(|client| {
        Ok(client
            .matchmaking()
            .lobby_members(lobby.into())
            .into_iter()
            .map(Into::into)
            .collect())
    })
}

pub fn owner(lobby: LobbyId) -> Result<SteamID, SteamError> {
    app::with_client(|client| Ok(client.matchmaking().lobby_owner(lobby.into()).into()))
}

pub fn info(lobby: LobbyId) -> Result<LobbyInfo, SteamError> {
    app::with_client(|client| {
        let matchmaking = client.matchmaking();
        let raw_lobby = lobby.into();
        let data = (0..matchmaking.lobby_data_count(raw_lobby))
            .filter_map(|idx| matchmaking.lobby_data_by_index(raw_lobby, idx))
            .collect();
        Ok(LobbyInfo {
            id: lobby,
            owner: matchmaking.lobby_owner(raw_lobby).into(),
            members: matchmaking
                .lobby_members(raw_lobby)
                .into_iter()
                .map(Into::into)
                .collect(),
            member_limit: matchmaking.lobby_member_limit(raw_lobby),
            data,
            game_server: matchmaking
                .get_lobby_game_server(raw_lobby)
                .map(|(addr, id)| (addr, id.map(Into::into))),
        })
    })
}

pub fn set_joinable(lobby: LobbyId, joinable: bool) -> Result<(), SteamError> {
    set_joinability(
        lobby,
        if joinable {
            LobbyJoinability::Open
        } else {
            LobbyJoinability::Closed
        },
    )
}

pub fn set_joinability(lobby: LobbyId, joinability: LobbyJoinability) -> Result<(), SteamError> {
    app::with_client(|client| {
        if client
            .matchmaking()
            .set_lobby_joinable(lobby.into(), joinability.as_bool())
        {
            Ok(())
        } else {
            Err(SteamError::CallFailed("matchmaking.set_lobby_joinable"))
        }
    })
}

pub fn send_chat(lobby: LobbyId, message: impl AsRef<[u8]>) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .matchmaking()
            .send_lobby_chat_message(lobby.into(), message.as_ref())
            .map_err(|_| SteamError::CallFailed("matchmaking.send_lobby_chat_message"))
    })
}

pub fn read_chat(lobby: LobbyId, chat_id: i32) -> Result<Vec<u8>, SteamError> {
    app::with_client(|client| {
        let mut buffer = vec![0; 4096];
        let data = client
            .matchmaking()
            .get_lobby_chat_entry(lobby.into(), chat_id, &mut buffer);
        Ok(data.to_vec())
    })
}
