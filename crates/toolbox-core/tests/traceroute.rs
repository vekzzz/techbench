//! Traceroute, exercised through the toolbox core's public interface (seam S1) over a scripted
//! network (seam S2) and tokio's paused clock.

mod common;

use common::{EchoScript, FakeNetwork, ip, ms};
use tokio_util::sync::CancellationToken;
use toolbox_core::network::EchoOutcome;
use toolbox_core::run::{Progress, RunEnding};
use toolbox_core::target::AddressFamily;
use toolbox_core::traceroute::{self, TracerouteEvent, TracerouteRequest, TracerouteResult};

fn request(target: &str) -> TracerouteRequest {
    TracerouteRequest {
        target: target.to_string(),
        family: AddressFamily::Auto,
    }
}

/// A router at `from` found the echo's TTL spent after `millis`.
fn router(from: &str, millis: u64) -> EchoScript {
    EchoScript::Answer(EchoOutcome::TimeExceeded {
        from: Some(ip(from)),
        round_trip: ms(millis),
    })
}

/// The Target itself answered after `millis`.
fn target_reply(millis: u64) -> EchoScript {
    EchoScript::Answer(EchoOutcome::Reply {
        round_trip: ms(millis),
        ttl: Some(117),
    })
}

/// Scripts one Hop: three probes at this TTL, each answered as given.
fn hop(network: FakeNetwork, ttl: u8, probes: [EchoScript; 3]) -> FakeNetwork {
    network.with_hop_echoes("8.8.8.8", ttl, probes)
}

async fn run_traceroute(network: &FakeNetwork) -> TracerouteResult {
    let traceroute = request("8.8.8.8")
        .validate()
        .expect("request should be valid");
    traceroute::run(network, &traceroute, &CancellationToken::new(), |_| {}).await
}

#[tokio::test(start_paused = true)]
async fn each_hop_lists_its_address_and_three_round_trips_and_the_traceroute_stops_at_the_target() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [
            router("192.168.1.1", 1),
            router("192.168.1.1", 2),
            router("192.168.1.1", 1),
        ],
    );
    let network = hop(
        network,
        2,
        [
            router("10.20.0.1", 9),
            router("10.20.0.1", 8),
            router("10.20.0.1", 10),
        ],
    );
    let network = hop(
        network,
        3,
        [target_reply(14), target_reply(15), target_reply(13)],
    );

    let result = run_traceroute(&network).await;

    assert_eq!(
        (
            result
                .hops
                .iter()
                .map(|hop| (hop.number, hop.address))
                .collect::<Vec<_>>(),
            result.summary()
        ),
        (
            vec![
                (1, Some(ip("192.168.1.1"))),
                (2, Some(ip("10.20.0.1"))),
                (3, Some(ip("8.8.8.8")))
            ],
            "Reached 8.8.8.8 in 3 hops".to_string()
        )
    );
}

#[tokio::test(start_paused = true)]
async fn the_text_shows_silent_probes_as_a_star_and_a_silent_hop_as_only_stars() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [
            router("192.168.1.1", 1),
            router("192.168.1.1", 2),
            router("192.168.1.1", 1),
        ],
    );
    let network = hop(
        network,
        2,
        [EchoScript::Silent, EchoScript::Silent, EchoScript::Silent],
    );
    let network = hop(
        network,
        3,
        [target_reply(14), EchoScript::Silent, target_reply(13)],
    );

    let result = run_traceroute(&network).await;

    assert_eq!(
        result.to_text(),
        "\
Traceroute
Target:    8.8.8.8
Resolved:  8.8.8.8
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

HOP  ADDRESS      HOSTNAME  PROBE 1  PROBE 2  PROBE 3
1    192.168.1.1  -         1 ms     2 ms     1 ms
2    *            -         *        *        *
3    8.8.8.8      -         14 ms    *        13 ms

Reached 8.8.8.8 in 3 hops
"
    );
}

#[tokio::test(start_paused = true)]
async fn a_hop_where_only_some_probes_answer_still_has_an_address() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [
            EchoScript::Silent,
            router("192.168.1.1", 3),
            EchoScript::Silent,
        ],
    );
    let network = hop(
        network,
        2,
        [target_reply(5), target_reply(5), target_reply(5)],
    );

    let result = run_traceroute(&network).await;

    assert_eq!(result.hops[0].address, Some(ip("192.168.1.1")));
}

#[tokio::test(start_paused = true)]
async fn the_three_probes_of_a_hop_are_sent_together_so_a_silent_hop_costs_one_timeout() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [EchoScript::Silent, EchoScript::Silent, EchoScript::Silent],
    );
    let network = hop(
        network,
        2,
        [target_reply(5), target_reply(5), target_reply(5)],
    );
    let started = tokio::time::Instant::now();

    run_traceroute(&network).await;

    // 1 s timeout for the silent hop; the answered hop takes no virtual time.
    assert_eq!(started.elapsed(), ms(1000));
}

#[tokio::test(start_paused = true)]
async fn a_target_that_never_answers_ends_the_traceroute_after_thirty_hops() {
    let mut network = FakeNetwork::new();
    for ttl in 1..=30 {
        network = hop(
            network,
            ttl,
            [EchoScript::Silent, EchoScript::Silent, EchoScript::Silent],
        );
    }

    let result = run_traceroute(&network).await;

    assert_eq!(
        (result.hops.len(), result.ending.clone(), result.summary()),
        (
            30,
            RunEnding::Completed,
            "Did not reach 8.8.8.8 within 30 hops".to_string()
        )
    );
}

#[tokio::test(start_paused = true)]
async fn each_probe_is_sent_with_its_hops_ttl_and_a_one_second_timeout() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [
            router("192.168.1.1", 1),
            router("192.168.1.1", 1),
            router("192.168.1.1", 1),
        ],
    );
    let network = hop(
        network,
        2,
        [target_reply(5), target_reply(5), target_reply(5)],
    );

    run_traceroute(&network).await;

    let sent: Vec<(Option<u8>, std::time::Duration)> = network
        .echo_requests()
        .iter()
        .map(|echo| (echo.ttl, echo.timeout))
        .collect();
    assert_eq!(
        sent,
        [(Some(1), ms(1000)); 3]
            .into_iter()
            .chain([(Some(2), ms(1000)); 3])
            .collect::<Vec<_>>()
    );
}

#[tokio::test(start_paused = true)]
async fn hostnames_arrive_as_later_events_and_never_delay_the_next_hop() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [
            router("192.168.1.1", 1),
            router("192.168.1.1", 1),
            router("192.168.1.1", 1),
        ],
    );
    let network = hop(
        network,
        2,
        [target_reply(5), target_reply(5), target_reply(5)],
    )
    .with_slow_reverse_name("192.168.1.1", ms(1500), "router.lan")
    .with_reverse_name("8.8.8.8", "dns.google");
    let traceroute = request("8.8.8.8").validate().unwrap();
    let mut events = Vec::new();
    let started = tokio::time::Instant::now();

    let result = traceroute::run(&network, &traceroute, &CancellationToken::new(), |event| {
        events.push(event)
    })
    .await;

    let order: Vec<String> = events
        .iter()
        .map(|event| match event {
            TracerouteEvent::Resolved(_) => "resolved".to_string(),
            TracerouteEvent::Hop(hop) => format!("hop {}", hop.number),
            TracerouteEvent::Hostname { hop, hostname } => format!("name {hop} {hostname}"),
        })
        .collect();
    assert_eq!(
        (
            order,
            result.hops[0].hostname.clone(),
            result.hops[1].hostname.clone(),
            started.elapsed()
        ),
        (
            vec![
                "resolved",
                "hop 1",
                "hop 2",
                "name 2 dns.google",
                "name 1 router.lan"
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            Some("router.lan".to_string()),
            Some("dns.google".to_string()),
            ms(1500)
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_reverse_lookup_that_takes_too_long_is_given_up_on_after_two_seconds() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [target_reply(5), target_reply(5), target_reply(5)],
    )
    .with_slow_reverse_name("8.8.8.8", ms(60_000), "dns.google");
    let started = tokio::time::Instant::now();

    let result = run_traceroute(&network).await;

    assert_eq!(
        (result.hops[0].hostname.clone(), started.elapsed()),
        (None, ms(2000))
    );
}

#[tokio::test(start_paused = true)]
async fn the_text_shows_hostnames_in_their_own_column() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [target_reply(5), target_reply(5), target_reply(5)],
    )
    .with_reverse_name("8.8.8.8", "dns.google");

    let text = run_traceroute(&network).await.to_text();

    assert!(
        text.contains("\n1    8.8.8.8  dns.google  5 ms     5 ms     5 ms\n"),
        "{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn stopping_mid_traceroute_keeps_the_hops_found_and_says_how_far_it_got() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [
            router("192.168.1.1", 1),
            router("192.168.1.1", 1),
            router("192.168.1.1", 1),
        ],
    )
    .with_hop_echoes(
        "8.8.8.8",
        2,
        [EchoScript::Hang, EchoScript::Hang, EchoScript::Hang],
    );
    let traceroute = request("8.8.8.8").validate().unwrap();
    let stop = CancellationToken::new();

    let (result, ()) = tokio::join!(
        traceroute::run(&network, &traceroute, &stop, |_| {}),
        async {
            tokio::time::sleep(ms(500)).await;
            stop.cancel();
        }
    );

    assert_eq!(
        (result.hops.len(), result.ending.clone(), result.summary()),
        (1, RunEnding::Stopped, "Stopped after 1 hop".to_string())
    );
}

#[tokio::test(start_paused = true)]
async fn the_summary_of_a_traceroute_still_going_says_how_far_it_has_got() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [
            router("192.168.1.1", 1),
            router("192.168.1.1", 1),
            router("192.168.1.1", 1),
        ],
    )
    .with_hop_echoes(
        "8.8.8.8",
        2,
        [EchoScript::Hang, EchoScript::Hang, EchoScript::Hang],
    );
    let traceroute = request("8.8.8.8").validate().unwrap();
    let progress = Progress::default();
    let stop = CancellationToken::new();

    let (_, summary_while_running) = tokio::join!(
        traceroute::run_with_progress(&network, &traceroute, &stop, &progress, |_| {}),
        async {
            tokio::time::sleep(ms(500)).await;
            let summary = progress.read(TracerouteResult::summary);
            stop.cancel();
            summary
        }
    );

    assert_eq!(
        summary_while_running.as_deref(),
        Some("Still running: 1 hop found so far")
    );
}

#[tokio::test(start_paused = true)]
async fn a_router_reporting_the_destination_unreachable_ends_the_traceroute_there() {
    let unreachable = || {
        EchoScript::Answer(EchoOutcome::Unreachable {
            from: Some(ip("10.20.0.1")),
        })
    };
    let network = hop(
        FakeNetwork::new(),
        1,
        [
            router("192.168.1.1", 1),
            router("192.168.1.1", 1),
            router("192.168.1.1", 1),
        ],
    );
    let network = hop(network, 2, [unreachable(), unreachable(), unreachable()]);

    let result = run_traceroute(&network).await;

    assert_eq!(
        result.summary(),
        "Destination unreachable: 10.20.0.1 reported it at hop 2"
    );
}

#[tokio::test(start_paused = true)]
async fn forcing_ipv6_traces_the_first_ipv6_address() {
    let network = FakeNetwork::new()
        .with_name("dns.example", &["8.8.8.8", "2001:4860:4860::8888"])
        .with_hop_echoes(
            "2001:4860:4860::8888",
            1,
            [target_reply(5), target_reply(5), target_reply(5)],
        );
    let traceroute = TracerouteRequest {
        target: "dns.example".to_string(),
        family: AddressFamily::Ipv6,
    }
    .validate()
    .unwrap();

    let result = traceroute::run(&network, &traceroute, &CancellationToken::new(), |_| {}).await;

    assert_eq!(result.header.resolved, Some(ip("2001:4860:4860::8888")));
}

#[tokio::test(start_paused = true)]
async fn a_hostname_that_does_not_resolve_ends_the_run_as_a_resolution_failure() {
    let network = FakeNetwork::new()
        .with_unresolvable_name("nosuchhost.corp.local", "No such host is known.");
    let traceroute = request("nosuchhost.corp.local").validate().unwrap();

    let result = traceroute::run(&network, &traceroute, &CancellationToken::new(), |_| {}).await;

    assert_eq!(
        result.summary(),
        "Could not resolve nosuchhost.corp.local: No such host is known."
    );
}

#[tokio::test(start_paused = true)]
async fn a_failure_to_send_from_this_pc_ends_the_traceroute_at_once_and_says_so() {
    let no_route = || EchoScript::Answer(EchoOutcome::Unreachable { from: None });
    let network = hop(FakeNetwork::new(), 1, [no_route(), no_route(), no_route()]);

    let result = run_traceroute(&network).await;

    assert_eq!(
        (result.hops.len(), result.summary()),
        (1, "Could not send probes from this PC to 8.8.8.8: there is no route, or the adapter is down".to_string())
    );
}

#[tokio::test(start_paused = true)]
async fn a_hop_answered_by_different_routers_shows_each_probes_own_address() {
    let network = hop(
        FakeNetwork::new(),
        1,
        [
            router("10.0.0.2", 1),
            router("10.0.0.3", 2),
            router("10.0.0.2", 1),
        ],
    );
    let network = hop(
        network,
        2,
        [target_reply(5), target_reply(5), target_reply(5)],
    );

    let text = run_traceroute(&network).await.to_text();

    assert!(
        text.contains("\n1    10.0.0.2  -         1 ms     2 ms (10.0.0.3)  1 ms\n"),
        "{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_stop_that_comes_after_the_target_was_reached_does_not_turn_the_traceroute_into_a_stopped_one()
 {
    let network = hop(
        FakeNetwork::new(),
        1,
        [target_reply(5), target_reply(5), target_reply(5)],
    )
    .with_slow_reverse_name("8.8.8.8", ms(1500), "dns.google");
    let traceroute = request("8.8.8.8").validate().unwrap();
    let stop = CancellationToken::new();

    let (result, ()) = tokio::join!(
        traceroute::run(&network, &traceroute, &stop, |_| {}),
        async {
            tokio::time::sleep(ms(1000)).await;
            stop.cancel();
        }
    );

    assert_eq!(
        (result.ending.clone(), result.summary()),
        (RunEnding::Completed, "Reached 8.8.8.8 in 1 hop".to_string())
    );
}
