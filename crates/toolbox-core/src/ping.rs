//! Ping: sends ICMP echo requests to a Target and reports reachability and round-trip time.

use std::net::IpAddr;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::InputError;
use crate::continuous_ping::PingStatistics;
use crate::network::{EchoOutcome, EchoRequest, Network};
use crate::run::{self, Opening, Progress, RunEnding, RunHeader, milliseconds, text_table};
use crate::target::{AddressFamily, ChosenTarget};

/// How long after one echo the next is sent.
pub(crate) const INTERVAL: Duration = Duration::from_secs(1);

/// Bytes of data in each echo unless the Technician chooses otherwise.
pub(crate) const DEFAULT_PAYLOAD_SIZE: u16 = 32;

/// How long each echo is given to answer unless the Technician chooses otherwise.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

/// A Ping exactly as the Technician filled it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PingRequest {
    pub target: String,
    /// How many echoes to send.
    pub count: String,
    /// Bytes of data in each echo.
    pub payload_size: String,
    /// How long to wait for each reply, in milliseconds.
    pub timeout_ms: String,
    pub dont_fragment: bool,
    pub family: AddressFamily,
}

impl Default for PingRequest {
    /// 4 echoes of 32 bytes, each given 2 s to answer.
    fn default() -> Self {
        Self {
            target: String::new(),
            count: "4".to_string(),
            payload_size: DEFAULT_PAYLOAD_SIZE.to_string(),
            timeout_ms: DEFAULT_TIMEOUT.as_millis().to_string(),
            dont_fragment: false,
            family: AddressFamily::Auto,
        }
    }
}

/// How a Ping sends its echoes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EchoSettings {
    pub count: u32,
    pub payload_size: u16,
    pub timeout: Duration,
    pub dont_fragment: bool,
}

/// A Ping whose input has been validated and is ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ping {
    target: ChosenTarget,
    settings: EchoSettings,
}

impl PingRequest {
    pub fn validate(&self) -> Result<Ping, InputError> {
        let target = ChosenTarget::parse(&self.target, self.family)?;
        let count = number_in(
            &self.count,
            1,
            1000,
            "the count is a number of echoes from 1 to 1000",
        )?;
        let payload_size = number_in(
            &self.payload_size,
            0,
            65500,
            "the size is from 0 to 65500 bytes",
        )?;
        let timeout_ms = number_in(
            &self.timeout_ms,
            100,
            60_000,
            "the timeout is from 100 to 60000 ms",
        )?;
        Ok(Ping {
            target,
            settings: EchoSettings {
                count,
                payload_size: payload_size as u16,
                timeout: Duration::from_millis(u64::from(timeout_ms)),
                dont_fragment: self.dont_fragment,
            },
        })
    }
}

impl Ping {
    /// How this Ping will send its echoes.
    pub fn settings(&self) -> EchoSettings {
        self.settings
    }
}

/// Parses a whole number within `min..=max`, or rejects it with `explanation`.
fn number_in(text: &str, min: u32, max: u32, explanation: &str) -> Result<u32, InputError> {
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|number| (min..=max).contains(number))
        .ok_or_else(|| InputError::new(explanation))
}

/// The echo request Continuous Ping sends: the same defaults as a Ping the Technician left alone.
pub(crate) fn default_echo_to(address: IpAddr) -> EchoRequest {
    EchoSettings {
        count: 1,
        payload_size: DEFAULT_PAYLOAD_SIZE,
        timeout: DEFAULT_TIMEOUT,
        dont_fragment: false,
    }
    .echo_to(address)
}

impl EchoSettings {
    /// The echo request these settings make for `address`. Don't Fragment applies to IPv4 only:
    /// Windows cannot forbid an IPv6 echo from being fragmented at its source.
    fn echo_to(&self, address: IpAddr) -> EchoRequest {
        EchoRequest {
            address,
            payload_size: self.payload_size,
            dont_fragment: self.dont_fragment && address.is_ipv4(),
            ttl: None,
            timeout: self.timeout,
        }
    }
}

/// Progress a Ping Run reports as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PingEvent {
    Resolved(IpAddr),
    Echo(EchoReport),
}

/// What happened to one echo, numbered from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EchoReport {
    pub sequence: u32,
    pub outcome: EchoOutcome,
}

/// What a Ping Run reports back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PingResult {
    pub header: RunHeader,
    pub settings: EchoSettings,
    /// The echoes that finished, in the order they were sent.
    pub echoes: Vec<EchoReport>,
    pub ending: RunEnding,
}

impl PingResult {
    /// The Result as plain text, ready to paste into a ticket.
    pub fn to_text(&self) -> String {
        let mut text = self.header.to_text("Ping");
        let settings = &self.settings;
        text.push_str(&format!(
            "Echoes:    {} x {} bytes, {} ms timeout{}\n\n",
            settings.count,
            settings.payload_size,
            settings.timeout.as_millis(),
            match (settings.dont_fragment, self.header.resolved) {
                (false, _) => "",
                (true, Some(IpAddr::V6(_))) => ", Don't Fragment not applied (IPv6)",
                (true, _) => ", Don't Fragment",
            },
        ));
        if !self.echoes.is_empty() {
            text.push_str(&self.echo_table());
            text.push('\n');
        }
        text.push_str(&self.summary());
        text.push('\n');
        text
    }

    /// The one-line conclusion: how the Run ended, the loss, and the round-trip times.
    pub fn summary(&self) -> String {
        if let RunEnding::ResolutionFailed { reason } = &self.ending {
            return format!("Could not resolve {}: {reason}", self.header.target);
        }
        let mut statistics = PingStatistics::default();
        for echo in &self.echoes {
            statistics.record(echo.outcome);
        }
        let sent = statistics.sent;
        if sent == 0 {
            return match self.ending {
                RunEnding::Running => "Still running: no echo has finished yet.".to_string(),
                RunEnding::Stopped | RunEnding::Completed | RunEnding::ResolutionFailed { .. } => {
                    "Stopped before the first echo finished.".to_string()
                }
            };
        }
        let count = self.settings.count;
        let lead = match self.ending {
            RunEnding::Stopped => format!("Stopped after {sent} of {count} echoes: "),
            RunEnding::Running => format!("Still running: {sent} of {count} echoes so far: "),
            RunEnding::Completed | RunEnding::ResolutionFailed { .. } => String::new(),
        };
        format!("{lead}{}", statistics.describe(false))
    }

    fn echo_table(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .echoes
            .iter()
            .map(|echo| {
                let (status, time, ttl) = match echo.outcome {
                    EchoOutcome::Reply { round_trip, ttl } => (
                        "Reply".to_string(),
                        milliseconds(round_trip),
                        ttl.map_or_else(|| "-".to_string(), |ttl| ttl.to_string()),
                    ),
                    EchoOutcome::TimedOut => {
                        ("Timed out".to_string(), "-".to_string(), "-".to_string())
                    }
                    EchoOutcome::Unreachable { from } => (
                        from.map_or_else(
                            || "Unreachable".to_string(),
                            |from| format!("Unreachable from {from}"),
                        ),
                        "-".to_string(),
                        "-".to_string(),
                    ),
                    EchoOutcome::TimeExceeded { from, .. } => (
                        from.map_or_else(
                            || "Time exceeded".to_string(),
                            |from| format!("Time exceeded from {from}"),
                        ),
                        "-".to_string(),
                        "-".to_string(),
                    ),
                    EchoOutcome::FragmentationNeeded => (
                        "Fragmentation needed".to_string(),
                        "-".to_string(),
                        "-".to_string(),
                    ),
                };
                vec![echo.sequence.to_string(), status, time, ttl]
            })
            .collect();
        text_table(&["SEQ", "STATUS", "TIME", "TTL"], &rows)
    }
}

/// Runs a Ping until every echo is answered or timed out, or `stop` is cancelled.
pub async fn run(
    network: &impl Network,
    ping: &Ping,
    stop: &CancellationToken,
    on_event: impl FnMut(PingEvent),
) -> PingResult {
    run_with_progress(network, ping, stop, &Progress::default(), on_event).await
}

/// Runs a Ping, keeping `progress` up to date so its Result can be read mid-Run.
pub async fn run_with_progress(
    network: &impl Network,
    ping: &Ping,
    stop: &CancellationToken,
    progress: &Progress<PingResult>,
    mut on_event: impl FnMut(PingEvent),
) -> PingResult {
    let settings = ping.settings;
    let (header, resolved) = match run::open(network, &ping.target, stop).await {
        Opening::Resolved { header, resolved } => (header, resolved),
        Opening::Ended { header, ending } => {
            return progress.finish(PingResult {
                header,
                settings,
                echoes: vec![],
                ending,
            });
        }
    };
    progress.set(PingResult {
        header,
        settings,
        echoes: vec![],
        ending: RunEnding::Running,
    });
    on_event(PingEvent::Resolved(resolved));

    let count = Some(u64::from(settings.count));
    let ending = send_paced(
        network,
        settings.echo_to(resolved),
        count,
        stop,
        |sequence, _, outcome| {
            let report = EchoReport {
                sequence: sequence as u32,
                outcome,
            };
            progress.update(|result| result.echoes.push(report));
            on_event(PingEvent::Echo(report));
        },
    )
    .await;
    progress.update(|result| result.ending = ending);
    progress
        .read(Clone::clone)
        .expect("the Run opened, so its Result exists")
}

/// Sends echoes one second apart until `count` have finished (with None, until stopped) or
/// `stop` is cancelled, handing each to `on_echo` with its sequence number and when it was sent.
/// The next echo goes a second after the previous one was sent, or at once if waiting for its
/// reply already took longer. Says how the Run ended.
pub(crate) async fn send_paced(
    network: &impl Network,
    request: EchoRequest,
    count: Option<u64>,
    stop: &CancellationToken,
    mut on_echo: impl FnMut(u64, tokio::time::Instant, EchoOutcome),
) -> RunEnding {
    let mut sent_at = tokio::time::Instant::now();
    for sequence in 1.. {
        if count.is_some_and(|count| sequence > count) {
            return RunEnding::Completed;
        }
        if sequence > 1 {
            let pause = tokio::time::sleep_until(sent_at + INTERVAL);
            if stop.run_until_cancelled(pause).await.is_none() {
                return RunEnding::Stopped;
            }
        }
        sent_at = tokio::time::Instant::now();
        let Some(outcome) = stop.run_until_cancelled(network.icmp_echo(request)).await else {
            return RunEnding::Stopped;
        };
        on_echo(sequence, sent_at, outcome);
    }
    unreachable!("the sequence numbers outlast any Run")
}
