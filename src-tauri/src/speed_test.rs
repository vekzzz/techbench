//! Speed Test commands: start a Run and stream its phases, server and speed to the UI.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use toolbox_core::ip_info::describe_public;
use toolbox_core::network::{SpeedTestServer, SystemNetwork};
use toolbox_core::run::Progress;
use toolbox_core::speed_test::{
    self, Provider, SpeedDirection, SpeedPhase, SpeedTestEnding, SpeedTestEvent, SpeedTestRequest,
    SpeedTestResult, UseCase,
};

use crate::runs::{AddressFamilyInput, RunId, Runs};

/// Who measures the line, as the UI sends it.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderInput {
    Cloudflare,
    Ookla,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedTestInput {
    family: AddressFamilyInput,
    provider: ProviderInput,
    /// The plan's speeds in Mbps as typed; blank for none.
    plan_download: String,
    plan_upload: String,
}

/// One message on a Speed Test Run's channel.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SpeedTestUpdate {
    Phase {
        phase: &'static str,
    },
    Server {
        server: ServerView,
    },
    #[serde(rename_all = "camelCase")]
    Latency {
        median_ms: f64,
        jitter_ms: f64,
    },
    #[serde(rename_all = "camelCase")]
    LoadedProbe {
        direction: &'static str,
        at_seconds: f64,
        latency_ms: f64,
    },
    #[serde(rename_all = "camelCase")]
    Sample {
        phase: &'static str,
        at_seconds: f64,
        mbps: f64,
    },
    Finished {
        result: Box<SpeedTestResultView>,
    },
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerView {
    provider: String,
    location: String,
    address: Option<String>,
}

/// One direction's figures: its speed, and how latency behaved while it ran.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DirectionView {
    mbps: Option<f64>,
    /// Whole percentage of the plan's speed, when the Technician gave one.
    plan_percent: Option<u64>,
    loaded_latency_ms: Option<f64>,
    increase_ms: Option<f64>,
    /// "Low", "Moderate" or "High".
    verdict: Option<&'static str>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RatingView {
    use_case: &'static str,
    /// 1 to 5; None when a figure it needs was not measured.
    stars: Option<u8>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeedTestResultView {
    server: Option<ServerView>,
    /// This PC's name, with the address its internet traffic leaves from and the Adapter it used.
    pc: String,
    /// The public address line, worded by the toolbox core, and whether the lookup failed.
    public: String,
    public_failed: bool,
    latency_median_ms: Option<f64>,
    jitter_ms: Option<f64>,
    download: DirectionView,
    upload: DirectionView,
    ratings: Vec<RatingView>,
    data_used_bytes: u64,
    /// "completed", "stopped", "noInternet", "rateLimited" or "interrupted".
    ending: &'static str,
    /// The one-line conclusion, worded by the toolbox core.
    summary: String,
    text: String,
}

#[derive(Serialize)]
pub struct StartedSpeedTest {
    id: RunId,
}

fn phase_name(phase: SpeedPhase) -> &'static str {
    match phase {
        SpeedPhase::Latency => "latency",
        SpeedPhase::Download => "download",
        SpeedPhase::Upload => "upload",
    }
}

fn direction_name(direction: SpeedDirection) -> &'static str {
    match direction {
        SpeedDirection::Download => "download",
        SpeedDirection::Upload => "upload",
    }
}

fn mbps(bits_per_second: u64) -> f64 {
    bits_per_second as f64 / 1_000_000.0
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

impl From<&SpeedTestServer> for ServerView {
    fn from(server: &SpeedTestServer) -> Self {
        Self {
            provider: server.provider.clone(),
            location: server.location.clone(),
            address: server.address.map(|address| address.to_string()),
        }
    }
}

impl From<SpeedTestEvent> for SpeedTestUpdate {
    fn from(event: SpeedTestEvent) -> Self {
        match event {
            SpeedTestEvent::Phase(phase) => SpeedTestUpdate::Phase {
                phase: phase_name(phase),
            },
            SpeedTestEvent::Server(server) => SpeedTestUpdate::Server {
                server: (&server).into(),
            },
            SpeedTestEvent::Latency(latency) => SpeedTestUpdate::Latency {
                median_ms: milliseconds(latency.median),
                jitter_ms: milliseconds(latency.jitter),
            },
            SpeedTestEvent::LoadedProbe(probe) => SpeedTestUpdate::LoadedProbe {
                direction: direction_name(probe.direction),
                at_seconds: probe.at.as_secs_f64(),
                latency_ms: milliseconds(probe.latency),
            },
            SpeedTestEvent::Sample(sample) => SpeedTestUpdate::Sample {
                phase: phase_name(sample.phase),
                at_seconds: sample.at.as_secs_f64(),
                mbps: mbps(sample.bits_per_second),
            },
        }
    }
}

fn direction_view(result: &SpeedTestResult, direction: SpeedDirection) -> DirectionView {
    let (speed, loaded) = match direction {
        SpeedDirection::Download => (result.download, result.download_loaded),
        SpeedDirection::Upload => (result.upload, result.upload_loaded),
    };
    let bufferbloat = result.bufferbloat(direction);
    DirectionView {
        mbps: speed.map(|speed| mbps(speed.bits_per_second)),
        plan_percent: result.plan_percent(direction),
        loaded_latency_ms: loaded.map(milliseconds),
        increase_ms: bufferbloat.map(|(increase, _)| milliseconds(increase)),
        verdict: bufferbloat.map(|(_, verdict)| verdict.name()),
    }
}

impl From<SpeedTestResult> for SpeedTestResultView {
    fn from(result: SpeedTestResult) -> Self {
        let header = &result.header;
        let mut pc = header.pc_name.clone();
        if let Some(address) = header.pc_address {
            pc.push_str(&format!(" ({address})"));
        }
        if let Some(adapter) = &header.adapter {
            pc.push_str(&format!(" via {adapter}"));
        }
        Self {
            server: header.server.as_ref().map(ServerView::from),
            pc,
            public: describe_public(header.public.as_ref()),
            public_failed: !matches!(header.public, Some(Ok(_))),
            latency_median_ms: result.latency.map(|latency| milliseconds(latency.median)),
            jitter_ms: result.latency.map(|latency| milliseconds(latency.jitter)),
            download: direction_view(&result, SpeedDirection::Download),
            upload: direction_view(&result, SpeedDirection::Upload),
            ratings: result
                .ratings()
                .into_iter()
                .map(|rating| RatingView {
                    use_case: use_case_name(rating.use_case),
                    stars: rating.stars.map(|stars| stars.0),
                })
                .collect(),
            data_used_bytes: result.data_used,
            ending: match result.ending {
                SpeedTestEnding::Running => "running",
                SpeedTestEnding::Completed => "completed",
                SpeedTestEnding::Stopped => "stopped",
                SpeedTestEnding::Unavailable { .. } => "unavailable",
                SpeedTestEnding::NoInternet => "noInternet",
                SpeedTestEnding::RateLimited { .. } => "rateLimited",
                SpeedTestEnding::Interrupted { .. } => "interrupted",
            },
            summary: result.ending.describe(),
            text: result.to_text(),
        }
    }
}

fn use_case_name(use_case: UseCase) -> &'static str {
    use_case.name()
}

/// Whether Ookla's own `speedtest.exe` is installed, so the screen can offer it.
#[tauri::command]
pub async fn ookla_installed() -> bool {
    speed_test::ookla_installed(&SystemNetwork).await
}

/// Why starting a Speed Test needs the Technician's confirmation first (a metered, roaming or
/// over-limit connection), or None when it can just start.
#[tauri::command]
pub async fn speed_test_cost_warning() -> Option<String> {
    speed_test::check_connection(&SystemNetwork).await
}

/// Validates the plan speeds, then starts a Speed Test in the background. Invalid input is
/// rejected here, before any Run starts.
#[tauri::command]
pub fn start_speed_test(
    input: SpeedTestInput,
    on_update: Channel<SpeedTestUpdate>,
    runs: tauri::State<'_, Arc<Runs>>,
) -> Result<StartedSpeedTest, String> {
    let test = SpeedTestRequest {
        family: input.family.into(),
        provider: match input.provider {
            ProviderInput::Cloudflare => Provider::Cloudflare,
            ProviderInput::Ookla => Provider::Ookla,
        },
        plan_download: input.plan_download,
        plan_upload: input.plan_upload,
    }
    .validate()
    .map_err(|error| error.message)?;
    let progress = Progress::default();
    let id = runs.spawn_copyable(&progress, SpeedTestResult::to_text, {
        let progress = progress.clone();
        move |stop| async move {
            let result =
                speed_test::run_with_progress(&SystemNetwork, &test, &stop, &progress, |event| {
                    let _ = on_update.send(event.into());
                })
                .await;
            let _ = on_update.send(SpeedTestUpdate::Finished {
                result: Box::new(result.into()),
            });
        }
    });
    Ok(StartedSpeedTest { id })
}
