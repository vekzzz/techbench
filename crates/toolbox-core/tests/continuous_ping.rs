//! Continuous Ping, exercised through the toolbox core's public interface (seam S1) over a
//! scripted network (seam S2) and tokio's paused clock.

mod common;

use common::{EchoScript, FakeNetwork, ms};
use tokio_util::sync::CancellationToken;
use toolbox_core::continuous_ping::{self, ContinuousPingRequest, ContinuousPingResult};
use toolbox_core::network::EchoOutcome;
use toolbox_core::run::Progress;

fn reply(millis: u64) -> EchoScript {
    EchoScript::Answer(EchoOutcome::Reply {
        round_trip: ms(millis),
        ttl: Some(128),
    })
}

fn request(target: &str) -> ContinuousPingRequest {
    ContinuousPingRequest {
        target: target.to_string(),
        ..ContinuousPingRequest::default()
    }
}

/// Runs a Continuous Ping and stops it after `stop_after_ms` of (paused) time.
async fn run_for(network: &FakeNetwork, target: &str, stop_after_ms: u64) -> ContinuousPingResult {
    let ping = request(target).validate().expect("request should be valid");
    let stop = CancellationToken::new();
    let (result, ()) = tokio::join!(continuous_ping::run(network, &ping, &stop, |_| {}), async {
        tokio::time::sleep(ms(stop_after_ms)).await;
        stop.cancel();
    });
    result
}

#[tokio::test(start_paused = true)]
async fn it_keeps_pinging_until_stopped_and_sums_up_loss_round_trip_and_jitter() {
    let network = FakeNetwork::new().with_echoes(
        "10.0.0.5",
        [
            reply(10),
            reply(14),
            EchoScript::Silent,
            reply(12),
            reply(10),
            EchoScript::Hang,
        ],
    );

    let result = run_for(&network, "10.0.0.5", 6500).await;

    assert_eq!(
        result.summary(),
        "Stopped after 6s: 5 sent, 4 received, 20% loss; round trip min 10 ms, avg 11 ms, max 14 ms; jitter 2 ms"
    );
}

#[tokio::test(start_paused = true)]
async fn consecutive_lost_echoes_are_grouped_with_when_the_loss_began() {
    let network = FakeNetwork::new().with_echoes(
        "10.0.0.5",
        [
            reply(3),
            EchoScript::Silent,
            EchoScript::Silent,
            reply(3),
            EchoScript::Answer(EchoOutcome::Unreachable { from: None }),
            reply(3),
            EchoScript::Hang,
        ],
    );

    let result = run_for(&network, "10.0.0.5", 8500).await;

    assert!(
        result.to_text().contains(
            "\
LOST   STARTED   ECHOES
#2-#3  14:05:10  2
#5     14:05:15  1
"
        ),
        "{}",
        result.to_text()
    );
}

#[tokio::test(start_paused = true)]
async fn copy_as_text_gives_the_header_any_lost_echoes_and_the_running_totals() {
    let network = FakeNetwork::new().with_echoes(
        "10.0.0.5",
        [reply(4), EchoScript::Silent, reply(6), EchoScript::Hang],
    );

    let result = run_for(&network, "10.0.0.5", 4500).await;

    assert_eq!(
        result.to_text(),
        "\
Continuous Ping
Target:    10.0.0.5
Resolved:  10.0.0.5
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)
Echoes:    every 1 s, 32 bytes, 2000 ms timeout

LOST  STARTED   ECHOES
#2    14:05:10  1

Stopped after 4s: 3 sent, 2 received, 33% loss; round trip min 4 ms, avg 5 ms, max 6 ms; jitter 2 ms
"
    );
}

#[tokio::test(start_paused = true)]
async fn with_no_lost_echoes_the_text_says_so() {
    let network =
        FakeNetwork::new().with_echoes("10.0.0.5", [reply(4), reply(4), EchoScript::Hang]);

    let text = run_for(&network, "10.0.0.5", 1500).await.to_text();

    assert!(text.contains("\nNo echoes were lost.\n"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn the_summary_of_a_continuous_ping_still_going_says_how_long_it_has_run() {
    let network = FakeNetwork::new()
        .with_echoes("10.0.0.5", [reply(4), reply(8), reply(6), EchoScript::Hang]);
    let ping = request("10.0.0.5").validate().unwrap();
    let progress = Progress::default();
    let stop = CancellationToken::new();

    let (_, summary_while_running) = tokio::join!(
        continuous_ping::run_with_progress(&network, &ping, &stop, &progress, |_| {}),
        async {
            tokio::time::sleep(ms(2500)).await;
            let summary = progress.read(ContinuousPingResult::summary);
            stop.cancel();
            summary
        }
    );

    assert_eq!(
        summary_while_running.as_deref(),
        Some(
            "Running for 2s: 3 sent, 3 received, 0% loss; round trip min 4 ms, avg 6 ms, max 8 ms; jitter 3 ms"
        )
    );
}

#[tokio::test(start_paused = true)]
async fn three_runs_side_by_side_keep_their_own_totals() {
    let network = FakeNetwork::new()
        .with_echoes("10.0.0.1", [reply(1), reply(1), EchoScript::Hang])
        .with_echoes("8.8.8.8", [reply(20), EchoScript::Silent, EchoScript::Hang])
        .with_echoes("1.1.1.1", [EchoScript::Silent, EchoScript::Hang]);
    let (gateway, google, cloudflare) = (
        request("10.0.0.1").validate().unwrap(),
        request("8.8.8.8").validate().unwrap(),
        request("1.1.1.1").validate().unwrap(),
    );
    let stop = CancellationToken::new();

    let (gateway, google, cloudflare, ()) = tokio::join!(
        continuous_ping::run(&network, &gateway, &stop, |_| {}),
        continuous_ping::run(&network, &google, &stop, |_| {}),
        continuous_ping::run(&network, &cloudflare, &stop, |_| {}),
        async {
            tokio::time::sleep(ms(3500)).await;
            stop.cancel();
        }
    );

    assert_eq!(
        [gateway.summary(), google.summary(), cloudflare.summary()],
        [
            "Stopped after 3s: 2 sent, 2 received, 0% loss; round trip min 1 ms, avg 1 ms, max 1 ms; jitter <1 ms",
            "Stopped after 3s: 2 sent, 1 received, 50% loss; round trip min 20 ms, avg 20 ms, max 20 ms",
            "Stopped after 3s: 1 sent, 0 received, 100% loss",
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_hostname_that_does_not_resolve_ends_the_run_as_a_resolution_failure() {
    let network = FakeNetwork::new()
        .with_unresolvable_name("nosuchhost.corp.local", "No such host is known.");

    let result = run_for(&network, "nosuchhost.corp.local", 500).await;

    assert_eq!(
        result.summary(),
        "Could not resolve nosuchhost.corp.local: No such host is known."
    );
}

#[tokio::test(start_paused = true)]
async fn a_flapping_link_keeps_only_the_latest_lost_runs_and_counts_the_older_ones() {
    // Alternating lost and answered echoes: every second echo starts a new lost run.
    let echoes = (0..600).flat_map(|_| [EchoScript::Silent, reply(3)]);
    let network = FakeNetwork::new().with_echoes("10.0.0.5", echoes.chain([EchoScript::Hang]));

    let result = run_for(&network, "10.0.0.5", 1_199_500).await;

    assert_eq!(
        (
            result.lost.len(),
            result.older_lost_runs,
            result.statistics.sent - result.statistics.received
        ),
        (200, 200, 400)
    );
}

#[tokio::test(start_paused = true)]
async fn the_text_says_how_many_older_lost_runs_were_left_out() {
    let echoes = (0..300).flat_map(|_| [EchoScript::Silent, reply(3)]);
    let network = FakeNetwork::new().with_echoes("10.0.0.5", echoes.chain([EchoScript::Hang]));

    let text = run_for(&network, "10.0.0.5", 899_500).await.to_text();

    assert!(
        text.contains("\n... and 100 earlier lost runs not listed\n"),
        "{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_loss_that_rounds_to_zero_percent_still_shows_as_under_one_percent() {
    let echoes = std::iter::once(EchoScript::Silent).chain((0..999).map(|_| reply(3)));
    let network = FakeNetwork::new().with_echoes("10.0.0.5", echoes.chain([EchoScript::Hang]));

    let result = run_for(&network, "10.0.0.5", 1_001_500).await;

    assert!(
        result
            .summary()
            .contains("1000 sent, 999 received, <1% loss"),
        "{}",
        result.summary()
    );
}
