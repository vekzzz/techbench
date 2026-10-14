//! Traceroute: lists each router Hop between this PC and a Target. Probes go out with a growing
//! TTL; each router that finds it spent answers, so each answer names the next Hop.

use std::net::IpAddr;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::future::join_all;
use futures_util::stream::FuturesUnordered;
use tokio_util::sync::CancellationToken;

use crate::InputError;
use crate::network::{EchoOutcome, EchoRequest, Network};
use crate::ping::DEFAULT_PAYLOAD_SIZE;
use crate::run::{self, Opening, Progress, RunEnding, RunHeader, milliseconds, text_table};
use crate::target::{AddressFamily, ChosenTarget};

/// How many Hops a trace follows before giving up.
const MAX_HOPS: u8 = 30;

/// Probes sent for each Hop.
const PROBES_PER_HOP: usize = 3;

/// How long each probe has to be answered.
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// How long a Hop's reverse lookup may take before its address is left without a hostname.
const HOSTNAME_TIMEOUT: Duration = Duration::from_secs(2);

/// A Traceroute exactly as the Technician filled it in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TracerouteRequest {
    pub target: String,
    pub family: AddressFamily,
}

/// A Traceroute whose input has been validated and is ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Traceroute {
    target: ChosenTarget,
}

impl TracerouteRequest {
    pub fn validate(&self) -> Result<Traceroute, InputError> {
        Ok(Traceroute {
            target: ChosenTarget::parse(&self.target, self.family)?,
        })
    }
}

/// What came back for one probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// A router, or the Target itself, answered after `round_trip`.
    Answered { from: IpAddr, round_trip: Duration },
    /// A router reported the destination unreachable.
    Unreachable { from: IpAddr },
    /// No answer before the timeout: shown as `*`.
    Silent,
}

/// One router on the path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    /// Its position on the path, from 1.
    pub number: u8,
    pub probes: Vec<Probe>,
    /// The first address that answered; None when every probe was silent.
    pub address: Option<IpAddr>,
    /// Its name, once a reverse lookup has found one.
    pub hostname: Option<String>,
    /// Whether the Target itself answered at this Hop.
    pub reached_target: bool,
}

/// Progress a Traceroute reports as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TracerouteEvent {
    Resolved(IpAddr),
    Hop(Hop),
    /// A Hop's hostname, found after the Hop itself was reported.
    Hostname {
        hop: u8,
        hostname: String,
    },
}

/// What a Traceroute reports, so far or at the end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TracerouteResult {
    pub header: RunHeader,
    pub hops: Vec<Hop>,
    pub ending: RunEnding,
}

impl TracerouteResult {
    /// The Result as plain text, ready to paste into a ticket.
    pub fn to_text(&self) -> String {
        let mut text = self.header.to_text("Traceroute");
        text.push('\n');
        if !self.hops.is_empty() {
            text.push_str(&self.hop_table());
            text.push('\n');
        }
        text.push_str(&self.summary());
        text.push('\n');
        text
    }

    /// The one-line conclusion: whether the Target was reached, and how far the trace got.
    pub fn summary(&self) -> String {
        let target = self
            .header
            .resolved
            .map_or_else(|| self.header.target.clone(), |address| address.to_string());
        let found = self.hops.len();
        let hops = |count: usize| format!("{count} hop{}", if count == 1 { "" } else { "s" });
        if let RunEnding::ResolutionFailed { reason } = &self.ending {
            return format!("Could not resolve {}: {reason}", self.header.target);
        }
        if let Some(last) = self.hops.last() {
            if last.reached_target {
                return format!("Reached {target} in {}", hops(found));
            }
            if let Some(Probe::Unreachable { from }) = last
                .probes
                .iter()
                .find(|probe| matches!(probe, Probe::Unreachable { .. }))
            {
                return format!(
                    "Destination unreachable: {from} reported it at hop {}",
                    last.number
                );
            }
        }
        match self.ending {
            RunEnding::Running => format!("Still running: {} found so far", hops(found)),
            RunEnding::Stopped => format!("Stopped after {}", hops(found)),
            RunEnding::Completed | RunEnding::ResolutionFailed { .. } => {
                format!("Did not reach {target} within {MAX_HOPS} hops")
            }
        }
    }

    fn hop_table(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .hops
            .iter()
            .map(|hop| {
                let mut row = vec![
                    hop.number.to_string(),
                    hop.address
                        .map_or_else(|| "*".to_string(), |address| address.to_string()),
                    hop.hostname.clone().unwrap_or_else(|| "-".to_string()),
                ];
                row.extend(hop.probes.iter().map(|probe| match probe {
                    Probe::Answered { round_trip, .. } => milliseconds(*round_trip),
                    Probe::Unreachable { .. } => "!".to_string(),
                    Probe::Silent => "*".to_string(),
                }));
                row
            })
            .collect();
        text_table(
            &[
                "HOP", "ADDRESS", "HOSTNAME", "PROBE 1", "PROBE 2", "PROBE 3",
            ],
            &rows,
        )
    }
}

/// Runs a Traceroute until the Target answers, the Hop limit is reached, or `stop` is cancelled.
pub async fn run(
    network: &impl Network,
    traceroute: &Traceroute,
    stop: &CancellationToken,
    on_event: impl FnMut(TracerouteEvent),
) -> TracerouteResult {
    run_with_progress(network, traceroute, stop, &Progress::default(), on_event).await
}

/// Runs a Traceroute, keeping `progress` up to date so its Result can be read mid-Run.
pub async fn run_with_progress(
    network: &impl Network,
    traceroute: &Traceroute,
    stop: &CancellationToken,
    progress: &Progress<TracerouteResult>,
    mut on_event: impl FnMut(TracerouteEvent),
) -> TracerouteResult {
    let (header, resolved) = match run::open(network, &traceroute.target, stop).await {
        Opening::Resolved { header, resolved } => (header, resolved),
        Opening::Ended { header, ending } => {
            return progress.finish(TracerouteResult {
                header,
                hops: vec![],
                ending,
            });
        }
    };
    progress.set(TracerouteResult {
        header,
        hops: vec![],
        ending: RunEnding::Running,
    });
    on_event(TracerouteEvent::Resolved(resolved));

    // Reverse lookups run beside the probing, so a slow name never holds up the next Hop.
    let mut lookups = FuturesUnordered::new();
    let mut ending = RunEnding::Completed;

    'hops: for ttl in 1..=MAX_HOPS {
        let probes =
            join_all((0..PROBES_PER_HOP).map(|_| network.icmp_echo(probe_to(resolved, ttl))));
        tokio::pin!(probes);
        let outcomes = loop {
            tokio::select! {
                outcomes = &mut probes => break outcomes,
                Some((hop, hostname)) = lookups.next(), if !lookups.is_empty() => {
                    record_hostname(progress, &mut on_event, hop, hostname);
                }
                () = stop.cancelled() => {
                    ending = RunEnding::Stopped;
                    break 'hops;
                }
            }
        };

        let hop = hop_from(ttl, resolved, &outcomes);
        let finished = hop.reached_target || is_unreachable(&hop);
        if let Some(address) = hop.address {
            lookups.push(look_up_hostname(network, ttl, address));
        }
        progress.update(|result| result.hops.push(hop.clone()));
        on_event(TracerouteEvent::Hop(hop));
        if finished {
            break;
        }
    }

    // The last names may still be coming in.
    if ending != RunEnding::Stopped {
        while let Some(found) = stop.run_until_cancelled(lookups.next()).await {
            match found {
                Some((hop, hostname)) => record_hostname(progress, &mut on_event, hop, hostname),
                None => break,
            }
        }
        if stop.is_cancelled() {
            ending = RunEnding::Stopped;
        }
    }

    progress.update(|result| result.ending = ending);
    progress
        .read(Clone::clone)
        .expect("the Run opened, so its Result exists")
}

fn probe_to(address: IpAddr, ttl: u8) -> EchoRequest {
    EchoRequest {
        address,
        payload_size: DEFAULT_PAYLOAD_SIZE,
        dont_fragment: false,
        ttl: Some(ttl),
        timeout: PROBE_TIMEOUT,
    }
}

/// Reverse-looks-up `address`, giving up after [`HOSTNAME_TIMEOUT`]. Yields the Hop it was for.
async fn look_up_hostname(
    network: &impl Network,
    hop: u8,
    address: IpAddr,
) -> (u8, Option<String>) {
    let name = tokio::time::timeout(HOSTNAME_TIMEOUT, network.reverse_lookup(address))
        .await
        .ok()
        .flatten();
    (hop, name)
}

fn record_hostname(
    progress: &Progress<TracerouteResult>,
    on_event: &mut impl FnMut(TracerouteEvent),
    hop: u8,
    hostname: Option<String>,
) {
    let Some(hostname) = hostname else { return };
    progress.update(|result| {
        if let Some(found) = result.hops.iter_mut().find(|found| found.number == hop) {
            found.hostname = Some(hostname.clone());
        }
    });
    on_event(TracerouteEvent::Hostname { hop, hostname });
}

fn hop_from(number: u8, target: IpAddr, outcomes: &[EchoOutcome]) -> Hop {
    let probes: Vec<Probe> = outcomes
        .iter()
        .map(|outcome| match *outcome {
            EchoOutcome::Reply { round_trip, .. } => Probe::Answered {
                from: target,
                round_trip,
            },
            EchoOutcome::TimeExceeded {
                from: Some(from),
                round_trip,
            } => Probe::Answered { from, round_trip },
            EchoOutcome::Unreachable { from: Some(from) } => Probe::Unreachable { from },
            EchoOutcome::TimeExceeded { from: None, .. }
            | EchoOutcome::Unreachable { from: None }
            | EchoOutcome::TimedOut
            | EchoOutcome::FragmentationNeeded => Probe::Silent,
        })
        .collect();
    let address = probes.iter().find_map(|probe| match probe {
        Probe::Answered { from, .. } | Probe::Unreachable { from } => Some(*from),
        Probe::Silent => None,
    });
    let reached_target = outcomes
        .iter()
        .any(|outcome| matches!(outcome, EchoOutcome::Reply { .. }));
    Hop {
        number,
        probes,
        address,
        hostname: None,
        reached_target,
    }
}

/// A Hop where a router said the destination is unreachable and nothing got further.
fn is_unreachable(hop: &Hop) -> bool {
    hop.probes
        .iter()
        .any(|probe| matches!(probe, Probe::Unreachable { .. }))
        && !hop
            .probes
            .iter()
            .any(|probe| matches!(probe, Probe::Answered { .. }))
}
