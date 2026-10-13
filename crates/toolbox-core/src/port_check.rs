//! Port Check: tests whether chosen TCP ports on a single Target accept connections.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use futures_util::{StreamExt, stream};
use tokio_util::sync::CancellationToken;

use crate::InputError;
use crate::network::{ConnectOutcome, Network};
use crate::run::{self, Opening, RunEnding, RunHeader, milliseconds, text_table};
use crate::target::{AddressFamily, ChosenTarget};

/// How long a port has to accept a connection before it counts as Filtered.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// The most ports one Run may check: enough for a check, too few for a scan.
const MAX_PORTS: usize = 1024;

/// How many connects a Run keeps waiting on at once.
const PARALLEL_CONNECTS: usize = 32;

/// A Port Check exactly as the Technician filled it in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortCheckRequest {
    pub target: String,
    /// Ports as typed: a single port, a comma-separated list, or ranges such as `8000-8010`.
    pub ports: String,
    pub family: AddressFamily,
}

/// A Port Check whose input has been validated and is ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortCheck {
    target: ChosenTarget,
    ports: Vec<u16>,
}

impl PortCheckRequest {
    pub fn validate(&self) -> Result<PortCheck, InputError> {
        let target = ChosenTarget::parse(&self.target, self.family)?;
        let ports = parse_ports(&self.ports)?;
        Ok(PortCheck { target, ports })
    }
}

impl PortCheck {
    /// The ports this check will try, in order, each once.
    pub fn ports(&self) -> &[u16] {
        &self.ports
    }
}

/// Parses comma-separated ports and inclusive ranges such as `8000-8010`.
fn parse_ports(text: &str) -> Result<Vec<u16>, InputError> {
    if text.trim().is_empty() {
        return Err(InputError::new(
            "enter at least one port, such as 443 or 22, 80, 8000-8010",
        ));
    }
    let mut ports = Vec::new();
    for entry in text.split(',').map(str::trim) {
        let invalid = || {
            InputError::new(format!(
                "\"{entry}\" is not a port from 1 to 65535 or a range such as 8000-8010"
            ))
        };
        match entry.split_once('-') {
            Some((first, last)) => {
                let (first, last) = (
                    parse_port(first).ok_or_else(invalid)?,
                    parse_port(last).ok_or_else(invalid)?,
                );
                if first > last {
                    return Err(InputError::new(format!(
                        "\"{entry}\" is a backwards range; write it as {last}-{first}"
                    )));
                }
                ports.extend(first..=last);
            }
            None => ports.push(parse_port(entry).ok_or_else(invalid)?),
        }
    }
    ports.sort_unstable();
    ports.dedup();
    if ports.len() > MAX_PORTS {
        return Err(InputError::new(format!(
            "{} ports is more than a Port Check allows ({MAX_PORTS}); narrow the list or ranges",
            ports.len()
        )));
    }
    Ok(ports)
}

fn parse_port(text: &str) -> Option<u16> {
    text.trim().parse::<u16>().ok().filter(|port| *port != 0)
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
    /// The ports checked so far, in port order.
    pub ports: Vec<PortReport>,
    /// How many ports the Run set out to check.
    pub ports_requested: usize,
    pub ending: RunEnding,
}

impl PortCheckResult {
    /// The Result as plain text, ready to paste into a ticket.
    pub fn to_text(&self) -> String {
        let mut text = self.header.to_text("Port Check");
        text.push('\n');
        let has_table =
            !self.ports.is_empty() && !matches!(self.ending, RunEnding::ResolutionFailed { .. });
        if has_table {
            text.push_str(&self.port_table());
            text.push('\n');
        }
        text.push_str(&self.summary());
        text.push('\n');
        text
    }

    /// The one-line conclusion: how the Run ended and how many ports were Open, Closed and Filtered.
    pub fn summary(&self) -> String {
        let checked = self.ports.len();
        let tally = |progress: String| {
            let count = |wanted: fn(&PortOutcome) -> bool| {
                self.ports
                    .iter()
                    .filter(|report| wanted(&report.outcome))
                    .count()
            };
            format!(
                "{progress}: {} Open, {} Closed, {} Filtered",
                count(|outcome| matches!(outcome, PortOutcome::Open { .. })),
                count(|outcome| matches!(outcome, PortOutcome::Closed)),
                count(|outcome| matches!(outcome, PortOutcome::Filtered)),
            )
        };
        match &self.ending {
            RunEnding::ResolutionFailed { reason } => {
                format!("Could not resolve {}: {reason}", self.header.target)
            }
            RunEnding::Stopped if checked == 0 => {
                "Stopped before any port was checked.".to_string()
            }
            RunEnding::Stopped => tally(format!(
                "Stopped after {checked} of {} ports",
                self.ports_requested
            )),
            RunEnding::Completed => tally(format!(
                "{checked} port{} checked",
                if checked == 1 { "" } else { "s" }
            )),
        }
    }

    fn port_table(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .ports
            .iter()
            .map(|report| {
                let (outcome, connect_time) = match report.outcome {
                    PortOutcome::Open { connect_time } => ("Open", milliseconds(connect_time)),
                    PortOutcome::Closed => ("Closed", "-".to_string()),
                    PortOutcome::Filtered => ("Filtered", "-".to_string()),
                };
                let service = service_name(report.port).unwrap_or("-");
                vec![
                    report.port.to_string(),
                    service.to_string(),
                    outcome.to_string(),
                    connect_time,
                ]
            })
            .collect();
        text_table(&["PORT", "SERVICE", "OUTCOME", "CONNECT TIME"], &rows)
    }
}

/// Runs a Port Check until it finishes or `stop` is cancelled.
pub async fn run(
    network: &impl Network,
    check: &PortCheck,
    stop: &CancellationToken,
    mut on_event: impl FnMut(PortCheckEvent),
) -> PortCheckResult {
    let (header, resolved) = match run::open(network, &check.target, stop).await {
        Opening::Resolved { header, resolved } => (header, resolved),
        Opening::Ended { header, ending } => return without_ports(header, check, ending),
    };
    on_event(PortCheckEvent::Resolved(resolved));

    let mut checking = stream::iter(check.ports.iter().copied())
        .map(|port| async move {
            let answer = network
                .tcp_connect(SocketAddr::new(resolved, port), CONNECT_TIMEOUT)
                .await;
            let outcome = match answer {
                ConnectOutcome::Accepted { connect_time } => PortOutcome::Open { connect_time },
                ConnectOutcome::Refused => PortOutcome::Closed,
                ConnectOutcome::TimedOut | ConnectOutcome::Unreachable => PortOutcome::Filtered,
            };
            PortReport { port, outcome }
        })
        .buffer_unordered(PARALLEL_CONNECTS);

    let mut reports = Vec::new();
    let ending = loop {
        match stop.run_until_cancelled(checking.next()).await {
            Some(Some(report)) => {
                on_event(PortCheckEvent::Port(report));
                reports.push(report);
            }
            Some(None) => break RunEnding::Completed,
            None => break RunEnding::Stopped,
        }
    };
    reports.sort_unstable_by_key(|report| report.port);

    PortCheckResult {
        header,
        ports: reports,
        ports_requested: check.ports.len(),
        ending,
    }
}

/// A Result that ended before any port was checked.
fn without_ports(header: RunHeader, check: &PortCheck, ending: RunEnding) -> PortCheckResult {
    PortCheckResult {
        header,
        ports: vec![],
        ports_requested: check.ports.len(),
        ending,
    }
}

/// A hint at what usually listens on a well-known TCP port.
pub fn service_name(port: u16) -> Option<&'static str> {
    let name = match port {
        20 => "FTP data",
        21 => "FTP",
        22 => "SSH",
        23 => "Telnet",
        25 => "SMTP",
        53 => "DNS",
        80 => "HTTP",
        88 => "Kerberos",
        110 => "POP3",
        135 => "RPC",
        139 => "NetBIOS",
        143 => "IMAP",
        389 => "LDAP",
        443 => "HTTPS",
        445 => "SMB",
        464 => "Kerberos password",
        465 => "SMTPS",
        515 => "LPD",
        587 => "SMTP submission",
        631 => "IPP",
        636 => "LDAPS",
        993 => "IMAPS",
        995 => "POP3S",
        1433 => "SQL Server",
        1521 => "Oracle",
        3268 => "Global Catalog",
        3269 => "Global Catalog SSL",
        3306 => "MySQL",
        3389 => "RDP",
        5432 => "PostgreSQL",
        5900 => "VNC",
        5985 => "WinRM",
        5986 => "WinRM HTTPS",
        8080 => "HTTP alt",
        8443 => "HTTPS alt",
        9100 => "JetDirect",
        _ => return None,
    };
    Some(name)
}

/// A named set of commonly checked ports, offered by Port Check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortPreset {
    pub name: &'static str,
    pub ports: &'static [u16],
}

/// The built-in Port Presets. They are not editable.
pub const PORT_PRESETS: [PortPreset; 7] = [
    PortPreset {
        name: "Windows",
        ports: &[135, 445, 3389, 5985, 5986],
    },
    PortPreset {
        name: "Directory",
        ports: &[53, 88, 389, 636, 3268],
    },
    PortPreset {
        name: "Web",
        ports: &[80, 443, 8080, 8443],
    },
    PortPreset {
        name: "Remote",
        ports: &[22, 3389, 5900],
    },
    PortPreset {
        name: "Mail",
        ports: &[25, 465, 587, 993, 995],
    },
    PortPreset {
        name: "Database",
        ports: &[1433, 3306, 5432],
    },
    PortPreset {
        name: "Printing",
        ports: &[9100, 631, 515],
    },
];
