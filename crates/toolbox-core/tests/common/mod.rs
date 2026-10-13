//! A scripted network adapter (seam S2) so toolbox-core tests are deterministic.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use std::collections::VecDeque;

use toolbox_core::network::{
    Adapter, ConnectOutcome, EchoOutcome, EchoRequest, Network, PublicIp, ResolveError,
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
        let script = self
            .echoes
            .lock()
            .unwrap()
            .get_mut(&request.address)
            .and_then(VecDeque::pop_front);
        match script {
            Some(EchoScript::Answer(outcome)) => outcome,
            Some(EchoScript::Silent) => {
                tokio::time::sleep(request.timeout).await;
                EchoOutcome::TimedOut
            }
            Some(EchoScript::Hang) => std::future::pending().await,
            None => panic!(
                "test did not script echo {} to {}",
                self.echo_requests().len(),
                request.address
            ),
        }
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
