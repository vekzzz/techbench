//! IP Info commands: read this PC's adapters, then its public address, streaming both to the UI.

use std::net::IpAddr;
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use toolbox_core::ip_info::{
    self, IpInfoEvent, IpInfoResult, describe_dhcp, describe_public, format_mac, ipv4_addresses,
    ipv6_addresses,
};
use toolbox_core::network::{Adapter, AdapterKind, PublicIp, SystemNetwork};
use toolbox_core::run::format_time;

use crate::runs::{RunId, Runs};

/// One message on an IP Info Run's channel.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum IpInfoUpdate {
    Adapters {
        adapters: Vec<AdapterView>,
        default_route: Option<String>,
    },
    Public {
        public: PublicView,
    },
    Finished {
        result: IpInfoResultView,
    },
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdapterView {
    name: String,
    description: String,
    kind: &'static str,
    up: bool,
    mac: Option<String>,
    /// Each as `address/prefix`.
    ipv4: Vec<String>,
    ipv6: Vec<String>,
    gateways: Vec<String>,
    dns_servers: Vec<String>,
    /// "On, server …", "On" or "Off", worded by the toolbox core.
    dhcp: String,
    lease_obtained: Option<String>,
    lease_expires: Option<String>,
}

/// The public address line, worded by the toolbox core, and whether the lookup failed.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PublicView {
    text: String,
    failed: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IpInfoResultView {
    /// This PC's name, with the address its internet traffic leaves from when there is a route.
    pc: String,
    adapters: Vec<AdapterView>,
    default_route: Option<String>,
    public: PublicView,
    text: String,
}

#[derive(Serialize)]
pub struct StartedIpInfo {
    id: RunId,
}

impl From<&Adapter> for AdapterView {
    fn from(adapter: &Adapter) -> Self {
        let strings = |addresses: &[IpAddr]| addresses.iter().map(ToString::to_string).collect();
        let lease = adapter.dhcp.as_ref();
        Self {
            name: adapter.name.clone(),
            description: adapter.description.clone(),
            kind: match adapter.kind {
                AdapterKind::Ethernet => "ethernet",
                AdapterKind::Wireless => "wireless",
                AdapterKind::Loopback => "loopback",
                AdapterKind::Tunnel => "tunnel",
                AdapterKind::Other => "other",
            },
            up: adapter.up,
            mac: adapter.mac.map(format_mac),
            ipv4: ipv4_addresses(adapter),
            ipv6: ipv6_addresses(adapter),
            gateways: strings(&adapter.gateways),
            dns_servers: strings(&adapter.dns_servers),
            dhcp: describe_dhcp(lease),
            lease_obtained: lease.and_then(|lease| lease.obtained).map(format_time),
            lease_expires: lease.and_then(|lease| lease.expires).map(format_time),
        }
    }
}

/// The public line for a lookup that has answered, using the core's wording.
fn public_view(public: &Result<PublicIp, String>) -> PublicView {
    PublicView {
        text: describe_public(Some(public)),
        failed: public.is_err(),
    }
}

impl From<IpInfoEvent> for IpInfoUpdate {
    fn from(event: IpInfoEvent) -> Self {
        match event {
            IpInfoEvent::Adapters {
                adapters,
                default_route,
            } => IpInfoUpdate::Adapters {
                adapters: adapters.iter().map(AdapterView::from).collect(),
                default_route,
            },
            IpInfoEvent::Public(public) => IpInfoUpdate::Public {
                public: public_view(&public),
            },
        }
    }
}

impl From<IpInfoResult> for IpInfoResultView {
    fn from(result: IpInfoResult) -> Self {
        Self {
            pc: match result.pc_address {
                Some(address) => format!("{} ({address})", result.pc_name),
                None => result.pc_name.clone(),
            },
            adapters: result.adapters.iter().map(AdapterView::from).collect(),
            default_route: result.default_route.clone(),
            public: PublicView {
                text: describe_public(result.public.as_ref()),
                failed: !matches!(result.public, Some(Ok(_))),
            },
            text: result.to_text(),
        }
    }
}

/// Reads this PC's adapters and public address in the background.
#[tauri::command]
pub fn start_ip_info(
    on_update: Channel<IpInfoUpdate>,
    runs: tauri::State<'_, Arc<Runs>>,
) -> StartedIpInfo {
    let id = runs.spawn(move |stop| async move {
        let result = ip_info::run(&SystemNetwork, &stop, |event| {
            let _ = on_update.send(event.into());
        })
        .await;
        let _ = on_update.send(IpInfoUpdate::Finished {
            result: result.into(),
        });
    });
    StartedIpInfo { id }
}
