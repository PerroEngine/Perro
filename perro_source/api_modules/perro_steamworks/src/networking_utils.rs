use crate::{app, error::SteamError};

pub type NetworkingAvailability = steamworks::networking_types::NetworkingAvailability;
pub type NetworkingAvailabilityError = steamworks::networking_types::NetworkingAvailabilityError;
pub type RelayNetworkStatus = steamworks::networking_utils::RelayNetworkStatus;

pub fn init_relay_network_access() -> Result<(), SteamError> {
    app::with_client(|client| {
        client.networking_utils().init_relay_network_access();
        Ok(())
    })
}

pub fn get_relay_network_status()
-> Result<Result<NetworkingAvailability, NetworkingAvailabilityError>, SteamError> {
    app::with_client(|client| Ok(client.networking_utils().relay_network_status()))
}

pub fn get_detailed_relay_network_status() -> Result<RelayNetworkStatus, SteamError> {
    app::with_client(|client| Ok(client.networking_utils().detailed_relay_network_status()))
}
