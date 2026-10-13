//! This PC's network adapters through the Windows IP Helper API: GetAdaptersAddresses for
//! almost everything, and GetAdaptersInfo for DHCP lease times, which only that older call reports.

use std::collections::HashMap;
use std::ffi::{CStr, c_void};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ptr::null;

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
    let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    let Some(buffer) = query(16 * 1024, |buffer, size| {
        // SAFETY: the buffer is writable for `size` bytes and `size` outlives the call.
        unsafe { GetAdaptersAddresses(u32::from(AF_UNSPEC), flags, null(), buffer.cast(), size) }
    }) else {
        return vec![];
    };
    // SAFETY: GetAdaptersAddresses filled `buffer` with a linked list of these structures, every
    // node of which lies inside `buffer`.
    let entries = unsafe {
        linked(
            &buffer,
            buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>(),
            |entry| entry.Next,
        )
    };
    entries
        .into_iter()
        // SAFETY: each entry, and everything it points to, lies inside `buffer`, still alive here.
        .map(|entry| unsafe { adapter_from(entry, &leases) })
        .collect()
}

/// Calls an IP Helper function that fills a caller-sized buffer, growing the buffer while the
/// function reports that it needs more. The buffer is 8-byte aligned for the structures inside.
fn query(
    initial_size: u32,
    mut call: impl FnMut(*mut c_void, &mut u32) -> u32,
) -> Option<Vec<u64>> {
    let mut size = initial_size;
    for _ in 0..3 {
        let mut buffer = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
        match call(buffer.as_mut_ptr().cast(), &mut size) {
            NO_ERROR => return Some(buffer),
            ERROR_BUFFER_OVERFLOW => continue,
            _ => return None,
        }
    }
    None
}

/// # Safety
/// `entry` and everything it points to (its lists, strings and socket addresses) must be valid.
unsafe fn adapter_from(entry: &IP_ADAPTER_ADDRESSES_LH, leases: &LeaseTimes) -> Adapter {
    let kind = match entry.IfType {
        IF_TYPE_ETHERNET_CSMACD => AdapterKind::Ethernet,
        IF_TYPE_IEEE80211 => AdapterKind::Wireless,
        IF_TYPE_SOFTWARE_LOOPBACK => AdapterKind::Loopback,
        IF_TYPE_TUNNEL => AdapterKind::Tunnel,
        _ => AdapterKind::Other,
    };
    let mac = (entry.PhysicalAddressLength == 6)
        .then(|| entry.PhysicalAddress[..6].try_into().expect("six bytes"));

    // SAFETY: the caller promises `entry`'s lists and the socket addresses in them are valid.
    let addresses = unsafe { linked(entry, entry.FirstUnicastAddress, |unicast| unicast.Next) }
        .into_iter()
        .filter_map(|unicast| {
            // SAFETY: as above.
            unsafe { socket_ip(&unicast.Address) }.map(|address| AdapterAddress {
                address,
                prefix: unicast.OnLinkPrefixLength,
            })
        })
        .collect();
    // SAFETY: as above.
    let gateways = unsafe { linked(entry, entry.FirstGatewayAddress, |gateway| gateway.Next) }
        .into_iter()
        // SAFETY: as above.
        .filter_map(|gateway| unsafe { socket_ip(&gateway.Address) })
        .collect();
    // SAFETY: as above.
    let dns_servers = unsafe { linked(entry, entry.FirstDnsServerAddress, |server| server.Next) }
        .into_iter()
        // SAFETY: as above.
        .filter_map(|server| unsafe { socket_ip(&server.Address) })
        .collect();

    // SAFETY: Flags is a plain u32 view of the union's bitfield.
    let dhcp_enabled = unsafe { entry.Anonymous2.Flags } & IP_ADAPTER_DHCP_ENABLED != 0;
    let dhcp = dhcp_enabled.then(|| {
        // SAFETY: the caller promises AdapterName is a valid NUL-terminated ANSI string.
        let guid = unsafe { CStr::from_ptr(entry.AdapterName.cast()) }.to_string_lossy();
        let (obtained, expires) = leases.get(guid.as_ref()).copied().unwrap_or((None, None));
        DhcpLease {
            // SAFETY: the caller promises the DHCP server's socket address is valid.
            server: unsafe { socket_ip(&entry.Dhcpv4Server) },
            obtained,
            expires,
        }
    });

    Adapter {
        // SAFETY: the caller promises these are valid NUL-terminated UTF-16 strings.
        name: unsafe { wide_string(entry.FriendlyName) },
        // SAFETY: as above.
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
    let Some(buffer) = query(16 * 1024, |buffer, size| {
        // SAFETY: the buffer is writable for `size` bytes and `size` outlives the call.
        unsafe { GetAdaptersInfo(buffer.cast(), size) }
    }) else {
        return LeaseTimes::new();
    };
    // SAFETY: GetAdaptersInfo filled `buffer` with a linked list whose every node lies inside it.
    let entries = unsafe {
        linked(&buffer, buffer.as_ptr().cast::<IP_ADAPTER_INFO>(), |info| {
            info.Next
        })
    };
    entries
        .into_iter()
        .filter(|info| info.DhcpEnabled != 0)
        .filter_map(|info| {
            let name = info.AdapterName.map(|c| c as u8);
            let guid = CStr::from_bytes_until_nul(&name)
                .ok()?
                .to_string_lossy()
                .into_owned();
            Some((
                guid,
                (
                    local_time(info.LeaseObtained),
                    local_time(info.LeaseExpires),
                ),
            ))
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

/// # Safety
/// `address.lpSockaddr` must be null or point at a valid sockaddr of the family it declares.
unsafe fn socket_ip(address: &SOCKET_ADDRESS) -> Option<IpAddr> {
    if address.lpSockaddr.is_null() {
        return None;
    }
    // SAFETY: the caller promises a non-null lpSockaddr points at a valid sockaddr whose family
    // says which structure it is; the address fields read are plain data in either union view.
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
    // SAFETY: the `length` units before the terminator were just read, so they are valid.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) })
}

/// Collects a C linked list into references that live no longer than `owner`, the memory the
/// list was written into.
///
/// # Safety
/// `first` must be null or head a valid list whose every node lies inside `owner`.
unsafe fn linked<T, Owner: ?Sized>(
    owner: &Owner,
    first: *const T,
    next: impl Fn(&T) -> *mut T,
) -> Vec<&T> {
    let _ = owner;
    let mut nodes = Vec::new();
    let mut current = first;
    while !current.is_null() {
        // SAFETY: the caller promises every node is valid and inside `owner`, which the returned
        // references borrow.
        let node = unsafe { &*current };
        nodes.push(node);
        current = next(node);
    }
    nodes
}
