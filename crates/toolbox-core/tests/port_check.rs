//! Port Check, exercised through the toolbox core's public interface (seam S1) over a scripted network (seam S2).

mod common;

use std::time::Duration;

use common::{FakeNetwork, ip, ms};
use tokio_util::sync::CancellationToken;
use toolbox_core::network::ConnectOutcome;
use toolbox_core::port_check::{
    self, PortCheckEvent, PortCheckRequest, PortCheckResult, PortOutcome, PortReport,
};
use toolbox_core::run::{Progress, RunEnding};
use toolbox_core::target::AddressFamily;

fn request(target: &str, ports: &str) -> PortCheckRequest {
    PortCheckRequest {
        target: target.to_string(),
        ports: ports.to_string(),
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

PORT  SERVICE  OUTCOME  CONNECT TIME
445   SMB      Open     3 ms

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

Stopped before any port was checked.
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

    assert!(text.contains("Open     <1 ms\n"), "{text}");
}

#[tokio::test]
async fn a_comma_separated_list_checks_every_port_and_reports_them_in_port_order() {
    let network = FakeNetwork::new()
        .with_connect("10.0.0.5:22", ConnectOutcome::Refused)
        .with_connect(
            "10.0.0.5:80",
            ConnectOutcome::Accepted {
                connect_time: ms(2),
            },
        )
        .with_connect("10.0.0.5:443", ConnectOutcome::TimedOut);

    let result = run(&network, request("10.0.0.5", "443, 22,80")).await;

    assert_eq!(
        result.ports,
        vec![
            PortReport {
                port: 22,
                outcome: PortOutcome::Closed
            },
            PortReport {
                port: 80,
                outcome: PortOutcome::Open {
                    connect_time: ms(2)
                }
            },
            PortReport {
                port: 443,
                outcome: PortOutcome::Filtered
            },
        ]
    );
}

#[test]
fn ranges_are_inclusive_and_the_ports_are_deduplicated_and_sorted() {
    let check = request("10.0.0.5", "8000-8002, 22, 8001, 20-22")
        .validate()
        .unwrap();

    assert_eq!(check.ports(), [20, 21, 22, 8000, 8001, 8002]);
}

#[test]
fn malformed_port_entries_are_rejected() {
    for ports in ["80-", "-80", "90-80", "1-99999", "22;80", "22, , 80"] {
        assert!(
            request("10.0.0.5", ports).validate().is_err(),
            "{ports:?} should be rejected"
        );
    }
}

#[test]
fn the_rejection_names_the_entry_that_is_wrong() {
    let error = request("10.0.0.5", "22, 443, 90-80")
        .validate()
        .unwrap_err();

    assert!(error.message.contains("90-80"), "{}", error.message);
}

#[test]
fn a_run_may_check_up_to_1024_ports() {
    assert_eq!(
        request("10.0.0.5", "1-1024")
            .validate()
            .map(|check| check.ports().len()),
        Ok(1024)
    );
}

#[test]
fn more_than_1024_ports_are_rejected_with_a_clear_message() {
    let error = request("10.0.0.5", "1-1025").validate().unwrap_err();

    assert!(error.message.contains("1024"), "{}", error.message);
}

#[tokio::test(start_paused = true)]
async fn silent_ports_are_waited_on_together_so_a_preset_takes_one_timeout() {
    let network = FakeNetwork::new().with_silent_ports("10.0.0.5", [135, 445, 3389, 5985, 5986]);
    let started = tokio::time::Instant::now();

    let result = run(&network, request("10.0.0.5", "135, 445, 3389, 5985, 5986")).await;

    assert_eq!((result.ports.len(), started.elapsed()), (5, ms(2000)));
}

#[tokio::test(start_paused = true)]
async fn no_more_than_32_ports_are_waited_on_at_once() {
    let network = FakeNetwork::new().with_silent_ports("10.0.0.5", 1..=40);
    let started = tokio::time::Instant::now();

    let result = run(&network, request("10.0.0.5", "1-40")).await;

    // 32 in the first wave, the remaining 8 in a second: two timeouts in all.
    assert_eq!((result.ports.len(), started.elapsed()), (40, ms(4000)));
}

#[tokio::test(start_paused = true)]
async fn outcomes_are_reported_as_each_port_finishes_while_the_result_stays_in_port_order() {
    let network = FakeNetwork::new()
        .with_silent_ports("10.0.0.5", [22])
        .with_connect("10.0.0.5:80", ConnectOutcome::Refused);
    let check = request("10.0.0.5", "22, 80").validate().unwrap();
    let mut events = Vec::new();

    let result = port_check::run(&network, &check, &CancellationToken::new(), |event| {
        events.push(event)
    })
    .await;

    let closed_80 = PortReport {
        port: 80,
        outcome: PortOutcome::Closed,
    };
    let filtered_22 = PortReport {
        port: 22,
        outcome: PortOutcome::Filtered,
    };
    assert_eq!(
        (events, result.ports),
        (
            vec![
                PortCheckEvent::Resolved(ip("10.0.0.5")),
                PortCheckEvent::Port(closed_80),
                PortCheckEvent::Port(filtered_22)
            ],
            vec![filtered_22, closed_80]
        )
    );
}

#[tokio::test]
async fn stopping_partway_keeps_the_ports_already_checked_and_says_how_far_it_got() {
    let network = FakeNetwork::new()
        .with_connect("10.0.0.5:22", ConnectOutcome::Refused)
        .with_hanging_connect("10.0.0.5:445");

    let result = run_then_stop(&network, request("10.0.0.5", "22, 445")).await;

    assert_eq!(
        result.to_text(),
        "\
Port Check
Target:    10.0.0.5
Resolved:  10.0.0.5
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

PORT  SERVICE  OUTCOME  CONNECT TIME
22    SSH      Closed   -

Stopped after 1 of 2 ports: 0 Open, 1 Closed, 0 Filtered
"
    );
}

#[test]
fn well_known_ports_carry_a_service_name_hint() {
    let hints: Vec<_> = [3389, 5985, 445, 12345]
        .into_iter()
        .map(port_check::service_name)
        .collect();

    assert_eq!(hints, [Some("RDP"), Some("WinRM"), Some("SMB"), None]);
}

#[test]
fn the_seven_port_presets_hold_exactly_the_ports_the_spec_lists() {
    let presets: Vec<(&str, &[u16])> = port_check::PORT_PRESETS
        .iter()
        .map(|preset| (preset.name, preset.ports))
        .collect();

    assert_eq!(
        presets,
        [
            ("Windows", &[135, 445, 3389, 5985, 5986][..]),
            ("Directory", &[53, 88, 389, 636, 3268]),
            ("Web", &[80, 443, 8080, 8443]),
            ("Remote", &[22, 3389, 5900]),
            ("Mail", &[25, 465, 587, 993, 995]),
            ("Database", &[1433, 3306, 5432]),
            ("Printing", &[9100, 631, 515]),
        ]
    );
}

#[tokio::test]
async fn copy_as_text_of_several_ports_aligns_each_column_and_counts_every_outcome() {
    let network = FakeNetwork::new()
        .with_connect("10.0.0.5:22", ConnectOutcome::Refused)
        .with_connect(
            "10.0.0.5:80",
            ConnectOutcome::Accepted {
                connect_time: ms(3),
            },
        )
        .with_connect("10.0.0.5:3389", ConnectOutcome::TimedOut);

    let result = run(&network, request("10.0.0.5", "3389, 80, 22")).await;

    assert_eq!(
        result.to_text(),
        "\
Port Check
Target:    10.0.0.5
Resolved:  10.0.0.5
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

PORT  SERVICE  OUTCOME   CONNECT TIME
22    SSH      Closed    -
80    HTTP     Open      3 ms
3389  RDP      Filtered  -

3 ports checked: 1 Open, 1 Closed, 1 Filtered
"
    );
}

#[test]
fn an_empty_port_field_asks_for_a_port() {
    let error = request("10.0.0.5", "  ").validate().unwrap_err();

    assert_eq!(
        error.message,
        "enter at least one port, such as 443 or 22, 80, 8000-8010"
    );
}

#[tokio::test]
async fn the_text_of_a_run_still_going_says_how_far_it_has_got() {
    let network = FakeNetwork::new()
        .with_connect("10.0.0.5:22", ConnectOutcome::Refused)
        .with_hanging_connect("10.0.0.5:445");
    let check = request("10.0.0.5", "22, 445").validate().unwrap();
    let progress = Progress::default();
    let stop = CancellationToken::new();

    let (_, text_while_running) = tokio::join!(
        port_check::run_with_progress(&network, &check, &stop, &progress, |_| {}),
        async {
            tokio::task::yield_now().await;
            let text = progress.read(PortCheckResult::to_text);
            stop.cancel();
            text
        }
    );

    assert_eq!(
        text_while_running.as_deref(),
        Some(
            "\
Port Check
Target:    10.0.0.5
Resolved:  10.0.0.5
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

PORT  SERVICE  OUTCOME  CONNECT TIME
22    SSH      Closed   -

Still running: 1 of 2 ports checked so far: 0 Open, 1 Closed, 0 Filtered
"
        )
    );
}
