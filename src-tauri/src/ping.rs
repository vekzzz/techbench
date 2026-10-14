//! Ping commands: start a Run and stream each echo to the UI.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use toolbox_core::network::{EchoOutcome, SystemNetwork};
use toolbox_core::ping::{self, EchoReport, PingEvent, PingRequest, PingResult};
use toolbox_core::run::Progress;

use crate::runs::{AddressFamilyInput, EndingView, RunId, Runs};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingInput {
    target: String,
    count: String,
    payload_size: String,
    timeout_ms: String,
    dont_fragment: bool,
    family: AddressFamilyInput,
}

/// The editable fields' starting values, as the toolbox core defines them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingDefaults {
    count: String,
    payload_size: String,
    timeout_ms: String,
}

/// One message on a Ping Run's channel.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PingUpdate {
    Resolved { address: String },
    Echo { echo: EchoView },
    Finished { result: PingResultView },
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EchoView {
    sequence: u64,
    status: &'static str,
    round_trip_ms: Option<f64>,
    ttl: Option<u8>,
    /// Who reported the destination unreachable, when known.
    from: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PingResultView {
    resolved: Option<String>,
    echoes: Vec<EchoView>,
    #[serde(flatten)]
    ending: EndingView,
    /// The one-line conclusion, worded by the toolbox core.
    summary: String,
    text: String,
}

/// A Ping Run the shell has accepted.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedPing {
    id: RunId,
    echoes_requested: u32,
}

impl EchoView {
    pub(crate) fn new(sequence: u64, outcome: EchoOutcome) -> Self {
        let (status, round_trip_ms, ttl, from) = match outcome {
            EchoOutcome::Reply { round_trip, ttl } => {
                ("reply", Some(round_trip.as_secs_f64() * 1000.0), ttl, None)
            }
            EchoOutcome::TimedOut => ("timedOut", None, None, None),
            EchoOutcome::Unreachable { from } => (
                "unreachable",
                None,
                None,
                from.map(|address| address.to_string()),
            ),
            EchoOutcome::FragmentationNeeded => ("fragmentationNeeded", None, None, None),
        };
        Self {
            sequence,
            status,
            round_trip_ms,
            ttl,
            from,
        }
    }
}

impl From<EchoReport> for EchoView {
    fn from(echo: EchoReport) -> Self {
        EchoView::new(u64::from(echo.sequence), echo.outcome)
    }
}

impl From<PingEvent> for PingUpdate {
    fn from(event: PingEvent) -> Self {
        match event {
            PingEvent::Resolved(address) => PingUpdate::Resolved {
                address: address.to_string(),
            },
            PingEvent::Echo(echo) => PingUpdate::Echo { echo: echo.into() },
        }
    }
}

impl From<PingResult> for PingResultView {
    fn from(result: PingResult) -> Self {
        let text = result.to_text();
        let summary = result.summary();
        Self {
            resolved: result.header.resolved.map(|address| address.to_string()),
            echoes: result.echoes.into_iter().map(EchoView::from).collect(),
            ending: result.ending.into(),
            summary,
            text,
        }
    }
}

#[tauri::command]
pub fn ping_defaults() -> PingDefaults {
    let defaults = PingRequest::default();
    PingDefaults {
        count: defaults.count,
        payload_size: defaults.payload_size,
        timeout_ms: defaults.timeout_ms,
    }
}

/// Validates the input, then runs the Ping in the background. Invalid input is rejected
/// here, before any Run starts.
#[tauri::command]
pub fn start_ping(
    input: PingInput,
    on_update: Channel<PingUpdate>,
    runs: tauri::State<'_, Arc<Runs>>,
) -> Result<StartedPing, String> {
    let request = PingRequest {
        target: input.target,
        count: input.count,
        payload_size: input.payload_size,
        timeout_ms: input.timeout_ms,
        dont_fragment: input.dont_fragment,
        family: input.family.into(),
    };
    let ping = request.validate().map_err(|error| error.message)?;
    let echoes_requested = ping.settings().count;
    let progress = Progress::default();
    let id = runs.spawn_copyable(&progress, PingResult::to_text, {
        let progress = progress.clone();
        move |stop| async move {
            let result =
                ping::run_with_progress(&SystemNetwork, &ping, &stop, &progress, |event| {
                    let _ = on_update.send(event.into());
                })
                .await;
            let _ = on_update.send(PingUpdate::Finished {
                result: result.into(),
            });
        }
    });
    Ok(StartedPing {
        id,
        echoes_requested,
    })
}
