//! Continuous Ping commands: start a Run per Target and stream each echo, with the running
//! totals, to the UI.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use toolbox_core::continuous_ping::{
    self, ContinuousPingEvent, ContinuousPingRequest, ContinuousPingResult, PingStatistics,
};
use toolbox_core::ip_info;
use toolbox_core::network::SystemNetwork;
use toolbox_core::run::Progress;

use crate::ping::EchoView;
use crate::runs::{AddressFamilyInput, EndingView, RunId, Runs};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuousPingInput {
    target: String,
    family: AddressFamilyInput,
}

/// The running totals after an echo, in milliseconds where they are times.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsView {
    sent: u64,
    received: u64,
    loss_percent: u64,
    min_ms: Option<f64>,
    avg_ms: Option<f64>,
    max_ms: Option<f64>,
    jitter_ms: Option<f64>,
}

impl From<PingStatistics> for StatisticsView {
    fn from(statistics: PingStatistics) -> Self {
        let ms = |time: Option<std::time::Duration>| time.map(|time| time.as_secs_f64() * 1000.0);
        Self {
            sent: statistics.sent,
            received: statistics.received,
            loss_percent: statistics.loss_percent(),
            min_ms: ms(statistics.min),
            avg_ms: ms(statistics.average()),
            max_ms: ms(statistics.max),
            jitter_ms: ms(statistics.jitter()),
        }
    }
}

/// One message on a Continuous Ping Run's channel.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ContinuousPingUpdate {
    Resolved {
        address: String,
    },
    Echo {
        echo: EchoView,
        statistics: StatisticsView,
    },
    Finished {
        result: ContinuousPingResultView,
    },
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContinuousPingResultView {
    #[serde(flatten)]
    ending: EndingView,
    /// The one-line conclusion, worded by the toolbox core.
    summary: String,
    text: String,
}

#[derive(Serialize)]
pub struct StartedContinuousPing {
    id: RunId,
}

impl From<ContinuousPingEvent> for ContinuousPingUpdate {
    fn from(event: ContinuousPingEvent) -> Self {
        match event {
            ContinuousPingEvent::Resolved(address) => ContinuousPingUpdate::Resolved {
                address: address.to_string(),
            },
            ContinuousPingEvent::Echo {
                sequence,
                outcome,
                statistics,
            } => ContinuousPingUpdate::Echo {
                echo: EchoView::new(sequence, outcome),
                statistics: statistics.into(),
            },
        }
    }
}

impl From<ContinuousPingResult> for ContinuousPingResultView {
    fn from(result: ContinuousPingResult) -> Self {
        Self {
            summary: result.summary(),
            text: result.to_text(),
            ending: result.ending.into(),
        }
    }
}

/// The Default Route Adapter's gateway, offered as a quick Target; None when there is none to offer.
#[tauri::command]
pub async fn default_gateway() -> Option<String> {
    ip_info::default_gateway(&SystemNetwork)
        .await
        .map(|gateway| gateway.to_string())
}

/// Validates the input, then pings the Target every second until the Run is stopped.
#[tauri::command]
pub fn start_continuous_ping(
    input: ContinuousPingInput,
    on_update: Channel<ContinuousPingUpdate>,
    runs: tauri::State<'_, Arc<Runs>>,
) -> Result<StartedContinuousPing, String> {
    let request = ContinuousPingRequest {
        target: input.target,
        family: input.family.into(),
    };
    let ping = request.validate().map_err(|error| error.message)?;
    let progress = Progress::default();
    let id = runs.spawn_copyable(&progress, ContinuousPingResult::to_text, {
        let progress = progress.clone();
        move |stop| async move {
            let result = continuous_ping::run_with_progress(
                &SystemNetwork,
                &ping,
                &stop,
                &progress,
                |event| {
                    let _ = on_update.send(event.into());
                },
            )
            .await;
            let _ = on_update.send(ContinuousPingUpdate::Finished {
                result: result.into(),
            });
        }
    });
    Ok(StartedContinuousPing { id })
}
