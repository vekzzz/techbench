//! Smoke tests: LAN Scan through the real network adapter. They stay on this PC: loopback and
//! this PC's own address.

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use toolbox_core::lan_scan::{self, Device, LanScanRequest, LanScanResult};
use toolbox_core::network::{Network, SystemNetwork};
use toolbox_core::run::RunEnding;

async fn scan(range: &str) -> LanScanResult {
    let request = LanScanRequest {
        range: range.to_string(),
        confirmed: false,
    };
    lan_scan::run(
        &SystemNetwork,
        &request.validate().unwrap(),
        &CancellationToken::new(),
        |_| {},
    )
    .await
}

/// This PC's address on the network its internet traffic leaves by, if it has one.
fn own_address() -> Option<Ipv4Addr> {
    match SystemNetwork.source_address(Ipv4Addr::new(8, 8, 8, 8).into()) {
        Some(IpAddr::V4(address)) => Some(address),
        _ => None,
    }
}

#[tokio::test]
async fn scanning_this_pc_by_its_loopback_address_finds_it() {
    let result = scan("127.0.0.1").await;

    assert!(
        result.ending == RunEnding::Completed
            && matches!(
                result.devices.as_slice(),
                [Device {
                    response_time: Some(_),
                    ..
                }]
            ),
        "{}",
        result.to_text()
    );
}

#[tokio::test]
async fn scanning_this_pcs_own_address_finds_it_with_its_mac() {
    let Some(address) = own_address() else {
        return; // No network at all: nothing to find.
    };

    let result = scan(&address.to_string()).await;

    assert!(
        matches!(result.devices.as_slice(), [device] if device.address == address && device.on_own_subnet && device.mac.is_some()),
        "{}",
        result.to_text()
    );
}

#[tokio::test]
async fn the_default_range_is_something_the_scan_accepts() {
    if let Some(range) = lan_scan::default_range(&SystemNetwork).await {
        let request = LanScanRequest {
            range,
            confirmed: false,
        };
        assert!(request.validate().is_ok(), "{request:?}");
    }
}

#[tokio::test]
async fn arp_for_an_address_nothing_holds_gives_up_within_the_timeout() {
    let Some(address) = own_address() else {
        return;
    };
    // The last host of a /24 beside this PC is very probably unused; either way the call returns.
    let neighbour = Ipv4Addr::new(
        address.octets()[0],
        address.octets()[1],
        address.octets()[2],
        253,
    );
    let timeout = Duration::from_millis(500);
    let started = std::time::Instant::now();

    let _ = SystemNetwork.arp_resolve(neighbour, timeout).await;

    assert!(started.elapsed() < Duration::from_secs(2));
}
