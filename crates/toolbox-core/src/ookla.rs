//! Speed Test through Ookla's `speedtest.exe`, which the Technician installs and whose licence
//! they accept themselves: the app never bundles the program and never passes its licence flags.
//! The program prints one JSON object per line (`-f jsonl`); this module turns them into a Result.

use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::network::{Network, OoklaFailure, PublicIp, SpeedTestServer};
use crate::run::Progress;
use crate::speed_test::{
    Latency, Sample, SpeedPhase, SpeedTestEnding, SpeedTestEvent, SpeedTestResult, Throughput,
};

const NOT_INSTALLED: &str = "Ookla's speedtest.exe is not installed. Install it (winget install Ookla.Speedtest.CLI), run it once in a terminal to accept its licence, then try again.";

/// Runs `speedtest.exe`, updating `result` and reporting events as its lines arrive.
pub(crate) async fn run(
    network: &impl Network,
    stop: &CancellationToken,
    progress: &Progress<SpeedTestResult>,
    mut result: SpeedTestResult,
    on_event: &mut (impl FnMut(SpeedTestEvent) + Send),
) -> SpeedTestResult {
    on_event(SpeedTestEvent::Phase(SpeedPhase::Latency));
    let state = Mutex::new((result.clone(), Some(SpeedPhase::Latency)));
    let events = Mutex::new(on_event);

    let handle_line = |line: String| {
        let Ok(line) = serde_json::from_str::<Value>(&line) else {
            return;
        };
        let mut guard = state.lock().unwrap();
        let (result, phase) = &mut *guard;
        let mut emit = |event| (events.lock().unwrap())(event);
        read_line(&line, result, phase, &mut emit);
        progress.set(result.clone());
    };

    let outcome = stop
        .run_until_cancelled(network.ookla_run(&handle_line))
        .await;
    result = state.into_inner().unwrap().0;
    result.ending = match outcome {
        None => SpeedTestEnding::Stopped,
        Some(Ok(())) if result.ending == SpeedTestEnding::Completed => SpeedTestEnding::Completed,
        Some(Ok(())) => SpeedTestEnding::Unavailable {
            reason: "speedtest.exe finished without a result.".to_string(),
        },
        Some(Err(OoklaFailure::NotInstalled)) => SpeedTestEnding::Unavailable {
            reason: NOT_INSTALLED.to_string(),
        },
        Some(Err(OoklaFailure::Failed { message })) => {
            SpeedTestEnding::Unavailable { reason: message }
        }
    };
    progress.finish(result)
}

/// Folds one line of the program's output into the Result.
fn read_line(
    line: &Value,
    result: &mut SpeedTestResult,
    phase: &mut Option<SpeedPhase>,
    emit: &mut impl FnMut(SpeedTestEvent),
) {
    read_who(line, result, emit);
    let kind = line["type"].as_str().unwrap_or_default();
    match kind {
        "ping" => {
            let ping = &line["ping"];
            let finished = ping["progress"].as_f64().unwrap_or(0.0) >= 1.0;
            if let (true, Some(latency)) = (finished, latency_of(ping)) {
                result.latency = Some(latency);
                emit(SpeedTestEvent::Latency(latency));
            }
        }
        "download" | "upload" => {
            let direction = if kind == "download" {
                SpeedPhase::Download
            } else {
                SpeedPhase::Upload
            };
            if *phase != Some(direction) {
                *phase = Some(direction);
                emit(SpeedTestEvent::Phase(direction));
            }
            let body = &line[kind];
            if let (Some(bandwidth), Some(elapsed)) =
                (body["bandwidth"].as_f64(), body["elapsed"].as_f64())
            {
                emit(SpeedTestEvent::Sample(Sample {
                    phase: direction,
                    at: Duration::from_millis(elapsed as u64),
                    bits_per_second: bits(bandwidth),
                }));
            }
        }
        "result" => {
            result.latency = latency_of(&line["ping"]).or(result.latency);
            result.download = line["download"]["bandwidth"].as_f64().map(|b| Throughput {
                bits_per_second: bits(b),
            });
            result.upload = line["upload"]["bandwidth"].as_f64().map(|b| Throughput {
                bits_per_second: bits(b),
            });
            result.download_loaded = milliseconds(&line["download"]["latency"]["iqm"]);
            result.upload_loaded = milliseconds(&line["upload"]["latency"]["iqm"]);
            result.data_used = ["download", "upload"]
                .iter()
                .filter_map(|direction| line[*direction]["bytes"].as_u64())
                .sum();
            result.ending = SpeedTestEnding::Completed;
        }
        _ => {}
    }
}

/// The server, ISP and public address, which the first and the last line both carry.
fn read_who(line: &Value, result: &mut SpeedTestResult, emit: &mut impl FnMut(SpeedTestEvent)) {
    let server = &line["server"];
    if let Some(name) = server["name"].as_str() {
        let location = match server["location"].as_str() {
            Some(place) => format!("{name}, {place}"),
            None => name.to_string(),
        };
        let found = SpeedTestServer {
            provider: "Ookla".to_string(),
            location,
            address: server["ip"].as_str().and_then(|ip| ip.parse().ok()),
        };
        if result.header.server.as_ref() != Some(&found) {
            result.header.server = Some(found.clone());
            emit(SpeedTestEvent::Server(found));
        }
    }
    if let Some(address) = line["interface"]["externalIp"]
        .as_str()
        .and_then(|ip| ip.parse().ok())
    {
        result.header.public = Some(Ok(PublicIp {
            address,
            isp: line["isp"].as_str().map(str::to_string),
            city: None,
        }));
    }
}

/// Ookla reports bandwidth in bytes per second.
fn bits(bandwidth: f64) -> u64 {
    (bandwidth * 8.0) as u64
}

fn milliseconds(value: &Value) -> Option<Duration> {
    value
        .as_f64()
        .map(|ms| Duration::from_micros((ms * 1000.0).round() as u64))
}

fn latency_of(ping: &Value) -> Option<Latency> {
    Some(Latency {
        median: milliseconds(&ping["latency"])?,
        jitter: milliseconds(&ping["jitter"]).unwrap_or(Duration::ZERO),
    })
}
