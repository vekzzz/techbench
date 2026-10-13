//! A scripted network adapter (seam S2) so toolbox-core tests are deterministic.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use toolbox_core::network::{ConnectOutcome, Network, ResolveError};

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

pub struct FakeNetwork {
    names: HashMap<String, NameScript>,
    connects: HashMap<SocketAddr, ConnectScript>,
    connect_timeouts: Mutex<Vec<Duration>>,
}

impl FakeNetwork {
    pub fn new() -> Self {
        Self {
            names: HashMap::new(),
            connects: HashMap::new(),
            connect_timeouts: Mutex::new(Vec::new()),
        }
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

    /// Every listed port on `host` stays silent until the connect times out.
    pub fn with_silent_ports(mut self, host: &str, ports: impl IntoIterator<Item = u16>) -> Self {
        for port in ports {
            self.connects
                .insert(SocketAddr::new(ip(host), port), ConnectScript::Silent);
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

    fn source_address(&self, _destination: IpAddr) -> Option<IpAddr> {
        Some(ip(PC_ADDRESS))
    }

    fn hostname(&self) -> String {
        PC_NAME.to_string()
    }

    fn now(&self) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(NOW).unwrap()
    }
}
