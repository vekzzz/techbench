//! Continuous Ping: a Ping that repeats until the Technician stops it, tracking latency, loss and
//! jitter as it goes. Its memory stays constant however long it runs: running totals, plus one
//! entry per run of lost echoes.

use std::net::IpAddr;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use tokio_util::sync::CancellationToken;

use crate::InputError;
use crate::network::{EchoOutcome, EchoRequest, Network};
use crate::ping::{self, DEFAULT_PAYLOAD_SIZE, DEFAULT_TIMEOUT, INTERVAL};
use crate::run::{self, Opening, Progress, RunEnding, RunHeader, milliseconds, text_table};
use crate::target::{AddressFamily, ChosenTarget};

/// A Continuous Ping exactly as the Technician filled it in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContinuousPingRequest {
    pub target: String,
    pub family: AddressFamily,
}

/// A Continuous Ping whose input has been validated and is ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousPing {
    target: ChosenTarget,
}

impl ContinuousPingRequest {
    pub fn validate(&self) -> Result<ContinuousPing, InputError> {
        Ok(ContinuousPing {
            target: ChosenTarget::parse(&self.target, self.family)?,
        })
    }
}

/// Running totals for a Continuous Ping, kept in constant memory.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PingStatistics {
    pub sent: u64,
    pub received: u64,
    pub min: Option<Duration>,
    pub max: Option<Duration>,
    total_round_trip: Duration,
    last_round_trip: Option<Duration>,
    total_jitter: Duration,
    jitter_samples: u32,
}

impl PingStatistics {
    fn record(&mut self, outcome: EchoOutcome) {
        self.sent += 1;
        let EchoOutcome::Reply { round_trip, .. } = outcome else {
            return;
        };
        self.received += 1;
        self.min = Some(self.min.map_or(round_trip, |min| min.min(round_trip)));
        self.max = Some(self.max.map_or(round_trip, |max| max.max(round_trip)));
        self.total_round_trip += round_trip;
        // Jitter compares each reply with the previous reply, skipping lost echoes between them.
        if let Some(previous) = self.last_round_trip {
            self.total_jitter += round_trip.abs_diff(previous);
            self.jitter_samples += 1;
        }
        self.last_round_trip = Some(round_trip);
    }

    /// The share of echoes that got no reply, rounded to a whole percent.
    pub fn loss_percent(&self) -> u64 {
        if self.sent == 0 {
            return 0;
        }
        ((self.sent - self.received) * 100 + self.sent / 2) / self.sent
    }

    pub fn average(&self) -> Option<Duration> {
        let received = u32::try_from(self.received)
            .ok()
            .filter(|received| *received > 0)?;
        Some(self.total_round_trip / received)
    }

    /// The mean absolute difference between consecutive successful round-trip times.
    pub fn jitter(&self) -> Option<Duration> {
        (self.jitter_samples > 0).then(|| self.total_jitter / self.jitter_samples)
    }
}

/// Consecutive echoes that got no reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LostEchoes {
    pub first: u64,
    pub last: u64,
    /// When the first of them was sent.
    pub started_at: DateTime<FixedOffset>,
}

/// Progress a Continuous Ping reports as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuousPingEvent {
    Resolved(IpAddr),
    Echo {
        sequence: u64,
        outcome: EchoOutcome,
        statistics: PingStatistics,
    },
}

/// What a Continuous Ping reports, so far or at the end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousPingResult {
    pub header: RunHeader,
    pub statistics: PingStatistics,
    /// Every run of lost echoes, oldest first.
    pub lost: Vec<LostEchoes>,
    /// How long echoes have been going out, as of the last update.
    pub running_for: Duration,
    pub ending: RunEnding,
}

impl ContinuousPingResult {
    fn opened(header: RunHeader, ending: RunEnding) -> Self {
        Self {
            header,
            statistics: PingStatistics::default(),
            lost: vec![],
            running_for: Duration::ZERO,
            ending,
        }
    }

    fn record(&mut self, sequence: u64, sent_at: DateTime<FixedOffset>, outcome: EchoOutcome) {
        self.statistics.record(outcome);
        if matches!(outcome, EchoOutcome::Reply { .. }) {
            return;
        }
        match self.lost.last_mut() {
            Some(lost) if lost.last + 1 == sequence => lost.last = sequence,
            _ => self.lost.push(LostEchoes {
                first: sequence,
                last: sequence,
                started_at: sent_at,
            }),
        }
    }

    /// The Result as plain text, ready to paste into a ticket.
    pub fn to_text(&self) -> String {
        let mut text = self.header.to_text("Continuous Ping");
        text.push_str(&format!(
            "Echoes:    every {} s, {DEFAULT_PAYLOAD_SIZE} bytes, {} ms timeout\n\n",
            INTERVAL.as_secs(),
            DEFAULT_TIMEOUT.as_millis(),
        ));
        if !matches!(self.ending, RunEnding::ResolutionFailed { .. }) && self.statistics.sent > 0 {
            if self.lost.is_empty() {
                text.push_str("No echoes were lost.\n");
            } else {
                text.push_str(&self.lost_table());
            }
            text.push('\n');
        }
        text.push_str(&self.summary());
        text.push('\n');
        text
    }

    /// The one-line conclusion: how long it has run, the loss, round trip and jitter.
    pub fn summary(&self) -> String {
        let span = format_span(self.running_for);
        let lead = match &self.ending {
            RunEnding::ResolutionFailed { reason } => {
                return format!("Could not resolve {}: {reason}", self.header.target);
            }
            RunEnding::Running if self.statistics.sent == 0 => {
                return "Running: no echo has finished yet.".to_string();
            }
            _ if self.statistics.sent == 0 => {
                return "Stopped before the first echo finished.".to_string();
            }
            RunEnding::Running => format!("Running for {span}"),
            _ => format!("Stopped after {span}"),
        };
        let statistics = &self.statistics;
        let mut summary = format!(
            "{lead}: {} sent, {} received, {}% loss",
            statistics.sent,
            statistics.received,
            statistics.loss_percent()
        );
        if let (Some(min), Some(average), Some(max)) =
            (statistics.min, statistics.average(), statistics.max)
        {
            summary.push_str(&format!(
                "; round trip min {}, avg {}, max {}",
                milliseconds(min),
                milliseconds(average),
                milliseconds(max)
            ));
        }
        if let Some(jitter) = statistics.jitter() {
            summary.push_str(&format!("; jitter {}", milliseconds(jitter)));
        }
        summary
    }

    fn lost_table(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .lost
            .iter()
            .map(|lost| {
                let echoes = if lost.first == lost.last {
                    format!("#{}", lost.first)
                } else {
                    format!("#{}-#{}", lost.first, lost.last)
                };
                vec![
                    echoes,
                    lost.started_at.format("%H:%M:%S").to_string(),
                    (lost.last - lost.first + 1).to_string(),
                ]
            })
            .collect();
        text_table(&["LOST", "STARTED", "ECHOES"], &rows)
    }
}

/// A span of time as "45s", "3m 05s" or "2h 03m 05s".
fn format_span(span: Duration) -> String {
    let seconds = span.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    match (hours, minutes) {
        (0, 0) => format!("{seconds}s"),
        (0, _) => format!("{minutes}m {seconds:02}s"),
        _ => format!("{hours}h {minutes:02}m {seconds:02}s"),
    }
}

/// Runs a Continuous Ping until `stop` is cancelled.
pub async fn run(
    network: &impl Network,
    ping: &ContinuousPing,
    stop: &CancellationToken,
    on_event: impl FnMut(ContinuousPingEvent),
) -> ContinuousPingResult {
    run_with_progress(network, ping, stop, &Progress::default(), on_event).await
}

/// Runs a Continuous Ping, keeping `progress` up to date so its Result can be read mid-Run.
pub async fn run_with_progress(
    network: &impl Network,
    ping: &ContinuousPing,
    stop: &CancellationToken,
    progress: &Progress<ContinuousPingResult>,
    mut on_event: impl FnMut(ContinuousPingEvent),
) -> ContinuousPingResult {
    let (header, resolved) = match run::open(network, &ping.target, stop).await {
        Opening::Resolved { header, resolved } => (header, resolved),
        Opening::Ended { header, ending } => {
            return progress.finish(ContinuousPingResult::opened(header, ending));
        }
    };
    let started_at = header.started_at;
    progress.set(ContinuousPingResult::opened(header, RunEnding::Running));
    on_event(ContinuousPingEvent::Resolved(resolved));

    let opened = tokio::time::Instant::now();
    let request = EchoRequest {
        address: resolved,
        payload_size: DEFAULT_PAYLOAD_SIZE,
        dont_fragment: false,
        timeout: DEFAULT_TIMEOUT,
    };
    let ending = ping::send_paced(
        network,
        request,
        None,
        stop,
        |sequence, sent_at, outcome| {
            let sent_at = started_at + (sent_at - opened);
            let mut statistics = PingStatistics::default();
            progress.update(|result| {
                result.record(sequence, sent_at, outcome);
                result.running_for = opened.elapsed();
                statistics = result.statistics;
            });
            on_event(ContinuousPingEvent::Echo {
                sequence,
                outcome,
                statistics,
            });
        },
    )
    .await;

    progress.update(|result| {
        result.ending = ending;
        result.running_for = opened.elapsed();
    });
    progress
        .read(Clone::clone)
        .expect("the Run opened, so its Result exists")
}
