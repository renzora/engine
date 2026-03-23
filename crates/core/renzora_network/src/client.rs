//! Client-side networking helpers.
//!
//! Connection is driven dynamically by `PendingNetworkConnect` (from lifecycle
//! graph or scripts). The Lightyear `ClientPlugins` infrastructure is added
//! by `NetworkPlugin` so it's always available.

use bevy::prelude::*;
use lightyear::prelude::client::*;

use crate::status::{ConnectionState, NetworkStatus};

/// Generate a pseudo-random client ID from system time.
pub fn rand_client_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42)
}

/// Update `NetworkStatus` resource from Lightyear client connection state.
pub fn update_network_status(
    mut status: ResMut<NetworkStatus>,
    connected_query: Query<(), With<Connected>>,
    connecting_query: Query<(), With<Connecting>>,
) {
    if !connected_query.is_empty() {
        if status.state != ConnectionState::Connected {
            status.state = ConnectionState::Connected;
            info!("[network] Connected to server");
        }
    } else if !connecting_query.is_empty() {
        if status.state != ConnectionState::Connecting {
            status.state = ConnectionState::Connecting;
        }
    } else if status.state != ConnectionState::Disconnected {
        status.state = ConnectionState::Disconnected;
    }
}
