//! IP Info: this PC's network adapters, which one carries the default route, and its public address.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use chrono::{DateTime, FixedOffset};
use tokio_util::sync::CancellationToken;

use crate::network::{Adapter, AdapterKind, DhcpLease, Network, PublicIp};
use crate::run::format_time;

/// Public addresses used only to ask the routing table which way internet traffic leaves.
/// Nothing is ever sent to them.
const INTERNET_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
const INTERNET_V6: IpAddr = IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888));

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
    /// The address this PC's internet traffic leaves from, when there is a route to the internet.
    pub pc_address: Option<IpAddr>,
    /// The adapter carrying the default route first, then other active adapters, then inactive ones.
    pub adapters: Vec<Adapter>,
    /// The name of the adapter carrying the default route, if there is one.
    pub default_route: Option<String>,
    /// None when the Run was stopped before the public lookup answered.
    pub public: Option<Result<PublicIp, String>>,
}

impl IpInfoResult {
    /// The Result as plain text, ready to paste into a ticket.
    pub fn to_text(&self) -> String {
        let pc = match self.pc_address {
            Some(address) => format!("{} ({address})", self.pc_name),
            None => self.pc_name.clone(),
        };
        let mut text = format!(
            "IP Info\nTime:      {}\nPC:        {pc}\nPublic:    {}\n",
            format_time(self.taken_at),
            describe_public(self.public.as_ref()),
        );
        for adapter in &self.adapters {
            text.push('\n');
            text.push_str(&self.adapter_text(adapter));
        }
        text
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
        let mut line = |label: &str, value: String| {
            if !value.is_empty() {
                text.push_str(&format!("  {label:<13} {value}\n"));
            }
        };
        let joined = |addresses: &[IpAddr]| {
            addresses
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };

        line(
            "Status:",
            if adapter.up { "Up" } else { "Down" }.to_string(),
        );
        line("Description:", adapter.description.clone());
        line("MAC:", adapter.mac.map(format_mac).unwrap_or_default());
        line("IPv4:", ipv4_addresses(adapter).join(", "));
        line("IPv6:", ipv6_addresses(adapter).join(", "));
        line("Gateway:", joined(&adapter.gateways));
        line("DNS servers:", joined(&adapter.dns_servers));
        line("DHCP:", describe_dhcp(adapter.dhcp.as_ref()));
        if let Some(lease) = &adapter.dhcp {
            let times: Vec<String> = [("obtained", lease.obtained), ("expires", lease.expires)]
                .into_iter()
                .filter_map(|(label, time)| {
                    time.map(|time| format!("{label} {}", format_time(time)))
                })
                .collect();
            line("Lease:", times.join(", "));
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

/// Whether an adapter uses DHCP, and from which server: "On, server 192.168.1.1", "On" or "Off".
pub fn describe_dhcp(dhcp: Option<&DhcpLease>) -> String {
    match dhcp {
        None => "Off".to_string(),
        Some(DhcpLease {
            server: Some(server),
            ..
        }) => format!("On, server {server}"),
        Some(_) => "On".to_string(),
    }
}

/// An adapter's IPv4 addresses, each as `address/prefix`.
pub fn ipv4_addresses(adapter: &Adapter) -> Vec<String> {
    with_prefixes(adapter, IpAddr::is_ipv4)
}

/// An adapter's IPv6 addresses, each as `address/prefix`.
pub fn ipv6_addresses(adapter: &Adapter) -> Vec<String> {
    with_prefixes(adapter, IpAddr::is_ipv6)
}

fn with_prefixes(adapter: &Adapter, wanted: fn(&IpAddr) -> bool) -> Vec<String> {
    adapter
        .addresses
        .iter()
        .filter(|assigned| wanted(&assigned.address))
        .map(|assigned| format!("{}/{}", assigned.address, assigned.prefix))
        .collect()
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
    let pc_address = internet_route_source(network);
    let stopped = |adapters, default_route| IpInfoResult {
        taken_at,
        pc_name: pc_name.clone(),
        pc_address,
        adapters,
        default_route,
        public: None,
    };

    let Some(listed) = stop.run_until_cancelled(network.adapters()).await else {
        return stopped(vec![], None);
    };
    let mut adapters = shown_adapters(listed);
    let default_route =
        default_route_adapter(&adapters, pc_address).map(|adapter| adapter.name.clone());
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
        pc_address,
        adapters,
        default_route,
        public: Some(public),
    }
}

/// The adapter carrying this PC's default route, read fresh: for Tools that need the default
/// gateway, DNS server or subnet.
pub async fn find_default_route(network: &impl Network) -> Option<Adapter> {
    let adapters = shown_adapters(network.adapters().await);
    default_route_adapter(&adapters, internet_route_source(network)).cloned()
}

/// The gateway of the Default Route Adapter, for a quick Target: its IPv4 gateway, or failing
/// that a global IPv6 one. A link-local IPv6 gateway is left out, as it can't be pinged without
/// the zone the adapter list doesn't carry.
pub async fn default_gateway(network: &impl Network) -> Option<IpAddr> {
    let adapter = find_default_route(network).await?;
    let is_link_local =
        |gateway: &IpAddr| matches!(gateway, IpAddr::V6(v6) if v6.is_unicast_link_local());
    let gateways = || adapter.gateways.iter().copied();
    gateways()
        .find(IpAddr::is_ipv4)
        .or_else(|| gateways().find(|gateway| !is_link_local(gateway)))
}

/// The adapter carrying the default route. The routing table decides: the active adapter holding
/// `route_source`, the address internet traffic leaves from, which also finds a full-tunnel VPN
/// with no gateway. Without that answer, the active adapter with an IPv4 gateway and the lowest
/// IPv4 metric wins; failing that, the same for IPv6.
pub fn default_route_adapter(
    adapters: &[Adapter],
    route_source: Option<IpAddr>,
) -> Option<&Adapter> {
    let holds_route_source = |adapter: &&Adapter| {
        adapter.up
            && adapter
                .addresses
                .iter()
                .any(|assigned| Some(assigned.address) == route_source)
    };
    let with_gateway = |ipv4: bool| {
        adapters.iter().filter(move |adapter| {
            adapter.up
                && adapter
                    .gateways
                    .iter()
                    .any(|gateway| gateway.is_ipv4() == ipv4 && !gateway.is_unspecified())
        })
    };
    adapters
        .iter()
        .find(holds_route_source)
        .or_else(|| with_gateway(true).min_by_key(|adapter| adapter.ipv4_metric))
        .or_else(|| with_gateway(false).min_by_key(|adapter| adapter.ipv6_metric))
}

/// Asks the routing table which local address internet traffic leaves from. Sends nothing.
fn internet_route_source(network: &impl Network) -> Option<IpAddr> {
    network
        .source_address(INTERNET_V4)
        .or_else(|| network.source_address(INTERNET_V6))
}

/// The adapters worth showing: loopback is the PC talking to itself, and tunnels such as Teredo
/// report `::` as a gateway, which routes nothing.
fn shown_adapters(listed: Vec<Adapter>) -> Vec<Adapter> {
    listed
        .into_iter()
        .filter(|adapter| adapter.kind != AdapterKind::Loopback)
        .map(|mut adapter| {
            adapter.gateways.retain(|gateway| !gateway.is_unspecified());
            adapter
        })
        .collect()
}
