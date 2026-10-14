//! ARP resolution through the Windows SendARP call (iphlpapi), which needs no Administrator
//! rights (ADR 0002). It blocks until the device answers or Windows gives up, so callers run it
//! off the async runtime.

use std::ffi::c_void;
use std::net::Ipv4Addr;

use windows_sys::Win32::Foundation::NO_ERROR;
use windows_sys::Win32::NetworkManagement::IpHelper::SendARP;

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
