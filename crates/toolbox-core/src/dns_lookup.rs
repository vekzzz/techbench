//! DNS Lookup: asks a DNS server for a name's records, or for an address's name. The server is
//! asked directly, so the hosts file never gets a say: the Technician sees what DNS says.

use std::net::{IpAddr, SocketAddr};

use chrono::{DateTime, FixedOffset};
use tokio_util::sync::CancellationToken;

use crate::InputError;
use crate::ip_info;
use crate::network::{DnsAnswer, DnsRecord, DnsReply, Network, RecordType};
use crate::run::{format_time, milliseconds, text_table};
use crate::target::{AddressFamily, ChosenTarget};

/// The port DNS servers listen on.
const DNS_PORT: u16 = 53;

/// A DNS Lookup exactly as the Technician filled it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsLookupRequest {
    /// A name to look up, or an IP address to look the name of up.
    pub query: String,
    /// The record type wanted: `A`, `AAAA`, `CNAME`, `MX`, `TXT`, `NS`, `PTR` or `SOA`. Ignored
    /// when `query` is an IP address, which is always looked up as `PTR`.
    pub record_type: String,
    /// The DNS server's IP address. Empty: this PC's own, from the Default Route Adapter.
    pub server: String,
}

impl Default for DnsLookupRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            record_type: "A".to_string(),
            server: String::new(),
        }
    }
}

/// A DNS Lookup whose input has been validated and is ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsLookup {
    /// What the Technician typed.
    query: String,
    /// The name actually asked for: `query`, or the reverse name of an IP address.
    asked: String,
    record_type: RecordType,
    /// Whether the type was chosen because `query` is an IP address.
    automatic_type: bool,
    server: Option<IpAddr>,
}

impl DnsLookupRequest {
    pub fn validate(&self) -> Result<DnsLookup, InputError> {
        let target = ChosenTarget::parse(&self.query, AddressFamily::Auto)?;
        let query = self.query.trim().to_string();
        let (asked, record_type, automatic_type) = match target.address() {
            Some(address) => (reverse_name(address), RecordType::Ptr, true),
            None => (
                query.trim_end_matches('.').to_string(),
                parse_record_type(&self.record_type)?,
                false,
            ),
        };
        let server = match self.server.trim() {
            "" => None,
            text => Some(text.parse::<IpAddr>().map_err(|_| {
                InputError::new(format!("{text} is not a DNS server's IP address"))
            })?),
        };
        Ok(DnsLookup {
            query,
            asked,
            record_type,
            automatic_type,
            server,
        })
    }
}

fn parse_record_type(text: &str) -> Result<RecordType, InputError> {
    RecordType::ALL
        .into_iter()
        .find(|record_type| record_type.name().eq_ignore_ascii_case(text.trim()))
        .ok_or_else(|| {
            InputError::new(format!(
                "{text} is not a record type: choose A, AAAA, CNAME, MX, TXT, NS, PTR or SOA"
            ))
        })
}

/// The name DNS stores an address's PTR record under.
fn reverse_name(address: IpAddr) -> String {
    match address {
        IpAddr::V4(v4) => {
            let [a, b, c, d] = v4.octets();
            format!("{d}.{c}.{b}.{a}.in-addr.arpa")
        }
        IpAddr::V6(v6) => {
            let nibbles: Vec<String> = v6
                .octets()
                .iter()
                .flat_map(|byte| [byte >> 4, byte & 0xF])
                .rev()
                .map(|nibble| format!("{nibble:x}"))
                .collect();
            format!("{}.ip6.arpa", nibbles.join("."))
        }
    }
}

/// How a DNS Lookup went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsOutcome {
    /// The server was asked and replied (with records, or with one of the failure answers).
    Replied(DnsReply),
    /// No server was chosen and this PC has none configured.
    NoServer,
    /// The Technician stopped the Run before the server answered.
    Stopped,
}

/// What a DNS Lookup reports back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsLookupResult {
    pub taken_at: DateTime<FixedOffset>,
    pub pc_name: String,
    /// The address this PC's internet traffic leaves from, when there is a route.
    pub pc_address: Option<IpAddr>,
    pub query: String,
    pub asked: String,
    pub record_type: RecordType,
    pub automatic_type: bool,
    /// The server that was asked, if one could be found.
    pub server: Option<IpAddr>,
    pub outcome: DnsOutcome,
}

impl DnsLookupResult {
    /// The Result as plain text, ready to paste into a ticket.
    pub fn to_text(&self) -> String {
        let from = match self.pc_address {
            Some(address) => format!("{} ({address})", self.pc_name),
            None => self.pc_name.clone(),
        };
        let mut text = format!("DNS Lookup\nQuery:     {}\n", self.query);
        if self.asked != self.query {
            text.push_str(&format!("Asked:     {}\n", self.asked));
        }
        text.push_str(&format!(
            "Type:      {}{}\nServer:    {}\nTime:      {}\nFrom:      {from}\n",
            self.record_type.name(),
            if self.automatic_type {
                " (chosen automatically)"
            } else {
                ""
            },
            self.server
                .map_or_else(|| "(none)".to_string(), |server| server.to_string()),
            format_time(self.taken_at),
        ));
        if let DnsOutcome::Replied(reply) = &self.outcome {
            text.push_str(&format!(
                "Answered:  in {}\n",
                milliseconds(reply.round_trip)
            ));
        }
        text.push('\n');
        if let DnsOutcome::Replied(DnsReply {
            answer: DnsAnswer::Records(records),
            ..
        }) = &self.outcome
        {
            text.push_str(&record_table(records));
            text.push('\n');
        }
        text.push_str(&self.summary());
        text.push('\n');
        text
    }

    /// The one-line conclusion, in plain language.
    pub fn summary(&self) -> String {
        let server = self
            .server
            .map_or_else(|| "The server".to_string(), |server| server.to_string());
        let reply = match &self.outcome {
            DnsOutcome::Replied(reply) => reply,
            DnsOutcome::NoServer => {
                return "This PC has no DNS server configured: choose one to ask".to_string();
            }
            DnsOutcome::Stopped => return "Stopped before the server answered".to_string(),
        };
        match &reply.answer {
            DnsAnswer::Records(records) => {
                format!(
                    "{} record{}",
                    records.len(),
                    if records.len() == 1 { "" } else { "s" }
                )
            }
            DnsAnswer::NameDoesNotExist => format!("{} does not exist (NXDOMAIN)", self.asked),
            DnsAnswer::NoRecords => {
                format!(
                    "{} exists, but has no {} records",
                    self.asked,
                    self.record_type.name()
                )
            }
            DnsAnswer::ServerError { code: 2 } => format!("{server} failed to answer (SERVFAIL)"),
            DnsAnswer::ServerError { code: 5 } => format!("{server} refused to answer (REFUSED)"),
            DnsAnswer::ServerError { code } => format!("{server} answered with error code {code}"),
            DnsAnswer::BadResponse => format!("{server} sent an answer that could not be read"),
            DnsAnswer::NoResponse => format!("No response from {server}"),
        }
    }
}

fn record_table(records: &[DnsRecord]) -> String {
    let rows: Vec<Vec<String>> = records
        .iter()
        .map(|record| {
            vec![
                record.name.clone(),
                record.record_type.name().to_string(),
                record.ttl.to_string(),
                record.value.clone(),
            ]
        })
        .collect();
    text_table(&["NAME", "TYPE", "TTL", "VALUE"], &rows)
}

/// The DNS server this PC would use: the first one on the Default Route Adapter. A link-local
/// IPv6 server is left out, as it cannot be reached without the zone the adapter list doesn't carry.
pub async fn default_server(network: &impl Network) -> Option<IpAddr> {
    let adapter = ip_info::find_default_route(network).await?;
    adapter
        .dns_servers
        .into_iter()
        .find(|server| !matches!(server, IpAddr::V6(v6) if v6.is_unicast_link_local()))
}

/// Runs a DNS Lookup until the server answers, or `stop` is cancelled.
pub async fn run(
    network: &impl Network,
    lookup: &DnsLookup,
    stop: &CancellationToken,
) -> DnsLookupResult {
    let taken_at = network.now();
    let pc_name = network.hostname();
    let pc_address = ip_info::internet_route_source(network);
    let finish = |server, outcome| DnsLookupResult {
        taken_at,
        pc_name: pc_name.clone(),
        pc_address,
        query: lookup.query.clone(),
        asked: lookup.asked.clone(),
        record_type: lookup.record_type,
        automatic_type: lookup.automatic_type,
        server,
        outcome,
    };

    let server = match lookup.server {
        Some(server) => Some(server),
        None => match stop.run_until_cancelled(default_server(network)).await {
            Some(server) => server,
            None => return finish(None, DnsOutcome::Stopped),
        },
    };
    let Some(server) = server else {
        return finish(None, DnsOutcome::NoServer);
    };

    let asking = network.dns_query(
        SocketAddr::new(server, DNS_PORT),
        &lookup.asked,
        lookup.record_type,
    );
    match stop.run_until_cancelled(asking).await {
        Some(reply) => finish(Some(server), DnsOutcome::Replied(reply)),
        None => finish(Some(server), DnsOutcome::Stopped),
    }
}
