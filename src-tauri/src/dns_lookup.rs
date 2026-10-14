//! DNS Lookup commands: ask a DNS server, then hand the UI its answer.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use toolbox_core::dns_lookup::{self, DnsLookupRequest, DnsLookupResult, DnsOutcome};
use toolbox_core::network::{DnsAnswer, DnsReply, SystemNetwork};

use crate::runs::{RunId, Runs};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsLookupInput {
    query: String,
    record_type: String,
    server: String,
}

/// The one message on a DNS Lookup Run's channel: its Result.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DnsLookupUpdate {
    Finished { result: DnsLookupResultView },
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecordView {
    name: String,
    record_type: &'static str,
    ttl: u32,
    value: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DnsLookupResultView {
    /// The name actually asked for: the reverse name when the query was an IP address.
    asked: String,
    record_type: &'static str,
    /// Whether the type was chosen because the query is an IP address.
    automatic_type: bool,
    server: Option<String>,
    answered_ms: Option<f64>,
    records: Vec<RecordView>,
    /// Whether the Run ended with an answer worth showing as a failure rather than records.
    failed: bool,
    /// The one-line conclusion, worded by the toolbox core.
    summary: String,
    text: String,
}

#[derive(Serialize)]
pub struct StartedDnsLookup {
    id: RunId,
}

impl From<DnsLookupResult> for DnsLookupResultView {
    fn from(result: DnsLookupResult) -> Self {
        let reply = match &result.outcome {
            DnsOutcome::Replied(reply) => Some(reply),
            DnsOutcome::NoServer | DnsOutcome::Stopped => None,
        };
        let records = match reply {
            Some(DnsReply {
                answer: DnsAnswer::Records(records),
                ..
            }) => records
                .iter()
                .map(|record| RecordView {
                    name: record.name.clone(),
                    record_type: record.record_type.name(),
                    ttl: record.ttl,
                    value: record.value.clone(),
                })
                .collect(),
            _ => vec![],
        };
        Self {
            asked: result.asked.clone(),
            record_type: result.record_type.name(),
            automatic_type: result.automatic_type,
            server: result.server.map(|server| server.to_string()),
            answered_ms: reply.map(|reply| reply.round_trip.as_secs_f64() * 1000.0),
            failed: records.is_empty(),
            records,
            summary: result.summary(),
            text: result.to_text(),
        }
    }
}

/// This PC's own DNS server (the Default Route Adapter's first), for the screen to show.
#[tauri::command]
pub async fn default_dns_server() -> Option<String> {
    dns_lookup::default_server(&SystemNetwork)
        .await
        .map(|server| server.to_string())
}

/// Validates the input, then asks the DNS server in the background. Invalid input is rejected
/// here, before any Run starts.
#[tauri::command]
pub fn start_dns_lookup(
    input: DnsLookupInput,
    on_update: Channel<DnsLookupUpdate>,
    runs: tauri::State<'_, Arc<Runs>>,
) -> Result<StartedDnsLookup, String> {
    let request = DnsLookupRequest {
        query: input.query,
        record_type: input.record_type,
        server: input.server,
    };
    let lookup = request.validate().map_err(|error| error.message)?;
    let id = runs.spawn(move |stop| async move {
        let result = dns_lookup::run(&SystemNetwork, &lookup, &stop).await;
        let _ = on_update.send(DnsLookupUpdate::Finished {
            result: result.into(),
        });
    });
    Ok(StartedDnsLookup { id })
}
