use crate::{app, error::SteamError};
use std::net::SocketAddr;

pub type ListenSocket = steamworks::networking_sockets::ListenSocket;
pub type NetConnection = steamworks::networking_sockets::NetConnection;
pub type NetPollGroup = steamworks::networking_sockets::NetPollGroup;
pub type NetworkingConfigEntry = steamworks::networking_types::NetworkingConfigEntry;
pub type NetworkingIdentity = steamworks::networking_types::NetworkingIdentity;
pub type NetworkingAvailability = steamworks::networking_types::NetworkingAvailability;
pub type NetworkingAvailabilityError = steamworks::networking_types::NetworkingAvailabilityError;

pub fn listen_ip(
    addr: SocketAddr,
    options: Vec<NetworkingConfigEntry>,
) -> Result<ListenSocket, SteamError> {
    app::with_client(|client| {
        client
            .networking_sockets()
            .create_listen_socket_ip(addr, options)
            .map_err(|_| SteamError::CallFailed("networking_sockets.listen_ip"))
    })
}

pub fn connect_ip(
    addr: SocketAddr,
    options: Vec<NetworkingConfigEntry>,
) -> Result<NetConnection, SteamError> {
    app::with_client(|client| {
        client
            .networking_sockets()
            .connect_by_ip_address(addr, options)
            .map_err(|_| SteamError::CallFailed("networking_sockets.connect_ip"))
    })
}

pub fn listen_p2p(
    local_virtual_port: i32,
    options: Vec<NetworkingConfigEntry>,
) -> Result<ListenSocket, SteamError> {
    app::with_client(|client| {
        client
            .networking_sockets()
            .create_listen_socket_p2p(local_virtual_port, options)
            .map_err(|_| SteamError::CallFailed("networking_sockets.listen_p2p"))
    })
}

pub fn connect_p2p(
    identity: NetworkingIdentity,
    remote_virtual_port: i32,
    options: Vec<NetworkingConfigEntry>,
) -> Result<NetConnection, SteamError> {
    app::with_client(|client| {
        client
            .networking_sockets()
            .connect_p2p(identity, remote_virtual_port, options)
            .map_err(|_| SteamError::CallFailed("networking_sockets.connect_p2p"))
    })
}

pub fn init_authentication()
-> Result<Result<NetworkingAvailability, NetworkingAvailabilityError>, SteamError> {
    app::with_client(|client| Ok(client.networking_sockets().init_authentication()))
}

pub fn auth_status()
-> Result<Result<NetworkingAvailability, NetworkingAvailabilityError>, SteamError> {
    app::with_client(|client| Ok(client.networking_sockets().get_authentication_status()))
}
