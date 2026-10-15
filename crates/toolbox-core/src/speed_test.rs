//! Speed Test: how fast the internet line is, measured against a public server (ADR 0003).

use std::net::IpAddr;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::InputError;
use crate::ip_info::{self, describe_public};
use crate::network::{ConnectionCost, Network, PublicIp, SpeedFailure, SpeedTestServer};
use crate::run::{Progress, format_time};
use crate::target::AddressFamily;

/// Empty requests sent to measure idle latency.
const IDLE_PROBES: usize = 20;

/// Connections that download at once: a single one cannot fill a fast line over a long path.
const CONNECTIONS: usize = 4;

/// How long each direction transfers.
const PHASE: Duration = Duration::from_secs(10);

/// The start of a transfer, while TCP is still speeding up, which the speed does not count.
const WARM_UP: Duration = Duration::from_secs(2);

/// How often a probe is sent while a transfer is saturating the line.
const LOADED_PROBE_EVERY: Duration = Duration::from_millis(250);

/// How often a transfer reports the speed it is carrying right now.
const SAMPLE_EVERY: Duration = Duration::from_secs(1);

/// Bytes asked for per download request; the server refuses 100 MB and more.
const DOWNLOAD_REQUEST: u64 = 25_000_000;

/// Bytes sent per upload request.
const UPLOAD_REQUEST: u64 = 10_000_000;

/// The speeds the Technician's ISP plan promises, for comparing a Result against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Plan {
    pub download_bits_per_second: Option<u64>,
    pub upload_bits_per_second: Option<u64>,
}

/// Who measures the line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Provider {
    /// Cloudflare's public speed endpoints, measured by this app (ADR 0003).
    #[default]
    Cloudflare,
    /// Ookla's own `speedtest.exe`, which the Technician installs (ADR 0004).
    Ookla,
}

/// Whether Ookla's program is installed, so the screen can offer it.
pub async fn ookla_installed(network: &impl Network) -> bool {
    network.ookla_installed().await
}

/// A Speed Test exactly as the Technician set it up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpeedTest {
    pub family: AddressFamily,
    pub provider: Provider,
    pub plan: Plan,
}

/// A Speed Test as typed: plan speeds in Mbps, blank for none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedTestRequest {
    pub family: AddressFamily,
    pub provider: Provider,
    pub plan_download: String,
    pub plan_upload: String,
}

impl SpeedTestRequest {
    /// Checks the plan speeds, so a typo is refused before the Run starts.
    pub fn validate(&self) -> Result<SpeedTest, InputError> {
        Ok(SpeedTest {
            family: self.family,
            provider: self.provider,
            plan: Plan {
                download_bits_per_second: plan_speed(&self.plan_download, "download")?,
                upload_bits_per_second: plan_speed(&self.plan_upload, "upload")?,
            },
        })
    }
}

/// A plan speed typed in Mbps as bits per second; None when left blank.
fn plan_speed(text: &str, direction: &str) -> Result<Option<u64>, InputError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse::<f64>() {
        Ok(mbps) if mbps.is_finite() && mbps > 0.0 => Ok(Some((mbps * 1_000_000.0) as u64)),
        _ => Err(InputError::new(format!(
            "The plan {direction} speed must be a positive number of Mbps, such as 200."
        ))),
    }
}

/// Asks whether starting a Run needs the Technician's say-so first: None on a connection that
/// is free to use, otherwise why a Run's data matters here.
pub async fn check_connection(network: &impl Network) -> Option<String> {
    let reason = match network.connection_cost().await {
        ConnectionCost::Unmetered | ConnectionCost::Unknown => return None,
        ConnectionCost::Metered => "This connection is metered",
        ConnectionCost::Roaming => "This connection is roaming",
        ConnectionCost::OverLimit => "This connection is over the data limit set in Windows",
    };
    Some(format!(
        "{reason}. A Speed Test moves a lot of data, 10 s each way: about 0.25 GB on a 100 Mbps line and up to about 2.5 GB at 1 Gbps. Run it anyway?"
    ))
}

/// The stages of a Speed Test Run, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedPhase {
    Latency,
    Download,
    Upload,
}

/// Which way a transfer moves data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedDirection {
    Download,
    Upload,
}

impl SpeedDirection {
    fn phase(self) -> SpeedPhase {
        match self {
            SpeedDirection::Download => SpeedPhase::Download,
            SpeedDirection::Upload => SpeedPhase::Upload,
        }
    }
}

/// One latency probe sent while a transfer was saturating the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadedProbe {
    pub direction: SpeedDirection,
    /// How far into the phase the probe was answered.
    pub at: Duration,
    pub latency: Duration,
}

/// How much latency grows while a direction is saturated, graded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bufferbloat {
    /// Under 30 ms more than idle.
    Low,
    /// 30 to 100 ms more than idle.
    Moderate,
    /// Over 100 ms more than idle: calls and games will suffer while this direction is busy.
    High,
}

impl Bufferbloat {
    fn grade(increase: Duration) -> Self {
        if increase < Duration::from_millis(30) {
            Bufferbloat::Low
        } else if increase <= Duration::from_millis(100) {
            Bufferbloat::Moderate
        } else {
            Bufferbloat::High
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Bufferbloat::Low => "Low",
            Bufferbloat::Moderate => "Moderate",
            Bufferbloat::High => "High",
        }
    }
}

/// Progress a Speed Test Run reports as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeedTestEvent {
    Phase(SpeedPhase),
    Server(SpeedTestServer),
    Latency(Latency),
    LoadedProbe(LoadedProbe),
    /// The speed a transfer carried over the last second, warm-up included.
    Sample(Sample),
}

/// The speed a transfer carried over one second of a phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub phase: SpeedPhase,
    /// How far into the phase this second ended.
    pub at: Duration,
    pub bits_per_second: u64,
}

/// How quickly the server answers an empty request while the line is otherwise idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Latency {
    pub median: Duration,
    /// The mean absolute difference between consecutive latencies, as Continuous Ping defines it.
    pub jitter: Duration,
}

/// How fast a direction carried data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throughput {
    pub bits_per_second: u64,
}

/// How a Speed Test Run finished, or that it is still going.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeedTestEnding {
    /// Not ended yet: the Result so far, as read through [`Progress`].
    Running,
    Completed,
    Stopped,
    /// The provider's program could not run, for a reason it gave.
    Unavailable {
        reason: String,
    },
    /// The server could not be reached at the start.
    NoInternet,
    RateLimited {
        retry_after: Option<Duration>,
    },
    /// The connection broke part-way; whatever finished before is kept.
    Interrupted {
        reason: String,
    },
}

/// When, from where and against what a Speed Test Run happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedTestHeader {
    pub started_at: DateTime<FixedOffset>,
    pub pc_name: String,
    /// The address this PC's internet traffic leaves from, when there is a route to the internet.
    pub pc_address: Option<IpAddr>,
    /// The name of the Adapter carrying the default route.
    pub adapter: Option<String>,
    /// The server that answered; None until one has.
    pub server: Option<SpeedTestServer>,
    /// None when the Run was stopped before the public lookup answered.
    pub public: Option<Result<PublicIp, String>>,
}

/// What a Speed Test Run reports back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedTestResult {
    pub header: SpeedTestHeader,
    pub latency: Option<Latency>,
    /// None when the download did not run to its end.
    pub download: Option<Throughput>,
    /// None when the upload did not run to its end.
    pub upload: Option<Throughput>,
    /// Median latency while downloading, after the warm-up; None unless the download finished.
    pub download_loaded: Option<Duration>,
    /// Median latency while uploading, after the warm-up; None unless the upload finished.
    pub upload_loaded: Option<Duration>,
    /// What the Technician's plan promises, for comparison.
    pub plan: Plan,
    /// Bytes transferred over the line, warm-up included, even by a phase that did not finish.
    pub data_used: u64,
    pub ending: SpeedTestEnding,
}

impl SpeedTestResult {
    /// The Result as plain text, ready to paste into a ticket.
    pub fn to_text(&self) -> String {
        let header = &self.header;
        let server = match &header.server {
            Some(server) => match server.address {
                Some(address) => format!("{} {} ({address})", server.provider, server.location),
                None => format!("{} {}", server.provider, server.location),
            },
            None => "(none answered)".to_string(),
        };
        let mut pc = header.pc_name.clone();
        if let Some(address) = header.pc_address {
            pc.push_str(&format!(" ({address})"));
        }
        if let Some(adapter) = &header.adapter {
            pc.push_str(&format!(" via {adapter}"));
        }
        let latency = match self.latency {
            Some(latency) => format!(
                "{} (jitter {})",
                tenths_of_a_millisecond(latency.median),
                tenths_of_a_millisecond(latency.jitter)
            ),
            None => "not measured".to_string(),
        };
        let mut lines: Vec<String> = vec![
            "Speed Test".to_string(),
            format!("Server:    {server}"),
            format!("Time:      {}", format_time(header.started_at)),
            format!("PC:        {pc}"),
            format!("Public:    {}", describe_public(header.public.as_ref())),
            String::new(),
            format!("Idle latency:  {latency}"),
            format!(
                "Download:      {}",
                self.describe_speed(SpeedDirection::Download)
            ),
            self.under_load_line(SpeedDirection::Download),
            format!(
                "Upload:        {}",
                self.describe_speed(SpeedDirection::Upload)
            ),
            self.under_load_line(SpeedDirection::Upload),
            format!("Data used:     {}", describe_bytes(self.data_used)),
            String::new(),
        ];
        let ratings = self.ratings();
        if ratings.iter().any(|rating| rating.stars.is_some()) {
            lines.push("Ratings:".to_string());
            for rating in ratings {
                let stars = rating
                    .stars
                    .map_or("not rated".to_string(), |stars| stars.to_text());
                lines.push(format!("  {:<12}  {stars}", rating.use_case.name()));
            }
            lines.push(String::new());
        }
        lines.push(self.ending.describe());
        lines.retain(|line| line != "\u{0}");
        lines.join("\n") + "\n"
    }
}

impl SpeedTestEnding {
    /// The closing line of a Result: how the Run ended, and what that means for its figures.
    pub fn describe(&self) -> String {
        match self {
            SpeedTestEnding::Running => "Still running.".to_string(),
            SpeedTestEnding::Completed => "Completed.".to_string(),
            SpeedTestEnding::Stopped => {
                "Stopped before it finished; phases that did not finish are not measured."
                    .to_string()
            }
            SpeedTestEnding::Unavailable { reason } => format!("Could not run the test: {reason}"),
            SpeedTestEnding::NoInternet => {
                "No internet: the speed test server could not be reached.".to_string()
            }
            SpeedTestEnding::RateLimited {
                retry_after: Some(wait),
            } => format!(
                "Rate limited: Cloudflare asked to wait {} before trying again.",
                describe_wait(*wait)
            ),
            SpeedTestEnding::RateLimited { retry_after: None } => {
                "Rate limited: Cloudflare asked to wait before trying again.".to_string()
            }
            SpeedTestEnding::Interrupted { reason } => {
                format!("Interrupted: {reason}. Phases that did not finish are not measured.")
            }
        }
    }
}

/// A wait the way a person says it: `90 s`, or `about 58 min` from two minutes up.
fn describe_wait(wait: Duration) -> String {
    let seconds = wait.as_secs();
    if seconds < 120 {
        format!("{seconds} s")
    } else {
        format!("about {} min", (seconds + 30) / 60)
    }
}

/// A duration in milliseconds with one decimal, such as `17.5 ms`.
fn tenths_of_a_millisecond(duration: Duration) -> String {
    format!("{:.1} ms", duration.as_secs_f64() * 1000.0)
}

/// A byte count as `250.0 MB`, or `1.25 GB` from a gigabyte up.
fn describe_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1e9)
    } else {
        format!("{:.1} MB", bytes as f64 / 1e6)
    }
}

/// Runs a Speed Test, unless `stop` is cancelled first.
pub async fn run(
    network: &impl Network,
    test: &SpeedTest,
    stop: &CancellationToken,
    on_event: impl FnMut(SpeedTestEvent) + Send,
) -> SpeedTestResult {
    run_with_progress(network, test, stop, &Progress::default(), on_event).await
}

/// Runs a Speed Test, keeping `progress` up to date so its Result can be read mid-Run.
pub async fn run_with_progress(
    network: &impl Network,
    test: &SpeedTest,
    stop: &CancellationToken,
    progress: &Progress<SpeedTestResult>,
    mut on_event: impl FnMut(SpeedTestEvent) + Send,
) -> SpeedTestResult {
    let mut result = SpeedTestResult {
        header: SpeedTestHeader {
            started_at: network.now(),
            pc_name: network.hostname(),
            pc_address: ip_info::internet_route_source(network),
            adapter: ip_info::find_default_route(network)
                .await
                .map(|adapter| adapter.name),
            server: None,
            public: None,
        },
        latency: None,
        download: None,
        upload: None,
        download_loaded: None,
        upload_loaded: None,
        plan: test.plan,
        data_used: 0,
        ending: SpeedTestEnding::Running,
    };
    progress.set(result.clone());

    if test.provider == Provider::Ookla {
        return crate::ookla::run(network, stop, progress, result, &mut on_event).await;
    }

    on_event(SpeedTestEvent::Phase(SpeedPhase::Latency));
    let (latency, public) = tokio::join!(
        measure_latency(network, test.family, stop, &mut on_event),
        stop.run_until_cancelled(network.public_ip()),
    );
    result.header.public = public;
    result.header.server = latency.server;
    match latency.outcome {
        Ok(measured) => result.latency = Some(measured),
        Err(ending) => {
            result.ending = ending;
            return progress.finish(result);
        }
    }
    progress.set(result.clone());

    for direction in [SpeedDirection::Download, SpeedDirection::Upload] {
        on_event(SpeedTestEvent::Phase(direction.phase()));
        let transfer = measure_transfer(network, test.family, direction, stop, &mut on_event).await;
        result.data_used += transfer.bytes;
        match direction {
            SpeedDirection::Download => {
                result.download = transfer.speed;
                result.download_loaded = transfer.loaded_latency;
            }
            SpeedDirection::Upload => {
                result.upload = transfer.speed;
                result.upload_loaded = transfer.loaded_latency;
            }
        }
        match transfer.end {
            PhaseEnd::Finished => progress.set(result.clone()),
            PhaseEnd::Stopped => {
                result.ending = SpeedTestEnding::Stopped;
                return progress.finish(result);
            }
            PhaseEnd::Failed(failure) => {
                result.ending = ending_for(failure, false);
                return progress.finish(result);
            }
        }
    }
    result.ending = SpeedTestEnding::Completed;
    progress.finish(result)
}

/// What the idle latency phase found.
struct LatencyPhase {
    server: Option<SpeedTestServer>,
    /// The latency, or how the Run ended instead.
    outcome: Result<Latency, SpeedTestEnding>,
}

async fn measure_latency(
    network: &impl Network,
    family: AddressFamily,
    stop: &CancellationToken,
    on_event: &mut impl FnMut(SpeedTestEvent),
) -> LatencyPhase {
    let mut server = None;
    let mut samples = Vec::with_capacity(IDLE_PROBES);
    for _ in 0..IDLE_PROBES {
        let Some(probe) = stop.run_until_cancelled(network.speed_probe(family)).await else {
            return LatencyPhase {
                server,
                outcome: Err(SpeedTestEnding::Stopped),
            };
        };
        match probe {
            Ok(probe) => {
                if server.is_none() {
                    server = Some(probe.server.clone());
                    on_event(SpeedTestEvent::Server(probe.server));
                }
                samples.push(probe.round_trip.saturating_sub(probe.server_time));
            }
            Err(failure) => {
                return LatencyPhase {
                    server,
                    outcome: Err(ending_for(failure, samples.is_empty())),
                };
            }
        }
    }
    let latency = Latency {
        jitter: jitter(&samples),
        median: median(&mut samples),
    };
    on_event(SpeedTestEvent::Latency(latency));
    LatencyPhase {
        server,
        outcome: Ok(latency),
    }
}

/// How a transfer phase ended.
enum PhaseEnd {
    Finished,
    Stopped,
    Failed(SpeedFailure),
}

/// What a transfer phase found.
struct TransferPhase {
    /// None unless the phase ran to its end.
    speed: Option<Throughput>,
    /// Median latency after the warm-up; None unless the phase ran to its end.
    loaded_latency: Option<Duration>,
    bytes: u64,
    end: PhaseEnd,
}

/// Moves data in one direction over several connections for [`PHASE`]. The speed counts only
/// what moved after the warm-up, and only when the phase ran to its end.
async fn measure_transfer(
    network: &impl Network,
    family: AddressFamily,
    direction: SpeedDirection,
    stop: &CancellationToken,
    on_event: &mut impl FnMut(SpeedTestEvent),
) -> TransferPhase {
    let on_event = Mutex::new(on_event);
    let started = Instant::now();
    let arrivals: Mutex<Vec<(Instant, u64)>> = Mutex::new(Vec::new());
    let bytes_between = |from: Instant, to: Instant| -> u64 {
        arrivals
            .lock()
            .unwrap()
            .iter()
            .filter(|(at, _)| *at > from && *at <= to)
            .map(|(_, bytes)| bytes)
            .sum()
    };
    let record = |bytes: u64| arrivals.lock().unwrap().push((Instant::now(), bytes));
    let connections = async {
        let mut all: FuturesUnordered<_> = (0..CONNECTIONS)
            .map(|_| async {
                loop {
                    let moved = match direction {
                        SpeedDirection::Upload => {
                            network.speed_upload(family, UPLOAD_REQUEST, &record).await
                        }
                        SpeedDirection::Download => {
                            network
                                .speed_download(family, DOWNLOAD_REQUEST, &record)
                                .await
                        }
                    };
                    if let Err(failure) = moved {
                        return failure;
                    }
                }
            })
            .collect();
        // Connections only ever end by failing.
        all.next().await.expect("there are connections")
    };
    let sampling = async {
        let mut at = started;
        loop {
            at += SAMPLE_EVERY;
            tokio::time::sleep_until(at).await;
            (on_event.lock().unwrap_or_else(PoisonError::into_inner))(SpeedTestEvent::Sample(
                Sample {
                    phase: direction.phase(),
                    at: at - started,
                    bits_per_second: bits_per_second(
                        bytes_between(at - SAMPLE_EVERY, at),
                        SAMPLE_EVERY,
                    ),
                },
            ));
        }
    };
    let loaded: Mutex<Vec<(Instant, Duration)>> = Mutex::new(Vec::new());
    let probing = async {
        let mut at = started;
        loop {
            at += LOADED_PROBE_EVERY;
            tokio::time::sleep_until(at).await;
            // A probe that fails says nothing about latency; the transfers report real failures.
            if let Ok(probe) = network.speed_probe(family).await {
                let latency = probe.round_trip.saturating_sub(probe.server_time);
                loaded.lock().unwrap().push((Instant::now(), latency));
                (on_event.lock().unwrap_or_else(PoisonError::into_inner))(
                    SpeedTestEvent::LoadedProbe(LoadedProbe {
                        direction,
                        at: Instant::now() - started,
                        latency,
                    }),
                );
            }
        }
    };
    let end = tokio::select! {
        biased;
        failure = connections => PhaseEnd::Failed(failure),
        () = tokio::time::sleep_until(started + PHASE) => PhaseEnd::Finished,
        () = stop.cancelled() => PhaseEnd::Stopped,
        () = sampling => unreachable!("sampling never ends"),
        () = probing => unreachable!("probing never ends"),
    };

    let measured = bytes_between(started + WARM_UP, started + PHASE);
    let speed = matches!(end, PhaseEnd::Finished).then(|| Throughput {
        bits_per_second: bits_per_second(measured, PHASE - WARM_UP),
    });
    let bytes = arrivals
        .lock()
        .unwrap()
        .iter()
        .map(|(_, bytes)| bytes)
        .sum();
    let loaded_latency = matches!(end, PhaseEnd::Finished)
        .then(|| {
            let mut settled: Vec<Duration> = loaded
                .lock()
                .unwrap()
                .iter()
                .filter(|(at, _)| *at > started + WARM_UP)
                .map(|(_, latency)| *latency)
                .collect();
            (!settled.is_empty()).then(|| median(&mut settled))
        })
        .flatten();
    TransferPhase {
        speed,
        loaded_latency,
        bytes,
        end,
    }
}

fn bits_per_second(bytes: u64, over: Duration) -> u64 {
    (u128::from(bytes) * 8 * 1000 / over.as_millis()) as u64
}

/// How a failed request ends the Run: before anything was measured the line is simply not there.
fn ending_for(failure: SpeedFailure, nothing_measured: bool) -> SpeedTestEnding {
    match failure {
        SpeedFailure::Unreachable if nothing_measured => SpeedTestEnding::NoInternet,
        SpeedFailure::Unreachable => SpeedTestEnding::Interrupted {
            reason: "the server stopped answering".to_string(),
        },
        SpeedFailure::RateLimited { retry_after } => SpeedTestEnding::RateLimited { retry_after },
        SpeedFailure::Interrupted { reason } => SpeedTestEnding::Interrupted { reason },
    }
}

/// The middle value, or the mean of the middle two. `samples` must not be empty.
fn median(samples: &mut [Duration]) -> Duration {
    samples.sort();
    let middle = samples.len() / 2;
    if samples.len() % 2 == 1 {
        samples[middle]
    } else {
        (samples[middle - 1] + samples[middle]) / 2
    }
}

/// The mean absolute difference between consecutive samples; zero for fewer than two.
fn jitter(samples: &[Duration]) -> Duration {
    let steps = samples.len().saturating_sub(1);
    if steps == 0 {
        return Duration::ZERO;
    }
    let total: Duration = samples
        .windows(2)
        .map(|pair| pair[0].abs_diff(pair[1]))
        .sum();
    total / steps as u32
}

/// What a Speed Test result says a line can be used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseCase {
    WebBrowsing,
    Streaming,
    Gaming,
    VideoCalls,
}

impl UseCase {
    pub const ALL: [UseCase; 4] = [
        UseCase::WebBrowsing,
        UseCase::Streaming,
        UseCase::Gaming,
        UseCase::VideoCalls,
    ];

    pub fn name(self) -> &'static str {
        match self {
            UseCase::WebBrowsing => "Web Browsing",
            UseCase::Streaming => "Streaming",
            UseCase::Gaming => "Gaming",
            UseCase::VideoCalls => "Video Calls",
        }
    }
}

/// How well a line handles a use case, from 1 to 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stars(pub u8);

impl Stars {
    /// `***--`: filled stars, then empty ones, in plain ASCII for pasting anywhere.
    pub fn to_text(self) -> String {
        "*".repeat(usize::from(self.0)) + &"-".repeat(usize::from(5 - self.0))
    }
}

/// One use case's rating; None when a figure it needs was not measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rating {
    pub use_case: UseCase,
    pub stars: Option<Stars>,
}

/// Stars for a figure where more is better: `limits` are the least for 5, 4, 3 and 2 stars.
fn stars_at_least(value: f64, limits: [f64; 4]) -> u8 {
    5 - limits.iter().take_while(|limit| value < **limit).count() as u8
}

/// Stars for a figure where less is better: `limits` are the most for 5, 4, 3 and 2 stars.
fn stars_at_most(value: f64, limits: [f64; 4]) -> u8 {
    5 - limits.iter().take_while(|limit| value > **limit).count() as u8
}

impl SpeedTestResult {
    /// How much of the plan's speed `direction` reached, as a whole percentage; None without a
    /// plan speed or a measured one. Can pass 100.
    pub fn plan_percent(&self, direction: SpeedDirection) -> Option<u64> {
        let (measured, plan) = match direction {
            SpeedDirection::Download => (self.download?, self.plan.download_bits_per_second?),
            SpeedDirection::Upload => (self.upload?, self.plan.upload_bits_per_second?),
        };
        Some((measured.bits_per_second * 100 + plan / 2) / plan)
    }

    /// A speed as `38.0 Mbps`, with `(19% of 200 Mbps plan)` after it when there is a plan.
    fn describe_speed(&self, direction: SpeedDirection) -> String {
        let (speed, plan) = match direction {
            SpeedDirection::Download => (self.download, self.plan.download_bits_per_second),
            SpeedDirection::Upload => (self.upload, self.plan.upload_bits_per_second),
        };
        let Some(speed) = speed else {
            return "not measured".to_string();
        };
        let mut text = format!("{:.1} Mbps", speed.bits_per_second as f64 / 1_000_000.0);
        if let (Some(percent), Some(plan)) = (self.plan_percent(direction), plan) {
            let plan = plan as f64 / 1_000_000.0;
            let plan = if plan.fract() == 0.0 {
                format!("{plan:.0}")
            } else {
                format!("{plan:.1}")
            };
            text.push_str(&format!(" ({percent}% of {plan} Mbps plan)"));
        }
        text
    }

    /// The line under a speed with the latency while it ran, or a marker `to_text` drops.
    fn under_load_line(&self, direction: SpeedDirection) -> String {
        let loaded = match direction {
            SpeedDirection::Download => self.download_loaded,
            SpeedDirection::Upload => self.upload_loaded,
        };
        match (loaded, self.bufferbloat(direction)) {
            (Some(loaded), Some((increase, verdict))) => format!(
                "  under load:  {} (+{}, {})",
                tenths_of_a_millisecond(loaded),
                tenths_of_a_millisecond(increase),
                verdict.name()
            ),
            _ => "\u{0}".to_string(),
        }
    }

    /// How much latency grew while `direction` was saturated, and how bad that is. None unless
    /// both idle and loaded latency were measured.
    pub fn bufferbloat(&self, direction: SpeedDirection) -> Option<(Duration, Bufferbloat)> {
        let loaded = match direction {
            SpeedDirection::Download => self.download_loaded,
            SpeedDirection::Upload => self.upload_loaded,
        }?;
        let increase = loaded.saturating_sub(self.latency?.median);
        Some((increase, Bufferbloat::grade(increase)))
    }

    /// How well the measured line handles each use case. Each rating is the lowest of the stars
    /// its figures earn, so one weak figure cannot hide behind strong ones. A use case is not
    /// rated when a figure it needs was not measured.
    pub fn ratings(&self) -> Vec<Rating> {
        if self.ending != SpeedTestEnding::Completed {
            // A Run that did not finish says too little to rate a connection by.
            return UseCase::ALL
                .into_iter()
                .map(|use_case| Rating {
                    use_case,
                    stars: None,
                })
                .collect();
        }
        let mbps = |throughput: Option<Throughput>| {
            throughput.map(|throughput| throughput.bits_per_second as f64 / 1_000_000.0)
        };
        let download = mbps(self.download);
        let upload = mbps(self.upload);
        let latency = self.latency.map(|l| l.median.as_secs_f64() * 1000.0);
        let jitter = self.latency.map(|l| l.jitter.as_secs_f64() * 1000.0);

        let lowest = |stars: [Option<u8>; 3]| -> Option<Stars> {
            stars
                .into_iter()
                .collect::<Option<Vec<u8>>>()
                .and_then(|all| all.into_iter().min())
                .map(Stars)
        };
        UseCase::ALL
            .into_iter()
            .map(|use_case| {
                let stars = match use_case {
                    UseCase::WebBrowsing => lowest([
                        download.map(|v| stars_at_least(v, [25.0, 10.0, 5.0, 2.0])),
                        latency.map(|v| stars_at_most(v, [50.0, 100.0, 200.0, 400.0])),
                        Some(5),
                    ]),
                    UseCase::Streaming => lowest([
                        download.map(|v| stars_at_least(v, [50.0, 25.0, 10.0, 5.0])),
                        jitter.map(|v| stars_at_most(v, [30.0, 60.0, 100.0, 200.0])),
                        Some(5),
                    ]),
                    UseCase::Gaming => lowest([
                        latency.map(|v| stars_at_most(v, [30.0, 60.0, 100.0, 150.0])),
                        jitter.map(|v| stars_at_most(v, [10.0, 20.0, 40.0, 80.0])),
                        Some(5),
                    ]),
                    UseCase::VideoCalls => lowest([
                        upload.map(|v| stars_at_least(v, [10.0, 5.0, 3.0, 1.0])),
                        latency.map(|v| stars_at_most(v, [100.0, 150.0, 250.0, 400.0])),
                        jitter.map(|v| stars_at_most(v, [20.0, 30.0, 50.0, 100.0])),
                    ]),
                };
                Rating { use_case, stars }
            })
            .collect()
    }
}
