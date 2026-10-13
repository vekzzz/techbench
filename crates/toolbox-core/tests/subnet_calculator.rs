//! Subnet Calculator, exercised through the toolbox core's public interface (seam S1).
//! Expected values are worked examples, never recomputed the way the code does.

use std::net::IpAddr;

use toolbox_core::subnet_calculator::{SubnetReport, calculate};

fn ip(text: &str) -> IpAddr {
    text.parse().unwrap()
}

#[test]
fn ipv4_slash_24_reports_network_broadcast_usable_range_and_masks() {
    let report = calculate("192.168.1.10/24").unwrap();

    assert_eq!(
        report,
        SubnetReport {
            network: ip("192.168.1.0"),
            prefix: 24,
            broadcast: Some(ip("192.168.1.255")),
            first_usable: ip("192.168.1.1"),
            last_usable: ip("192.168.1.254"),
            usable_count: "254".to_string(),
            mask: Some(ip("255.255.255.0")),
            wildcard: Some(ip("0.0.0.255")),
        }
    );
}

#[test]
fn ipv4_address_with_dotted_mask_is_understood() {
    let report = calculate("10.0.0.77 255.255.255.192").unwrap();

    assert_eq!(
        report,
        SubnetReport {
            network: ip("10.0.0.64"),
            prefix: 26,
            broadcast: Some(ip("10.0.0.127")),
            first_usable: ip("10.0.0.65"),
            last_usable: ip("10.0.0.126"),
            usable_count: "62".to_string(),
            mask: Some(ip("255.255.255.192")),
            wildcard: Some(ip("0.0.0.63")),
        }
    );
}

#[test]
fn ipv4_prefix_inside_an_octet_splits_that_octet() {
    let report = calculate("10.20.30.40/19").unwrap();

    assert_eq!(
        report,
        SubnetReport {
            network: ip("10.20.0.0"),
            prefix: 19,
            broadcast: Some(ip("10.20.31.255")),
            first_usable: ip("10.20.0.1"),
            last_usable: ip("10.20.31.254"),
            usable_count: "8190".to_string(),
            mask: Some(ip("255.255.224.0")),
            wildcard: Some(ip("0.0.31.255")),
        }
    );
}

#[test]
fn ipv4_slash_31_is_a_point_to_point_link_with_two_usable_addresses_and_no_broadcast() {
    let report = calculate("192.168.0.5/31").unwrap();

    assert_eq!(
        report,
        SubnetReport {
            network: ip("192.168.0.4"),
            prefix: 31,
            broadcast: None,
            first_usable: ip("192.168.0.4"),
            last_usable: ip("192.168.0.5"),
            usable_count: "2".to_string(),
            mask: Some(ip("255.255.255.254")),
            wildcard: Some(ip("0.0.0.1")),
        }
    );
}

#[test]
fn ipv4_slash_32_is_a_single_host() {
    let report = calculate("10.1.1.1/32").unwrap();

    assert_eq!(
        report,
        SubnetReport {
            network: ip("10.1.1.1"),
            prefix: 32,
            broadcast: None,
            first_usable: ip("10.1.1.1"),
            last_usable: ip("10.1.1.1"),
            usable_count: "1".to_string(),
            mask: Some(ip("255.255.255.255")),
            wildcard: Some(ip("0.0.0.0")),
        }
    );
}

#[test]
fn ipv4_slash_0_covers_the_whole_address_space() {
    let report = calculate("0.0.0.0/0").unwrap();

    assert_eq!(
        report,
        SubnetReport {
            network: ip("0.0.0.0"),
            prefix: 0,
            broadcast: Some(ip("255.255.255.255")),
            first_usable: ip("0.0.0.1"),
            last_usable: ip("255.255.255.254"),
            usable_count: "4294967294".to_string(),
            mask: Some(ip("0.0.0.0")),
            wildcard: Some(ip("255.255.255.255")),
        }
    );
}

#[test]
fn contains_check_says_an_address_in_the_range_is_inside() {
    let report = calculate("10.0.0.77/26").unwrap();

    assert_eq!(report.contains("10.0.0.100"), Ok(true));
}

#[test]
fn contains_check_says_an_address_past_the_broadcast_is_outside() {
    let report = calculate("10.0.0.77/26").unwrap();

    assert_eq!(report.contains("10.0.0.128"), Ok(false));
}

#[test]
fn contains_check_rejects_something_that_is_not_an_address() {
    let report = calculate("10.0.0.77/26").unwrap();

    assert!(report.contains("10.0.0.999").is_err());
}

#[test]
fn ipv4_prefix_longer_than_32_is_rejected() {
    assert!(calculate("10.0.0.0/33").is_err());
}

#[test]
fn non_contiguous_dotted_mask_is_rejected() {
    assert!(calculate("10.0.0.1 255.0.255.0").is_err());
}

#[test]
fn octet_above_255_is_rejected() {
    assert!(calculate("192.168.1.300/24").is_err());
}

#[test]
fn text_that_is_not_a_subnet_is_rejected() {
    assert!(calculate("printer on floor 2").is_err());
}

#[test]
fn ipv6_slash_64_reports_first_and_last_address_and_address_count() {
    let report = calculate("2001:db8:abcd:12::1/64").unwrap();

    assert_eq!(
        report,
        SubnetReport {
            network: ip("2001:db8:abcd:12::"),
            prefix: 64,
            broadcast: None,
            first_usable: ip("2001:db8:abcd:12::"),
            last_usable: ip("2001:db8:abcd:12:ffff:ffff:ffff:ffff"),
            usable_count: "18446744073709551616".to_string(),
            mask: None,
            wildcard: None,
        }
    );
}

#[test]
fn ipv6_slash_128_is_a_single_address() {
    let report = calculate("2001:db8::7/128").unwrap();

    assert_eq!(
        report,
        SubnetReport {
            network: ip("2001:db8::7"),
            prefix: 128,
            broadcast: None,
            first_usable: ip("2001:db8::7"),
            last_usable: ip("2001:db8::7"),
            usable_count: "1".to_string(),
            mask: None,
            wildcard: None,
        }
    );
}

#[test]
fn ipv6_slash_0_counts_all_two_to_the_128_addresses() {
    let report = calculate("::/0").unwrap();

    assert_eq!(
        report.usable_count,
        "340282366920938463463374607431768211456"
    );
}

#[test]
fn ipv6_prefix_longer_than_128_is_rejected() {
    assert!(calculate("2001:db8::/129").is_err());
}

#[test]
fn ipv6_address_with_dotted_mask_is_rejected() {
    assert!(calculate("2001:db8::1 255.255.255.0").is_err());
}

#[test]
fn contains_check_works_for_ipv6_subnets() {
    let report = calculate("2001:db8:abcd:12::1/64").unwrap();

    assert_eq!(report.contains("2001:db8:abcd:12:ffff::9"), Ok(true));
}
