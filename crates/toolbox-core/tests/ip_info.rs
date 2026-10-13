//! IP Info, exercised through the toolbox core's public interface (seam S1) over a scripted network (seam S2).

mod common;

use common::{FakeNetwork, ip};
use tokio_util::sync::CancellationToken;
use toolbox_core::ip_info::{self, IpInfoEvent, IpInfoResult};
use toolbox_core::network::{Adapter, AdapterAddress, AdapterKind, DhcpLease, PublicIp};

/// An active Ethernet adapter with no addresses; tests fill in what they need.
fn adapter(name: &str) -> Adapter {
    Adapter {
        name: name.to_string(),
        description: format!("{name} controller"),
        kind: AdapterKind::Ethernet,
        up: true,
        mac: None,
        addresses: vec![],
        gateways: vec![],
        dns_servers: vec![],
        dhcp: None,
        ipv4_metric: 25,
        ipv6_metric: 25,
    }
}

fn down(adapter: Adapter) -> Adapter {
    Adapter {
        up: false,
        ..adapter
    }
}

fn with_gateway(adapter: Adapter, gateway: &str, metric: u32) -> Adapter {
    Adapter {
        gateways: vec![ip(gateway)],
        ipv4_metric: metric,
        addresses: vec![AdapterAddress {
            address: ip("192.168.1.20"),
            prefix: 24,
        }],
        ..adapter
    }
}

async fn run(network: &FakeNetwork) -> IpInfoResult {
    ip_info::run(network, &CancellationToken::new(), |_| {}).await
}

fn names(result: &IpInfoResult) -> Vec<&str> {
    result
        .adapters
        .iter()
        .map(|adapter| adapter.name.as_str())
        .collect()
}

#[tokio::test]
async fn the_default_route_adapter_comes_first_then_other_active_ones_then_inactive_ones() {
    let network = FakeNetwork::new().with_adapters(vec![
        down(adapter("Bluetooth")),
        adapter("vEthernet (WSL)"),
        Adapter {
            kind: AdapterKind::Loopback,
            ..adapter("Loopback Pseudo-Interface 1")
        },
        with_gateway(adapter("Wi-Fi"), "192.168.1.1", 35),
        down(adapter("Ethernet 2")),
        adapter("Ethernet"),
    ]);

    let result = run(&network).await;

    assert_eq!(
        (names(&result), result.default_route.as_deref()),
        (
            vec![
                "Wi-Fi",
                "Ethernet",
                "vEthernet (WSL)",
                "Bluetooth",
                "Ethernet 2"
            ],
            Some("Wi-Fi")
        )
    );
}

fn default_route(adapters: &[Adapter]) -> Option<&str> {
    ip_info::default_route_adapter(adapters).map(|adapter| adapter.name.as_str())
}

#[test]
fn of_several_adapters_with_a_gateway_the_lowest_ipv4_metric_carries_the_default_route() {
    let adapters = [
        with_gateway(adapter("Wi-Fi"), "192.168.1.1", 35),
        with_gateway(adapter("Ethernet"), "10.0.0.1", 25),
    ];

    assert_eq!(default_route(&adapters), Some("Ethernet"));
}

#[test]
fn an_inactive_adapter_never_carries_the_default_route() {
    let adapters = [
        down(with_gateway(adapter("Ethernet"), "10.0.0.1", 5)),
        with_gateway(adapter("Wi-Fi"), "192.168.1.1", 35),
    ];

    assert_eq!(default_route(&adapters), Some("Wi-Fi"));
}

#[test]
fn an_adapter_with_only_an_ipv6_gateway_carries_the_default_route_when_none_has_an_ipv4_one() {
    let adapters = [
        adapter("Ethernet"),
        Adapter {
            gateways: vec![ip("fe80::1")],
            ..adapter("Wi-Fi")
        },
    ];

    assert_eq!(default_route(&adapters), Some("Wi-Fi"));
}

#[test]
fn with_no_gateway_anywhere_there_is_no_default_route() {
    assert_eq!(
        default_route(&[adapter("Ethernet"), adapter("Wi-Fi")]),
        None
    );
}

#[tokio::test]
async fn a_failed_public_lookup_is_reported_after_the_local_details_which_still_arrive() {
    let wifi = with_gateway(adapter("Wi-Fi"), "192.168.1.1", 35);
    let network = FakeNetwork::new()
        .with_adapters(vec![wifi.clone()])
        .with_public_ip(Err("no internet connection".to_string()));
    let mut events = Vec::new();

    let result = ip_info::run(&network, &CancellationToken::new(), |event| {
        events.push(event)
    })
    .await;

    assert_eq!(
        (events, result.adapters, result.public),
        (
            vec![
                IpInfoEvent::Adapters {
                    adapters: vec![wifi.clone()],
                    default_route: Some("Wi-Fi".to_string())
                },
                IpInfoEvent::Public(Err("no internet connection".to_string())),
            ],
            vec![wifi],
            Some(Err("no internet connection".to_string()))
        )
    );
}

#[tokio::test]
async fn stopping_while_the_public_lookup_is_slow_keeps_the_local_details() {
    let network = FakeNetwork::new()
        .with_adapters(vec![adapter("Ethernet")])
        .with_hanging_public_ip();
    let stop = CancellationToken::new();

    let (result, ()) = tokio::join!(ip_info::run(&network, &stop, |_| {}), async {
        tokio::task::yield_now().await;
        stop.cancel();
    });

    assert_eq!(
        (names(&result), result.public.clone()),
        (vec!["Ethernet"], None)
    );
}

#[tokio::test]
async fn copy_as_text_gives_the_public_address_then_each_adapter_with_only_the_details_it_has() {
    let lease_time = |text: &str| Some(chrono::DateTime::parse_from_rfc3339(text).unwrap());
    let wifi = Adapter {
        kind: AdapterKind::Wireless,
        mac: Some([0xAA, 0xBB, 0xCC, 0x01, 0x02, 0x03]),
        addresses: vec![
            AdapterAddress {
                address: ip("192.168.1.20"),
                prefix: 24,
            },
            AdapterAddress {
                address: ip("fe80::1c2d:3e4f:5a6b:7c8d"),
                prefix: 64,
            },
        ],
        gateways: vec![ip("192.168.1.1")],
        dns_servers: vec![ip("192.168.1.1"), ip("8.8.8.8")],
        dhcp: Some(DhcpLease {
            server: Some(ip("192.168.1.1")),
            obtained: lease_time("2026-10-13T08:00:00+08:00"),
            expires: lease_time("2026-10-14T08:00:00+08:00"),
        }),
        ipv4_metric: 35,
        ..adapter("Wi-Fi")
    };
    let network = FakeNetwork::new()
        .with_adapters(vec![down(adapter("Ethernet")), wifi])
        .with_public_ip(Ok(PublicIp {
            address: ip("203.0.113.7"),
            isp: Some("Example Telecom".to_string()),
            city: Some("Manila".to_string()),
        }));

    let result = run(&network).await;

    assert_eq!(
        result.to_text(),
        "\
IP Info
Time:      2026-10-13 14:05:09 +08:00
PC:        TECH-PC
Public:    203.0.113.7 (Example Telecom, Manila)

Wi-Fi (default route)
  Status:       Up
  Description:  Wi-Fi controller
  MAC:          AA-BB-CC-01-02-03
  IPv4:         192.168.1.20/24
  IPv6:         fe80::1c2d:3e4f:5a6b:7c8d/64
  Gateway:      192.168.1.1
  DNS servers:  192.168.1.1, 8.8.8.8
  DHCP:         On, server 192.168.1.1
  Lease:        obtained 2026-10-13 08:00:00 +08:00, expires 2026-10-14 08:00:00 +08:00

Ethernet
  Status:       Down
  Description:  Ethernet controller
  DHCP:         Off
"
    );
}

#[tokio::test]
async fn copy_as_text_says_why_the_public_lookup_failed() {
    let network = FakeNetwork::new()
        .with_adapters(vec![adapter("Ethernet")])
        .with_public_ip(Err("no internet connection".to_string()));

    let text = run(&network).await.to_text();

    assert!(
        text.contains("\nPublic:    lookup failed: no internet connection\n"),
        "{text}"
    );
}

#[tokio::test]
async fn an_unspecified_gateway_such_as_teredos_is_no_gateway_at_all() {
    let teredo = Adapter {
        kind: AdapterKind::Tunnel,
        gateways: vec![ip("::")],
        ..adapter("Teredo Tunneling Pseudo-Interface")
    };
    let network = FakeNetwork::new().with_adapters(vec![teredo, adapter("Ethernet")]);

    let result = run(&network).await;

    assert_eq!(
        (
            result.default_route.clone(),
            result.to_text().contains("Gateway:")
        ),
        (None, false)
    );
}

#[test]
fn the_default_route_rule_itself_ignores_an_unspecified_gateway() {
    let teredo = Adapter {
        gateways: vec![ip("::")],
        ..adapter("Teredo Tunneling Pseudo-Interface")
    };

    assert_eq!(default_route(&[teredo]), None);
}
