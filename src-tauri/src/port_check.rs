//! Port Check commands: start a Run and stream its progress to the UI.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use toolbox_core::network::SystemNetwork;
use toolbox_core::port_check::{
    self, PortCheckEvent, PortCheckRequest, PortCheckResult, PortOutcome, PortReport,
};
use toolbox_core::run::RunEnding;
use toolbox_core::target::AddressFamily;

use crate::runs::{RunId, Runs};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortCheckInput {
    target: String,
    port: String,
    family: Family,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Family {
    Auto,
    Ipv4,
    Ipv6,
}

impl From<Family> for AddressFamily {
    fn from(family: Family) -> Self {
        match family {
            Family::Auto => AddressFamily::Auto,
            Family::Ipv4 => AddressFamily::Ipv4,
            Family::Ipv6 => AddressFamily::Ipv6,
        }
    }
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
    outcome: &'static str,
    connect_time_ms: Option<f64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PortCheckResultView {
    resolved: Option<String>,
    ports: Vec<PortView>,
    ending: &'static str,
    failure_reason: Option<String>,
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
        let (ending, failure_reason) = match result.ending {
            RunEnding::Completed => ("completed", None),
            RunEnding::Stopped => ("stopped", None),
            RunEnding::ResolutionFailed { reason } => ("resolutionFailed", Some(reason)),
        };
        Self {
            resolved: result.header.resolved.map(|address| address.to_string()),
            ports: result.ports.into_iter().map(PortView::from).collect(),
            ending,
            failure_reason,
            text,
        }
    }
}

/// Validates the input, then runs the Port Check in the background. Invalid input is
/// rejected here, before any Run starts.
#[tauri::command]
pub fn start_port_check(
    input: PortCheckInput,
    on_update: Channel<PortCheckUpdate>,
    runs: tauri::State<'_, Arc<Runs>>,
) -> Result<RunId, String> {
    let request = PortCheckRequest {
        target: input.target,
        port: input.port,
        family: input.family.into(),
    };
    let check = request.validate().map_err(|error| error.message)?;
    let (id, stop) = runs.start();
    let runs = Arc::clone(&runs);

    tauri::async_runtime::spawn(async move {
        let result = port_check::run(&SystemNetwork, &check, &stop, |event| {
            let _ = on_update.send(event.into());
        })
        .await;
        let _ = on_update.send(PortCheckUpdate::Finished {
            result: result.into(),
        });
        runs.finish(id);
    });
    Ok(id)
}
