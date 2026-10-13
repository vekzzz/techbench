//! Smoke tests: Ping through the real network adapter (seam S1 over the real S2), against this PC only.

use tokio_util::sync::CancellationToken;
use toolbox_core::network::{EchoOutcome, SystemNetwork};
use toolbox_core::ping::{self, PingRequest, PingResult};

async fn ping(target: &str, count: u32) -> PingResult {
    let request = PingRequest {
        target: target.to_string(),
        count: count.to_string(),
        ..PingRequest::default()
    };
    ping::run(
        &SystemNetwork,
        &request.validate().unwrap(),
        &CancellationToken::new(),
        |_| {},
    )
    .await
}

fn all_replied(result: &PingResult) -> bool {
    !result.echoes.is_empty()
        && result
            .echoes
            .iter()
            .all(|echo| matches!(echo.outcome, EchoOutcome::Reply { .. }))
}

#[tokio::test]
async fn pinging_this_pc_over_ipv4_gets_replies_with_a_ttl() {
    let result = ping("127.0.0.1", 2).await;

    assert!(
        all_replied(&result)
            && result.echoes.len() == 2
            && result
                .echoes
                .iter()
                .all(|echo| matches!(echo.outcome, EchoOutcome::Reply { ttl: Some(_), .. })),
        "{result:#?}"
    );
}

#[tokio::test]
async fn pinging_this_pc_over_ipv6_gets_a_reply() {
    let result = ping("::1", 1).await;

    assert!(all_replied(&result), "{result:#?}");
}
