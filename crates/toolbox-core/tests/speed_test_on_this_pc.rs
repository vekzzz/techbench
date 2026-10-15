//! Smoke tests: the Speed Test operations of the real network adapter, against the real
//! Cloudflare speed endpoints. They reach an outside service and move real data, so they are
//! opt-in (`cargo test -- --ignored`) and keep their transfers small.

use std::sync::atomic::{AtomicU64, Ordering};

use toolbox_core::network::{Network, SystemNetwork};
use toolbox_core::target::AddressFamily;

#[tokio::test]
#[ignore = "calls an outside service; run with --ignored"]
async fn a_probe_reaches_cloudflare_and_names_the_location_that_answered() {
    let network = SystemNetwork;

    // The second probe reuses the first one's connection, so it is the quicker of the two.
    let first = network.speed_probe(AddressFamily::Auto).await;
    let second = network.speed_probe(AddressFamily::Auto).await;

    let probe = second.unwrap_or_else(|failure| panic!("{failure:?} (first: {first:?})"));
    println!("{probe:?}");
    let location = &probe.server.location;
    assert!(
        location.len() == 3 && location.chars().all(|letter| letter.is_ascii_uppercase()),
        "expected a three-letter location code, got {location:?}"
    );
    assert_eq!(probe.server.provider, "Cloudflare");
    assert!(probe.round_trip > probe.server_time, "{probe:?}");
}

#[tokio::test]
#[ignore = "calls an outside service; run with --ignored"]
async fn a_one_megabyte_download_delivers_exactly_one_megabyte() {
    let received = AtomicU64::new(0);

    let result = SystemNetwork
        .speed_download(AddressFamily::Auto, 1_000_000, &|piece| {
            received.fetch_add(piece, Ordering::Relaxed);
        })
        .await;

    assert_eq!(result, Ok(()));
    assert_eq!(received.load(Ordering::Relaxed), 1_000_000);
}

#[tokio::test]
async fn windows_can_say_what_this_connection_costs() {
    // Any answer will do: the call must work unelevated and not crash, even with no connection.
    let cost = SystemNetwork.connection_cost().await;

    println!("{cost:?}");
}

#[tokio::test]
#[ignore = "calls an outside service; run with --ignored"]
async fn a_one_megabyte_upload_hands_over_exactly_one_megabyte() {
    let sent = AtomicU64::new(0);

    let result = SystemNetwork
        .speed_upload(AddressFamily::Auto, 1_000_000, &|piece| {
            sent.fetch_add(piece, Ordering::Relaxed);
        })
        .await;

    assert_eq!(result, Ok(()));
    assert_eq!(sent.load(Ordering::Relaxed), 1_000_000);
}
