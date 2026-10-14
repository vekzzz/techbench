//! Reverse DNS through the Windows resolver (getnameinfo), the same one `ping -a` and `tracert` use.
//! It blocks, so callers run it off the async runtime.

use std::net::IpAddr;
use std::sync::Once;

use windows_sys::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, GetNameInfoW, IN_ADDR, IN_ADDR_0, IN6_ADDR, IN6_ADDR_0, NI_NAMEREQD,
    SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6, WSADATA, WSAStartup,
};

/// The largest name DNS allows, plus its terminator.
const NAME_CAPACITY: usize = 256;

pub(crate) fn reverse_lookup(address: IpAddr) -> Option<String> {
    start_winsock();
    let mut name = [0u16; NAME_CAPACITY];
    let status = match address {
        IpAddr::V4(v4) => {
            let socket = SOCKADDR_IN {
                sin_family: AF_INET,
                sin_port: 0,
                sin_addr: IN_ADDR {
                    S_un: IN_ADDR_0 {
                        S_addr: u32::from_ne_bytes(v4.octets()),
                    },
                },
                sin_zero: [0; 8],
            };
            // SAFETY: `socket` is a valid SOCKADDR_IN of the stated size and `name` is writable
            // for NAME_CAPACITY UTF-16 units; no service buffer is requested.
            unsafe {
                lookup(
                    std::ptr::from_ref(&socket).cast(),
                    size_of::<SOCKADDR_IN>(),
                    &mut name,
                )
            }
        }
        IpAddr::V6(v6) => {
            let socket = SOCKADDR_IN6 {
                sin6_family: AF_INET6,
                sin6_addr: IN6_ADDR {
                    u: IN6_ADDR_0 { Byte: v6.octets() },
                },
                ..SOCKADDR_IN6::default()
            };
            // SAFETY: as above, for a SOCKADDR_IN6.
            unsafe {
                lookup(
                    std::ptr::from_ref(&socket).cast(),
                    size_of::<SOCKADDR_IN6>(),
                    &mut name,
                )
            }
        }
    };
    if status != 0 {
        return None;
    }
    let length = name.iter().position(|unit| *unit == 0)?;
    let found = String::from_utf16_lossy(&name[..length]);
    // Windows answers with the address itself when it has nothing better; that is not a name.
    (found != address.to_string()).then_some(found)
}

/// # Safety
/// `socket` must point at a valid sockaddr of `length` bytes.
unsafe fn lookup(socket: *const SOCKADDR, length: usize, name: &mut [u16; NAME_CAPACITY]) -> i32 {
    // SAFETY: the caller promises the sockaddr; `name` is writable for its length.
    unsafe {
        GetNameInfoW(
            socket,
            length as i32,
            name.as_mut_ptr(),
            NAME_CAPACITY as u32,
            std::ptr::null_mut(),
            0,
            NI_NAMEREQD as i32,
        )
    }
}

/// Winsock must be started before name lookups; starting it again is harmless (it is counted).
fn start_winsock() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        let mut data = WSADATA::default();
        // SAFETY: `data` is a writable WSADATA; 0x0202 asks for Winsock 2.2.
        unsafe { WSAStartup(0x0202, &mut data) };
    });
}
