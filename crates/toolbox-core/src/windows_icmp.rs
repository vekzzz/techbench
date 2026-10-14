//! ICMP echo through the Windows ICMP helper API (iphlpapi), which needs no Administrator
//! rights (ADR 0002). Each call blocks until the reply arrives or the timeout passes.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ptr::{null, null_mut};
use std::time::Duration;

use windows_sys::Win32::Foundation::{GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    ICMP_ECHO_REPLY, ICMPV6_ECHO_REPLY_LH, IP_FLAG_DF, IP_OPTION_INFORMATION, IP_PACKET_TOO_BIG,
    IP_REQ_TIMED_OUT, IP_SUCCESS, IP_TTL_EXPIRED_REASSEM, IP_TTL_EXPIRED_TRANSIT, Icmp6CreateFile,
    Icmp6SendEcho2, IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho2,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET6, IN6_ADDR, IN6_ADDR_0, SOCKADDR_IN6};

use crate::network::{EchoOutcome, EchoRequest};

/// The TTL Windows' own ping sends with.
const DEFAULT_TTL: u8 = 128;

/// Room after the reply structure for the echoed data, an ICMP error header and the
/// IO_STATUS_BLOCK the API may write.
const REPLY_SLACK: usize = 64;

pub(crate) fn echo(request: EchoRequest) -> EchoOutcome {
    match request.address {
        IpAddr::V4(destination) => echo_v4(destination, request),
        IpAddr::V6(destination) => echo_v6(destination, request),
    }
}

fn echo_v4(destination: Ipv4Addr, request: EchoRequest) -> EchoOutcome {
    // SAFETY: IcmpCreateFile takes no arguments; IcmpHandle checks the handle it returns.
    let Some(handle) = IcmpHandle::open(unsafe { IcmpCreateFile() }) else {
        return EchoOutcome::Unreachable { from: None };
    };
    let payload = payload(request.payload_size);
    let options = options(&request);
    let mut reply_buffer = ReplyBuffer::new(size_of::<ICMP_ECHO_REPLY>() + payload.len());

    // SAFETY: the handle is open; the payload, options and reply buffer outlive the call and
    // their sizes are passed alongside them; with no event or APC routine the call is synchronous.
    let replies = unsafe {
        IcmpSendEcho2(
            handle.0,
            null_mut(),
            None,
            null(),
            u32::from_ne_bytes(destination.octets()),
            payload.as_ptr().cast(),
            request.payload_size,
            &options,
            reply_buffer.as_mut_ptr(),
            reply_buffer.len_bytes(),
            timeout_ms(request.timeout),
        )
    };
    if replies == 0 {
        // SAFETY: GetLastError only reads this thread's last-error value.
        return outcome_of(unsafe { GetLastError() }, None, Duration::ZERO);
    }
    // SAFETY: the call reported a reply, which it writes at the start of the aligned buffer;
    // `reply_buffer` lives until the end of this function, outliving the reference.
    let reply = unsafe { &*reply_buffer.as_ptr().cast::<ICMP_ECHO_REPLY>() };
    let from = IpAddr::V4(Ipv4Addr::from(reply.Address.to_ne_bytes()));
    match reply.Status {
        IP_SUCCESS => EchoOutcome::Reply {
            round_trip: Duration::from_millis(u64::from(reply.RoundTripTime)),
            ttl: Some(reply.Options.Ttl),
        },
        status => outcome_of(
            status,
            Some(from),
            Duration::from_millis(u64::from(reply.RoundTripTime)),
        ),
    }
}

fn echo_v6(destination: Ipv6Addr, request: EchoRequest) -> EchoOutcome {
    // SAFETY: Icmp6CreateFile takes no arguments; IcmpHandle checks the handle it returns.
    let Some(handle) = IcmpHandle::open(unsafe { Icmp6CreateFile() }) else {
        return EchoOutcome::Unreachable { from: None };
    };
    let payload = payload(request.payload_size);
    let options = options(&request);
    let mut reply_buffer = ReplyBuffer::new(size_of::<ICMPV6_ECHO_REPLY_LH>() + payload.len());
    // An unspecified source lets Windows pick the address the route leaves from.
    let source = socket_address(Ipv6Addr::UNSPECIFIED);
    let destination = socket_address(destination);

    // SAFETY: as for IPv4; both socket addresses outlive the call.
    let replies = unsafe {
        Icmp6SendEcho2(
            handle.0,
            null_mut(),
            None,
            null(),
            &source,
            &destination,
            payload.as_ptr().cast(),
            request.payload_size,
            &options,
            reply_buffer.as_mut_ptr(),
            reply_buffer.len_bytes(),
            timeout_ms(request.timeout),
        )
    };
    if replies == 0 {
        // SAFETY: GetLastError only reads this thread's last-error value.
        return outcome_of(unsafe { GetLastError() }, None, Duration::ZERO);
    }
    // SAFETY: the call reported a reply, which it writes at the start of the aligned buffer;
    // `reply_buffer` lives until the end of this function, outliving the reference.
    let reply = unsafe { &*reply_buffer.as_ptr().cast::<ICMPV6_ECHO_REPLY_LH>() };
    let words = reply.Address.sin6_addr.map(u16::from_be);
    let from = IpAddr::V6(Ipv6Addr::from(words));
    match reply.Status {
        // ICMPv6 replies don't report the hop limit they arrived with.
        IP_SUCCESS => EchoOutcome::Reply {
            round_trip: Duration::from_millis(u64::from(reply.RoundTripTime)),
            ttl: None,
        },
        status => outcome_of(
            status,
            Some(from),
            Duration::from_millis(u64::from(reply.RoundTripTime)),
        ),
    }
}

fn outcome_of(status: u32, from: Option<IpAddr>, round_trip: Duration) -> EchoOutcome {
    match status {
        IP_REQ_TIMED_OUT => EchoOutcome::TimedOut,
        IP_PACKET_TOO_BIG => EchoOutcome::FragmentationNeeded,
        IP_TTL_EXPIRED_TRANSIT | IP_TTL_EXPIRED_REASSEM => {
            EchoOutcome::TimeExceeded { from, round_trip }
        }
        // The IP_DEST_*_UNREACHABLE statuses, and anything else (a bad route, a general
        // failure when the network is down): the echo didn't arrive.
        _ => EchoOutcome::Unreachable { from },
    }
}

/// The same alphabet pattern Windows' own ping fills its echoes with.
fn payload(size: u16) -> Vec<u8> {
    (0..size).map(|i| b'a' + (i % 23) as u8).collect()
}

fn options(request: &EchoRequest) -> IP_OPTION_INFORMATION {
    IP_OPTION_INFORMATION {
        Ttl: request.ttl.unwrap_or(DEFAULT_TTL),
        Tos: 0,
        Flags: if request.dont_fragment {
            IP_FLAG_DF as u8
        } else {
            0
        },
        OptionsSize: 0,
        OptionsData: null_mut(),
    }
}

fn timeout_ms(timeout: Duration) -> u32 {
    u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX)
}

fn socket_address(address: Ipv6Addr) -> SOCKADDR_IN6 {
    SOCKADDR_IN6 {
        sin6_family: AF_INET6,
        sin6_addr: IN6_ADDR {
            u: IN6_ADDR_0 {
                Byte: address.octets(),
            },
        },
        ..SOCKADDR_IN6::default()
    }
}

/// An open ICMP handle, closed when dropped.
struct IcmpHandle(HANDLE);

impl IcmpHandle {
    fn open(handle: HANDLE) -> Option<Self> {
        (handle != INVALID_HANDLE_VALUE).then_some(Self(handle))
    }
}

impl Drop for IcmpHandle {
    fn drop(&mut self) {
        // SAFETY: the handle came from IcmpCreateFile/Icmp6CreateFile and is closed only here.
        unsafe { IcmpCloseHandle(self.0) };
    }
}

/// A reply buffer aligned for the reply structures the API writes into it.
struct ReplyBuffer(Vec<u64>);

impl ReplyBuffer {
    fn new(minimum_bytes: usize) -> Self {
        Self(vec![
            0;
            (minimum_bytes + REPLY_SLACK).div_ceil(size_of::<u64>())
        ])
    }

    fn as_ptr(&self) -> *const core::ffi::c_void {
        self.0.as_ptr().cast()
    }

    fn as_mut_ptr(&mut self) -> *mut core::ffi::c_void {
        self.0.as_mut_ptr().cast()
    }

    fn len_bytes(&self) -> u32 {
        (self.0.len() * size_of::<u64>()) as u32
    }
}
