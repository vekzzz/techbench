//! LAN Scan: sweeps an IPv4 address range to discover which Devices are present. On a subnet
//! this PC is directly connected to, ARP runs beside the ping, so a Windows PC that drops ping
//! is still found; elsewhere only the ping can find a Device.

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use futures_util::{StreamExt, stream};
use tokio_util::sync::CancellationToken;

use crate::InputError;
use crate::ip_info::{self, format_mac};
use crate::network::{AdapterKind, EchoOutcome, EchoRequest, Network};
use crate::ping::DEFAULT_PAYLOAD_SIZE;
use crate::run::{Progress, RunEnding, RunHeader, TIME_FORMAT, milliseconds, text_table};

/// How many addresses a Run keeps waiting on at once.
const PARALLEL_PROBES: usize = 64;

/// How long each address has to answer the ping, and again the ARP request.
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// The most addresses a range may span and still run without the Technician's confirmation.
const FREE_LIMIT: u64 = 1024;

/// The most addresses a range may span at all (a /16): anything larger is refused.
const REFUSED_ABOVE: u64 = 65_536;

/// A LAN Scan exactly as the Technician filled it in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanScanRequest {
    /// The range as typed: CIDR (`192.168.1.0/24`), start–end (`192.168.1.10-192.168.1.50`) or one address.
    pub range: String,
    /// Whether the Technician has agreed to a range big enough to need agreement.
    pub confirmed: bool,
}

/// How big a range is, and whether scanning it needs the Technician's confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeCheck {
    /// How many addresses will be probed: a CIDR range's network and broadcast addresses are not.
    pub addresses: usize,
    pub needs_confirmation: bool,
}

/// A LAN Scan whose input has been validated and is ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanScan {
    text: String,
    /// The first and last address to probe, inclusive.
    first: u32,
    last: u32,
}

impl LanScan {
    /// The addresses this scan will probe, in order.
    pub fn addresses(&self) -> impl Iterator<Item = Ipv4Addr> + use<> {
        (self.first..=self.last).map(Ipv4Addr::from)
    }

    /// How many addresses this scan will probe.
    pub fn address_count(&self) -> usize {
        (self.last - self.first) as usize + 1
    }

    /// The range as the Technician typed it.
    pub fn range(&self) -> &str {
        &self.text
    }
}

/// A range as parsed, before the size policy.
struct ParsedRange {
    text: String,
    /// The first and last address to probe, inclusive.
    first: u32,
    last: u32,
    /// How many addresses the range spans, counting a CIDR range's network and broadcast addresses.
    spanned: u64,
}

impl LanScanRequest {
    /// Reads the range and applies the size policy: up to 1,024 addresses runs freely, up to
    /// 65,536 needs confirmation, and anything larger is refused.
    pub fn check_range(&self) -> Result<RangeCheck, InputError> {
        let range = parse_range(&self.range)?;
        if range.spanned > REFUSED_ABOVE {
            return Err(InputError::new(format!(
                "{} addresses is more than LAN Scan will sweep (65,536, a /16); narrow the range",
                with_thousands(range.spanned)
            )));
        }
        Ok(RangeCheck {
            addresses: (range.last - range.first) as usize + 1,
            needs_confirmation: range.spanned > FREE_LIMIT,
        })
    }

    pub fn validate(&self) -> Result<LanScan, InputError> {
        let check = self.check_range()?;
        if check.needs_confirmation && !self.confirmed {
            return Err(InputError::new(format!(
                "{} addresses is a big sweep; confirm to scan it",
                with_thousands(check.addresses as u64)
            )));
        }
        let range = parse_range(&self.range)?;
        Ok(LanScan {
            text: range.text,
            first: range.first,
            last: range.last,
        })
    }
}

fn parse_range(text: &str) -> Result<ParsedRange, InputError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(InputError::new(
            "enter the range to scan, such as 192.168.1.0/24 or 192.168.1.10-192.168.1.50",
        ));
    }
    if text.contains(':') || text.starts_with('[') {
        return Err(InputError::new(
            "LAN Scan is IPv4 only; enter an IPv4 range such as 192.168.1.0/24",
        ));
    }
    let invalid = || {
        InputError::new(format!(
            "\"{text}\" is not an IPv4 range; write it as 192.168.1.0/24 or 192.168.1.10-192.168.1.50"
        ))
    };
    let address = |part: &str| part.trim().parse::<Ipv4Addr>().map_err(|_| invalid());

    let (first, last, spanned, skips_edges) = match (text.split_once('/'), text.split_once('-')) {
        (Some(_), Some(_)) => return Err(invalid()),
        (Some((network, prefix)), None) => {
            let prefix = prefix
                .trim()
                .parse::<u8>()
                .ok()
                .filter(|prefix| *prefix <= 32)
                .ok_or_else(invalid)?;
            let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
            let first = u32::from(address(network)?) & mask;
            (first, first | !mask, 1u64 << (32 - prefix), prefix <= 30)
        }
        (None, Some((first, last))) => {
            let (first, last) = (address(first)?, address(last)?);
            if first > last {
                return Err(InputError::new(format!(
                    "\"{text}\" is a backwards range; write it as {last}-{first}"
                )));
            }
            let (first, last) = (u32::from(first), u32::from(last));
            (first, last, u64::from(last - first) + 1, false)
        }
        (None, None) => {
            let single = u32::from(address(text)?);
            (single, single, 1, false)
        }
    };
    // A CIDR range's first and last addresses name the network and its broadcast, not Devices.
    let (probe_first, probe_last) = if skips_edges {
        (first + 1, last - 1)
    } else {
        (first, last)
    };
    Ok(ParsedRange {
        text: text.to_string(),
        first: probe_first,
        last: probe_last,
        spanned,
    })
}

/// A number with thousands separators: 65536 as `65,536`.
fn with_thousands(number: u64) -> String {
    let digits = number.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// The range LAN Scan offers first: the IPv4 network of the Default Route Adapter. None when
/// that adapter has no IPv4 address (or there is no default route): the Technician must enter one.
pub async fn default_range(network: &impl Network) -> Option<String> {
    let adapter = ip_info::find_default_route(network).await?;
    let mut addresses = adapter
        .addresses
        .iter()
        .filter_map(|assigned| match assigned.address {
            IpAddr::V4(address) if assigned.prefix <= 32 => Some((address, assigned.prefix)),
            _ => None,
        });
    // A self-assigned 169.254 address means DHCP failed: prefer a real one when there is one.
    let (address, prefix) = addresses
        .clone()
        .find(|(address, _)| !address.is_link_local())
        .or_else(|| addresses.next())?;
    let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
    Some(format!(
        "{}/{prefix}",
        Ipv4Addr::from(u32::from(address) & mask)
    ))
}

/// Something LAN Scan found present at an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub address: Ipv4Addr,
    /// Only known for a Device on this PC's own subnet, where ARP reaches.
    pub mac: Option<[u8; 6]>,
    /// How long the ping took to be answered; None when it was not.
    pub response_time: Option<Duration>,
    /// It answered ARP but not the ping: typically a Windows PC whose firewall drops echoes.
    pub no_ping_reply: bool,
    /// Whether the address is on a subnet this PC is directly connected to.
    pub on_own_subnet: bool,
}

/// Progress a LAN Scan Run reports as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanScanEvent {
    Started {
        addresses: usize,
    },
    Device(Device),
    /// How many addresses have been probed so far; not sent for every address of a big scan.
    Progress {
        probed: usize,
        total: usize,
    },
}

/// What a LAN Scan Run reports back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanScanResult {
    /// Its `target` is the range as typed; a range has no Resolved Address.
    pub header: RunHeader,
    /// The Devices found so far, in numeric IP order.
    pub devices: Vec<Device>,
    pub addresses_total: usize,
    pub addresses_probed: usize,
    pub ending: RunEnding,
}

impl LanScanResult {
    /// The Result as plain text, ready to paste into a ticket.
    pub fn to_text(&self) -> String {
        let header = &self.header;
        let from = match header.source_address {
            Some(address) => format!("{} ({address})", header.source_hostname),
            None => header.source_hostname.clone(),
        };
        let mut text = format!(
            "LAN Scan\nRange:     {} ({})\nTime:      {}\nFrom:      {from}\n\n",
            header.target,
            count(self.addresses_total, "address", "addresses"),
            header.started_at.format(TIME_FORMAT),
        );
        if !self.devices.is_empty() {
            text.push_str(&self.device_table());
            text.push('\n');
        }
        text.push_str(&self.summary());
        text.push('\n');
        if self.devices.iter().any(|device| !device.on_own_subnet) {
            text.push_str(
                "MAC is blank for devices beyond this PC's own subnet: ARP does not cross routers.\n",
            );
        }
        text
    }

    /// The one-line conclusion: how far the scan got and what it found.
    pub fn summary(&self) -> String {
        let devices = count(self.devices.len(), "Device", "Devices");
        let total = self.addresses_total;
        match self.ending {
            RunEnding::Running => format!(
                "Still scanning: {} of {} probed, {devices} found so far",
                self.addresses_probed,
                count(total, "address", "addresses"),
            ),
            RunEnding::Stopped if self.addresses_probed == 0 => {
                "Stopped before any address was probed.".to_string()
            }
            RunEnding::Stopped => format!(
                "Stopped after probing {} of {}: {devices} found",
                self.addresses_probed,
                count(total, "address", "addresses"),
            ),
            RunEnding::Completed | RunEnding::ResolutionFailed { .. } => {
                let addresses = count(total, "address", "addresses");
                if self.devices.is_empty() {
                    return format!("No Devices found among {addresses}");
                }
                let silent = self
                    .devices
                    .iter()
                    .filter(|device| device.no_ping_reply)
                    .count();
                let note = if silent > 0 {
                    format!("; {silent} with no ping reply")
                } else {
                    String::new()
                };
                format!("Found {devices} among {addresses}{note}")
            }
        }
    }

    fn device_table(&self) -> String {
        let rows: Vec<Vec<String>> = self
            .devices
            .iter()
            .map(|device| {
                vec![
                    device.address.to_string(),
                    device.mac.map_or_else(|| "-".to_string(), format_mac),
                    device
                        .response_time
                        .map_or_else(|| "-".to_string(), milliseconds),
                    if device.no_ping_reply {
                        "no ping reply".to_string()
                    } else {
                        String::new()
                    },
                ]
            })
            .collect();
        text_table(&["IP", "MAC", "RESPONSE TIME", "NOTE"], &rows)
    }
}

/// `1 Device`, `3 Devices`.
fn count(number: usize, one: &str, many: &str) -> String {
    format!("{number} {}", if number == 1 { one } else { many })
}

/// Runs a LAN Scan until every address has been probed or `stop` is cancelled.
pub async fn run(
    network: &impl Network,
    scan: &LanScan,
    stop: &CancellationToken,
    on_event: impl FnMut(LanScanEvent),
) -> LanScanResult {
    run_with_progress(network, scan, stop, &Progress::default(), on_event).await
}

/// Runs a LAN Scan, keeping `progress` up to date so its Result can be read mid-Run.
pub async fn run_with_progress(
    network: &impl Network,
    scan: &LanScan,
    stop: &CancellationToken,
    progress: &Progress<LanScanResult>,
    mut on_event: impl FnMut(LanScanEvent),
) -> LanScanResult {
    let total = scan.address_count();
    let header = RunHeader {
        target: scan.range().to_string(),
        resolved: None,
        started_at: network.now(),
        source_hostname: network.hostname(),
        source_address: network.source_address(IpAddr::V4(Ipv4Addr::from(scan.first))),
    };
    let result = |ending| LanScanResult {
        header: header.clone(),
        devices: vec![],
        addresses_total: total,
        addresses_probed: 0,
        ending,
    };

    let Some(adapters) = stop.run_until_cancelled(network.adapters()).await else {
        return progress.finish(result(RunEnding::Stopped));
    };
    let own_subnets: Vec<Subnet> = adapters
        .iter()
        .filter(|adapter| {
            adapter.up && !matches!(adapter.kind, AdapterKind::Loopback | AdapterKind::Tunnel)
        })
        .flat_map(|adapter| &adapter.addresses)
        .filter_map(|assigned| match assigned.address {
            IpAddr::V4(address) if assigned.prefix <= 32 => {
                Some(Subnet::new(address, assigned.prefix))
            }
            _ => None,
        })
        .collect();

    progress.set(result(RunEnding::Running));
    on_event(LanScanEvent::Started { addresses: total });

    // A big scan would otherwise report hundreds of thousands of near-identical progress events.
    let progress_every = (total / 100).max(1);
    let mut probing = stream::iter(scan.addresses())
        .map(|address| probe(network, address, &own_subnets))
        .buffer_unordered(PARALLEL_PROBES);

    let mut probed = 0;
    let ending = loop {
        match stop.run_until_cancelled(probing.next()).await {
            Some(Some(found)) => {
                probed += 1;
                if let Some(device) = found {
                    // Kept in IP order as they arrive, so the Result reads well at any moment.
                    progress.update(|result| {
                        let at = result
                            .devices
                            .partition_point(|listed| listed.address < device.address);
                        result.devices.insert(at, device.clone());
                    });
                    on_event(LanScanEvent::Device(device));
                }
                progress.update(|result| result.addresses_probed = probed);
                if probed % progress_every == 0 || probed == total {
                    on_event(LanScanEvent::Progress { probed, total });
                }
            }
            Some(None) => break RunEnding::Completed,
            None => break RunEnding::Stopped,
        }
    };
    progress.update(|result| result.ending = ending);
    progress
        .read(Clone::clone)
        .expect("the Run opened, so its Result exists")
}

/// Pings `address`, and on this PC's own subnet asks ARP at the same time. A Device exists if
/// either answered.
async fn probe(
    network: &impl Network,
    address: Ipv4Addr,
    own_subnets: &[Subnet],
) -> Option<Device> {
    let echo = network.icmp_echo(EchoRequest {
        address: IpAddr::V4(address),
        payload_size: DEFAULT_PAYLOAD_SIZE,
        dont_fragment: false,
        ttl: None,
        timeout: PROBE_TIMEOUT,
    });
    let on_own_subnet = own_subnets.iter().any(|subnet| subnet.contains(address));
    let (echo, mac) = if on_own_subnet {
        let (echo, mac) = tokio::join!(echo, network.arp_resolve(address, PROBE_TIMEOUT));
        (echo, mac)
    } else {
        (echo.await, None)
    };
    let response_time = match echo {
        EchoOutcome::Reply { round_trip, .. } => Some(round_trip),
        _ => None,
    };
    if response_time.is_none() && mac.is_none() {
        return None;
    }
    Some(Device {
        address,
        mac,
        response_time,
        no_ping_reply: response_time.is_none(),
        on_own_subnet,
    })
}

/// A subnet this PC is directly connected to.
struct Subnet {
    network: u32,
    mask: u32,
}

impl Subnet {
    fn new(address: Ipv4Addr, prefix: u8) -> Self {
        let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
        Self {
            network: u32::from(address) & mask,
            mask,
        }
    }

    fn contains(&self, address: Ipv4Addr) -> bool {
        u32::from(address) & self.mask == self.network
    }
}
