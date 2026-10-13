//! What every Run records about itself, whichever Tool it belongs to.

use std::net::IpAddr;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
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
            self.started_at.format("%Y-%m-%d %H:%M:%S %:z"),
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
