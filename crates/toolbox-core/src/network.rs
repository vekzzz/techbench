//! The network adapter (test seam S2): the only way the toolbox core touches the operating system.
//! [`SystemNetwork`] is the real one; tests substitute a scripted fake.

use std::future::Future;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use chrono::{DateTime, FixedOffset, Local};
use tokio::net::TcpSocket;

use crate::target::AddressFamily;

/// What happened when a TCP connection was attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectOutcome {
    Accepted { connect_time: Duration },
    Refused,
    TimedOut,
    Unreachable,
}

/// One ICMP echo request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EchoRequest {
    /// The Resolved Address the echo is sent to.
    pub address: IpAddr,
    /// Bytes of data carried by the echo.
    pub payload_size: u16,
    /// Forbids routers from fragmenting the packet, so an oversized one is refused instead.
    pub dont_fragment: bool,
    /// The TTL (IPv6: hop limit) the echo is sent with. A router that finds it spent answers
    /// with [`EchoOutcome::TimeExceeded`]: how Traceroute finds each Hop. None: the system's usual.
    pub ttl: Option<u8>,
    pub timeout: Duration,
}

/// What happened to one ICMP echo request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EchoOutcome {
    /// The Target answered. IPv6 replies carry no TTL.
    Reply {
        round_trip: Duration,
        ttl: Option<u8>,
    },
    /// No answer before the timeout.
    TimedOut,
    /// A router (`from`) found the echo's TTL spent before it reached the Target, `round_trip` after it was sent.
    TimeExceeded {
        from: Option<IpAddr>,
        round_trip: Duration,
    },
    /// A router (`from`) or the local stack reported the destination unreachable.
    Unreachable { from: Option<IpAddr> },
    /// The packet was too big for a link on the path and Don't Fragment forbade splitting it.
    FragmentationNeeded,
}

/// What kind of link a network adapter is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    Ethernet,
    Wireless,
    Loopback,
    Tunnel,
    Other,
}

/// An address assigned to a network adapter, with its on-link prefix length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterAddress {
    pub address: IpAddr,
    pub prefix: u8,
}

/// The DHCP lease of an adapter that has DHCP enabled; any part may be unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DhcpLease {
    pub server: Option<IpAddr>,
    pub obtained: Option<DateTime<FixedOffset>>,
    pub expires: Option<DateTime<FixedOffset>>,
}

/// One of this PC's network adapters, as the operating system reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adapter {
    /// The name Windows shows, such as "Ethernet" or "Wi-Fi".
    pub name: String,
    /// The hardware or driver description.
    pub description: String,
    pub kind: AdapterKind,
    pub up: bool,
    pub mac: Option<[u8; 6]>,
    pub addresses: Vec<AdapterAddress>,
    pub gateways: Vec<IpAddr>,
    pub dns_servers: Vec<IpAddr>,
    /// Present when DHCP is enabled.
    pub dhcp: Option<DhcpLease>,
    /// Route metrics: when several adapters have a gateway, the lowest metric carries the default route.
    pub ipv4_metric: u32,
    pub ipv6_metric: u32,
}

/// This PC's address as the internet sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicIp {
    pub address: IpAddr,
    pub isp: Option<String>,
    pub city: Option<String>,
}

/// The DNS record types DNS Lookup can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordType {
    A,
    Aaaa,
    Cname,
    Mx,
    Txt,
    Ns,
    Ptr,
    Soa,
}

impl RecordType {
    /// Every type, in the order DNS Lookup offers them.
    pub const ALL: [RecordType; 8] = [
        RecordType::A,
        RecordType::Aaaa,
        RecordType::Cname,
        RecordType::Mx,
        RecordType::Txt,
        RecordType::Ns,
        RecordType::Ptr,
        RecordType::Soa,
    ];

    /// The type as DNS tools write it: `A`, `AAAA`, `CNAME` and so on.
    pub fn name(self) -> &'static str {
        match self {
            RecordType::A => "A",
            RecordType::Aaaa => "AAAA",
            RecordType::Cname => "CNAME",
            RecordType::Mx => "MX",
            RecordType::Txt => "TXT",
            RecordType::Ns => "NS",
            RecordType::Ptr => "PTR",
            RecordType::Soa => "SOA",
        }
    }
}

/// One DNS record, its value already written the way `nslookup` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsRecord {
    pub name: String,
    pub record_type: RecordType,
    /// Seconds the record may be cached.
    pub ttl: u32,
    /// For example `93.184.216.34`, `10 mail.example.com` (MX) or `"v=spf1 -all"` (TXT).
    pub value: String,
}

/// What a DNS server said to a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsAnswer {
    Records(Vec<DnsRecord>),
    /// NXDOMAIN: the name does not exist at all.
    NameDoesNotExist,
    /// The name exists, but has no records of the type asked for.
    NoRecords,
    /// The server answered with an error code: 2 is SERVFAIL, 5 is REFUSED.
    ServerError {
        code: u8,
    },
    /// The server's answer could not be read.
    BadResponse,
    /// Nothing came back before the timeout.
    NoResponse,
}

/// A DNS server's answer and how long it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsReply {
    pub answer: DnsAnswer,
    pub round_trip: Duration,
}

/// Why the system resolver could not turn a hostname into addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError {
    pub message: String,
}

/// What Windows says the internet connection costs to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionCost {
    Unmetered,
    /// A data cap or per-use charge applies, such as a phone hotspot or a capped mobile line.
    Metered,
    Roaming,
    /// The data limit the Technician set in Windows has been passed.
    OverLimit,
    /// Windows could not say, for instance because there is no connection.
    Unknown,
}

/// Why Ookla's `speedtest.exe` could not produce a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OoklaFailure {
    /// The Technician has not installed Ookla's program.
    NotInstalled,
    /// It ran and failed, or could not start; `message` is what it said, such as a licence prompt.
    Failed { message: String },
}

/// The public server a Speed Test measures against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedTestServer {
    /// Who runs the server, such as `Cloudflare`.
    pub provider: String,
    /// Where the server is, as its provider names it: Cloudflare's three-letter code such as `HKG`.
    pub location: String,
    /// The address the requests went to, when the system could say.
    pub address: Option<IpAddr>,
}

/// One empty request to a Speed Test Server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedProbe {
    /// From sending the request until the answer began to arrive.
    pub round_trip: Duration,
    /// How much of the round trip the server says it spent itself.
    pub server_time: Duration,
    pub server: SpeedTestServer,
}

/// Why a request to a Speed Test Server did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeedFailure {
    /// The server could not be reached at all: no route, no DNS, no connection, no answer in time.
    Unreachable,
    /// The server asked the Technician to wait.
    RateLimited { retry_after: Option<Duration> },
    /// The connection was made but broke or was refused part-way.
    Interrupted { reason: String },
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

    /// Sends one ICMP echo request and waits for its outcome.
    fn icmp_echo(&self, request: EchoRequest) -> impl Future<Output = EchoOutcome> + Send;

    /// Asks the DNS server at `server` for `name`'s `record_type` records, directly: the hosts
    /// file and the system's resolver are not consulted.
    fn dns_query(
        &self,
        server: SocketAddr,
        name: &str,
        record_type: RecordType,
    ) -> impl Future<Output = DnsReply> + Send;

    /// The name the system's DNS gives `address` (a reverse lookup), if any.
    fn reverse_lookup(&self, address: IpAddr) -> impl Future<Output = Option<String>> + Send;

    /// The hardware (MAC) address of the device on this PC's own network that holds `address`,
    /// found by an ARP request, or None when nothing answered within `timeout`. Works only for
    /// addresses on a subnet this PC is directly connected to.
    fn arp_resolve(
        &self,
        address: Ipv4Addr,
        timeout: Duration,
    ) -> impl Future<Output = Option<[u8; 6]>> + Send;

    /// The MAC this PC's neighbour (ARP) cache holds for `address`, if any. Sends nothing, so it
    /// says nothing about whether the device is there now: an entry can outlive the device.
    fn cached_mac(&self, address: Ipv4Addr) -> impl Future<Output = Option<[u8; 6]>> + Send;

    /// This PC's network adapters.
    fn adapters(&self) -> impl Future<Output = Vec<Adapter>> + Send;

    /// This PC's public address, ISP and city, from one HTTPS call to an outside service.
    /// Fails with a reason the Technician can read. Outside calls are made only by Tools whose
    /// subject is the internet itself: this one and the Speed Test operations below.
    fn public_ip(&self) -> impl Future<Output = Result<PublicIp, String>> + Send;

    /// Sends one empty request to the Speed Test Server over `family` and reports how long it
    /// took. Successive probes reuse the connection the first opened.
    fn speed_probe(
        &self,
        family: AddressFamily,
    ) -> impl Future<Output = Result<SpeedProbe, SpeedFailure>> + Send;

    /// Downloads `bytes` (at most 25 MB) from the Speed Test Server over `family`, calling
    /// `received` with the size of each piece as it arrives. Ends when everything has arrived or
    /// the request fails; dropping the future abandons the transfer.
    fn speed_download(
        &self,
        family: AddressFamily,
        bytes: u64,
        received: &(dyn Fn(u64) + Send + Sync),
    ) -> impl Future<Output = Result<(), SpeedFailure>> + Send;

    /// Uploads `bytes` (at most 10 MB) to the Speed Test Server over `family`, calling `sent`
    /// with the size of each piece as it leaves. Ends when everything has been sent and the
    /// server has answered, or the request fails; dropping the future abandons the transfer.
    fn speed_upload(
        &self,
        family: AddressFamily,
        bytes: u64,
        sent: &(dyn Fn(u64) + Send + Sync),
    ) -> impl Future<Output = Result<(), SpeedFailure>> + Send;

    /// Whether Ookla's own `speedtest.exe` is installed. Sends nothing.
    fn ookla_installed(&self) -> impl Future<Output = bool> + Send;

    /// Runs Ookla's `speedtest.exe`, calling `on_line` with each JSON line it prints (`-f jsonl`),
    /// until it exits. Never accepts Ookla's licence for the Technician. Dropping the future
    /// ends the program.
    fn ookla_run(
        &self,
        on_line: &(dyn Fn(String) + Send + Sync),
    ) -> impl Future<Output = Result<(), OoklaFailure>> + Send;

    /// What Windows says the connection internet traffic uses costs. Sends nothing.
    fn connection_cost(&self) -> impl Future<Output = ConnectionCost> + Send;

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
        limit_syn_retransmissions(&socket);

        let started = Instant::now();
        match tokio::time::timeout(timeout, socket.connect(address)).await {
            Ok(Ok(_stream)) => ConnectOutcome::Accepted {
                connect_time: started.elapsed(),
            },
            Ok(Err(error)) => match error.kind() {
                ErrorKind::ConnectionRefused => ConnectOutcome::Refused,
                ErrorKind::TimedOut => ConnectOutcome::TimedOut,
                _ => ConnectOutcome::Unreachable,
            },
            Err(_elapsed) => ConnectOutcome::TimedOut,
        }
    }

    async fn icmp_echo(&self, request: EchoRequest) -> EchoOutcome {
        #[cfg(windows)]
        {
            tokio::task::spawn_blocking(move || crate::windows_icmp::echo(request))
                .await
                .unwrap_or(EchoOutcome::Unreachable { from: None })
        }
        #[cfg(not(windows))]
        {
            let _ = request;
            unimplemented!("ICMP echo is implemented for Windows only (ADR 0002)")
        }
    }

    async fn dns_query(&self, server: SocketAddr, name: &str, record_type: RecordType) -> DnsReply {
        crate::dns_client::query(server, name, record_type).await
    }

    async fn reverse_lookup(&self, address: IpAddr) -> Option<String> {
        #[cfg(windows)]
        {
            tokio::task::spawn_blocking(move || crate::windows_names::reverse_lookup(address))
                .await
                .ok()
                .flatten()
        }
        #[cfg(not(windows))]
        {
            let _ = address;
            unimplemented!("reverse lookup is implemented for Windows only (ADR 0002)")
        }
    }

    async fn arp_resolve(&self, address: Ipv4Addr, timeout: Duration) -> Option<[u8; 6]> {
        #[cfg(windows)]
        {
            // The ARP call cannot be given a timeout, so stop waiting on it; the blocking call
            // finishes on its own thread shortly after.
            let resolving =
                tokio::task::spawn_blocking(move || crate::windows_arp::resolve(address));
            tokio::time::timeout(timeout, resolving)
                .await
                .ok()
                .and_then(Result::ok)
                .flatten()
        }
        #[cfg(not(windows))]
        {
            let _ = (address, timeout);
            unimplemented!("ARP resolution is implemented for Windows only (ADR 0002)")
        }
    }

    async fn cached_mac(&self, address: Ipv4Addr) -> Option<[u8; 6]> {
        #[cfg(windows)]
        {
            tokio::task::spawn_blocking(move || crate::windows_arp::cached(address))
                .await
                .ok()
                .flatten()
        }
        #[cfg(not(windows))]
        {
            let _ = address;
            unimplemented!("the neighbour cache is read on Windows only (ADR 0002)")
        }
    }

    async fn adapters(&self) -> Vec<Adapter> {
        #[cfg(windows)]
        {
            tokio::task::spawn_blocking(crate::windows_adapters::adapters)
                .await
                .unwrap_or_default()
        }
        #[cfg(not(windows))]
        unimplemented!("adapter enumeration is implemented for Windows only (ADR 0002)")
    }

    async fn public_ip(&self) -> Result<PublicIp, String> {
        tokio::task::spawn_blocking(crate::public_ip::look_up)
            .await
            .unwrap_or_else(|_| Err("the public lookup failed unexpectedly".to_string()))
    }

    async fn speed_probe(&self, family: AddressFamily) -> Result<SpeedProbe, SpeedFailure> {
        tokio::task::spawn_blocking(move || crate::speed_client::probe(family))
            .await
            .unwrap_or_else(|_| {
                Err(SpeedFailure::Interrupted {
                    reason: "the speed probe failed unexpectedly".to_string(),
                })
            })
    }

    async fn speed_download(
        &self,
        family: AddressFamily,
        bytes: u64,
        received: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), SpeedFailure> {
        crate::speed_client::download(family, bytes, received).await
    }

    async fn speed_upload(
        &self,
        family: AddressFamily,
        bytes: u64,
        sent: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), SpeedFailure> {
        crate::speed_client::upload(family, bytes, sent).await
    }

    async fn ookla_installed(&self) -> bool {
        crate::ookla_client::installed().await
    }

    async fn ookla_run(
        &self,
        on_line: &(dyn Fn(String) + Send + Sync),
    ) -> Result<(), OoklaFailure> {
        crate::ookla_client::run(on_line).await
    }

    async fn connection_cost(&self) -> ConnectionCost {
        #[cfg(windows)]
        {
            tokio::task::spawn_blocking(crate::windows_cost::cost)
                .await
                .unwrap_or(ConnectionCost::Unknown)
        }
        #[cfg(not(windows))]
        unimplemented!("the connection cost is read on Windows only (ADR 0002)")
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

/// Windows retries a SYN even after the Target refuses it: with its default two retries a
/// refusal takes about 2 s to surface, so a Closed port would hit the Port Check timeout and
/// look Filtered. Allowing one retransmission makes a refusal surface in about half a second,
/// while a silent Target still gets its SYN resent once before the timeout. (No retransmissions
/// would be faster still, but Windows then abandons a silent Target at its 1 s initial RTO,
/// cutting the timeout short and never resending a lost SYN.)
/// Best effort: if Windows rejects the option, the connect works as before.
#[cfg(windows)]
fn limit_syn_retransmissions(socket: &TcpSocket) {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{
        SIO_TCP_INITIAL_RTO, TCP_INITIAL_RTO_PARAMETERS, WSAIoctl,
    };

    // The Windows header defines this as (USHORT)-1: keep the system's initial RTO.
    const UNSPECIFIED_RTT: u16 = u16::MAX;
    const ONE_SYN_RETRANSMISSION: u8 = 1;

    let parameters = TCP_INITIAL_RTO_PARAMETERS {
        Rtt: UNSPECIFIED_RTT,
        MaxSynRetransmissions: ONE_SYN_RETRANSMISSION,
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
