//! What Windows says the internet connection costs, from its connection cost API. No elevation.

use windows::Networking::Connectivity::{NetworkCostType, NetworkInformation};

use crate::network::ConnectionCost;

/// Reads the cost of the connection internet traffic uses. Any failure, such as having no
/// connection at all, is [`ConnectionCost::Unknown`].
pub(crate) fn cost() -> ConnectionCost {
    let Ok(profile) = NetworkInformation::GetInternetConnectionProfile() else {
        return ConnectionCost::Unknown;
    };
    let Ok(cost) = profile.GetConnectionCost() else {
        return ConnectionCost::Unknown;
    };
    if cost.Roaming().unwrap_or(false) {
        ConnectionCost::Roaming
    } else if cost.OverDataLimit().unwrap_or(false) {
        ConnectionCost::OverLimit
    } else {
        match cost.NetworkCostType() {
            Ok(NetworkCostType::Fixed | NetworkCostType::Variable) => ConnectionCost::Metered,
            Ok(NetworkCostType::Unrestricted) => ConnectionCost::Unmetered,
            _ => ConnectionCost::Unknown,
        }
    }
}
