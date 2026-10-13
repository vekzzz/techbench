//! Port Check, exercised through the toolbox core's public interface (seam S1) over a scripted network (seam S2).

mod common;

use std::time::Duration;

use common::{FakeNetwork, ip, ms};
use tokio_util::sync::CancellationToken;
use toolbox_core::network::ConnectOutcome;
use toolbox_core::port_check::{
    self, PortCheckEvent, PortCheckRequest, PortCheckResult, PortOutcome, PortReport,
};
use toolbox_core::run::RunEnding;
use toolbox_core::target::AddressFamily;

fn request(target: &str, port: &str) -> PortCheckRequest {
    PortCheckRequest {
        target: target.to_string(),
        port: port.to_string(),
        family: AddressFamily::Auto,
    }
}

async fn run(network: &FakeNetwork, request: PortCheckRequest) -> PortCheckResult {
    let check = request.validate().expect("request should be valid");
    port_check::run(network, &check, &CancellationToken::new(), |_| {}).await
}

/// Starts the Run, and stops it as soon as it has to wait on the network.
async fn run_then_stop(network: &FakeNetwork, request: PortCheckRequest) -> PortCheckResult {
    let check = request.validate().expect("request should be valid");
    let stop = CancellationToken::new();
    let (result, ()) = tokio::join!(port_check::run(network, &check, &stop, |_| {}), async {
        tokio::task::yield_now().await;
        stop.cancel();
    });
    result
}

#[tokio::test]
async fn a_port_that_accepts_the_connection_is_open_with_its_connect_time() {
    let network = FakeNetwork::new().with_connect(
        "10.0.0.5:445",
        ConnectOutcome::Accepted {
            connect_time: ms(3),
        },
    );

    let result = run(&network, request("10.0.0.5", "445")).await;

    assert_eq!(
        result.ports,
        vec![PortReport {
            port: 445,
            outcome: PortOutcome::Open {
                connect_time: ms(3)
            }
        }]
    );
}

#[tokio::test]
async fn a_port_that_refuses_the_connection_is_closed() {
    let network = FakeNetwork::new().with_connect("10.0.0.5:3389", ConnectOutcome::Refused);

    let result = run(&network, request("10.0.0.5", "3389")).await;

    assert_eq!(
        result.ports,
        vec![PortReport {
            port: 3389,
            outcome: PortOutcome::Closed
        }]
    );
}

#[tokio::test]
async fn a_port_that_gives_no_answer_within_two_seconds_is_filtered() {
    let network = FakeNetwork::new().with_connect("10.0.0.5:22", ConnectOutcome::TimedOut);

    let result = run(&network, request("10.0.0.5", "22")).await;

    assert_eq!(
        result.ports,
        vec![PortReport {
            port: 22,
            outcome: PortOutcome::Filtered
        }]
    );
    assert_eq!(network.connect_timeouts(), vec![ms(2000)]);
}

#[tokio::test]
async fn a_port_behind_an_unreachable_error_is_filtered() {
    let network = FakeNetwork::new().with_connect("10.0.0.5:443", ConnectOutcome::Unreachable);

    let result = run(&network, request("10.0.0.5", "443")).await;

    assert_eq!(
        result.ports,
        vec![PortReport {
            port: 443,
            outcome: PortOutcome::Filtered
        }]
    );
}

#[tokio::test]
async fn a_hostname_with_several_addresses_uses_its_first_ipv4_address() {
    let network = FakeNetwork::new()
        .with_name("fs01.corp.local", &["fd00::5", "10.0.0.5", "10.0.0.6"])
        .with_connect("10.0.0.5:445", ConnectOutcome::Refused);

    let result = run(&network, request("fs01.corp.local", "445")).await;

    assert_eq!(result.header.resolved, Some(ip("10.0.0.5")));
}

#[tokio::test]
async fn forcing_ipv6_uses_the_first_ipv6_address() {
    let network = FakeNetwork::new()
        .with_name("fs01.corp.local", &["10.0.0.5", "fd00::5", "fd00::6"])
        .with_connect("[fd00::5]:445", ConnectOutcome::Refused);

    let result = run(
        &network,
        PortCheckRequest {
            family: AddressFamily::Ipv6,
            ..request("fs01.corp.local", "445")
        },
    )
    .await;

    assert_eq!(result.header.resolved, Some(ip("fd00::5")));
}

#[tokio::test]
async fn a_hostname_with_only_ipv6_addresses_falls_back_to_ipv6() {
    let network = FakeNetwork::new()
        .with_name("v6only.corp.local", &["fd00::9"])
        .with_connect("[fd00::9]:80", ConnectOutcome::Refused);

    let result = run(&network, request("v6only.corp.local", "80")).await;

    assert_eq!(result.header.resolved, Some(ip("fd00::9")));
}

#[tokio::test]
async fn a_hostname_that_does_not_resolve_ends_the_run_as_a_resolution_failure_not_filtered() {
    let network = FakeNetwork::new()
        .with_unresolvable_name("nosuchhost.corp.local", "No such host is known.");

    let result = run(&network, request("nosuchhost.corp.local", "445")).await;

    assert_eq!(
        (result.ending, result.ports, result.header.resolved),
        (
            RunEnding::ResolutionFailed {
                reason: "No such host is known.".to_string()
            },
            vec![],
            None
        )
    );
}

#[tokio::test]
async fn forcing_ipv4_when_the_hostname_has_only_ipv6_addresses_is_a_resolution_failure() {
    let network = FakeNetwork::new().with_name("v6only.corp.local", &["fd00::9"]);

    let result = run(
        &network,
        PortCheckRequest {
            family: AddressFamily::Ipv4,
            ..request("v6only.corp.local", "80")
        },
    )
    .await;

    assert_eq!(
        result.ending,
        RunEnding::ResolutionFailed {
            reason: "v6only.corp.local has no IPv4 address".to_string()
        }
    );
}

#[test]
fn malformed_targets_are_rejected_before_the_run_starts() {
    for target in [
        "",
        "fs01..corp.local",
        "-fs01.corp.local",
        "fs01 .corp.local",
        "300.1.1.1",
        "fs_01!.corp",
    ] {
        assert!(
            request(target, "445").validate().is_err(),
            "{target:?} should be rejected"
        );
    }
}

#[test]
fn ordinary_targets_are_accepted() {
    for target in [
        "fs01.corp.local",
        "fs01",
        "printer_2.corp.local.",
        "10.0.0.5",
        "fd00::5",
        "[fd00::5]",
    ] {
        assert!(
            request(target, "445").validate().is_ok(),
            "{target:?} should be accepted"
        );
    }
}

#[test]
fn ports_outside_1_to_65535_are_rejected_before_the_run_starts() {
    for port in ["", "0", "65536", "http", "-1"] {
        assert!(
            request("10.0.0.5", port).validate().is_err(),
            "{port:?} should be rejected"
        );
    }
}

#[test]
fn an_ip_address_that_contradicts_the_forced_ip_version_is_rejected() {
    let ipv4_forced_to_v6 = PortCheckRequest {
        family: AddressFamily::Ipv6,
        ..request("10.0.0.5", "445")
    };
    let ipv6_forced_to_v4 = PortCheckRequest {
        family: AddressFamily::Ipv4,
        ..request("fd00::5", "445")
    };

    assert!(ipv4_forced_to_v6.validate().is_err() && ipv6_forced_to_v4.validate().is_err());
}

#[tokio::test]
async fn stopping_a_run_ends_it_with_a_stopped_result_holding_what_was_collected() {
    let network = FakeNetwork::new().with_hanging_connect("10.0.0.5:445");

    let result = run_then_stop(&network, request("10.0.0.5", "445")).await;

    assert_eq!(
        (result.ending, result.ports, result.header.resolved),
        (RunEnding::Stopped, vec![], Some(ip("10.0.0.5")))
    );
}

#[tokio::test]
async fn stopping_a_run_while_its_hostname_is_still_resolving_ends_it_as_stopped() {
    let network = FakeNetwork::new().with_hanging_name("slow.corp.local");

    let result = run_then_stop(&network, request("slow.corp.local", "445")).await;

    assert_eq!(
        (result.ending, result.header.resolved),
        (RunEnding::Stopped, None)
    );
}

#[tokio::test]
async fn a_run_reports_its_resolved_address_then_each_port_as_it_goes() {
    let network = FakeNetwork::new()
        .with_name("fs01.corp.local", &["10.0.0.5"])
        .with_connect("10.0.0.5:445", ConnectOutcome::Refused);
    let check = request("fs01.corp.local", "445").validate().unwrap();
    let mut events = Vec::new();

    port_check::run(&network, &check, &CancellationToken::new(), |event| {
        events.push(event)
    })
    .await;

    assert_eq!(
        events,
        vec![
            PortCheckEvent::Resolved(ip("10.0.0.5")),
            PortCheckEvent::Port(PortReport {
                port: 445,
                outcome: PortOutcome::Closed
            }),
        ]
    );
}

#[tokio::test]
async fn copy_as_text_gives_a_header_aligned_rows_and_a_summary() {
    let network = FakeNetwork::new()
        .with_name("fs01.corp.local", &["10.0.0.5"])
        .with_connect(
            "10.0.0.5:445",
            ConnectOutcome::Accepted {
                connect_time: ms(3),
            },
        );

    let result = run(&network, request("fs01.corp.local", "445")).await;

    assert_eq!(
        result.to_text(),
        "\
Port Check
Target:    fs01.corp.local
Resolved:  10.0.0.5
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

PORT  OUTCOME  CONNECT TIME
445   Open     3 ms

1 port checked: 1 Open, 0 Closed, 0 Filtered
"
    );
}

#[tokio::test]
async fn copy_as_text_of_a_resolution_failure_says_why_instead_of_listing_ports() {
    let network = FakeNetwork::new()
        .with_unresolvable_name("nosuchhost.corp.local", "No such host is known.");

    let result = run(&network, request("nosuchhost.corp.local", "445")).await;

    assert_eq!(
        result.to_text(),
        "\
Port Check
Target:    nosuchhost.corp.local
Resolved:  (not resolved)
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC

Could not resolve nosuchhost.corp.local: No such host is known.
"
    );
}

#[tokio::test]
async fn copy_as_text_of_a_stopped_run_says_it_was_stopped() {
    let network = FakeNetwork::new().with_hanging_connect("10.0.0.5:445");

    let result = run_then_stop(&network, request("10.0.0.5", "445")).await;

    assert_eq!(
        result.to_text(),
        "\
Port Check
Target:    10.0.0.5
Resolved:  10.0.0.5
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

Stopped before any port answered.
"
    );
}

#[tokio::test]
async fn copy_as_text_shows_a_sub_millisecond_connect_as_under_one_millisecond() {
    let network = FakeNetwork::new().with_connect(
        "127.0.0.1:8080",
        ConnectOutcome::Accepted {
            connect_time: Duration::from_micros(400),
        },
    );

    let text = run(&network, request("127.0.0.1", "8080")).await.to_text();

    assert!(text.contains("8080  Open     <1 ms\n"), "{text}");
}
