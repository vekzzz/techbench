//! Port Check: tests whether a TCP port on a single Target accepts connections.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::InputError;
use crate::network::{ConnectOutcome, Network};
use crate::run::{RunEnding, RunHeader, milliseconds, text_table};
use crate::target::{self, AddressFamily, Target};

/// How long a port has to accept a connection before it counts as Filtered.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// A Port Check exactly as the Technician filled it in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortCheckRequest {
    pub target: String,
    pub port: String,
    pub family: AddressFamily,
}

/// A Port Check whose input has been validated and is ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortCheck {
    target: Target,
    port: u16,
    family: AddressFamily,
}

impl PortCheckRequest {
    pub fn validate(&self) -> Result<PortCheck, InputError> {
        let target = Target::parse_for(&self.target, self.family)?;
        let port = self
            .port
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| InputError::new("a port is a number from 1 to 65535"))?;
        Ok(PortCheck {
            target,
            port,
            family: self.family,
        })
    }
}

/// Progress a Port Check Run reports as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortCheckEvent {
    Resolved(IpAddr),
    Port(PortReport),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortReport {
    pub port: u16,
    pub outcome: PortOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortOutcome {
    /// The Target accepted the connection.
    Open { connect_time: Duration },
    /// The Target actively refused the connection: it is up, but nothing is listening.
    Closed,
    /// No answer before the timeout, usually because a firewall dropped it.
    Filtered,
}

/// What a Port Check Run reports back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortCheckResult {
    pub header: RunHeader,
    pub ports: Vec<PortReport>,
    pub ending: RunEnding,
}

impl PortCheckResult {
    /// The Result as plain text, ready to paste into a ticket.
    pub fn to_text(&self) -> String {
        let mut text = self.header.to_text("Port Check");
        text.push('\n');
        match &self.ending {
            RunEnding::Completed => {}
            RunEnding::ResolutionFailed { reason } => {
                text.push_str(&format!(
                    "Could not resolve {}: {reason}\n",
                    self.header.target
                ));
                return text;
            }
            RunEnding::Stopped => {
                text.push_str("Stopped before any port answered.\n");
                return text;
            }
        }

        let rows: Vec<Vec<String>> = self
            .ports
            .iter()
            .map(|report| {
                let (outcome, connect_time) = match report.outcome {
                    PortOutcome::Open { connect_time } => ("Open", milliseconds(connect_time)),
                    PortOutcome::Closed => ("Closed", "-".to_string()),
                    PortOutcome::Filtered => ("Filtered", "-".to_string()),
                };
                vec![report.port.to_string(), outcome.to_string(), connect_time]
            })
            .collect();
        text.push_str(&text_table(&["PORT", "OUTCOME", "CONNECT TIME"], &rows));

        let count = |wanted: fn(&PortOutcome) -> bool| {
            self.ports
                .iter()
                .filter(|report| wanted(&report.outcome))
                .count()
        };
        text.push_str(&format!(
            "\n{} port{} checked: {} Open, {} Closed, {} Filtered\n",
            self.ports.len(),
            if self.ports.len() == 1 { "" } else { "s" },
            count(|outcome| matches!(outcome, PortOutcome::Open { .. })),
            count(|outcome| matches!(outcome, PortOutcome::Closed)),
            count(|outcome| matches!(outcome, PortOutcome::Filtered)),
        ));
        text
    }
}

/// Runs a Port Check until it finishes or `stop` is cancelled.
pub async fn run(
    network: &impl Network,
    check: &PortCheck,
    stop: &CancellationToken,
    mut on_event: impl FnMut(PortCheckEvent),
) -> PortCheckResult {
    let started_at = network.now();
    let mut header = RunHeader {
        target: check.target.to_string(),
        resolved: None,
        started_at,
        source_hostname: network.hostname(),
        source_address: None,
    };
    let resolution = tokio::select! {
        resolution = target::resolve(network, &check.target, check.family) => resolution,
        () = stop.cancelled() => {
            return PortCheckResult { header, ports: vec![], ending: RunEnding::Stopped };
        }
    };
    let resolved = match resolution {
        Ok(address) => address,
        Err(reason) => {
            return PortCheckResult {
                header,
                ports: vec![],
                ending: RunEnding::ResolutionFailed { reason },
            };
        }
    };
    header.resolved = Some(resolved);
    header.source_address = network.source_address(resolved);
    on_event(PortCheckEvent::Resolved(resolved));

    let connect = network.tcp_connect(SocketAddr::new(resolved, check.port), CONNECT_TIMEOUT);
    let answer = tokio::select! {
        answer = connect => answer,
        () = stop.cancelled() => {
            return PortCheckResult { header, ports: vec![], ending: RunEnding::Stopped };
        }
    };
    let outcome = match answer {
        ConnectOutcome::Accepted { connect_time } => PortOutcome::Open { connect_time },
        ConnectOutcome::Refused => PortOutcome::Closed,
        ConnectOutcome::TimedOut | ConnectOutcome::Unreachable => PortOutcome::Filtered,
    };
    let report = PortReport {
        port: check.port,
        outcome,
    };
    on_event(PortCheckEvent::Port(report));

    PortCheckResult {
        header,
        ports: vec![report],
        ending: RunEnding::Completed,
    }
}
