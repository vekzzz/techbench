//! LAN Scan, exercised through the toolbox core's public interface (seam S1) over a scripted network (seam S2).

mod common;

use std::net::Ipv4Addr;

use common::{EchoScript, FakeNetwork, ip, ms};
use tokio_util::sync::CancellationToken;
use toolbox_core::lan_scan::{
    self, Device, LanScanEvent, LanScanRequest, LanScanResult, RangeCheck,
};
use toolbox_core::network::{Adapter, AdapterAddress, AdapterKind, EchoOutcome};
use toolbox_core::run::{Progress, RunEnding};

const MAC_A: [u8; 6] = [0xAA, 0xBB, 0xCC, 0x01, 0x02, 0x03];
const MAC_B: [u8; 6] = [0xAA, 0xBB, 0xCC, 0x01, 0x02, 0x04];
/// Registered to the Raspberry Pi Foundation.
const MAC_PI: [u8; 6] = [0xB8, 0x27, 0xEB, 0x01, 0x02, 0x03];
/// Registered to Google.
const MAC_GOOGLE: [u8; 6] = [0x00, 0x1A, 0x11, 0x0A, 0x0B, 0x0C];

fn request(range: &str) -> LanScanRequest {
    LanScanRequest {
        range: range.to_string(),
        confirmed: false,
    }
}

fn confirmed(range: &str) -> LanScanRequest {
    LanScanRequest {
        confirmed: true,
        ..request(range)
    }
}

fn v4(text: &str) -> Ipv4Addr {
    text.parse().unwrap()
}

/// The addresses a valid range will probe, as text.
fn probed(range: &str) -> Vec<String> {
    confirmed(range)
        .validate()
        .expect("range should be valid")
        .addresses()
        .map(|address| address.to_string())
        .collect()
}

fn rejection(range: &str) -> String {
    request(range)
        .validate()
        .expect_err("range should be rejected")
        .message
}

/// An active Ethernet adapter holding `address/prefix`.
fn adapter(name: &str, address: &str, prefix: u8) -> Adapter {
    Adapter {
        name: name.to_string(),
        description: format!("{name} controller"),
        kind: AdapterKind::Ethernet,
        up: true,
        mac: None,
        addresses: vec![AdapterAddress {
            address: ip(address),
            prefix,
        }],
        gateways: vec![ip("10.0.0.1")],
        dns_servers: vec![],
        dhcp: None,
        ipv4_metric: 25,
        ipv6_metric: 25,
    }
}

/// A PC on 10.0.0.0/24 whose silent addresses time out and whose ARP answers are scripted.
fn pc_on_lan() -> FakeNetwork {
    FakeNetwork::new()
        .with_adapters(vec![adapter("Ethernet", "10.0.0.23", 24)])
        .with_silent_echoes_unless_scripted()
}

fn reply(address: &str, millis: u64) -> impl FnOnce(FakeNetwork) -> FakeNetwork {
    let address = address.to_string();
    move |network| {
        network.with_echoes(
            &address,
            [EchoScript::Answer(EchoOutcome::Reply {
                round_trip: ms(millis),
                ttl: Some(128),
            })],
        )
    }
}

async fn scan(network: &FakeNetwork, range: &str) -> LanScanResult {
    let scan = confirmed(range).validate().expect("range should be valid");
    lan_scan::run(network, &scan, &CancellationToken::new(), |_| {}).await
}

/// Starts the scan, and stops it once `after` has run.
async fn scan_then_stop(
    network: &FakeNetwork,
    range: &str,
    after: std::time::Duration,
) -> LanScanResult {
    let scan = confirmed(range).validate().expect("range should be valid");
    let stop = CancellationToken::new();
    let (result, ()) = tokio::join!(lan_scan::run(network, &scan, &stop, |_| {}), async {
        tokio::time::sleep(after).await;
        stop.cancel();
    });
    result
}

// --- Range parsing ---------------------------------------------------------------------------

#[test]
fn a_cidr_range_skips_its_network_and_broadcast_addresses() {
    let addresses = probed("192.168.1.0/24");

    assert_eq!(
        (addresses.len(), addresses.first(), addresses.last()),
        (
            254,
            Some(&"192.168.1.1".to_string()),
            Some(&"192.168.1.254".to_string())
        )
    );
}

#[test]
fn a_cidr_range_with_host_bits_set_means_the_whole_network() {
    assert_eq!(probed("192.168.1.77/30"), ["192.168.1.77", "192.168.1.78"]);
}

#[test]
fn a_30_has_two_usable_addresses_and_a_31_or_32_keeps_every_address() {
    assert_eq!(
        (
            probed("10.0.0.4/30"),
            probed("10.0.0.4/31"),
            probed("10.0.0.4/32")
        ),
        (
            vec!["10.0.0.5".to_string(), "10.0.0.6".to_string()],
            vec!["10.0.0.4".to_string(), "10.0.0.5".to_string()],
            vec!["10.0.0.4".to_string()],
        )
    );
}

#[test]
fn a_start_end_range_is_inclusive_and_probes_exactly_the_addresses_named() {
    assert_eq!(
        probed("10.0.0.254 - 10.0.1.1"),
        ["10.0.0.254", "10.0.0.255", "10.0.1.0", "10.0.1.1"]
    );
}

#[test]
fn a_single_address_is_a_range_of_one() {
    assert_eq!(probed("10.0.0.7"), ["10.0.0.7"]);
}

#[test]
fn surrounding_spaces_are_ignored() {
    assert_eq!(probed("  10.0.0.7 -  10.0.0.8 "), ["10.0.0.7", "10.0.0.8"]);
}

#[test]
fn malformed_ranges_are_rejected_before_the_scan_starts() {
    for range in [
        "",
        "   ",
        "hello",
        "10.0.0.0/33",
        "10.0.0.0/",
        "10.0.0/24",
        "10.0.0.300",
        "10.0.0.1-",
        "-10.0.0.9",
        "10.0.0.1-10.0.0.2-10.0.0.3",
        "10.0.0.1/24-10.0.0.9",
    ] {
        assert!(
            request(range).validate().is_err(),
            "{range:?} should be rejected"
        );
    }
}

#[test]
fn a_backwards_start_end_range_is_rejected_with_a_hint() {
    let message = rejection("10.0.0.9-10.0.0.2");

    assert!(message.contains("10.0.0.2-10.0.0.9"), "{message}");
}

#[test]
fn an_empty_range_asks_for_one() {
    assert_eq!(
        rejection(""),
        "enter the range to scan, such as 192.168.1.0/24 or 192.168.1.10-192.168.1.50"
    );
}

#[test]
fn ipv6_ranges_are_rejected_because_lan_scan_is_ipv4_only() {
    for range in ["fd00::/64", "fd00::1-fd00::9", "::1", "[fd00::5]"] {
        let message = rejection(range);

        assert!(message.contains("IPv4 only"), "{range:?}: {message}");
    }
}

// --- Size policy -----------------------------------------------------------------------------

#[test]
fn up_to_1024_addresses_run_without_confirmation() {
    let check = request("10.0.0.0/22").check_range();

    assert_eq!(
        check,
        Ok(RangeCheck {
            addresses: 1022,
            needs_confirmation: false
        })
    );
    assert!(request("10.0.0.0/22").validate().is_ok());
}

#[test]
fn a_start_end_range_of_exactly_1024_addresses_runs_freely() {
    assert_eq!(
        request("10.0.0.0-10.0.3.255").check_range(),
        Ok(RangeCheck {
            addresses: 1024,
            needs_confirmation: false
        })
    );
}

#[test]
fn a_range_of_1025_addresses_needs_confirmation() {
    let range = "10.0.0.0-10.0.4.0";

    let check = request(range).check_range();
    let unconfirmed = request(range).validate().unwrap_err();

    assert_eq!(
        check,
        Ok(RangeCheck {
            addresses: 1025,
            needs_confirmation: true
        })
    );
    assert!(
        unconfirmed.message.contains("confirm"),
        "{}",
        unconfirmed.message
    );
    assert!(confirmed(range).validate().is_ok());
}

#[test]
fn a_slash_16_needs_confirmation_and_then_runs() {
    let check = request("10.1.0.0/16").check_range().unwrap();
    let scan = confirmed("10.1.0.0/16").validate().unwrap();

    assert_eq!(
        (
            check.addresses,
            check.needs_confirmation,
            scan.addresses().count()
        ),
        (65534, true, 65534)
    );
}

#[test]
fn anything_larger_than_a_slash_16_is_refused_even_when_confirmed() {
    for range in ["10.0.0.0/15", "10.0.0.0-10.1.0.0", "0.0.0.0/0"] {
        let message = confirmed(range)
            .validate()
            .expect_err("range should be refused")
            .message;

        assert!(message.contains("65,536"), "{range:?}: {message}");
        assert!(request(range).check_range().is_err(), "{range:?}");
    }
}

// --- The default range -----------------------------------------------------------------------

#[tokio::test]
async fn the_default_range_is_the_ipv4_network_of_the_default_route_adapter() {
    let network = FakeNetwork::new().with_adapters(vec![
        adapter("Ethernet", "192.168.1.77", 24),
        Adapter {
            gateways: vec![],
            ..adapter("vEthernet (WSL)", "172.20.0.1", 20)
        },
    ]);

    assert_eq!(
        lan_scan::default_range(&network).await.as_deref(),
        Some("192.168.1.0/24")
    );
}

#[tokio::test]
async fn the_default_range_follows_the_adapter_prefix() {
    let network = FakeNetwork::new().with_adapters(vec![adapter("Ethernet", "10.20.30.40", 22)]);

    assert_eq!(
        lan_scan::default_range(&network).await.as_deref(),
        Some("10.20.28.0/22")
    );
}

#[tokio::test]
async fn there_is_no_default_range_when_the_default_route_adapter_has_no_ipv4_address() {
    let ipv6_only = Adapter {
        addresses: vec![AdapterAddress {
            address: ip("fd00::23"),
            prefix: 64,
        }],
        gateways: vec![ip("fd00::1")],
        ..adapter("Ethernet", "10.0.0.23", 24)
    };
    let network = FakeNetwork::new()
        .with_adapters(vec![ipv6_only])
        .with_route_source(Some("fd00::23"));

    assert_eq!(lan_scan::default_range(&network).await, None);
}

#[tokio::test]
async fn there_is_no_default_range_without_a_default_route() {
    let network = FakeNetwork::new().with_route_source(None);

    assert_eq!(lan_scan::default_range(&network).await, None);
}

// --- Finding Devices -------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_device_that_answers_the_ping_is_found_with_its_response_time() {
    let network = reply("10.0.0.5", 3)(pc_on_lan());

    let result = scan(&network, "10.0.0.4-10.0.0.6").await;

    assert_eq!(
        result.devices,
        vec![Device {
            address: v4("10.0.0.5"),
            mac: None,
            response_time: Some(ms(3)),
            no_ping_reply: false,
            on_own_subnet: true,
            hostname: None,
            vendor: None,
        }]
    );
}

#[tokio::test(start_paused = true)]
async fn on_the_pcs_own_subnet_a_device_that_answers_arp_but_not_ping_is_found_and_flagged() {
    let network = pc_on_lan().with_arp("10.0.0.9", MAC_A);

    let result = scan(&network, "10.0.0.8-10.0.0.10").await;

    assert_eq!(
        result.devices,
        vec![Device {
            address: v4("10.0.0.9"),
            mac: Some(MAC_A),
            response_time: None,
            no_ping_reply: true,
            on_own_subnet: true,
            hostname: None,
            vendor: None,
        }]
    );
}

#[tokio::test(start_paused = true)]
async fn on_the_pcs_own_subnet_a_device_that_answers_both_shows_its_mac_and_response_time_unflagged()
 {
    let network = reply("10.0.0.5", 2)(pc_on_lan()).with_arp("10.0.0.5", MAC_B);

    let result = scan(&network, "10.0.0.5").await;

    assert_eq!(
        result.devices,
        vec![Device {
            address: v4("10.0.0.5"),
            mac: Some(MAC_B),
            response_time: Some(ms(2)),
            no_ping_reply: false,
            on_own_subnet: true,
            hostname: None,
            vendor: None,
        }]
    );
}

#[tokio::test(start_paused = true)]
async fn only_addresses_on_the_pcs_own_subnet_are_looked_up_by_arp() {
    // 10.0.0.254 is on 10.0.0.0/24; 10.0.1.0 is beyond a router, so its ARP answer never counts.
    let network = pc_on_lan()
        .with_arp("10.0.0.254", MAC_A)
        .with_arp("10.0.1.0", MAC_B);

    let result = scan(&network, "10.0.0.254-10.0.1.1").await;

    let found: Vec<_> = result.devices.iter().map(|d| d.address).collect();
    let beyond = |asked: Vec<Ipv4Addr>| asked.iter().any(|a| a.octets()[2] == 1);
    assert_eq!(found, [v4("10.0.0.254")]);
    assert!(!beyond(network.arp_requests()) && !beyond(network.cache_reads()));
}

#[tokio::test(start_paused = true)]
async fn off_the_pcs_own_subnet_only_the_ping_can_find_a_device_and_it_has_no_mac() {
    let network = reply("172.16.0.5", 12)(pc_on_lan()).with_arp("172.16.0.9", MAC_A);

    let result = scan(&network, "172.16.0.4-172.16.0.10").await;

    assert_eq!(
        result.devices,
        vec![Device {
            address: v4("172.16.0.5"),
            mac: None,
            response_time: Some(ms(12)),
            no_ping_reply: false,
            on_own_subnet: false,
            hostname: None,
            vendor: None,
        }]
    );
    assert_eq!(network.arp_requests(), Vec::<Ipv4Addr>::new());
}

#[tokio::test(start_paused = true)]
async fn a_range_that_straddles_the_pcs_subnet_probes_each_address_by_where_it_is() {
    let network = reply("10.0.1.5", 4)(pc_on_lan()).with_arp("10.0.0.255", MAC_A);

    let result = scan(&network, "10.0.0.255-10.0.1.5").await;

    let found: Vec<_> = result
        .devices
        .iter()
        .map(|device| (device.address.to_string(), device.mac.is_some()))
        .collect();
    assert_eq!(
        found,
        [
            ("10.0.0.255".to_string(), true),
            ("10.0.1.5".to_string(), false)
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn every_adapter_that_is_up_has_its_own_subnet_but_a_tunnel_or_a_down_adapter_does_not() {
    let wifi = adapter("Wi-Fi", "192.168.50.7", 24);
    let down = Adapter {
        up: false,
        ..adapter("Ethernet 2", "172.30.0.2", 24)
    };
    let vpn = Adapter {
        kind: AdapterKind::Tunnel,
        ..adapter("VPN", "10.99.0.2", 24)
    };
    let network = FakeNetwork::new()
        .with_adapters(vec![adapter("Ethernet", "10.0.0.23", 24), wifi, down, vpn])
        .with_silent_echoes_unless_scripted()
        .with_arp("192.168.50.1", MAC_A)
        .with_arp("172.30.0.1", MAC_A)
        .with_arp("10.99.0.1", MAC_A);

    scan(&network, "192.168.50.1").await;
    scan(&network, "172.30.0.1").await;
    scan(&network, "10.99.0.1").await;

    assert_eq!(network.arp_requests(), vec![v4("192.168.50.1")]);
}

#[tokio::test(start_paused = true)]
async fn the_ping_uses_a_one_second_timeout_and_the_usual_echo() {
    let network = pc_on_lan();

    scan(&network, "10.0.0.5").await;

    let requests = network.echo_requests();
    assert_eq!(
        requests
            .iter()
            .map(|echo| (echo.address, echo.timeout, echo.ttl, echo.dont_fragment))
            .collect::<Vec<_>>(),
        [(ip("10.0.0.5"), ms(1000), None, false)]
    );
}

#[tokio::test(start_paused = true)]
async fn a_router_saying_the_destination_is_unreachable_does_not_make_a_device() {
    let network = pc_on_lan().with_echoes(
        "172.16.0.5",
        [EchoScript::Answer(EchoOutcome::Unreachable {
            from: Some(ip("10.0.0.1")),
        })],
    );

    let result = scan(&network, "172.16.0.5").await;

    assert_eq!(result.devices, vec![]);
}

#[tokio::test(start_paused = true)]
async fn devices_are_listed_in_numeric_ip_order_whatever_order_they_answered_in() {
    let network = pc_on_lan()
        .with_echoes("10.0.0.9", [EchoScript::Silent])
        .with_arp("10.0.0.9", MAC_A);
    let network = reply("10.0.0.20", 1)(reply("10.0.0.3", 1)(network));

    let result = scan(&network, "10.0.0.1-10.0.0.30").await;

    let order: Vec<_> = result
        .devices
        .iter()
        .map(|d| d.address.to_string())
        .collect();
    assert_eq!(order, ["10.0.0.3", "10.0.0.9", "10.0.0.20"]);
}

// --- Pace ------------------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_slash_24_of_silent_addresses_takes_four_timeouts_with_64_probed_at_a_time() {
    let network = pc_on_lan();
    let started = tokio::time::Instant::now();

    let result = scan(&network, "10.0.0.0/24").await;

    // 254 addresses at 64 a time: four waves of one second.
    assert_eq!(
        (result.addresses_probed, started.elapsed()),
        (254, ms(4000))
    );
}

#[tokio::test(start_paused = true)]
async fn no_more_than_64_addresses_are_probed_at_once() {
    let network = pc_on_lan();
    let started = tokio::time::Instant::now();

    scan(&network, "10.0.0.1-10.0.0.65").await;

    // 65 addresses: 64 in the first wave, the last one alone in a second.
    assert_eq!(started.elapsed(), ms(2000));
}

// --- Progress, stopping ----------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn devices_are_reported_as_they_are_found_followed_by_progress() {
    let network = reply("10.0.0.5", 3)(pc_on_lan());
    let scan = confirmed("10.0.0.5-10.0.0.6").validate().unwrap();
    let mut events = Vec::new();

    let result = lan_scan::run(&network, &scan, &CancellationToken::new(), |event| {
        events.push(event)
    })
    .await;

    assert_eq!(
        events,
        vec![
            LanScanEvent::Started { addresses: 2 },
            LanScanEvent::Device(result.devices[0].clone()),
            LanScanEvent::Progress {
                probed: 1,
                total: 2
            },
            LanScanEvent::Progress {
                probed: 2,
                total: 2
            },
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn stopping_keeps_the_devices_found_so_far_and_says_how_far_the_scan_got() {
    let network = reply("10.0.0.5", 3)(pc_on_lan()).with_echoes("10.0.0.6", [EchoScript::Hang]);

    let result = scan_then_stop(&network, "10.0.0.5-10.0.0.6", ms(500)).await;

    assert_eq!(
        (
            result.ending,
            result.devices.len(),
            result.addresses_probed,
            result.addresses_total
        ),
        (RunEnding::Stopped, 1, 1, 2)
    );
}

#[tokio::test(start_paused = true)]
async fn the_result_so_far_can_be_read_while_the_scan_is_running() {
    let network = reply("10.0.0.5", 3)(pc_on_lan()).with_echoes("10.0.0.6", [EchoScript::Hang]);
    let scan = confirmed("10.0.0.5-10.0.0.6").validate().unwrap();
    let progress = Progress::default();
    let stop = CancellationToken::new();

    let (_, text_while_running) = tokio::join!(
        lan_scan::run_with_progress(&network, &scan, &stop, &progress, |_| {}),
        async {
            tokio::time::sleep(ms(500)).await;
            let text = progress.read(LanScanResult::to_text);
            stop.cancel();
            text
        }
    );

    assert_eq!(
        text_while_running.as_deref(),
        Some(
            "LAN Scan
Range:     10.0.0.5-10.0.0.6 (2 addresses)
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

IP        HOSTNAME  MAC  VENDOR  RESPONSE TIME  NOTE
10.0.0.5  -         -    -       3 ms

Still scanning: 1 of 2 addresses probed, 1 Device found so far
"
        )
    );
}

// --- Copy as text ----------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn copy_as_text_gives_a_header_aligned_rows_and_a_summary() {
    let network = reply("10.0.0.1", 1)(pc_on_lan())
        .with_arp("10.0.0.1", MAC_A)
        .with_arp("10.0.0.9", MAC_B);
    let network = reply("10.0.0.30", 14)(network);

    let result = scan(&network, "10.0.0.0/24").await;

    assert_eq!(
        result.to_text(),
        "LAN Scan
Range:     10.0.0.0/24 (254 addresses)
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

IP         HOSTNAME  MAC                VENDOR  RESPONSE TIME  NOTE
10.0.0.1   -         AA-BB-CC-01-02-03  -       1 ms
10.0.0.9   -         AA-BB-CC-01-02-04  -       -              no ping reply
10.0.0.30  -         -                  -       14 ms

Found 3 Devices among 254 addresses; 1 with no ping reply
"
    );
}

#[tokio::test(start_paused = true)]
async fn copy_as_text_explains_a_blank_mac_for_devices_beyond_the_pcs_subnet() {
    let network = reply("172.16.0.5", 12)(pc_on_lan());

    let result = scan(&network, "172.16.0.5-172.16.0.6").await;

    assert_eq!(
        result.to_text(),
        "LAN Scan
Range:     172.16.0.5-172.16.0.6 (2 addresses)
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

IP          HOSTNAME  MAC  VENDOR  RESPONSE TIME  NOTE
172.16.0.5  -         -    -       12 ms

Found 1 Device among 2 addresses
MAC and vendor are blank for devices beyond this PC's own subnet: ARP does not cross routers.
"
    );
}

#[tokio::test(start_paused = true)]
async fn copy_as_text_of_a_scan_that_found_nothing_says_so() {
    let network = pc_on_lan();

    let result = scan(&network, "10.0.0.7-10.0.0.8").await;

    assert_eq!(
        result.to_text(),
        "LAN Scan
Range:     10.0.0.7-10.0.0.8 (2 addresses)
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

No Devices found among 2 addresses
"
    );
}

#[tokio::test(start_paused = true)]
async fn copy_as_text_of_a_stopped_scan_says_how_far_it_got() {
    let network = reply("10.0.0.5", 3)(pc_on_lan()).with_echoes("10.0.0.6", [EchoScript::Hang]);

    let result = scan_then_stop(&network, "10.0.0.5-10.0.0.6", ms(500)).await;

    assert_eq!(
        result.to_text(),
        "LAN Scan
Range:     10.0.0.5-10.0.0.6 (2 addresses)
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

IP        HOSTNAME  MAC  VENDOR  RESPONSE TIME  NOTE
10.0.0.5  -         -    -       3 ms

Stopped after probing 1 of 2 addresses: 1 Device found
"
    );
}

// --- Hostnames and vendors ---------------------------------------------------------------------

#[test]
fn the_vendor_comes_from_the_bundled_registry_by_mac_prefix() {
    assert_eq!(
        (
            toolbox_core::oui::vendor(MAC_PI),
            toolbox_core::oui::vendor(MAC_GOOGLE),
            toolbox_core::oui::vendor(MAC_A),
        ),
        (Some("Raspberry Pi Foundation"), Some("Google, Inc."), None)
    );
}

#[test]
fn only_the_first_three_bytes_of_the_mac_matter_to_the_vendor() {
    let other_device = [0xB8, 0x27, 0xEB, 0xFF, 0xFF, 0xFF];

    assert_eq!(
        toolbox_core::oui::vendor(other_device),
        toolbox_core::oui::vendor(MAC_PI)
    );
}

#[tokio::test(start_paused = true)]
async fn a_device_found_by_arp_shows_the_vendor_of_its_mac() {
    let network = pc_on_lan()
        .with_arp("10.0.0.9", MAC_PI)
        .with_arp("10.0.0.10", MAC_A);

    let result = scan(&network, "10.0.0.8-10.0.0.11").await;

    let vendors: Vec<_> = result
        .devices
        .iter()
        .map(|device| device.vendor.as_deref())
        .collect();
    assert_eq!(vendors, [Some("Raspberry Pi Foundation"), None]);
}

#[tokio::test(start_paused = true)]
async fn a_device_beyond_the_pcs_subnet_has_no_vendor() {
    let network = reply("172.16.0.5", 12)(pc_on_lan());

    let result = scan(&network, "172.16.0.5").await;

    assert_eq!(result.devices[0].vendor, None);
}

#[tokio::test(start_paused = true)]
async fn a_hostname_arrives_after_its_device_without_holding_up_the_scan() {
    let network = reply("10.0.0.5", 3)(pc_on_lan()).with_slow_reverse_name(
        "10.0.0.5",
        ms(1500),
        "fs01.corp.local",
    );
    let scan_of = confirmed("10.0.0.5-10.0.0.69").validate().unwrap();
    let mut events = Vec::new();
    let started = tokio::time::Instant::now();

    let result = lan_scan::run(&network, &scan_of, &CancellationToken::new(), |event| {
        events.push(event)
    })
    .await;

    // The 63 silent addresses take 1 s to probe; the 1.5 s name lookup runs beside them, so the
    // scan ends at 1.5 s, not at 1 s + 1.5 s.
    assert_eq!(started.elapsed(), ms(1500));
    assert_eq!(
        result.devices[0].hostname.as_deref(),
        Some("fs01.corp.local")
    );
    let device_at = events
        .iter()
        .position(|event| matches!(event, LanScanEvent::Device(_)))
        .unwrap();
    let hostname_at = events
        .iter()
        .position(|event| matches!(event, LanScanEvent::Hostname { .. }))
        .unwrap();
    assert!(device_at < hostname_at, "{events:?}");
    assert_eq!(
        events[hostname_at],
        LanScanEvent::Hostname {
            address: v4("10.0.0.5"),
            hostname: "fs01.corp.local".to_string()
        }
    );
}

#[tokio::test(start_paused = true)]
async fn the_device_event_carries_no_hostname_yet() {
    let network =
        reply("10.0.0.5", 3)(pc_on_lan()).with_reverse_name("10.0.0.5", "fs01.corp.local");
    let scan_of = confirmed("10.0.0.5").validate().unwrap();
    let mut events = Vec::new();

    lan_scan::run(&network, &scan_of, &CancellationToken::new(), |event| {
        events.push(event)
    })
    .await;

    let announced = events.iter().find_map(|event| match event {
        LanScanEvent::Device(device) => Some(device.hostname.clone()),
        _ => None,
    });
    assert_eq!(announced, Some(None));
}

#[tokio::test(start_paused = true)]
async fn a_device_whose_address_has_no_name_stays_without_one() {
    let network = reply("10.0.0.5", 3)(pc_on_lan());

    let result = scan(&network, "10.0.0.5").await;

    assert_eq!(result.devices[0].hostname, None);
}

#[tokio::test(start_paused = true)]
async fn a_name_lookup_that_takes_more_than_two_seconds_is_abandoned() {
    let network = reply("10.0.0.5", 3)(pc_on_lan()).with_slow_reverse_name(
        "10.0.0.5",
        ms(5000),
        "fs01.corp.local",
    );
    let started = tokio::time::Instant::now();

    let result = scan(&network, "10.0.0.5").await;

    assert_eq!(
        (
            result.devices[0].hostname.clone(),
            result.ending,
            started.elapsed()
        ),
        (None, RunEnding::Completed, ms(2000))
    );
}

#[tokio::test(start_paused = true)]
async fn the_result_so_far_holds_a_hostname_once_it_has_arrived() {
    let network = reply("10.0.0.5", 3)(pc_on_lan())
        .with_reverse_name("10.0.0.5", "fs01.corp.local")
        .with_echoes("10.0.0.6", [EchoScript::Hang]);
    let scan_of = confirmed("10.0.0.5-10.0.0.6").validate().unwrap();
    let progress = Progress::default();
    let stop = CancellationToken::new();

    let (_, hostname) = tokio::join!(
        lan_scan::run_with_progress(&network, &scan_of, &stop, &progress, |_| {}),
        async {
            tokio::time::sleep(ms(500)).await;
            let hostname = progress.read(|result| result.devices[0].hostname.clone());
            stop.cancel();
            hostname
        }
    );

    assert_eq!(hostname, Some(Some("fs01.corp.local".to_string())));
}

#[tokio::test(start_paused = true)]
async fn stopping_while_names_are_still_being_looked_up_keeps_the_scan_completed() {
    let network = reply("10.0.0.5", 3)(pc_on_lan()).with_slow_reverse_name(
        "10.0.0.5",
        ms(1500),
        "fs01.corp.local",
    );
    let scan_of = confirmed("10.0.0.5").validate().unwrap();
    let stop = CancellationToken::new();

    let (result, ()) = tokio::join!(lan_scan::run(&network, &scan_of, &stop, |_| {}), async {
        tokio::time::sleep(ms(500)).await;
        stop.cancel();
    });

    // Every address had been probed; only the names were abandoned.
    assert_eq!(
        (result.ending, result.devices[0].hostname.clone()),
        (RunEnding::Completed, None)
    );
}

#[tokio::test(start_paused = true)]
async fn copy_as_text_includes_hostname_and_vendor() {
    let network = reply("10.0.0.1", 1)(pc_on_lan())
        .with_arp("10.0.0.1", MAC_PI)
        .with_reverse_name("10.0.0.1", "gateway.corp.local")
        .with_arp("10.0.0.9", MAC_GOOGLE);
    let network = reply("10.0.0.30", 14)(network);

    let result = scan(&network, "10.0.0.0/24").await;

    assert_eq!(
        result.to_text(),
        "LAN Scan
Range:     10.0.0.0/24 (254 addresses)
Time:      2026-10-13 14:05:09 +08:00
From:      TECH-PC (10.0.0.23)

IP         HOSTNAME            MAC                VENDOR                   RESPONSE TIME  NOTE
10.0.0.1   gateway.corp.local  B8-27-EB-01-02-03  Raspberry Pi Foundation  1 ms
10.0.0.9   -                   00-1A-11-0A-0B-0C  Google, Inc.             -              no ping reply
10.0.0.30  -                   -                  -                        14 ms

Found 3 Devices among 254 addresses; 1 with no ping reply
"
    );
}

#[tokio::test(start_paused = true)]
async fn a_device_that_answered_the_ping_but_not_arp_gets_its_mac_from_the_pcs_arp_cache() {
    // A slow Wi-Fi client misses the ARP request yet answers the ping, and Windows resolved its MAC
    // to send that ping: the MAC is in the neighbour cache.
    let network = reply("10.0.0.5", 280)(pc_on_lan()).with_cached_mac("10.0.0.5", MAC_PI);

    let result = scan(&network, "10.0.0.5").await;

    assert_eq!(
        (
            result.devices[0].mac,
            result.devices[0].vendor.as_deref(),
            network.arp_requests().len()
        ),
        (Some(MAC_PI), Some("Raspberry Pi Foundation"), 0)
    );
}

#[tokio::test(start_paused = true)]
async fn a_cache_entry_alone_does_not_make_a_device() {
    // The cache can remember a device that has since left: only a fresh ARP answer counts.
    let network = pc_on_lan().with_cached_mac("10.0.0.5", MAC_PI);

    let result = scan(&network, "10.0.0.5").await;

    assert_eq!(
        (result.devices, network.arp_requests()),
        (vec![], vec![v4("10.0.0.5")])
    );
}

#[tokio::test(start_paused = true)]
async fn no_arp_request_is_sent_alongside_the_ping() {
    // Windows' SendARP disturbs its own resolution of the same address, so a device that answers
    // ping when asked alone can go silent with an ARP request beside it.
    let network = reply("10.0.0.5", 3)(pc_on_lan()).with_arp("10.0.0.5", MAC_A);

    let result = scan(&network, "10.0.0.5").await;

    assert_eq!(
        (result.devices[0].mac, network.arp_requests()),
        (Some(MAC_A), Vec::<Ipv4Addr>::new())
    );
}

#[tokio::test(start_paused = true)]
async fn an_address_windows_could_not_resolve_for_the_ping_is_not_asked_by_arp_again() {
    // Sending the echo already made Windows ask ARP for it; nothing answered.
    let network = pc_on_lan();

    let result = scan(&network, "10.0.0.5").await;

    assert_eq!(
        (result.devices, network.arp_requests()),
        (vec![], Vec::<Ipv4Addr>::new())
    );
}

#[tokio::test(start_paused = true)]
async fn a_device_beyond_the_pcs_subnet_is_never_looked_up_by_arp_even_after_answering_the_ping() {
    let network = reply("172.16.0.5", 12)(pc_on_lan());

    scan(&network, "172.16.0.5").await;

    assert_eq!(
        (network.arp_requests(), network.cache_reads()),
        (Vec::<Ipv4Addr>::new(), Vec::<Ipv4Addr>::new())
    );
}
