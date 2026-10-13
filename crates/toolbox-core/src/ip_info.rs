//! IP Info: this PC's network adapters, which one carries the default route, and its public address.

use chrono::{DateTime, FixedOffset};
use tokio_util::sync::CancellationToken;

use crate::network::{Adapter, AdapterKind, Network, PublicIp};
use crate::run::TIME_FORMAT;

/// Progress an IP Info Run reports as it goes: local details first, the slower public lookup after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpInfoEvent {
    Adapters {
        adapters: Vec<Adapter>,
        default_route: Option<String>,
    },
    Public(Result<PublicIp, String>),
}

/// What an IP Info Run reports back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpInfoResult {
    pub taken_at: DateTime<FixedOffset>,
    pub pc_name: String,
    /// The adapter carrying the default route first, then other active adapters, then inactive ones.
    pub adapters: Vec<Adapter>,
    /// The name of the adapter carrying the default route, if any adapter has a gateway.
    pub default_route: Option<String>,
    /// None when the Run was stopped before the public lookup answered.
    pub public: Option<Result<PublicIp, String>>,
}

impl IpInfoResult {
    /// The Result as plain text, ready to paste into a ticket.
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "IP Info\nTime:      {}\nPC:        {}\nPublic:    {}\n",
            self.taken_at.format(TIME_FORMAT),
            self.pc_name,
            self.public_text(),
        );
        for adapter in &self.adapters {
            text.push('\n');
            text.push_str(&self.adapter_text(adapter));
        }
        text
    }

    /// The public address, ISP and city, or why they are missing.
    pub fn public_text(&self) -> String {
        describe_public(self.public.as_ref())
    }

    fn adapter_text(&self, adapter: &Adapter) -> String {
        let carries_default_route = self.default_route.as_deref() == Some(adapter.name.as_str());
        let mut text = format!(
            "{}{}\n",
            adapter.name,
            if carries_default_route {
                " (default route)"
            } else {
                ""
            }
        );
        let mut line =
            |label: &str, value: String| text.push_str(&format!("  {label:<13} {value}\n"));
        let joined = |addresses: Vec<String>| addresses.join(", ");

        line(
            "Status:",
            if adapter.up { "Up" } else { "Down" }.to_string(),
        );
        line("Description:", adapter.description.clone());
        if let Some(mac) = adapter.mac {
            line("MAC:", format_mac(mac));
        }
        for (label, ipv4) in [("IPv4:", true), ("IPv6:", false)] {
            let addresses: Vec<String> = adapter
                .addresses
                .iter()
                .filter(|assigned| assigned.address.is_ipv4() == ipv4)
                .map(|assigned| format!("{}/{}", assigned.address, assigned.prefix))
                .collect();
            if !addresses.is_empty() {
                line(label, joined(addresses));
            }
        }
        if !adapter.gateways.is_empty() {
            line(
                "Gateway:",
                joined(adapter.gateways.iter().map(ToString::to_string).collect()),
            );
        }
        if !adapter.dns_servers.is_empty() {
            line(
                "DNS servers:",
                joined(
                    adapter
                        .dns_servers
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                ),
            );
        }
        match &adapter.dhcp {
            None => line("DHCP:", "Off".to_string()),
            Some(lease) => {
                line(
                    "DHCP:",
                    match lease.server {
                        Some(server) => format!("On, server {server}"),
                        None => "On".to_string(),
                    },
                );
                let times: Vec<String> = [("obtained", lease.obtained), ("expires", lease.expires)]
                    .into_iter()
                    .filter_map(|(label, time)| {
                        time.map(|time| format!("{label} {}", time.format(TIME_FORMAT)))
                    })
                    .collect();
                if !times.is_empty() {
                    line("Lease:", times.join(", "));
                }
            }
        }
        text
    }
}

/// The public address with its ISP and city; why it failed; or, when the lookup never
/// answered, that it was stopped.
pub fn describe_public(public: Option<&Result<PublicIp, String>>) -> String {
    match public {
        Some(Ok(public)) => {
            let about: Vec<&str> = [public.isp.as_deref(), public.city.as_deref()]
                .into_iter()
                .flatten()
                .collect();
            if about.is_empty() {
                public.address.to_string()
            } else {
                format!("{} ({})", public.address, about.join(", "))
            }
        }
        Some(Err(reason)) => format!("lookup failed: {reason}"),
        None => "not looked up (stopped)".to_string(),
    }
}

/// A MAC address in the form Windows shows it, such as `AA-BB-CC-01-02-03`.
pub fn format_mac(mac: [u8; 6]) -> String {
    mac.map(|byte| format!("{byte:02X}")).join("-")
}

/// Reads this PC's adapters, then looks up its public address, unless `stop` is cancelled first.
pub async fn run(
    network: &impl Network,
    stop: &CancellationToken,
    mut on_event: impl FnMut(IpInfoEvent),
) -> IpInfoResult {
    let taken_at = network.now();
    let pc_name = network.hostname();
    let stopped = |adapters, default_route| IpInfoResult {
        taken_at,
        pc_name: pc_name.clone(),
        adapters,
        default_route,
        public: None,
    };

    let Some(listed) = stop.run_until_cancelled(network.adapters()).await else {
        return stopped(vec![], None);
    };
    // Loopback is the PC talking to itself; it tells the Technician nothing about the network.
    let mut adapters: Vec<Adapter> = listed
        .into_iter()
        .filter(|adapter| adapter.kind != AdapterKind::Loopback)
        .map(|mut adapter| {
            // Tunnels such as Teredo report `::` as a gateway, which routes nothing.
            adapter.gateways.retain(|gateway| !gateway.is_unspecified());
            adapter
        })
        .collect();
    let default_route = default_route_adapter(&adapters).map(|adapter| adapter.name.clone());
    adapters.sort_by_key(|adapter| {
        let carries_default_route = default_route.as_deref() == Some(adapter.name.as_str());
        (
            !carries_default_route,
            !adapter.up,
            adapter.name.to_lowercase(),
        )
    });
    on_event(IpInfoEvent::Adapters {
        adapters: adapters.clone(),
        default_route: default_route.clone(),
    });

    let Some(public) = stop.run_until_cancelled(network.public_ip()).await else {
        return stopped(adapters, default_route);
    };
    on_event(IpInfoEvent::Public(public.clone()));

    IpInfoResult {
        taken_at,
        pc_name,
        adapters,
        default_route,
        public: Some(public),
    }
}

/// The active adapter that carries the default route: of those with an IPv4 gateway, the one
/// with the lowest IPv4 metric; failing that, the same for IPv6.
pub fn default_route_adapter(adapters: &[Adapter]) -> Option<&Adapter> {
    let with_gateway = |ipv4: bool| {
        adapters.iter().filter(move |adapter| {
            adapter.up
                && adapter
                    .gateways
                    .iter()
                    .any(|gateway| gateway.is_ipv4() == ipv4 && !gateway.is_unspecified())
        })
    };
    with_gateway(true)
        .min_by_key(|adapter| adapter.ipv4_metric)
        .or_else(|| with_gateway(false).min_by_key(|adapter| adapter.ipv6_metric))
}
