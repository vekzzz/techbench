//! A scripted network adapter (seam S2) so toolbox-core tests are deterministic.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use std::collections::VecDeque;

use toolbox_core::network::{
    Adapter, ConnectOutcome, DnsAnswer, DnsReply, EchoOutcome, EchoRequest, Network, PublicIp,
    RecordType, ResolveError,
};

pub const PC_NAME: &str = "TECH-PC";
pub const PC_ADDRESS: &str = "10.0.0.23";
pub const NOW: &str = "2026-10-13T14:05:09+08:00";

pub fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

pub fn ip(text: &str) -> IpAddr {
    text.parse().unwrap()
}

enum ConnectScript {
    Answer(ConnectOutcome),
    /// Waits out the whole timeout, then times out, like a firewall dropping the SYN.
    Silent,
    /// Never answers, so only stopping the Run ends it.
    Hang,
}

enum ReverseScript {
    Name(String),
    After(Duration, String),
}

enum NameScript {
    Answer(Result<Vec<IpAddr>, String>),
    /// The resolver never answers, so only stopping the Run ends it.
    Hang,
}

/// How the fake answers one ICMP echo.
pub enum EchoScript {
    Answer(EchoOutcome),
    /// Waits out the whole timeout, then times out, like a Target that drops pings.
    Silent,
    /// Never answers, so only stopping the Run ends it.
    Hang,
}

pub struct FakeNetwork {
    names: HashMap<String, NameScript>,
    connects: HashMap<SocketAddr, ConnectScript>,
    connect_timeouts: Mutex<Vec<Duration>>,
    echoes: Mutex<HashMap<IpAddr, VecDeque<EchoScript>>>,
    echo_requests: Mutex<Vec<EchoRequest>>,
    /// Unscripted echoes time out like a silent Target instead of failing the test.
    silent_unless_scripted: bool,
    arp_answers: HashMap<Ipv4Addr, [u8; 6]>,
    arp_requests: Mutex<Vec<Ipv4Addr>>,
    /// Echo scripts for requests that carry a TTL, by address and TTL.
    hops: Mutex<HashMap<(IpAddr, u8), VecDeque<EchoScript>>>,
    reverse_names: HashMap<IpAddr, ReverseScript>,
    dns_answers: HashMap<(SocketAddr, String, &'static str), DnsReply>,
    dns_queries: Mutex<Vec<(SocketAddr, String, RecordType)>>,
    adapters: Vec<Adapter>,
    route_source: Option<IpAddr>,
    /// None: the lookup never answers, so only stopping the Run ends it.
    public_ip: Option<Result<PublicIp, String>>,
}

impl FakeNetwork {
    pub fn new() -> Self {
        Self {
            names: HashMap::new(),
            connects: HashMap::new(),
            connect_timeouts: Mutex::new(Vec::new()),
            echoes: Mutex::new(HashMap::new()),
            echo_requests: Mutex::new(Vec::new()),
            silent_unless_scripted: false,
            arp_answers: HashMap::new(),
            arp_requests: Mutex::new(Vec::new()),
            hops: Mutex::new(HashMap::new()),
            reverse_names: HashMap::new(),
            dns_answers: HashMap::new(),
            dns_queries: Mutex::new(Vec::new()),
            adapters: Vec::new(),
            route_source: Some(ip(PC_ADDRESS)),
            public_ip: Some(Err("no public lookup scripted".to_string())),
        }
    }

    /// The PC's adapters, in the order the operating system lists them.
    pub fn with_adapters(mut self, adapters: Vec<Adapter>) -> Self {
        self.adapters = adapters;
        self
    }

    /// The local address the routing table says traffic leaves from; None for no route at all.
    pub fn with_route_source(mut self, address: Option<&str>) -> Self {
        self.route_source = address.map(ip);
        self
    }

    pub fn with_public_ip(mut self, lookup: Result<PublicIp, String>) -> Self {
        self.public_ip = Some(lookup);
        self
    }

    pub fn with_hanging_public_ip(mut self) -> Self {
        self.public_ip = None;
        self
    }

    /// `address` answers successive echoes as scripted, in order.
    pub fn with_echoes(self, address: &str, scripts: impl IntoIterator<Item = EchoScript>) -> Self {
        self.echoes
            .lock()
            .unwrap()
            .entry(ip(address))
            .or_default()
            .extend(scripts);
        self
    }

    /// Echoes nobody scripted time out, like a Target that drops pings, instead of failing the test.
    pub fn with_silent_echoes_unless_scripted(mut self) -> Self {
        self.silent_unless_scripted = true;
        self
    }

    /// The device at `address` answers ARP with `mac`; any other address does not answer.
    pub fn with_arp(mut self, address: &str, mac: [u8; 6]) -> Self {
        self.arp_answers.insert(address.parse().unwrap(), mac);
        self
    }

    /// Every address an ARP request was sent for, in order.
    pub fn arp_requests(&self) -> Vec<Ipv4Addr> {
        self.arp_requests.lock().unwrap().clone()
    }

    /// Echoes sent to `address` with this `ttl` get these scripted answers, in order.
    pub fn with_hop_echoes(
        self,
        address: &str,
        ttl: u8,
        scripts: impl IntoIterator<Item = EchoScript>,
    ) -> Self {
        self.hops
            .lock()
            .unwrap()
            .entry((ip(address), ttl))
            .or_default()
            .extend(scripts);
        self
    }

    /// The DNS server at `server` answers the query for `name`'s `record_type` records like this,
    /// after `after`.
    pub fn with_dns_answer(
        mut self,
        server: &str,
        name: &str,
        record_type: RecordType,
        after: Duration,
        answer: DnsAnswer,
    ) -> Self {
        let key = (
            server.parse().unwrap(),
            name.to_string(),
            record_type.name(),
        );
        self.dns_answers.insert(
            key,
            DnsReply {
                answer,
                round_trip: after,
            },
        );
        self
    }

    /// Every DNS query the Run sent, in order.
    pub fn dns_queries(&self) -> Vec<(SocketAddr, String, RecordType)> {
        self.dns_queries.lock().unwrap().clone()
    }

    /// A reverse lookup of `address` finds `name`.
    pub fn with_reverse_name(mut self, address: &str, name: &str) -> Self {
        self.reverse_names
            .insert(ip(address), ReverseScript::Name(name.to_string()));
        self
    }

    /// A reverse lookup of `address` takes this long, then finds `name`.
    pub fn with_slow_reverse_name(mut self, address: &str, delay: Duration, name: &str) -> Self {
        self.reverse_names
            .insert(ip(address), ReverseScript::After(delay, name.to_string()));
        self
    }

    /// Every echo request the Run sent, in order.
    pub fn echo_requests(&self) -> Vec<EchoRequest> {
        self.echo_requests.lock().unwrap().clone()
    }

    /// The system resolver answers `name` with these addresses, in this order.
    pub fn with_name(mut self, name: &str, addresses: &[&str]) -> Self {
        let addresses = addresses.iter().map(|text| ip(text)).collect();
        self.names
            .insert(name.to_string(), NameScript::Answer(Ok(addresses)));
        self
    }

    /// The system resolver fails for `name`.
    pub fn with_unresolvable_name(mut self, name: &str, reason: &str) -> Self {
        self.names.insert(
            name.to_string(),
            NameScript::Answer(Err(reason.to_string())),
        );
        self
    }

    pub fn with_hanging_name(mut self, name: &str) -> Self {
        self.names.insert(name.to_string(), NameScript::Hang);
        self
    }

    pub fn with_connect(mut self, address: &str, outcome: ConnectOutcome) -> Self {
        self.connects
            .insert(address.parse().unwrap(), ConnectScript::Answer(outcome));
        self
    }

    /// Every listed port on `address` stays silent until the connect times out.
    pub fn with_silent_ports(
        mut self,
        address: &str,
        ports: impl IntoIterator<Item = u16>,
    ) -> Self {
        for port in ports {
            self.connects
                .insert(SocketAddr::new(ip(address), port), ConnectScript::Silent);
        }
        self
    }

    pub fn with_hanging_connect(mut self, address: &str) -> Self {
        self.connects
            .insert(address.parse().unwrap(), ConnectScript::Hang);
        self
    }

    /// The timeouts every TCP connect was asked to use.
    pub fn connect_timeouts(&self) -> Vec<Duration> {
        self.connect_timeouts.lock().unwrap().clone()
    }
}

impl Network for FakeNetwork {
    async fn resolve(&self, hostname: &str) -> Result<Vec<IpAddr>, ResolveError> {
        match self.names.get(hostname) {
            Some(NameScript::Answer(Ok(addresses))) => Ok(addresses.clone()),
            Some(NameScript::Answer(Err(reason))) => Err(ResolveError {
                message: reason.clone(),
            }),
            Some(NameScript::Hang) => std::future::pending().await,
            None => panic!("test did not script a resolution for {hostname}"),
        }
    }

    async fn tcp_connect(&self, address: SocketAddr, timeout: Duration) -> ConnectOutcome {
        self.connect_timeouts.lock().unwrap().push(timeout);
        match self.connects.get(&address) {
            Some(ConnectScript::Answer(outcome)) => *outcome,
            Some(ConnectScript::Silent) => {
                tokio::time::sleep(timeout).await;
                ConnectOutcome::TimedOut
            }
            Some(ConnectScript::Hang) => std::future::pending().await,
            None => panic!("test did not script a connect to {address}"),
        }
    }

    async fn icmp_echo(&self, request: EchoRequest) -> EchoOutcome {
        self.echo_requests.lock().unwrap().push(request);
        let script = match request.ttl {
            Some(ttl) => self
                .hops
                .lock()
                .unwrap()
                .get_mut(&(request.address, ttl))
                .and_then(VecDeque::pop_front),
            None => self
                .echoes
                .lock()
                .unwrap()
                .get_mut(&request.address)
                .and_then(VecDeque::pop_front),
        };
        match script {
            Some(EchoScript::Answer(outcome)) => outcome,
            Some(EchoScript::Silent) => {
                tokio::time::sleep(request.timeout).await;
                EchoOutcome::TimedOut
            }
            Some(EchoScript::Hang) => std::future::pending().await,
            None if self.silent_unless_scripted => {
                tokio::time::sleep(request.timeout).await;
                EchoOutcome::TimedOut
            }
            None => panic!(
                "test did not script echo {} to {}",
                self.echo_requests().len(),
                request.address
            ),
        }
    }

    async fn dns_query(&self, server: SocketAddr, name: &str, record_type: RecordType) -> DnsReply {
        self.dns_queries
            .lock()
            .unwrap()
            .push((server, name.to_string(), record_type));
        let key = (server, name.to_string(), record_type.name());
        let reply = self
            .dns_answers
            .get(&key)
            .unwrap_or_else(|| panic!("test did not script a DNS answer for {key:?}"))
            .clone();
        tokio::time::sleep(reply.round_trip).await;
        reply
    }

    async fn reverse_lookup(&self, address: IpAddr) -> Option<String> {
        match self.reverse_names.get(&address)? {
            ReverseScript::Name(name) => Some(name.clone()),
            ReverseScript::After(delay, name) => {
                tokio::time::sleep(*delay).await;
                Some(name.clone())
            }
        }
    }

    async fn arp_resolve(&self, address: Ipv4Addr, _timeout: Duration) -> Option<[u8; 6]> {
        self.arp_requests.lock().unwrap().push(address);
        self.arp_answers.get(&address).copied()
    }

    async fn adapters(&self) -> Vec<Adapter> {
        self.adapters.clone()
    }

    async fn public_ip(&self) -> Result<PublicIp, String> {
        match &self.public_ip {
            Some(lookup) => lookup.clone(),
            None => std::future::pending().await,
        }
    }

    fn source_address(&self, _destination: IpAddr) -> Option<IpAddr> {
        self.route_source
    }

    fn hostname(&self) -> String {
        PC_NAME.to_string()
    }

    fn now(&self) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(NOW).unwrap()
    }
}
