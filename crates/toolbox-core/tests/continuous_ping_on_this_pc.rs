//! Smoke test: Continuous Ping through the real network adapter, against this PC only.

use std::time::Duration;

use tokio_util::sync::CancellationToken;
use toolbox_core::continuous_ping::{self, ContinuousPingRequest, ContinuousPingResult};
use toolbox_core::network::SystemNetwork;
use toolbox_core::run::{Progress, RunEnding};

#[tokio::test]
async fn pinging_this_pc_keeps_going_until_stopped_and_can_be_read_while_running() {
    let ping = ContinuousPingRequest {
        target: "127.0.0.1".to_string(),
        ..Default::default()
    }
    .validate()
    .unwrap();
    let progress = Progress::default();
    let stop = CancellationToken::new();

    let (result, text_while_running) = tokio::join!(
        continuous_ping::run_with_progress(&SystemNetwork, &ping, &stop, &progress, |_| {}),
        async {
            tokio::time::sleep(Duration::from_millis(2500)).await;
            let text = progress.read(ContinuousPingResult::to_text);
            stop.cancel();
            text
        }
    );

    let text = text_while_running.expect("the Run had opened by then");
    assert!(
        result.ending == RunEnding::Stopped
            && result.statistics.sent >= 2
            && result.statistics.received == result.statistics.sent
            && text.contains("Running for"),
        "{result:#?}\n{text}"
    );
}
