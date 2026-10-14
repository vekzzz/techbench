//! ARP through iphlpapi, which needs no Administrator rights (ADR 0002): SendARP asks the
//! network, and the neighbour table says what Windows already knows. Both block, so callers run
//! them off the async runtime.

use std::ffi::c_void;
use std::net::Ipv4Addr;

use windows_sys::Win32::Foundation::NO_ERROR;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIpNetTable2, MIB_IPNET_ROW2, MIB_IPNET_TABLE2, SendARP,
};
use windows_sys::Win32::Networking::WinSock::{
    AF_INET, NL_NEIGHBOR_STATE, NlnsDelay, NlnsPermanent, NlnsProbe, NlnsReachable, NlnsStale,
};

/// The MAC address of the device holding `address` on a directly connected subnet, if it answers.
pub(crate) fn resolve(address: Ipv4Addr) -> Option<[u8; 6]> {
    // SendARP writes the hardware address into a buffer and the number of bytes it wrote back
    // into `length`, which must start as the buffer's capacity.
    let mut buffer = [0u8; 8];
    let mut length = buffer.len() as u32;
    // SAFETY: `buffer` is writable for `length` bytes, both outlive the call, and a zero source
    // address lets Windows pick the interface that reaches the destination.
    let status = unsafe {
        SendARP(
            u32::from_ne_bytes(address.octets()),
            0,
            buffer.as_mut_ptr().cast::<c_void>(),
            &mut length,
        )
    };
    if status != NO_ERROR || length < 6 {
        return None;
    }
    let mac: [u8; 6] = buffer[..6].try_into().ok()?;
    // An all-zero hardware address is Windows' way of saying no device answered.
    (mac != [0; 6]).then_some(mac)
}

/// The MAC Windows' neighbour table holds for `address`, without sending anything. SendARP cannot
/// do this: it always asks the network again, even for an address Windows resolved a moment ago.
pub(crate) fn cached(address: Ipv4Addr) -> Option<[u8; 6]> {
    let mut table: *mut MIB_IPNET_TABLE2 = std::ptr::null_mut();
    // SAFETY: on success Windows points `table` at a table it allocated, freed below.
    if unsafe { GetIpNetTable2(AF_INET, &mut table) } != NO_ERROR || table.is_null() {
        return None;
    }
    let wanted = u32::from_ne_bytes(address.octets());
    // SAFETY: the table holds NumEntries rows laid out one after another from `Table`.
    let rows: &[MIB_IPNET_ROW2] = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    let found = rows.iter().find_map(|row| {
        // SAFETY: the table was asked for IPv4 only, so every address is a SOCKADDR_IN.
        let holder = unsafe { row.Address.Ipv4.sin_addr.S_un.S_addr };
        let usable = holder == wanted && row.PhysicalAddressLength == 6 && is_resolved(row.State);
        usable
            .then(|| row.PhysicalAddress[..6].try_into().ok())
            .flatten()
    });
    // SAFETY: `table` came from GetIpNetTable2 and is freed exactly once.
    unsafe { FreeMibTable(table.cast()) };
    found.filter(|mac: &[u8; 6]| *mac != [0; 6])
}

/// Whether a neighbour entry holds a resolved MAC (not one still being, or failed to be, resolved).
fn is_resolved(state: NL_NEIGHBOR_STATE) -> bool {
    [
        NlnsReachable,
        NlnsStale,
        NlnsDelay,
        NlnsProbe,
        NlnsPermanent,
    ]
    .contains(&state)
}
