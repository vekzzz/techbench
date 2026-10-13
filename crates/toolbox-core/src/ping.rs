//! Ping: sends ICMP echo requests to a Target and reports reachability and round-trip time.

use std::net::IpAddr;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::InputError;
use crate::network::{EchoOutcome, EchoRequest, Network};
use crate::run::{RunEnding, RunHeader, milliseconds, text_table};
use crate::target::{AddressFamily, ChosenTarget};

/// How long after one echo the next is sent.
const INTERVAL: Duration = Duration::from_secs(1);

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
            payload_size: "32".to_string(),
            timeout_ms: "2000".to_string(),
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

/// Parses a whole number within `min..=max`, or explains the allowed range.
fn number_in(text: &str, min: u32, max: u32, range: &str) -> Result<u32, InputError> {
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|number| (min..=max).contains(number))
        .ok_or_else(|| InputError::new(range))
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
            if settings.dont_fragment {
                ", Don't Fragment"
            } else {
                ""
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
        let sent = self.echoes.len();
        let round_trips: Vec<Duration> = self
            .echoes
            .iter()
            .filter_map(|echo| match echo.outcome {
                EchoOutcome::Reply { round_trip, .. } => Some(round_trip),
                _ => None,
            })
            .collect();
        if self.ending == RunEnding::Stopped && sent == 0 {
            return "Stopped before the first echo finished.".to_string();
        }
        let received = round_trips.len();
        let lost_percent = ((sent - received) * 100 + sent / 2) / sent;
        let mut summary = format!("{sent} sent, {received} received, {lost_percent}% loss");
        if self.ending == RunEnding::Stopped {
            summary = format!(
                "Stopped after {sent} of {} echoes: {summary}",
                self.settings.count
            );
        }
        if let (Some(min), Some(max)) = (round_trips.iter().min(), round_trips.iter().max()) {
            let average = round_trips.iter().sum::<Duration>() / received as u32;
            summary.push_str(&format!(
                "; round trip min {}, avg {}, max {}",
                milliseconds(*min),
                milliseconds(average),
                milliseconds(*max),
            ));
        }
        summary
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
    mut on_event: impl FnMut(PingEvent),
) -> PingResult {
    let settings = ping.settings;
    let mut header = RunHeader {
        target: ping.target.to_string(),
        resolved: None,
        started_at: network.now(),
        source_hostname: network.hostname(),
        source_address: None,
    };
    let finish = |header, echoes, ending| PingResult {
        header,
        settings,
        echoes,
        ending,
    };

    let Some(resolution) = stop.run_until_cancelled(ping.target.resolve(network)).await else {
        return finish(header, vec![], RunEnding::Stopped);
    };
    let resolved = match resolution {
        Ok(address) => address,
        Err(reason) => return finish(header, vec![], RunEnding::ResolutionFailed { reason }),
    };
    header.resolved = Some(resolved);
    header.source_address = network.source_address(resolved);
    on_event(PingEvent::Resolved(resolved));

    let mut echoes = Vec::new();
    let mut sent_at = tokio::time::Instant::now();
    for sequence in 1..=settings.count {
        if sequence > 1 {
            // The next echo goes a second after the previous one was sent, or at once if
            // waiting for its reply already took longer.
            let pause = tokio::time::sleep_until(sent_at + INTERVAL);
            if stop.run_until_cancelled(pause).await.is_none() {
                return finish(header, echoes, RunEnding::Stopped);
            }
        }
        sent_at = tokio::time::Instant::now();
        let echo = network.icmp_echo(EchoRequest {
            destination: resolved,
            payload_size: settings.payload_size,
            dont_fragment: settings.dont_fragment,
            timeout: settings.timeout,
        });
        let Some(outcome) = stop.run_until_cancelled(echo).await else {
            return finish(header, echoes, RunEnding::Stopped);
        };
        let report = EchoReport { sequence, outcome };
        on_event(PingEvent::Echo(report));
        echoes.push(report);
    }

    finish(header, echoes, RunEnding::Completed)
}
