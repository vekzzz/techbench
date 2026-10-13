//! This PC's network adapters through the Windows IP Helper API: GetAdaptersAddresses for
//! almost everything, and GetAdaptersInfo for DHCP lease times, which only that older call reports.

use std::collections::HashMap;
use std::ffi::CStr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ptr::{null, null_mut};

use chrono::{DateTime, FixedOffset, Local, TimeZone};
use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST,
    GetAdaptersAddresses, GetAdaptersInfo, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211,
    IF_TYPE_SOFTWARE_LOOPBACK, IF_TYPE_TUNNEL, IP_ADAPTER_ADDRESSES_LH, IP_ADAPTER_DHCP_ENABLED,
    IP_ADAPTER_INFO,
};
use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows_sys::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6, SOCKET_ADDRESS,
};

use crate::network::{Adapter, AdapterAddress, AdapterKind, DhcpLease};

/// When a DHCP lease was obtained and when it expires, by adapter GUID name.
type LeaseTimes = HashMap<String, (Option<DateTime<FixedOffset>>, Option<DateTime<FixedOffset>>)>;

pub(crate) fn adapters() -> Vec<Adapter> {
    let leases = lease_times();
    let Some(buffer) = adapter_addresses() else {
        return vec![];
    };
    // SAFETY: GetAdaptersAddresses filled `buffer` with a linked list of these structures, all
    // inside `buffer`, which lives until the end of this function.
    let entries = unsafe {
        linked(buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>(), |entry| {
            entry.Next
        })
    };
    entries
        .into_iter()
        .map(|entry| adapter_from(entry, &leases))
        .collect()
}

/// The raw GetAdaptersAddresses list, in an 8-byte-aligned buffer, retried if it grows meanwhile.
fn adapter_addresses() -> Option<Vec<u64>> {
    let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    let mut size: u32 = 16 * 1024;
    for _ in 0..3 {
        let mut buffer = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
        // SAFETY: the buffer is writable for `size` bytes and `size` outlives the call.
        let status = unsafe {
            GetAdaptersAddresses(
                u32::from(AF_UNSPEC),
                flags,
                null(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        match status {
            NO_ERROR => return Some(buffer),
            ERROR_BUFFER_OVERFLOW => continue,
            _ => return None,
        }
    }
    None
}

fn adapter_from(entry: &IP_ADAPTER_ADDRESSES_LH, leases: &LeaseTimes) -> Adapter {
    let kind = match entry.IfType {
        IF_TYPE_ETHERNET_CSMACD => AdapterKind::Ethernet,
        IF_TYPE_IEEE80211 => AdapterKind::Wireless,
        IF_TYPE_SOFTWARE_LOOPBACK => AdapterKind::Loopback,
        IF_TYPE_TUNNEL => AdapterKind::Tunnel,
        _ => AdapterKind::Other,
    };
    let mac = (entry.PhysicalAddressLength == 6)
        .then(|| entry.PhysicalAddress[..6].try_into().expect("six bytes"));

    // SAFETY (for every list below): GetAdaptersAddresses links these lists from `entry`, inside
    // the same buffer, which outlives this function.
    let addresses = unsafe { linked(entry.FirstUnicastAddress, |unicast| unicast.Next) }
        .into_iter()
        .filter_map(|unicast| {
            socket_ip(&unicast.Address).map(|address| AdapterAddress {
                address,
                prefix: unicast.OnLinkPrefixLength,
            })
        })
        .collect();
    let gateways = unsafe { linked(entry.FirstGatewayAddress, |gateway| gateway.Next) }
        .into_iter()
        .filter_map(|gateway| socket_ip(&gateway.Address))
        .collect();
    let dns_servers = unsafe { linked(entry.FirstDnsServerAddress, |server| server.Next) }
        .into_iter()
        .filter_map(|server| socket_ip(&server.Address))
        .collect();

    // SAFETY: Flags is a plain u32 view of the union's bitfield.
    let dhcp_enabled = unsafe { entry.Anonymous2.Flags } & IP_ADAPTER_DHCP_ENABLED != 0;
    let dhcp = dhcp_enabled.then(|| {
        // SAFETY: AdapterName is a NUL-terminated ANSI string inside the same buffer.
        let guid = unsafe { CStr::from_ptr(entry.AdapterName.cast()) }.to_string_lossy();
        let (obtained, expires) = leases.get(guid.as_ref()).copied().unwrap_or((None, None));
        DhcpLease {
            server: socket_ip(&entry.Dhcpv4Server),
            obtained,
            expires,
        }
    });

    Adapter {
        // SAFETY: FriendlyName and Description are NUL-terminated UTF-16 strings in the buffer.
        name: unsafe { wide_string(entry.FriendlyName) },
        description: unsafe { wide_string(entry.Description) },
        kind,
        up: entry.OperStatus == IfOperStatusUp,
        mac,
        addresses,
        gateways,
        dns_servers,
        dhcp,
        ipv4_metric: entry.Ipv4Metric,
        ipv6_metric: entry.Ipv6Metric,
    }
}

/// DHCP lease times from GetAdaptersInfo, keyed by the adapter's GUID name. IPv4 only, as the
/// API is; an empty map if the call fails.
fn lease_times() -> LeaseTimes {
    let mut size = 0u32;
    // SAFETY: a null buffer with size 0 only asks for the size needed.
    unsafe { GetAdaptersInfo(null_mut(), &mut size) };
    if size == 0 {
        return LeaseTimes::new();
    }
    let mut buffer = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
    // SAFETY: the buffer is writable for `size` bytes and `size` outlives the call.
    if unsafe { GetAdaptersInfo(buffer.as_mut_ptr().cast(), &mut size) } != NO_ERROR {
        return LeaseTimes::new();
    }
    // SAFETY: GetAdaptersInfo filled `buffer` with a linked list inside it; `buffer` outlives this.
    let entries = unsafe { linked(buffer.as_ptr().cast::<IP_ADAPTER_INFO>(), |info| info.Next) };
    entries
        .into_iter()
        .filter(|info| info.DhcpEnabled != 0)
        .map(|info| {
            // SAFETY: AdapterName is a NUL-terminated ANSI string within its fixed-size array.
            let guid = unsafe { CStr::from_ptr(info.AdapterName.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            (
                guid,
                (
                    local_time(info.LeaseObtained),
                    local_time(info.LeaseExpires),
                ),
            )
        })
        .collect()
}

/// A Unix time in seconds as local time; None for 0, which the API uses for "unknown".
fn local_time(seconds: i64) -> Option<DateTime<FixedOffset>> {
    (seconds > 0)
        .then(|| Local.timestamp_opt(seconds, 0).single())
        .flatten()
        .map(|time| time.fixed_offset())
}

fn socket_ip(address: &SOCKET_ADDRESS) -> Option<IpAddr> {
    if address.lpSockaddr.is_null() {
        return None;
    }
    // SAFETY: a non-null lpSockaddr points at a sockaddr whose family says which structure it is.
    unsafe {
        match (*address.lpSockaddr).sa_family {
            AF_INET => {
                let v4 = &*address.lpSockaddr.cast::<SOCKADDR_IN>();
                Some(IpAddr::V4(Ipv4Addr::from(
                    v4.sin_addr.S_un.S_addr.to_ne_bytes(),
                )))
            }
            AF_INET6 => {
                let v6 = &*address.lpSockaddr.cast::<SOCKADDR_IN6>();
                Some(IpAddr::V6(Ipv6Addr::from(v6.sin6_addr.u.Byte)))
            }
            _ => None,
        }
    }
}

/// # Safety
/// `text` must be null or point at a NUL-terminated UTF-16 string.
unsafe fn wide_string(text: *const u16) -> String {
    if text.is_null() {
        return String::new();
    }
    let mut length = 0;
    // SAFETY: the caller promises a NUL terminator, so every read up to it is in bounds.
    while unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) })
}

/// Collects a C linked list into references.
///
/// # Safety
/// `first` must be null or head a valid list whose every node outlives `'a`.
unsafe fn linked<'a, T>(first: *const T, next: impl Fn(&T) -> *mut T) -> Vec<&'a T> {
    let mut nodes = Vec::new();
    let mut current = first;
    while !current.is_null() {
        // SAFETY: the caller promises every node in the list is valid for `'a`.
        let node = unsafe { &*current };
        nodes.push(node);
        current = next(node);
    }
    nodes
}
