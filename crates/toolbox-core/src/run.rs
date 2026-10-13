//! What every Run records about itself, whichever Tool it belongs to.

use std::net::IpAddr;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};

use crate::network::Network;
use crate::target::ChosenTarget;
/// How Results show a moment in time: local time with its UTC offset.
pub(crate) const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S %:z";

/// A moment in time as Results show it, such as `2026-10-13 14:05:09 +08:00`.
pub fn format_time(time: DateTime<FixedOffset>) -> String {
    time.format(TIME_FORMAT).to_string()
}

/// Cancelling it stops a Run, which then finishes with a stopped Result.
pub use tokio_util::sync::CancellationToken;

/// When, from where and against what a Run happened: the facts a ticket reader always asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunHeader {
    /// The Target as the Technician typed it.
    pub target: String,
    /// The Resolved Address, once known.
    pub resolved: Option<IpAddr>,
    pub started_at: DateTime<FixedOffset>,
    pub source_hostname: String,
    /// The address this PC used to reach the Resolved Address.
    pub source_address: Option<IpAddr>,
}

impl RunHeader {
    /// The opening lines of a Result's Copy as text.
    pub(crate) fn to_text(&self, tool: &str) -> String {
        let resolved = self.resolved.map_or_else(
            || "(not resolved)".to_string(),
            |address| address.to_string(),
        );
        let from = match self.source_address {
            Some(address) => format!("{} ({address})", self.source_hostname),
            None => self.source_hostname.clone(),
        };
        format!(
            "{tool}\nTarget:    {}\nResolved:  {resolved}\nTime:      {}\nFrom:      {from}\n",
            self.target,
            self.started_at.format(TIME_FORMAT),
        )
    }
}

/// A duration as whole milliseconds, with anything quicker than 1 ms shown as "<1 ms".
pub(crate) fn milliseconds(duration: Duration) -> String {
    match duration.as_millis() {
        0 => "<1 ms".to_string(),
        millis => format!("{millis} ms"),
    }
}

/// Lays rows out as plain-text columns, two spaces apart, under their headings.
pub(crate) fn text_table(headings: &[&str], rows: &[Vec<String>]) -> String {
    let widths: Vec<usize> = (0..headings.len())
        .map(|column| {
            rows.iter()
                .map(|row| row[column].chars().count())
                .chain([headings[column].len()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: &[&str]| {
        let padded: Vec<String> = cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        format!("{}\n", padded.join("  ").trim_end())
    };

    let mut table = line(headings);
    for row in rows {
        table.push_str(&line(&row.iter().map(String::as_str).collect::<Vec<_>>()));
    }
    table
}

/// How a Run finished.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEnding {
    Completed,
    /// The Technician stopped the Run; its Result holds what was collected so far.
    Stopped,
    /// The Target's hostname could not be turned into a Resolved Address.
    ResolutionFailed {
        reason: String,
    },
}

/// How the start of a Run went: it has a Resolved Address to work with, or it has already ended.
pub(crate) enum Opening {
    Resolved {
        header: RunHeader,
        resolved: IpAddr,
    },
    Ended {
        header: RunHeader,
        ending: RunEnding,
    },
}

/// Starts a Run against `target`: notes when and from which PC, then resolves the Target,
/// unless `stop` is cancelled first.
pub(crate) async fn open(
    network: &impl Network,
    target: &ChosenTarget,
    stop: &CancellationToken,
) -> Opening {
    let mut header = RunHeader {
        target: target.to_string(),
        resolved: None,
        started_at: network.now(),
        source_hostname: network.hostname(),
        source_address: None,
    };
    match stop.run_until_cancelled(target.resolve(network)).await {
        None => Opening::Ended {
            header,
            ending: RunEnding::Stopped,
        },
        Some(Err(reason)) => Opening::Ended {
            header,
            ending: RunEnding::ResolutionFailed { reason },
        },
        Some(Ok(resolved)) => {
            header.resolved = Some(resolved);
            header.source_address = network.source_address(resolved);
            Opening::Resolved { header, resolved }
        }
    }
}
