//! The IT Tools toolbox core: every Tool's behaviour, with no dependency on Tauri or the UI.

pub mod continuous_ping;
pub mod ip_info;
pub mod network;
pub mod ping;
pub mod port_check;
mod public_ip;
pub mod run;
pub mod subnet_calculator;
pub mod target;
#[cfg(windows)]
mod windows_adapters;
#[cfg(windows)]
mod windows_icmp;

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
