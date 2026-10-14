//! The real DNS client (seam S2) against a small DNS server run by the test on this PC, so the
//! wire format is checked without the internet. Responses are built byte by byte from RFC 1035.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use toolbox_core::network::{DnsAnswer, DnsRecord, Network, RecordType, SystemNetwork};

const A: u16 = 1;
const NS: u16 = 2;
const CNAME: u16 = 5;
const SOA: u16 = 6;
const PTR: u16 = 12;
const MX: u16 = 15;
const TXT: u16 = 16;
const AAAA: u16 = 28;

/// Names as DNS writes them: length-prefixed labels, ending in a zero.
fn name(text: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = text
        .split('.')
        .flat_map(|label| [vec![label.len() as u8], label.as_bytes().to_vec()].concat())
        .collect();
    bytes.push(0);
    bytes
}

/// A pointer back to the question's name, which starts at byte 12 of the message.
const QUESTION_NAME: [u8; 2] = [0xC0, 0x0C];

/// One answer record whose owner name is the question's.
fn answer(record_type: u16, ttl: u32, data: &[u8]) -> Vec<u8> {
    [
        &QUESTION_NAME[..],
        &record_type.to_be_bytes(),
        &1u16.to_be_bytes(),
        &ttl.to_be_bytes(),
        &(data.len() as u16).to_be_bytes(),
        data,
    ]
    .concat()
}

/// A response to `query`: its id and question copied, `rcode` set, `answers` appended.
fn response(query: &[u8], rcode: u8, truncated: bool, answers: &[Vec<u8>]) -> Vec<u8> {
    let flags: u16 = 0x8180 | u16::from(rcode) | if truncated { 0x0200 } else { 0 };
    [
        &query[0..2],
        &flags.to_be_bytes(),
        &1u16.to_be_bytes(),
        &(answers.len() as u16).to_be_bytes(),
        &[0, 0, 0, 0],
        &query[12..],
        &answers.concat(),
    ]
    .concat()
}

/// A DNS server on 127.0.0.1 that answers each UDP query with whatever `script` returns for it
/// (any number of datagrams, including none). Returns its address and a count of queries seen.
async fn serve_udp(
    script: impl Fn(&[u8], usize) -> Vec<Vec<u8>> + Send + Sync + 'static,
) -> (SocketAddr, Arc<AtomicUsize>) {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = socket.local_addr().unwrap();
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&seen);
    tokio::spawn(async move {
        let mut buffer = [0u8; 512];
        loop {
            let Ok((length, from)) = socket.recv_from(&mut buffer).await else {
                return;
            };
            let number = counter.fetch_add(1, Ordering::SeqCst) + 1;
            for datagram in script(&buffer[..length], number) {
                let _ = socket.send_to(&datagram, from).await;
            }
        }
    });
    (address, seen)
}

/// Builds a response to a query: here, a deliberately broken one.
type HostileReply = Box<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

async fn ask(server: SocketAddr, record_type: RecordType) -> DnsAnswer {
    SystemNetwork
        .dns_query(server, "example.com", record_type)
        .await
        .answer
}

fn record(record_type: RecordType, ttl: u32, value: &str) -> DnsRecord {
    DnsRecord {
        name: "example.com".to_string(),
        record_type,
        ttl,
        value: value.to_string(),
    }
}

#[tokio::test]
async fn an_a_record_is_read_with_its_ttl() {
    let (server, _) = serve_udp(|query, _| {
        vec![response(
            query,
            0,
            false,
            &[answer(A, 300, &[93, 184, 216, 34])],
        )]
    })
    .await;

    assert_eq!(
        ask(server, RecordType::A).await,
        DnsAnswer::Records(vec![record(RecordType::A, 300, "93.184.216.34")])
    );
}

#[tokio::test]
async fn an_aaaa_record_is_written_the_way_ipv6_addresses_are() {
    let address = [
        0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
    ];
    let (server, _) =
        serve_udp(move |query, _| vec![response(query, 0, false, &[answer(AAAA, 60, &address)])])
            .await;

    assert_eq!(
        ask(server, RecordType::Aaaa).await,
        DnsAnswer::Records(vec![record(RecordType::Aaaa, 60, "2001:db8::1")])
    );
}

#[tokio::test]
async fn names_inside_records_follow_compression_pointers_back_to_the_question() {
    // MX 10 "mail" + pointer to example.com; NS and PTR use a full name and a pointer-only name.
    let mx = [
        &10u16.to_be_bytes()[..],
        &[4, b'm', b'a', b'i', b'l', 0xC0, 0x0C],
    ]
    .concat();
    let (server, _) =
        serve_udp(move |query, _| vec![response(query, 0, false, &[answer(MX, 3600, &mx)])]).await;

    assert_eq!(
        ask(server, RecordType::Mx).await,
        DnsAnswer::Records(vec![record(RecordType::Mx, 3600, "10 mail.example.com")])
    );
}

#[tokio::test]
async fn ns_cname_and_ptr_records_carry_a_name() {
    let (server, _) = serve_udp(|query, _| {
        vec![response(
            query,
            0,
            false,
            &[
                answer(NS, 86400, &name("ns1.example.net")),
                answer(CNAME, 300, &name("www.example.org")),
                answer(PTR, 21599, &[0xC0, 0x0C]),
            ],
        )]
    })
    .await;

    assert_eq!(
        ask(server, RecordType::Ns).await,
        DnsAnswer::Records(vec![
            record(RecordType::Ns, 86400, "ns1.example.net"),
            record(RecordType::Cname, 300, "www.example.org"),
            record(RecordType::Ptr, 21599, "example.com"),
        ])
    );
}

#[tokio::test]
async fn a_txt_record_with_several_strings_shows_each_in_quotes() {
    let txt = [&[8u8][..], b"v=spf1 -", &[4], b"all!"].concat();
    let (server, _) =
        serve_udp(move |query, _| vec![response(query, 0, false, &[answer(TXT, 120, &txt)])]).await;

    assert_eq!(
        ask(server, RecordType::Txt).await,
        DnsAnswer::Records(vec![record(RecordType::Txt, 120, "\"v=spf1 -\" \"all!\"")])
    );
}

#[tokio::test]
async fn an_soa_record_lists_its_names_and_timers() {
    let soa = [
        name("ns1.example.com"),
        name("hostmaster.example.com"),
        [2024u32, 7200, 3600, 1209600, 300]
            .iter()
            .flat_map(|n| n.to_be_bytes())
            .collect(),
    ]
    .concat();
    let (server, _) =
        serve_udp(move |query, _| vec![response(query, 0, false, &[answer(SOA, 900, &soa)])]).await;

    assert_eq!(
        ask(server, RecordType::Soa).await,
        DnsAnswer::Records(vec![record(
            RecordType::Soa,
            900,
            "ns1.example.com hostmaster.example.com serial 2024 refresh 7200 retry 3600 expire 1209600 minimum 300"
        )])
    );
}

#[tokio::test]
async fn the_failure_codes_come_back_as_their_own_answers() {
    for (code, expected) in [
        (3, DnsAnswer::NameDoesNotExist),
        (2, DnsAnswer::ServerError { code: 2 }),
        (5, DnsAnswer::ServerError { code: 5 }),
    ] {
        let (server, _) = serve_udp(move |query, _| vec![response(query, code, false, &[])]).await;

        assert_eq!(ask(server, RecordType::A).await, expected);
    }
}

#[tokio::test]
async fn a_successful_answer_with_no_records_is_an_empty_answer() {
    let (server, _) = serve_udp(|query, _| vec![response(query, 0, false, &[])]).await;

    assert_eq!(ask(server, RecordType::Aaaa).await, DnsAnswer::NoRecords);
}

#[tokio::test]
async fn an_answer_with_the_wrong_id_is_ignored_and_the_right_one_is_used() {
    let (server, _) = serve_udp(|query, _| {
        let mut wrong = response(query, 3, false, &[]);
        wrong[0] ^= 0xFF;
        vec![
            wrong,
            response(query, 0, false, &[answer(A, 10, &[1, 2, 3, 4])]),
        ]
    })
    .await;

    assert_eq!(
        ask(server, RecordType::A).await,
        DnsAnswer::Records(vec![record(RecordType::A, 10, "1.2.3.4")])
    );
}

#[tokio::test]
async fn an_answer_that_cannot_be_read_is_a_bad_response() {
    let (server, _) = serve_udp(|query, _| {
        vec![
            [
                &query[0..2],
                &[0x81, 0x80, 0, 1, 0, 5, 0, 0, 0, 0][..],
                &query[12..],
            ]
            .concat(),
        ]
    })
    .await;

    assert_eq!(ask(server, RecordType::A).await, DnsAnswer::BadResponse);
}

#[tokio::test]
async fn a_query_that_gets_no_answer_is_sent_a_second_time_before_giving_up() {
    let (server, seen) = serve_udp(|query, number| {
        if number == 1 {
            vec![]
        } else {
            vec![response(query, 0, false, &[answer(A, 5, &[9, 9, 9, 9])])]
        }
    })
    .await;

    assert_eq!(
        (
            ask(server, RecordType::A).await,
            seen.load(Ordering::SeqCst)
        ),
        (
            DnsAnswer::Records(vec![record(RecordType::A, 5, "9.9.9.9")]),
            2
        )
    );
}

#[tokio::test]
async fn a_server_that_never_answers_is_no_response_after_two_tries_of_two_seconds() {
    let (server, seen) = serve_udp(|_, _| vec![]).await;
    let started = std::time::Instant::now();

    let reply = SystemNetwork
        .dns_query(server, "example.com", RecordType::A)
        .await;

    assert_eq!(
        (
            reply.answer,
            seen.load(Ordering::SeqCst),
            (3900..4600).contains(&started.elapsed().as_millis()),
            reply.round_trip >= Duration::from_millis(3900)
        ),
        (DnsAnswer::NoResponse, 2, true, true)
    );
}

#[tokio::test]
async fn a_truncated_answer_is_asked_again_over_tcp() {
    // UDP and TCP on the same port: the UDP answer says "truncated"; the full one is over TCP.
    let (server, _) = serve_udp(|query, _| vec![response(query, 0, true, &[])]).await;
    let listener = TcpListener::bind(server).await.unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut length = [0u8; 2];
        stream.read_exact(&mut length).await.unwrap();
        let mut query = vec![0u8; u16::from_be_bytes(length) as usize];
        stream.read_exact(&mut query).await.unwrap();
        let full = response(&query, 0, false, &[answer(A, 77, &[5, 6, 7, 8])]);
        stream
            .write_all(&[&(full.len() as u16).to_be_bytes()[..], &full].concat())
            .await
            .unwrap();
    });

    assert_eq!(
        ask(server, RecordType::A).await,
        DnsAnswer::Records(vec![record(RecordType::A, 77, "5.6.7.8")])
    );
}

#[tokio::test]
async fn the_round_trip_is_how_long_the_server_took() {
    let (server, _) =
        serve_udp(|query, _| vec![response(query, 0, false, &[answer(A, 1, &[1, 1, 1, 1])])]).await;

    let reply = SystemNetwork
        .dns_query(server, "example.com", RecordType::A)
        .await;

    assert!(reply.round_trip < Duration::from_millis(500), "{reply:?}");
}

#[tokio::test]
#[ignore = "reaches public DNS on the internet; run with --ignored"]
async fn public_dns_answers_for_a_real_name_and_says_nxdomain_for_one_that_cannot_exist() {
    let google: SocketAddr = "8.8.8.8:53".parse().unwrap();

    let real = SystemNetwork
        .dns_query(google, "example.com", RecordType::A)
        .await
        .answer;
    let missing = SystemNetwork
        .dns_query(google, "nosuchhost.invalid", RecordType::A)
        .await
        .answer;

    assert!(
        matches!(real, DnsAnswer::Records(ref records) if !records.is_empty()),
        "{real:?}"
    );
    assert_eq!(missing, DnsAnswer::NameDoesNotExist);
}

#[tokio::test]
async fn a_stray_datagram_with_the_right_id_that_is_not_a_response_does_not_end_the_wait() {
    let (server, _) = serve_udp(|query, _| {
        let mut question_not_response = query.to_vec();
        question_not_response[2] = 0x01; // QR clear: a query, not a response
        vec![
            question_not_response,
            response(query, 0, false, &[answer(A, 10, &[4, 3, 2, 1])]),
        ]
    })
    .await;

    assert_eq!(
        ask(server, RecordType::A).await,
        DnsAnswer::Records(vec![record(RecordType::A, 10, "4.3.2.1")])
    );
}

#[tokio::test]
async fn an_answer_to_a_different_question_is_ignored() {
    let (server, _) = serve_udp(|query, _| {
        let mut other = response(query, 3, false, &[]);
        other[13] ^= 0x01; // a different first letter in the echoed question
        vec![
            other,
            response(query, 0, false, &[answer(A, 10, &[7, 7, 7, 7])]),
        ]
    })
    .await;

    assert_eq!(
        ask(server, RecordType::A).await,
        DnsAnswer::Records(vec![record(RecordType::A, 10, "7.7.7.7")])
    );
}

#[tokio::test]
async fn hostile_answers_are_bad_responses_not_hangs_or_crashes() {
    // A name that points at itself; a name over 255 bytes; an NS
    // whose name runs past the end of its own data.
    let looping_name = answer(NS, 1, &[0xC0, 0x1C]);
    let long_name = {
        let labels: Vec<u8> = (0..5)
            .flat_map(|_| [vec![63u8], vec![b'a'; 63]].concat())
            .chain([0])
            .collect();
        answer(NS, 1, &labels)
    };
    let overrunning_name = [
        &QUESTION_NAME[..],
        &NS.to_be_bytes(),
        &1u16.to_be_bytes(),
        &1u32.to_be_bytes(),
        &2u16.to_be_bytes(),
        &[7, b'e', b'x'],
    ]
    .concat();
    let replies: Vec<HostileReply> = vec![
        Box::new(move |query| response(query, 0, false, std::slice::from_ref(&looping_name))),
        Box::new(move |query| response(query, 0, false, std::slice::from_ref(&long_name))),
        Box::new(move |query| response(query, 0, false, std::slice::from_ref(&overrunning_name))),
    ];
    for reply in replies {
        let (server, _) = serve_udp(move |query, _| vec![reply(query)]).await;

        assert_eq!(ask(server, RecordType::Ns).await, DnsAnswer::BadResponse);
    }
}
