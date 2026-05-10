use crate::{app, error::SteamError};
use std::net::Ipv4Addr;

pub type MatchmakingServers = steamworks::MatchmakingServers;
pub type GameServerItem = steamworks::GameServerItem;
pub type ServerListRequest = steamworks::ServerListRequest;
pub type PingCallbacks = steamworks::PingCallbacks;
pub type ServerRulesCallbacks = steamworks::ServerRulesCallbacks;

pub fn ping_server(ip: Ipv4Addr, port: u16, callbacks: PingCallbacks) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .matchmaking_servers()
            .ping_server(ip, port, callbacks);
        Ok(())
    })
}

pub fn server_rules(
    ip: Ipv4Addr,
    port: u16,
    callbacks: ServerRulesCallbacks,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .matchmaking_servers()
            .server_rules(ip, port, callbacks);
        Ok(())
    })
}
