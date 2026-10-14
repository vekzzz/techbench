//! Traceroute commands: start a Run and stream each Hop, then its hostname, to the UI.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use toolbox_core::network::SystemNetwork;
use toolbox_core::run::Progress;
use toolbox_core::traceroute::{
    self, Hop, Probe, TracerouteEvent, TracerouteRequest, TracerouteResult,
};

use crate::runs::{AddressFamilyInput, EndingView, RunId, Runs};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracerouteInput {
    target: String,
    family: AddressFamilyInput,
}

/// One message on a Traceroute Run's channel.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TracerouteUpdate {
    Resolved { address: String },
    Hop { hop: HopView },
    Hostname { hop: u8, hostname: String },
    Finished { result: TracerouteResultView },
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HopView {
    number: u8,
    address: Option<String>,
    hostname: Option<String>,
    probes: Vec<ProbeView>,
    reached_target: bool,
}

/// One probe: `answered` carries its round trip, `unreachable` and `silent` do not.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProbeView {
    status: &'static str,
    round_trip_ms: Option<f64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TracerouteResultView {
    #[serde(flatten)]
    ending: EndingView,
    /// The one-line conclusion, worded by the toolbox core.
    summary: String,
    text: String,
}

#[derive(Serialize)]
pub struct StartedTraceroute {
    id: RunId,
}

impl From<&Hop> for HopView {
    fn from(hop: &Hop) -> Self {
        Self {
            number: hop.number,
            address: hop.address.map(|address| address.to_string()),
            hostname: hop.hostname.clone(),
            probes: hop
                .probes
                .iter()
                .map(|probe| match probe {
                    Probe::Answered { round_trip, .. } => ProbeView {
                        status: "answered",
                        round_trip_ms: Some(round_trip.as_secs_f64() * 1000.0),
                    },
                    Probe::Unreachable { .. } => ProbeView {
                        status: "unreachable",
                        round_trip_ms: None,
                    },
                    Probe::Silent => ProbeView {
                        status: "silent",
                        round_trip_ms: None,
                    },
                })
                .collect(),
            reached_target: hop.reached_target,
        }
    }
}

impl From<TracerouteEvent> for TracerouteUpdate {
    fn from(event: TracerouteEvent) -> Self {
        match event {
            TracerouteEvent::Resolved(address) => TracerouteUpdate::Resolved {
                address: address.to_string(),
            },
            TracerouteEvent::Hop(hop) => TracerouteUpdate::Hop {
                hop: HopView::from(&hop),
            },
            TracerouteEvent::Hostname { hop, hostname } => {
                TracerouteUpdate::Hostname { hop, hostname }
            }
        }
    }
}

impl From<TracerouteResult> for TracerouteResultView {
    fn from(result: TracerouteResult) -> Self {
        Self {
            summary: result.summary(),
            text: result.to_text(),
            ending: result.ending.into(),
        }
    }
}

/// Validates the input, then traces the path in the background. Invalid input is rejected here,
/// before any Run starts.
#[tauri::command]
pub fn start_traceroute(
    input: TracerouteInput,
    on_update: Channel<TracerouteUpdate>,
    runs: tauri::State<'_, Arc<Runs>>,
) -> Result<StartedTraceroute, String> {
    let request = TracerouteRequest {
        target: input.target,
        family: input.family.into(),
    };
    let traceroute = request.validate().map_err(|error| error.message)?;
    let progress = Progress::default();
    let id = runs.spawn_copyable(&progress, TracerouteResult::to_text, {
        let progress = progress.clone();
        move |stop| async move {
            let result = traceroute::run_with_progress(
                &SystemNetwork,
                &traceroute,
                &stop,
                &progress,
                |event| {
                    let _ = on_update.send(event.into());
                },
            )
            .await;
            let _ = on_update.send(TracerouteUpdate::Finished {
                result: result.into(),
            });
        }
    });
    Ok(StartedTraceroute { id })
}
