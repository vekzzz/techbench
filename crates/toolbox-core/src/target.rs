//! Targets: what a Tool is aimed at, and how one becomes the Resolved Address a Run uses.

use std::fmt;
use std::net::IpAddr;

use crate::InputError;
use crate::network::Network;

/// The hostname or IP address a Tool is aimed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Address(IpAddr),
    Hostname(String),
}

/// Which IP version a Run may use for its Resolved Address.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AddressFamily {
    /// IPv4 preferred, IPv6 when the Target has no IPv4 address.
    #[default]
    Auto,
    Ipv4,
    Ipv6,
}

impl Target {
    /// Parses what the Technician typed for a Run limited to `family`.
    pub fn parse_for(text: &str, family: AddressFamily) -> Result<Target, InputError> {
        let target = Target::parse(text)?;
        match (&target, family) {
            (Target::Address(IpAddr::V4(_)), AddressFamily::Ipv6) => Err(InputError::new(format!(
                "{target} is an IPv4 address, but IPv6 is forced"
            ))),
            (Target::Address(IpAddr::V6(_)), AddressFamily::Ipv4) => Err(InputError::new(format!(
                "{target} is an IPv6 address, but IPv4 is forced"
            ))),
            _ => Ok(target),
        }
    }

    /// Parses what the Technician typed.
    pub fn parse(text: &str) -> Result<Target, InputError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(InputError::new("enter a hostname or IP address"));
        }
        // IPv6 addresses are often written in brackets, as in URLs.
        let unbracketed = text
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .unwrap_or(text);
        if let Ok(address) = unbracketed.parse::<IpAddr>() {
            return Ok(Target::Address(address));
        }
        if text.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Err(InputError::new(format!(
                "{text} is not a valid IPv4 address"
            )));
        }
        if is_hostname(text) {
            Ok(Target::Hostname(text.to_string()))
        } else {
            Err(InputError::new(format!(
                "{text} is not a valid hostname or IP address"
            )))
        }
    }
}

/// Letters, digits, hyphens and underscores in dot-separated labels of 1–63 characters,
/// no label starting or ending with a hyphen, 253 characters at most. One trailing dot is allowed.
fn is_hostname(text: &str) -> bool {
    let name = text.strip_suffix('.').unwrap_or(text);
    name.len() <= 253
        && name.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Address(address) => address.fmt(f),
            Target::Hostname(name) => name.fmt(f),
        }
    }
}

/// Picks the single address a Run uses for `target`.
pub(crate) async fn resolve(
    network: &impl Network,
    target: &Target,
    family: AddressFamily,
) -> Result<IpAddr, String> {
    match target {
        Target::Address(address) => Ok(*address),
        Target::Hostname(name) => {
            let addresses = network.resolve(name).await.map_err(|error| error.message)?;
            let first_v4 = addresses.iter().copied().find(IpAddr::is_ipv4);
            let first_v6 = addresses.iter().copied().find(IpAddr::is_ipv6);
            match family {
                AddressFamily::Auto => first_v4
                    .or(first_v6)
                    .ok_or_else(|| format!("{name} has no addresses")),
                AddressFamily::Ipv4 => {
                    first_v4.ok_or_else(|| format!("{name} has no IPv4 address"))
                }
                AddressFamily::Ipv6 => {
                    first_v6.ok_or_else(|| format!("{name} has no IPv6 address"))
                }
            }
        }
    }
}
