//! The Techbench toolbox core: every Tool's behaviour, with no dependency on Tauri or the UI.

pub mod continuous_ping;
mod dns_client;
pub mod dns_lookup;
pub mod ip_info;
pub mod lan_scan;
pub mod network;
mod ookla;
mod ookla_client;
pub mod oui;
pub mod ping;
pub mod port_check;
mod public_ip;
pub mod run;
pub mod saved_targets;
mod speed_client;
pub mod speed_test;
pub mod subnet_calculator;
pub mod target;
pub mod traceroute;
#[cfg(windows)]
mod windows_adapters;
#[cfg(windows)]
mod windows_arp;
#[cfg(windows)]
mod windows_cost;
#[cfg(windows)]
mod windows_icmp;
#[cfg(windows)]
mod windows_names;

/// Why something the Technician typed was rejected before a Run could start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputError {
    pub message: String,
}

impl InputError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
