//! Smoke tests: Port Check through the real network adapter (seam S1 over the real S2), against this PC only.

use std::net::{IpAddr, Ipv4Addr, TcpListener};

use tokio_util::sync::CancellationToken;
use toolbox_core::network::SystemNetwork;
use toolbox_core::port_check::{self, PortCheckRequest, PortCheckResult, PortOutcome};
use toolbox_core::run::RunEnding;
use toolbox_core::target::AddressFamily;

async fn check(target: &str, port: u16) -> PortCheckResult {
    let request = PortCheckRequest {
        target: target.to_string(),
        port: port.to_string(),
        family: AddressFamily::Auto,
    };
    port_check::run(
        &SystemNetwork,
        &request.validate().unwrap(),
        &CancellationToken::new(),
        |_| {},
    )
    .await
}

#[tokio::test]
async fn a_port_with_a_listener_on_this_pc_is_open_and_localhost_resolves_to_ipv4() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();

    let result = check("localhost", port).await;

    assert!(
        matches!(result.ports[..], [report] if matches!(report.outcome, PortOutcome::Open { .. }))
            && result.header.resolved == Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        "{result:#?}"
    );
}

#[tokio::test]
async fn a_port_with_nothing_listening_on_this_pc_is_closed() {
    let unused_port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();

    let result = check("127.0.0.1", unused_port).await;

    assert!(
        matches!(result.ports[..], [report] if report.outcome == PortOutcome::Closed),
        "{result:#?}"
    );
}

#[tokio::test]
async fn a_hostname_that_cannot_exist_fails_to_resolve_with_a_plain_reason() {
    // RFC 2606 reserves .invalid, so no resolver can answer it.
    let result = check("nosuchhost.invalid", 445).await;

    let RunEnding::ResolutionFailed { reason } = &result.ending else {
        panic!("{result:#?}")
    };
    assert!(!reason.contains("os error"), "{reason}");
}
