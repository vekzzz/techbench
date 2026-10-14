//! Port Check commands: start a Run and stream its progress to the UI.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use toolbox_core::network::SystemNetwork;
use toolbox_core::port_check::{
    self, PortCheckEvent, PortCheckRequest, PortCheckResult, PortOutcome, PortReport,
};
use toolbox_core::run::Progress;

use crate::runs::{AddressFamilyInput, EndingView, RunId, Runs};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortCheckInput {
    target: String,
    ports: String,
    family: AddressFamilyInput,
}

/// One message on a Port Check Run's channel.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PortCheckUpdate {
    Resolved { address: String },
    Port { port: PortView },
    Finished { result: PortCheckResultView },
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PortView {
    port: u16,
    service: Option<&'static str>,
    outcome: &'static str,
    connect_time_ms: Option<f64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PortCheckResultView {
    resolved: Option<String>,
    ports: Vec<PortView>,
    ports_requested: usize,
    #[serde(flatten)]
    ending: EndingView,
    /// The one-line conclusion, worded by the toolbox core.
    summary: String,
    text: String,
}

impl From<PortReport> for PortView {
    fn from(report: PortReport) -> Self {
        let (outcome, connect_time_ms) = match report.outcome {
            PortOutcome::Open { connect_time } => {
                ("open", Some(connect_time.as_secs_f64() * 1000.0))
            }
            PortOutcome::Closed => ("closed", None),
            PortOutcome::Filtered => ("filtered", None),
        };
        Self {
            port: report.port,
            service: port_check::service_name(report.port),
            outcome,
            connect_time_ms,
        }
    }
}

impl From<PortCheckEvent> for PortCheckUpdate {
    fn from(event: PortCheckEvent) -> Self {
        match event {
            PortCheckEvent::Resolved(address) => PortCheckUpdate::Resolved {
                address: address.to_string(),
            },
            PortCheckEvent::Port(report) => PortCheckUpdate::Port {
                port: report.into(),
            },
        }
    }
}

impl From<PortCheckResult> for PortCheckResultView {
    fn from(result: PortCheckResult) -> Self {
        let text = result.to_text();
        let summary = result.summary();
        Self {
            resolved: result.header.resolved.map(|address| address.to_string()),
            ports: result.ports.into_iter().map(PortView::from).collect(),
            ports_requested: result.ports_requested,
            ending: result.ending.into(),
            summary,
            text,
        }
    }
}

/// A Port Check Run the shell has accepted.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedPortCheck {
    id: RunId,
    ports_requested: usize,
}

#[derive(Serialize)]
pub struct PortPresetView {
    name: &'static str,
    ports: &'static [u16],
}

#[tauri::command]
pub fn port_presets() -> Vec<PortPresetView> {
    port_check::PORT_PRESETS
        .iter()
        .map(|preset| PortPresetView {
            name: preset.name,
            ports: preset.ports,
        })
        .collect()
}

/// Validates the input, then runs the Port Check in the background. Invalid input is
/// rejected here, before any Run starts.
#[tauri::command]
pub fn start_port_check(
    input: PortCheckInput,
    on_update: Channel<PortCheckUpdate>,
    runs: tauri::State<'_, Arc<Runs>>,
) -> Result<StartedPortCheck, String> {
    let request = PortCheckRequest {
        target: input.target,
        ports: input.ports,
        family: input.family.into(),
    };
    let check = request.validate().map_err(|error| error.message)?;
    let ports_requested = check.ports().len();
    let progress = Progress::default();
    let id = runs.spawn_copyable(&progress, PortCheckResult::to_text, {
        let progress = progress.clone();
        move |stop| async move {
            let result =
                port_check::run_with_progress(&SystemNetwork, &check, &stop, &progress, |event| {
                    let _ = on_update.send(event.into());
                })
                .await;
            let _ = on_update.send(PortCheckUpdate::Finished {
                result: result.into(),
            });
        }
    });
    Ok(StartedPortCheck {
        id,
        ports_requested,
    })
}
