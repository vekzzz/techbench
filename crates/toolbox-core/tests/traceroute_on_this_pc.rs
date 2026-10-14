//! Smoke tests: Traceroute through the real network adapter. Loopback stays on this PC; the test
//! to a public address reaches the internet, so it is opt-in: `cargo test -- --ignored`.

use tokio_util::sync::CancellationToken;
use toolbox_core::network::SystemNetwork;
use toolbox_core::run::RunEnding;
use toolbox_core::traceroute::{self, Probe, TracerouteRequest, TracerouteResult};

async fn traceroute_to(target: &str) -> TracerouteResult {
    let request = TracerouteRequest {
        target: target.to_string(),
        ..Default::default()
    };
    traceroute::run(
        &SystemNetwork,
        &request.validate().unwrap(),
        &CancellationToken::new(),
        |_| {},
    )
    .await
}

#[tokio::test]
async fn tracing_this_pc_finds_the_target_at_the_first_hop() {
    let result = traceroute_to("127.0.0.1").await;

    assert!(
        result.hops.len() == 1
            && result.hops[0].reached_target
            && result.ending == RunEnding::Completed,
        "{result:#?}"
    );
}

#[tokio::test]
async fn tracing_this_pc_over_ipv6_also_finds_the_target_at_the_first_hop() {
    let result = traceroute_to("::1").await;

    assert!(
        result.hops.len() == 1 && result.hops[0].reached_target,
        "{result:#?}"
    );
}

#[tokio::test]
#[ignore = "reaches the internet; run with --ignored"]
async fn tracing_a_public_address_names_the_routers_on_the_way() {
    let result = traceroute_to("8.8.8.8").await;

    let hops_with_an_address = result
        .hops
        .iter()
        .filter(|hop| hop.address.is_some())
        .count();
    let answered = result
        .hops
        .iter()
        .flat_map(|hop| &hop.probes)
        .any(|probe| matches!(probe, Probe::Answered { .. }));
    assert!(
        result.hops.len() >= 2 && hops_with_an_address >= 2 && answered,
        "{}",
        result.to_text()
    );
}
