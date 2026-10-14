//! A small DNS client: it asks one chosen server one question, directly (RFC 1035), so neither
//! the hosts file nor the system's resolver is consulted. UDP first, with one retry; TCP when the
//! server says its answer was truncated.

use std::net::{Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::{timeout, timeout_at};

use crate::network::{DnsAnswer, DnsRecord, DnsReply, RecordType};

/// How long the server has to answer each try.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(2);

/// How many times the query is sent over UDP before giving up.
const UDP_ATTEMPTS: usize = 2;

/// Names may point back at earlier names in the message; this bounds how far that is followed.
const MAX_NAME_JUMPS: usize = 16;

/// The fixed header every DNS message starts with.
const HEADER_LENGTH: usize = 12;

/// No DNS name is longer than this, written out in labels.
const MAX_NAME_LENGTH: usize = 255;

const CLASS_IN: u16 = 1;
const FLAG_RESPONSE: u16 = 0x8000;
const FLAG_TRUNCATED: u16 = 0x0200;
const FLAG_RECURSION_DESIRED: u16 = 0x0100;

pub(crate) async fn query(server: SocketAddr, name: &str, record_type: RecordType) -> DnsReply {
    let started = Instant::now();
    let id = next_id();
    let answer = match encode_query(id, name, record_type) {
        None => DnsAnswer::BadResponse,
        Some(packet) => ask(server, id, &packet).await,
    };
    DnsReply {
        answer,
        round_trip: started.elapsed(),
    }
}

async fn ask(server: SocketAddr, id: u16, packet: &[u8]) -> DnsAnswer {
    let Some(message) = over_udp(server, id, packet).await else {
        return DnsAnswer::NoResponse;
    };
    match decode(&message) {
        Decoded::Answer(answer) => answer,
        Decoded::Truncated => match over_tcp(server, id, packet).await {
            Some(message) => match decode(&message) {
                Decoded::Answer(answer) => answer,
                Decoded::Truncated => DnsAnswer::BadResponse,
            },
            None => DnsAnswer::NoResponse,
        },
    }
}

/// Sends the query up to [`UDP_ATTEMPTS`] times and returns the first message carrying `id`.
async fn over_udp(server: SocketAddr, id: u16, packet: &[u8]) -> Option<Vec<u8>> {
    let local = if server.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(local).await.ok()?;
    socket.connect(server).await.ok()?;
    // The largest a UDP datagram can be, so no answer is cut short.
    let mut buffer = vec![0u8; 65535];
    for _ in 0..UDP_ATTEMPTS {
        socket.send(packet).await.ok()?;
        let deadline = tokio::time::Instant::now() + ATTEMPT_TIMEOUT;
        loop {
            match timeout_at(deadline, socket.recv(&mut buffer)).await {
                Ok(Ok(length)) if answers(&buffer[..length], id, packet) => {
                    return Some(buffer[..length].to_vec());
                }
                // Not the answer to this query: keep waiting.
                Ok(Ok(_)) => {}
                // The port is closed on the server: a reset, not silence, so don't retry.
                Ok(Err(_)) => return None,
                Err(_elapsed) => break,
            }
        }
    }
    None
}

/// Asks over TCP, where each message is preceded by its length in two bytes.
async fn over_tcp(server: SocketAddr, id: u16, packet: &[u8]) -> Option<Vec<u8>> {
    let exchange = async {
        let mut stream = TcpStream::connect(server).await.ok()?;
        let length = u16::try_from(packet.len()).ok()?.to_be_bytes();
        stream
            .write_all(&[&length[..], packet].concat())
            .await
            .ok()?;
        let mut reply_length = [0u8; 2];
        stream.read_exact(&mut reply_length).await.ok()?;
        let mut message = vec![0u8; usize::from(u16::from_be_bytes(reply_length))];
        stream.read_exact(&mut message).await.ok()?;
        answers(&message, id, packet).then_some(message)
    };
    timeout(ATTEMPT_TIMEOUT, exchange).await.ok().flatten()
}

/// Whether `message` is a response to the query `packet` (id `id`): it carries the id, is marked
/// as a response, and repeats the question that was asked. Anything else is a stray datagram.
fn answers(message: &[u8], id: u16, packet: &[u8]) -> bool {
    let question = &packet[HEADER_LENGTH..];
    message.len() >= HEADER_LENGTH + question.len()
        && message[..2] == id.to_be_bytes()
        && u16::from_be_bytes([message[2], message[3]]) & FLAG_RESPONSE != 0
        // Names are compared without regard to letter case, as DNS does.
        && message[HEADER_LENGTH..HEADER_LENGTH + question.len()].eq_ignore_ascii_case(question)
}

/// A query id that differs from one call to the next. It only has to tell this query's answer
/// from a stray one; the random source port does the work of making answers hard to spoof.
fn next_id() -> u16 {
    static COUNTER: AtomicU16 = AtomicU16::new(0);
    let clock = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    (clock as u16) ^ COUNTER.fetch_add(0x9E37, Ordering::Relaxed)
}

fn type_code(record_type: RecordType) -> u16 {
    match record_type {
        RecordType::A => 1,
        RecordType::Ns => 2,
        RecordType::Cname => 5,
        RecordType::Soa => 6,
        RecordType::Ptr => 12,
        RecordType::Mx => 15,
        RecordType::Txt => 16,
        RecordType::Aaaa => 28,
    }
}

fn record_type_of(code: u16) -> Option<RecordType> {
    RecordType::ALL
        .into_iter()
        .find(|record_type| type_code(*record_type) == code)
}

/// The query message: header, then one question. None if a label is too long for DNS.
fn encode_query(id: u16, name: &str, record_type: RecordType) -> Option<Vec<u8>> {
    let mut packet = Vec::with_capacity(18 + name.len());
    packet.extend_from_slice(&id.to_be_bytes());
    packet.extend_from_slice(&FLAG_RECURSION_DESIRED.to_be_bytes());
    packet.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]);
    for label in name.trim_end_matches('.').split('.') {
        let length = u8::try_from(label.len())
            .ok()
            .filter(|length| (1..=63).contains(length))?;
        packet.push(length);
        packet.extend_from_slice(label.as_bytes());
    }
    packet.push(0);
    packet.extend_from_slice(&type_code(record_type).to_be_bytes());
    packet.extend_from_slice(&CLASS_IN.to_be_bytes());
    Some(packet)
}

enum Decoded {
    Answer(DnsAnswer),
    /// The server could not fit its answer in a UDP message: ask again over TCP.
    Truncated,
}

fn decode(message: &[u8]) -> Decoded {
    match try_decode(message) {
        Some(decoded) => decoded,
        None => Decoded::Answer(DnsAnswer::BadResponse),
    }
}

fn try_decode(message: &[u8]) -> Option<Decoded> {
    let flags = u16::from_be_bytes(message.get(2..4)?.try_into().ok()?);
    message.get(..HEADER_LENGTH)?;
    if flags & FLAG_RESPONSE == 0 {
        return None;
    }
    if flags & FLAG_TRUNCATED != 0 {
        return Some(Decoded::Truncated);
    }
    match flags & 0xF {
        0 => {}
        3 => return Some(Decoded::Answer(DnsAnswer::NameDoesNotExist)),
        code => return Some(Decoded::Answer(DnsAnswer::ServerError { code: code as u8 })),
    }
    let questions = u16::from_be_bytes(message.get(4..6)?.try_into().ok()?);
    let answers = u16::from_be_bytes(message.get(6..8)?.try_into().ok()?);

    let mut at = HEADER_LENGTH;
    for _ in 0..questions {
        let (_, after_name) = read_name(message, at)?;
        at = after_name.checked_add(4)?;
    }
    let mut records = Vec::new();
    for _ in 0..answers {
        let (owner, after_name) = read_name(message, at)?;
        let fixed = message.get(after_name..after_name.checked_add(10)?)?;
        let code = u16::from_be_bytes([fixed[0], fixed[1]]);
        let ttl = u32::from_be_bytes([fixed[4], fixed[5], fixed[6], fixed[7]]);
        let data_length = usize::from(u16::from_be_bytes([fixed[8], fixed[9]]));
        let data_start = after_name + 10;
        let data_end = data_start.checked_add(data_length)?;
        message.get(data_start..data_end)?;
        if let Some(record_type) = record_type_of(code) {
            let value = read_value(message, record_type, data_start, data_end)?;
            records.push(DnsRecord {
                name: owner,
                record_type,
                ttl,
                value,
            });
        }
        at = data_end;
    }
    Some(Decoded::Answer(if records.is_empty() {
        DnsAnswer::NoRecords
    } else {
        DnsAnswer::Records(records)
    }))
}

/// A record's value, written the way `nslookup` shows it. `start..end` is its data in `message`.
fn read_value(message: &[u8], record_type: RecordType, start: usize, end: usize) -> Option<String> {
    let data = &message[start..end];
    Some(match record_type {
        RecordType::A => {
            let octets: [u8; 4] = data.try_into().ok()?;
            std::net::Ipv4Addr::from(octets).to_string()
        }
        RecordType::Aaaa => {
            let octets: [u8; 16] = data.try_into().ok()?;
            Ipv6Addr::from(octets).to_string()
        }
        RecordType::Ns | RecordType::Cname | RecordType::Ptr => name_within(message, start, end)?.0,
        RecordType::Mx => {
            let preference = u16::from_be_bytes(data.get(0..2)?.try_into().ok()?);
            format!("{preference} {}", name_within(message, start + 2, end)?.0)
        }
        RecordType::Txt => {
            let mut strings = Vec::new();
            let mut rest = data;
            while let Some((&length, tail)) = rest.split_first() {
                let text = tail.get(..usize::from(length))?;
                strings.push(format!("\"{}\"", String::from_utf8_lossy(text)));
                rest = &tail[usize::from(length)..];
            }
            strings.join(" ")
        }
        RecordType::Soa => {
            let (primary, after_primary) = name_within(message, start, end)?;
            let (mailbox, after_mailbox) = name_within(message, after_primary, end)?;
            let timers = message.get(after_mailbox..after_mailbox.checked_add(20)?)?;
            if after_mailbox + 20 > end {
                return None;
            }
            let timer = |index: usize| {
                u32::from_be_bytes(
                    timers[index * 4..index * 4 + 4]
                        .try_into()
                        .expect("four bytes"),
                )
            };
            format!(
                "{primary} {mailbox} serial {} refresh {} retry {} expire {} minimum {}",
                timer(0),
                timer(1),
                timer(2),
                timer(3),
                timer(4)
            )
        }
    })
}

/// Like [`read_name`], but the name must be written inside its own record's data, which ends at `end`.
fn name_within(message: &[u8], at: usize, end: usize) -> Option<(String, usize)> {
    let (name, after) = read_name(message, at)?;
    (after <= end).then_some((name, after))
}

/// Reads a name starting at `at`, following compression pointers. Returns it, and the offset just
/// past where it was written (a pointer ends the name at the pointer itself).
fn read_name(message: &[u8], at: usize) -> Option<(String, usize)> {
    let mut labels: Vec<String> = Vec::new();
    let mut position = at;
    let mut end = None;
    let mut jumps = 0;
    let mut length_so_far = 0;
    loop {
        let length = *message.get(position)?;
        match length {
            0 => {
                let after = end.unwrap_or(position + 1);
                let name = if labels.is_empty() {
                    ".".to_string()
                } else {
                    labels.join(".")
                };
                return Some((name, after));
            }
            length if length & 0xC0 == 0xC0 => {
                let low = *message.get(position + 1)?;
                end.get_or_insert(position + 2);
                jumps += 1;
                if jumps > MAX_NAME_JUMPS {
                    return None;
                }
                position = (usize::from(length & 0x3F) << 8) | usize::from(low);
            }
            length if length & 0xC0 == 0 => {
                let label = message.get(position + 1..position + 1 + usize::from(length))?;
                length_so_far += 1 + label.len();
                if length_so_far > MAX_NAME_LENGTH {
                    return None;
                }
                labels.push(String::from_utf8_lossy(label).into_owned());
                position += 1 + usize::from(length);
            }
            _ => return None,
        }
    }
}
