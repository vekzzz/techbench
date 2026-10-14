//! DNS Lookup, exercised through the toolbox core's public interface (seam S1) over a scripted
//! network (seam S2) and tokio's paused clock.

mod common;

use common::{FakeNetwork, ip, ms};
use tokio_util::sync::CancellationToken;
use toolbox_core::dns_lookup::{self, DnsLookupRequest, DnsLookupResult};
use toolbox_core::network::{
    Adapter, AdapterAddress, AdapterKind, DnsAnswer, DnsRecord, RecordType,
};

const GOOGLE: &str = "8.8.8.8:53";

fn request(query: &str) -> DnsLookupRequest {
    DnsLookupRequest {
        query: query.to_string(),
        record_type: "A".to_string(),
        server: "8.8.8.8".to_string(),
    }
}

fn record(name: &str, record_type: RecordType, ttl: u32, value: &str) -> DnsRecord {
    DnsRecord {
        name: name.to_string(),
        record_type,
        ttl,
        value: value.to_string(),
    }
}

async fn lookup(network: &FakeNetwork, request: DnsLookupRequest) -> DnsLookupResult {
    let lookup = request.validate().expect("request should be valid");
    dns_lookup::run(network, &lookup, &CancellationToken::new()).await
}

fn answering(name: &str, record_type: RecordType, answer: DnsAnswer) -> FakeNetwork {
    FakeNetwork::new().with_dns_answer(GOOGLE, name, record_type, ms(23), answer)
}

#[tokio::test(start_paused = true)]
async fn copy_as_text_names_the_server_the_response_time_and_each_record_with_its_ttl() {
    let network = answering(
        "example.com",
        RecordType::A,
        DnsAnswer::Records(vec![
            record("example.com", RecordType::A, 300, "93.184.216.34"),
            record("example.com", RecordType::A, 300, "93.184.216.35"),
        ]),
    );

    let result = lookup(&network, request("example.com")).await;

    assert_eq!(
        result.to_text(),
        "\
DNS Lookup
Query:     example.com
Type:      A
Server:    8.8.8.8
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)
Answered:  in 23 ms

NAME         TYPE  TTL  VALUE
example.com  A     300  93.184.216.34
example.com  A     300  93.184.216.35

2 records
"
    );
}

#[tokio::test(start_paused = true)]
async fn each_record_type_is_asked_for_by_its_own_name() {
    let mut asked = Vec::new();
    for record_type in RecordType::ALL {
        let network = answering("example.com", record_type, DnsAnswer::NoRecords);
        lookup(
            &network,
            DnsLookupRequest {
                record_type: record_type.name().to_string(),
                ..request("example.com")
            },
        )
        .await;
        asked.extend(
            network
                .dns_queries()
                .into_iter()
                .map(|(_, _, asked)| asked.name()),
        );
    }

    assert_eq!(
        asked,
        ["A", "AAAA", "CNAME", "MX", "TXT", "NS", "PTR", "SOA"]
    );
}

#[tokio::test(start_paused = true)]
async fn record_values_such_as_mx_and_txt_are_shown_as_the_core_receives_them() {
    let network = answering(
        "example.com",
        RecordType::Mx,
        DnsAnswer::Records(vec![
            record("example.com", RecordType::Mx, 3600, "10 mail.example.com"),
            record("example.com", RecordType::Mx, 3600, "20 backup.example.com"),
        ]),
    );

    let text = lookup(
        &network,
        DnsLookupRequest {
            record_type: "MX".to_string(),
            ..request("example.com")
        },
    )
    .await
    .to_text();

    assert!(
        text.contains("example.com  MX    3600  10 mail.example.com\nexample.com  MX    3600  20 backup.example.com\n"),
        "{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn an_ipv4_address_is_looked_up_as_a_ptr_record_whatever_type_was_chosen() {
    let network = answering(
        "8.4.8.8.in-addr.arpa",
        RecordType::Ptr,
        DnsAnswer::Records(vec![record(
            "8.4.8.8.in-addr.arpa",
            RecordType::Ptr,
            21599,
            "dns.google",
        )]),
    );

    let result = lookup(&network, request("8.8.4.8")).await;

    assert_eq!(
        (
            network.dns_queries(),
            result.to_text().lines().take(4).collect::<Vec<_>>()
        ),
        (
            vec![(
                GOOGLE.parse().unwrap(),
                "8.4.8.8.in-addr.arpa".to_string(),
                RecordType::Ptr
            )],
            vec![
                "DNS Lookup",
                "Query:     8.8.4.8",
                "Asked:     8.4.8.8.in-addr.arpa",
                "Type:      PTR (chosen automatically)"
            ]
        )
    );
}

#[tokio::test(start_paused = true)]
async fn an_ipv6_address_is_looked_up_under_ip6_arpa_one_nibble_at_a_time() {
    let reverse = "1.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.8.b.d.0.1.0.0.2.ip6.arpa";
    let network = answering(reverse, RecordType::Ptr, DnsAnswer::NoRecords);

    lookup(&network, request("2001:db8::1")).await;

    assert_eq!(network.dns_queries()[0].1, reverse);
}

fn outcome_text(result: &DnsLookupResult) -> String {
    result.summary()
}

#[tokio::test(start_paused = true)]
async fn a_name_that_does_not_exist_is_reported_as_nxdomain() {
    let network = answering(
        "nosuch.example.com",
        RecordType::A,
        DnsAnswer::NameDoesNotExist,
    );

    let result = lookup(&network, request("nosuch.example.com")).await;

    assert_eq!(
        outcome_text(&result),
        "nosuch.example.com does not exist (NXDOMAIN)"
    );
}

#[tokio::test(start_paused = true)]
async fn a_server_that_fails_is_reported_as_servfail_or_refused_by_its_error_code() {
    let servfail = answering(
        "example.com",
        RecordType::A,
        DnsAnswer::ServerError { code: 2 },
    );
    let refused = answering(
        "example.com",
        RecordType::A,
        DnsAnswer::ServerError { code: 5 },
    );
    let odd = answering(
        "example.com",
        RecordType::A,
        DnsAnswer::ServerError { code: 4 },
    );

    assert_eq!(
        [
            lookup(&servfail, request("example.com")).await.summary(),
            lookup(&refused, request("example.com")).await.summary(),
            lookup(&odd, request("example.com")).await.summary(),
        ],
        [
            "8.8.8.8 failed to answer (SERVFAIL)",
            "8.8.8.8 refused to answer (REFUSED)",
            "8.8.8.8 answered with error code 4",
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_name_without_records_of_the_type_asked_is_reported_as_an_empty_answer() {
    let network = answering("example.com", RecordType::Aaaa, DnsAnswer::NoRecords);

    let result = lookup(
        &network,
        DnsLookupRequest {
            record_type: "AAAA".to_string(),
            ..request("example.com")
        },
    )
    .await;

    assert_eq!(
        result.summary(),
        "example.com exists, but has no AAAA records"
    );
}

#[tokio::test(start_paused = true)]
async fn a_server_that_never_answers_is_reported_as_no_response_after_the_timeout() {
    let network = answering("example.com", RecordType::A, DnsAnswer::NoResponse);

    let result = lookup(&network, request("example.com")).await;

    assert_eq!(result.summary(), "No response from 8.8.8.8");
}

#[tokio::test(start_paused = true)]
async fn an_answer_that_cannot_be_read_is_reported_as_such() {
    let network = answering("example.com", RecordType::A, DnsAnswer::BadResponse);

    let result = lookup(&network, request("example.com")).await;

    assert_eq!(
        result.summary(),
        "8.8.8.8 sent an answer that could not be read"
    );
}

fn adapter(name: &str, dns: &[&str], address: &str) -> Adapter {
    Adapter {
        name: name.to_string(),
        description: format!("{name} controller"),
        kind: AdapterKind::Ethernet,
        up: true,
        mac: None,
        addresses: vec![AdapterAddress {
            address: ip(address),
            prefix: 24,
        }],
        gateways: vec![ip("192.168.1.1")],
        dns_servers: dns.iter().map(|server| ip(server)).collect(),
        dhcp: None,
        ipv4_metric: 25,
        ipv6_metric: 25,
    }
}

#[tokio::test(start_paused = true)]
async fn with_no_server_chosen_the_first_dns_server_of_the_default_route_adapter_is_asked() {
    let network = FakeNetwork::new()
        .with_adapters(vec![
            adapter("vEthernet", &["172.16.0.1"], "172.16.0.2"),
            adapter("Wi-Fi", &["192.168.1.53", "9.9.9.9"], "10.0.0.23"),
        ])
        .with_route_source(Some("10.0.0.23"))
        .with_dns_answer(
            "192.168.1.53:53",
            "example.com",
            RecordType::A,
            ms(4),
            DnsAnswer::NoRecords,
        );

    let result = lookup(
        &network,
        DnsLookupRequest {
            server: String::new(),
            ..request("example.com")
        },
    )
    .await;

    assert_eq!(
        result.to_text().lines().nth(3),
        Some("Server:    192.168.1.53")
    );
}

#[tokio::test(start_paused = true)]
async fn a_link_local_ipv6_dns_server_is_skipped_because_it_cannot_be_reached_without_its_zone() {
    let network = FakeNetwork::new()
        .with_adapters(vec![adapter(
            "Wi-Fi",
            &["fe80::1", "192.168.1.53"],
            "10.0.0.23",
        )])
        .with_route_source(Some("10.0.0.23"))
        .with_dns_answer(
            "192.168.1.53:53",
            "example.com",
            RecordType::A,
            ms(4),
            DnsAnswer::NoRecords,
        );

    let result = lookup(
        &network,
        DnsLookupRequest {
            server: String::new(),
            ..request("example.com")
        },
    )
    .await;

    assert_eq!(
        result.to_text().lines().nth(3),
        Some("Server:    192.168.1.53")
    );
}

#[tokio::test(start_paused = true)]
async fn with_no_server_chosen_and_none_configured_the_lookup_says_to_choose_one() {
    let network = FakeNetwork::new()
        .with_adapters(vec![adapter("Wi-Fi", &[], "10.0.0.23")])
        .with_route_source(Some("10.0.0.23"));

    let result = lookup(
        &network,
        DnsLookupRequest {
            server: String::new(),
            ..request("example.com")
        },
    )
    .await;

    assert_eq!(
        result.summary(),
        "This PC has no DNS server configured: choose one to ask"
    );
}

#[tokio::test(start_paused = true)]
async fn the_default_dns_server_can_be_looked_up_for_the_screen_to_show() {
    let network = FakeNetwork::new()
        .with_adapters(vec![adapter("Wi-Fi", &["192.168.1.53"], "10.0.0.23")])
        .with_route_source(Some("10.0.0.23"));

    assert_eq!(
        dns_lookup::default_server(&network).await,
        Some(ip("192.168.1.53"))
    );
}

#[tokio::test(start_paused = true)]
async fn stopping_while_the_server_is_slow_ends_the_run_as_stopped() {
    let network = FakeNetwork::new().with_dns_answer(
        GOOGLE,
        "example.com",
        RecordType::A,
        ms(60_000),
        DnsAnswer::NoRecords,
    );
    let lookup = request("example.com").validate().unwrap();
    let stop = CancellationToken::new();

    let (result, ()) = tokio::join!(dns_lookup::run(&network, &lookup, &stop), async {
        tokio::time::sleep(ms(500)).await;
        stop.cancel();
    });

    assert_eq!(result.summary(), "Stopped before the server answered");
}

#[test]
fn malformed_input_is_rejected_before_the_lookup_starts() {
    let bad = [
        DnsLookupRequest {
            query: String::new(),
            ..request("x")
        },
        DnsLookupRequest {
            query: "bad..name".to_string(),
            ..request("x")
        },
        DnsLookupRequest {
            record_type: "SRV".to_string(),
            ..request("example.com")
        },
        DnsLookupRequest {
            server: "bad..server".to_string(),
            ..request("example.com")
        },
    ];
    for request in bad {
        assert!(
            request.validate().is_err(),
            "{request:?} should be rejected"
        );
    }
}

#[test]
fn a_query_with_an_underscore_is_accepted_because_txt_names_such_as_dmarc_use_them() {
    assert!(request("_dmarc.example.com").validate().is_ok());
}

#[tokio::test(start_paused = true)]
async fn a_silent_server_is_not_said_to_have_answered_so_no_response_time_is_shown() {
    let network = FakeNetwork::new().with_dns_answer(
        GOOGLE,
        "example.com",
        RecordType::A,
        ms(4000),
        DnsAnswer::NoResponse,
    );

    let text = lookup(&network, request("example.com")).await.to_text();

    assert!(
        !text.contains("Answered:") && text.ends_with("No response from 8.8.8.8\n"),
        "{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_dns_server_that_is_only_a_link_local_ipv6_address_is_explained_not_called_missing() {
    let network = FakeNetwork::new()
        .with_adapters(vec![adapter("Wi-Fi", &["fe80::1"], "10.0.0.23")])
        .with_route_source(Some("10.0.0.23"));

    let result = lookup(
        &network,
        DnsLookupRequest {
            server: String::new(),
            ..request("example.com")
        },
    )
    .await;

    assert_eq!(
        result.summary(),
        "This PC's only DNS server is fe80::1, a link-local IPv6 address that cannot be reached without its zone: choose one to ask"
    );
}

#[tokio::test(start_paused = true)]
async fn a_trailing_dot_on_the_name_is_not_reported_as_a_different_name_asked() {
    let network = answering("example.com", RecordType::A, DnsAnswer::NoRecords);

    let result = lookup(&network, request("example.com.")).await;

    assert!(!result.to_text().contains("Asked:"), "{}", result.to_text());
}

#[tokio::test(start_paused = true)]
async fn the_dns_server_may_be_typed_as_a_hostname_which_is_resolved_first() {
    let network = FakeNetwork::new()
        .with_name("dc01.corp.local", &["10.0.0.53"])
        .with_dns_answer(
            "10.0.0.53:53",
            "example.com",
            RecordType::A,
            ms(4),
            DnsAnswer::NoRecords,
        );

    let result = lookup(
        &network,
        DnsLookupRequest {
            server: "dc01.corp.local".to_string(),
            ..request("example.com")
        },
    )
    .await;

    assert_eq!(
        result.to_text().lines().nth(3),
        Some("Server:    10.0.0.53 (dc01.corp.local)")
    );
}

#[tokio::test(start_paused = true)]
async fn a_dns_server_hostname_that_does_not_resolve_is_reported_as_such() {
    let network =
        FakeNetwork::new().with_unresolvable_name("dc01.corp.local", "No such host is known.");

    let result = lookup(
        &network,
        DnsLookupRequest {
            server: "dc01.corp.local".to_string(),
            ..request("example.com")
        },
    )
    .await;

    assert_eq!(
        result.summary(),
        "Could not find the DNS server dc01.corp.local: No such host is known."
    );
}
