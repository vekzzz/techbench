//! The network adapter (test seam S2): the only way the toolbox core touches the operating system.
//! [`SystemNetwork`] is the real one; tests substitute a scripted fake.

use std::future::Future;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use chrono::{DateTime, FixedOffset, Local};
use tokio::net::TcpSocket;

/// What happened when a TCP connection was attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectOutcome {
    Accepted { connect_time: Duration },
    Refused,
    TimedOut,
    Unreachable,
}

/// Why the system resolver could not turn a hostname into addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError {
    pub message: String,
}

pub trait Network: Send + Sync {
    /// Resolves a hostname through the system resolver, in the order the system returns its addresses.
    fn resolve(
        &self,
        hostname: &str,
    ) -> impl Future<Output = Result<Vec<IpAddr>, ResolveError>> + Send;

    /// Attempts a TCP connection, giving up after `timeout`.
    fn tcp_connect(
        &self,
        address: SocketAddr,
        timeout: Duration,
    ) -> impl Future<Output = ConnectOutcome> + Send;

    /// The address of this PC that traffic to `destination` would leave from. Sends nothing.
    fn source_address(&self, destination: IpAddr) -> Option<IpAddr>;

    /// This PC's name.
    fn hostname(&self) -> String;

    /// The local time, with its UTC offset.
    fn now(&self) -> DateTime<FixedOffset>;
}

/// The real network adapter: this PC's own resolver, sockets and clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemNetwork;

impl Network for SystemNetwork {
    async fn resolve(&self, hostname: &str) -> Result<Vec<IpAddr>, ResolveError> {
        let found = tokio::net::lookup_host((hostname, 0))
            .await
            .map_err(|error| {
                // Keep the resolver's sentence, dropping the " (os error 11001)" code a Technician can't use.
                let message = error.to_string();
                let plain = message.split(" (os error").next().unwrap_or(&message);
                ResolveError {
                    message: plain.to_string(),
                }
            })?;
        let mut addresses = Vec::new();
        for address in found.map(|socket| socket.ip()) {
            if !addresses.contains(&address) {
                addresses.push(address);
            }
        }
        Ok(addresses)
    }

    async fn tcp_connect(&self, address: SocketAddr, timeout: Duration) -> ConnectOutcome {
        let socket = match address {
            SocketAddr::V4(_) => TcpSocket::new_v4(),
            SocketAddr::V6(_) => TcpSocket::new_v6(),
        };
        let Ok(socket) = socket else {
            return ConnectOutcome::Unreachable;
        };
        #[cfg(windows)]
        report_refusal_at_once(&socket);

        let started = Instant::now();
        match tokio::time::timeout(timeout, socket.connect(address)).await {
            Ok(Ok(_stream)) => ConnectOutcome::Accepted {
                connect_time: started.elapsed(),
            },
            Ok(Err(error)) if error.kind() == ErrorKind::ConnectionRefused => {
                ConnectOutcome::Refused
            }
            Ok(Err(_)) => ConnectOutcome::Unreachable,
            Err(_elapsed) => ConnectOutcome::TimedOut,
        }
    }

    fn source_address(&self, destination: IpAddr) -> Option<IpAddr> {
        // Connecting a UDP socket only asks the routing table which local address it would use.
        let unspecified: IpAddr = match destination {
            IpAddr::V4(_) => Ipv4Addr::UNSPECIFIED.into(),
            IpAddr::V6(_) => Ipv6Addr::UNSPECIFIED.into(),
        };
        let socket = UdpSocket::bind((unspecified, 0)).ok()?;
        socket.connect((destination, 9)).ok()?;
        Some(socket.local_addr().ok()?.ip())
    }

    fn hostname(&self) -> String {
        gethostname::gethostname().to_string_lossy().into_owned()
    }

    fn now(&self) -> DateTime<FixedOffset> {
        Local::now().fixed_offset()
    }
}

/// Windows retries a SYN even after the Target refuses it, so a Closed port would take
/// longer than the Port Check timeout to report and look Filtered. Turning off SYN
/// retransmissions makes a refusal fail the connect at once; a silent Target still times out.
/// Best effort: if Windows rejects the option, the connect works as before.
#[cfg(windows)]
fn report_refusal_at_once(socket: &TcpSocket) {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{
        SIO_TCP_INITIAL_RTO, TCP_INITIAL_RTO_PARAMETERS, WSAIoctl,
    };

    // The Windows header defines these as (USHORT)-1 and (UCHAR)-2; windows-sys mistypes the second as u16.
    const UNSPECIFIED_RTT: u16 = u16::MAX;
    const NO_SYN_RETRANSMISSIONS: u8 = 0xFE;

    let parameters = TCP_INITIAL_RTO_PARAMETERS {
        Rtt: UNSPECIFIED_RTT,
        MaxSynRetransmissions: NO_SYN_RETRANSMISSIONS,
    };
    let mut bytes_returned = 0u32;
    // SAFETY: the socket is open for the duration of the call, the input buffer is a live
    // TCP_INITIAL_RTO_PARAMETERS of the stated size, and no output buffer or overlap is used.
    unsafe {
        WSAIoctl(
            socket.as_raw_socket() as usize,
            SIO_TCP_INITIAL_RTO,
            std::ptr::from_ref(&parameters).cast(),
            size_of::<TCP_INITIAL_RTO_PARAMETERS>() as u32,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
            None,
        );
    }
}
