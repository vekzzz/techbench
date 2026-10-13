//! Ping, exercised through the toolbox core's public interface (seam S1) over a scripted network (seam S2).

mod common;

use common::{EchoScript, FakeNetwork, ip, ms};
use tokio_util::sync::CancellationToken;
use toolbox_core::network::{EchoOutcome, EchoRequest};
use toolbox_core::ping::{self, EchoReport, PingRequest, PingResult};
use toolbox_core::run::RunEnding;
use toolbox_core::target::AddressFamily;

fn request(target: &str) -> PingRequest {
    PingRequest {
        target: target.to_string(),
        ..PingRequest::default()
    }
}

fn reply(millis: u64) -> EchoScript {
    EchoScript::Answer(EchoOutcome::Reply {
        round_trip: ms(millis),
        ttl: Some(128),
    })
}

async fn run(network: &FakeNetwork, request: PingRequest) -> PingResult {
    let ping = request.validate().expect("request should be valid");
    ping::run(network, &ping, &CancellationToken::new(), |_| {}).await
}

#[tokio::test(start_paused = true)]
async fn by_default_four_echoes_are_sent_and_each_is_reported_by_sequence_number() {
    let network =
        FakeNetwork::new().with_echoes("10.0.0.5", [reply(3), reply(5), reply(4), reply(4)]);

    let result = run(&network, request("10.0.0.5")).await;

    let replied = |sequence, millis| EchoReport {
        sequence,
        outcome: EchoOutcome::Reply {
            round_trip: ms(millis),
            ttl: Some(128),
        },
    };
    assert_eq!(
        result.echoes,
        vec![replied(1, 3), replied(2, 5), replied(3, 4), replied(4, 4)]
    );
}

#[tokio::test(start_paused = true)]
async fn each_echo_carries_the_chosen_size_timeout_and_dont_fragment_setting() {
    let network = FakeNetwork::new().with_echoes("10.0.0.5", [reply(3), reply(3)]);
    let custom = PingRequest {
        count: "2".to_string(),
        payload_size: "1472".to_string(),
        timeout_ms: "500".to_string(),
        dont_fragment: true,
        ..request("10.0.0.5")
    };

    run(&network, custom).await;

    let sent = EchoRequest {
        address: ip("10.0.0.5"),
        payload_size: 1472,
        dont_fragment: true,
        timeout: ms(500),
    };
    assert_eq!(network.echo_requests(), vec![sent, sent]);
}

#[tokio::test(start_paused = true)]
async fn the_defaults_are_32_bytes_a_2_second_timeout_and_fragmenting_allowed() {
    let network =
        FakeNetwork::new().with_echoes("10.0.0.5", [reply(3), reply(3), reply(3), reply(3)]);

    run(&network, request("10.0.0.5")).await;

    let sent = EchoRequest {
        address: ip("10.0.0.5"),
        payload_size: 32,
        dont_fragment: false,
        timeout: ms(2000),
    };
    assert_eq!(network.echo_requests(), vec![sent; 4]);
}

#[tokio::test(start_paused = true)]
async fn echoes_are_sent_one_second_apart_with_no_wait_after_the_last() {
    let network =
        FakeNetwork::new().with_echoes("10.0.0.5", [reply(3), reply(3), reply(3), reply(3)]);
    let started = tokio::time::Instant::now();

    run(&network, request("10.0.0.5")).await;

    assert_eq!(started.elapsed(), ms(3000));
}

#[tokio::test(start_paused = true)]
async fn an_echo_that_times_out_is_followed_at_once_because_its_second_has_passed() {
    let network =
        FakeNetwork::new().with_echoes("10.0.0.5", [EchoScript::Silent, EchoScript::Silent]);
    let started = tokio::time::Instant::now();

    run(
        &network,
        PingRequest {
            count: "2".to_string(),
            ..request("10.0.0.5")
        },
    )
    .await;

    assert_eq!(started.elapsed(), ms(4000));
}

#[tokio::test(start_paused = true)]
async fn copy_as_text_lists_every_echo_and_sums_up_loss_and_round_trip() {
    let network = FakeNetwork::new().with_echoes(
        "10.0.0.5",
        [reply(3), EchoScript::Silent, reply(5), reply(4)],
    );

    let result = run(&network, request("10.0.0.5")).await;

    assert_eq!(
        result.to_text(),
        "\
Ping
Target:    10.0.0.5
Resolved:  10.0.0.5
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)
Echoes:    4 x 32 bytes, 2000 ms timeout

SEQ  STATUS     TIME  TTL
1    Reply      3 ms  128
2    Timed out  -     -
3    Reply      5 ms  128
4    Reply      4 ms  128

4 sent, 3 received, 25% loss; round trip min 3 ms, avg 4 ms, max 5 ms
"
    );
}

#[tokio::test(start_paused = true)]
async fn unreachable_fragmentation_needed_and_timed_out_are_told_apart() {
    let network = FakeNetwork::new().with_echoes(
        "10.0.0.5",
        [
            EchoScript::Answer(EchoOutcome::FragmentationNeeded),
            EchoScript::Answer(EchoOutcome::Unreachable {
                from: Some(ip("10.0.0.1")),
            }),
            EchoScript::Silent,
        ],
    );
    let mtu_probe = PingRequest {
        count: "3".to_string(),
        payload_size: "1472".to_string(),
        dont_fragment: true,
        ..request("10.0.0.5")
    };

    let text = run(&network, mtu_probe).await.to_text();

    assert!(
        text.ends_with(
            "\
Echoes:    3 x 1472 bytes, 2000 ms timeout, Don't Fragment

SEQ  STATUS                     TIME  TTL
1    Fragmentation needed       -     -
2    Unreachable from 10.0.0.1  -     -
3    Timed out                  -     -

3 sent, 0 received, 100% loss
"
        ),
        "{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn stopping_mid_run_keeps_the_finished_echoes_and_sums_them_up() {
    let network =
        FakeNetwork::new().with_echoes("10.0.0.5", [reply(3), reply(4), EchoScript::Hang]);
    let ping = request("10.0.0.5").validate().unwrap();
    let stop = CancellationToken::new();

    let (result, ()) = tokio::join!(ping::run(&network, &ping, &stop, |_| {}), async {
        tokio::time::sleep(ms(2500)).await;
        stop.cancel();
    });

    assert_eq!(
        (&result.ending, result.echoes.len(), result.summary()),
        (
            &RunEnding::Stopped,
            2,
            "Stopped after 2 of 4 echoes: 2 sent, 2 received, 0% loss; round trip min 3 ms, avg 3 ms, max 4 ms"
                .to_string()
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_hostname_that_does_not_resolve_ends_the_run_as_a_resolution_failure() {
    let network = FakeNetwork::new()
        .with_unresolvable_name("nosuchhost.corp.local", "No such host is known.");

    let result = run(&network, request("nosuchhost.corp.local")).await;

    assert_eq!(
        (&result.ending, result.summary()),
        (
            &RunEnding::ResolutionFailed {
                reason: "No such host is known.".to_string()
            },
            "Could not resolve nosuchhost.corp.local: No such host is known.".to_string()
        )
    );
}

#[tokio::test(start_paused = true)]
async fn forcing_ipv6_pings_the_first_ipv6_address() {
    let network = FakeNetwork::new()
        .with_name("fs01.corp.local", &["10.0.0.5", "fd00::5"])
        .with_echoes("fd00::5", [reply(1)]);

    let result = run(
        &network,
        PingRequest {
            count: "1".to_string(),
            family: AddressFamily::Ipv6,
            ..request("fs01.corp.local")
        },
    )
    .await;

    assert_eq!(result.header.resolved, Some(ip("fd00::5")));
}

#[test]
fn counts_sizes_and_timeouts_outside_their_ranges_are_rejected_by_name() {
    let cases = [
        (
            "count",
            PingRequest {
                count: "0".to_string(),
                ..request("10.0.0.5")
            },
        ),
        (
            "count",
            PingRequest {
                count: "1001".to_string(),
                ..request("10.0.0.5")
            },
        ),
        (
            "count",
            PingRequest {
                count: "four".to_string(),
                ..request("10.0.0.5")
            },
        ),
        (
            "size",
            PingRequest {
                payload_size: "65501".to_string(),
                ..request("10.0.0.5")
            },
        ),
        (
            "size",
            PingRequest {
                payload_size: "-1".to_string(),
                ..request("10.0.0.5")
            },
        ),
        (
            "timeout",
            PingRequest {
                timeout_ms: "99".to_string(),
                ..request("10.0.0.5")
            },
        ),
        (
            "timeout",
            PingRequest {
                timeout_ms: "60001".to_string(),
                ..request("10.0.0.5")
            },
        ),
    ];
    for (field, request) in cases {
        let message = request.validate().map(|_| ()).unwrap_err().message;
        assert!(
            message.contains(field),
            "{message:?} should mention {field}"
        );
    }
}

#[test]
fn the_edges_of_each_range_are_accepted() {
    let edges = [
        PingRequest {
            count: "1".to_string(),
            payload_size: "0".to_string(),
            timeout_ms: "100".to_string(),
            ..request("10.0.0.5")
        },
        PingRequest {
            count: "1000".to_string(),
            payload_size: "65500".to_string(),
            timeout_ms: "60000".to_string(),
            ..request("10.0.0.5")
        },
    ];
    for request in edges {
        assert!(request.validate().is_ok(), "{request:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn dont_fragment_is_not_applied_over_ipv6_and_the_result_says_so() {
    let network = FakeNetwork::new().with_echoes("fd00::5", [reply(2)]);
    let probe = PingRequest {
        count: "1".to_string(),
        dont_fragment: true,
        ..request("fd00::5")
    };

    let result = run(&network, probe).await;

    assert_eq!(
        (
            network.echo_requests()[0].dont_fragment,
            result.to_text().contains(
                "Echoes:    1 x 32 bytes, 2000 ms timeout, Don't Fragment not applied (IPv6)\n"
            )
        ),
        (false, true)
    );
}

#[tokio::test(start_paused = true)]
async fn copy_as_text_of_a_stopped_run_lists_the_finished_echoes_then_how_far_it_got() {
    let network = FakeNetwork::new().with_echoes("10.0.0.5", [reply(3), EchoScript::Hang]);
    let ping = request("10.0.0.5").validate().unwrap();
    let stop = CancellationToken::new();

    let (result, ()) = tokio::join!(ping::run(&network, &ping, &stop, |_| {}), async {
        tokio::time::sleep(ms(1500)).await;
        stop.cancel();
    });

    assert_eq!(
        result.to_text(),
        "\
Ping
Target:    10.0.0.5
Resolved:  10.0.0.5
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)
Echoes:    4 x 32 bytes, 2000 ms timeout

SEQ  STATUS  TIME  TTL
1    Reply   3 ms  128

Stopped after 1 of 4 echoes: 1 sent, 1 received, 0% loss; round trip min 3 ms, avg 3 ms, max 3 ms
"
    );
}

#[tokio::test(start_paused = true)]
async fn stopping_before_the_first_echo_finishes_says_so() {
    let network = FakeNetwork::new().with_echoes("10.0.0.5", [EchoScript::Hang]);
    let ping = request("10.0.0.5").validate().unwrap();
    let stop = CancellationToken::new();

    let (result, ()) = tokio::join!(ping::run(&network, &ping, &stop, |_| {}), async {
        tokio::time::sleep(ms(500)).await;
        stop.cancel();
    });

    assert_eq!(result.summary(), "Stopped before the first echo finished.");
}

#[tokio::test(start_paused = true)]
async fn copy_as_text_of_a_resolution_failure_says_why_instead_of_listing_echoes() {
    let network = FakeNetwork::new()
        .with_unresolvable_name("nosuchhost.corp.local", "No such host is known.");

    let result = run(&network, request("nosuchhost.corp.local")).await;

    assert_eq!(
        result.to_text(),
        "\
Ping
Target:    nosuchhost.corp.local
Resolved:  (not resolved)
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC
Echoes:    4 x 32 bytes, 2000 ms timeout

Could not resolve nosuchhost.corp.local: No such host is known.
"
    );
}
