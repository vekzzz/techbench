//! Smoke tests: IP Info through the real network adapter, on this PC only. The public lookup
//! reaches an outside service, so its test is opt-in: `cargo test -- --ignored`.

use tokio_util::sync::CancellationToken;
use toolbox_core::ip_info::{self, IpInfoEvent};
use toolbox_core::network::{AdapterKind, SystemNetwork};

#[tokio::test]
async fn this_pc_has_at_least_one_named_adapter_besides_loopback() {
    // Stopping as soon as the adapters arrive keeps this test off the internet: the public
    // lookup is never started once the Run is stopped.
    let stop = CancellationToken::new();
    let result = ip_info::run(&SystemNetwork, &stop, |event| {
        if matches!(event, IpInfoEvent::Adapters { .. }) {
            stop.cancel();
        }
    })
    .await;

    assert!(
        result.public.is_none()
            && result
                .adapters
                .iter()
                .any(|adapter| adapter.kind != AdapterKind::Loopback && !adapter.name.is_empty()),
        "{result:#?}"
    );
}

#[tokio::test]
#[ignore = "calls an outside service; run with --ignored"]
async fn the_public_lookup_finds_this_pcs_public_address() {
    let result = ip_info::run(&SystemNetwork, &CancellationToken::new(), |_| {}).await;

    assert!(matches!(result.public, Some(Ok(_))), "{:#?}", result.public);
}
