//! LAN Scan commands: start a Run and stream the Devices it finds to the UI.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use toolbox_core::ip_info::format_mac;
use toolbox_core::lan_scan::{self, Device, LanScanEvent, LanScanRequest, LanScanResult};
use toolbox_core::network::SystemNetwork;
use toolbox_core::run::Progress;

use crate::runs::{EndingView, RunId, Runs};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanScanInput {
    range: String,
    /// Whether the Technician has agreed to a range big enough to need agreement.
    confirmed: bool,
}

/// One message on a LAN Scan Run's channel.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LanScanUpdate {
    Started { addresses: usize },
    Device { device: DeviceView },
    Progress { probed: usize, total: usize },
    Finished { result: LanScanResultView },
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    address: String,
    mac: Option<String>,
    response_ms: Option<f64>,
    /// It answered ARP but not the ping.
    no_ping_reply: bool,
    /// Whether ARP could reach it, which is why a MAC may be blank.
    on_own_subnet: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LanScanResultView {
    devices: Vec<DeviceView>,
    addresses_total: usize,
    addresses_probed: usize,
    #[serde(flatten)]
    ending: EndingView,
    /// The one-line conclusion, worded by the toolbox core.
    summary: String,
    text: String,
}

impl From<Device> for DeviceView {
    fn from(device: Device) -> Self {
        Self {
            address: device.address.to_string(),
            mac: device.mac.map(format_mac),
            response_ms: device.response_time.map(|took| took.as_secs_f64() * 1000.0),
            no_ping_reply: device.no_ping_reply,
            on_own_subnet: device.on_own_subnet,
        }
    }
}

impl From<LanScanEvent> for LanScanUpdate {
    fn from(event: LanScanEvent) -> Self {
        match event {
            LanScanEvent::Started { addresses } => LanScanUpdate::Started { addresses },
            LanScanEvent::Device(device) => LanScanUpdate::Device {
                device: device.into(),
            },
            LanScanEvent::Progress { probed, total } => LanScanUpdate::Progress { probed, total },
        }
    }
}

impl From<LanScanResult> for LanScanResultView {
    fn from(result: LanScanResult) -> Self {
        let text = result.to_text();
        let summary = result.summary();
        Self {
            devices: result.devices.into_iter().map(DeviceView::from).collect(),
            addresses_total: result.addresses_total,
            addresses_probed: result.addresses_probed,
            ending: result.ending.into(),
            summary,
            text,
        }
    }
}

/// A LAN Scan Run the shell has accepted.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedLanScan {
    id: RunId,
    addresses: usize,
}

/// How big a range is, for the screen to ask for confirmation before a big scan.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeInfoView {
    addresses: usize,
    needs_confirmation: bool,
}

/// The range LAN Scan offers first: the network of the adapter carrying the default route.
/// None when that adapter has no IPv4 address, so the Technician must enter one.
#[tauri::command]
pub async fn default_lan_range() -> Option<String> {
    lan_scan::default_range(&SystemNetwork).await
}

/// Reads a range the Technician typed. Rejects with a message when it is invalid or too big.
#[tauri::command]
pub fn lan_range_info(range: String) -> Result<RangeInfoView, String> {
    let check = LanScanRequest {
        range,
        confirmed: false,
    }
    .check_range()
    .map_err(|error| error.message)?;
    Ok(RangeInfoView {
        addresses: check.addresses,
        needs_confirmation: check.needs_confirmation,
    })
}

/// Validates the input, then scans in the background. Invalid input, and a big range the
/// Technician has not confirmed, are rejected here, before any Run starts.
#[tauri::command]
pub fn start_lan_scan(
    input: LanScanInput,
    on_update: Channel<LanScanUpdate>,
    runs: tauri::State<'_, Arc<Runs>>,
) -> Result<StartedLanScan, String> {
    let request = LanScanRequest {
        range: input.range,
        confirmed: input.confirmed,
    };
    let scan = request.validate().map_err(|error| error.message)?;
    let addresses = scan.address_count();
    let progress = Progress::default();
    let id = runs.spawn_copyable(&progress, LanScanResult::to_text, {
        let progress = progress.clone();
        move |stop| async move {
            let result =
                lan_scan::run_with_progress(&SystemNetwork, &scan, &stop, &progress, |event| {
                    let _ = on_update.send(event.into());
                })
                .await;
            let _ = on_update.send(LanScanUpdate::Finished {
                result: result.into(),
            });
        }
    });
    Ok(StartedLanScan { id, addresses })
}
