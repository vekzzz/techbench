//! The real Speed Test calls: HTTPS to Cloudflare's public speed endpoints (ADR 0003). It blocks,
//! so callers run it off the async runtime. Every request carries an honest User-Agent, never
//! claims Cloudflare's own page as its origin, stays under 100 MB, and posts nothing back.

use std::io::Read;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use ureq::config::IpFamily;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

use crate::network::{SpeedFailure, SpeedProbe, SpeedTestServer};
use crate::target::AddressFamily;

const HOST: &str = "speed.cloudflare.com";
const USER_AGENT: &str = concat!(
    "Techbench/",
    env!("CARGO_PKG_VERSION"),
    " (Windows network diagnostics)"
);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// An address family's slot in the per-family client tables, and the HTTP client's name for it.
fn ip_family(family: AddressFamily) -> (usize, IpFamily) {
    match family {
        AddressFamily::Auto => (0, IpFamily::Any),
        AddressFamily::Ipv4 => (1, IpFamily::Ipv4Only),
        AddressFamily::Ipv6 => (2, IpFamily::Ipv6Only),
    }
}

/// One HTTP client per address family, kept for the life of the app so a probe reuses the
/// connection the one before it opened, as a browser's speed test does.
fn agent(family: AddressFamily) -> &'static ureq::Agent {
    static AGENTS: [OnceLock<ureq::Agent>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let (slot, ip_family) = ip_family(family);
    AGENTS[slot].get_or_init(|| {
        ureq::Agent::config_builder()
            .tls_config(
                TlsConfig::builder()
                    .provider(TlsProvider::NativeTls)
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .ip_family(ip_family)
            .user_agent(USER_AGENT)
            .http_status_as_error(false)
            .timeout_connect(Some(PROBE_TIMEOUT))
            .timeout_global(Some(PROBE_TIMEOUT))
            .build()
            .into()
    })
}

/// Like [`agent`], for transfers that run for seconds: a stalled read gives up, but a long one does not.
fn transfer_agent(family: AddressFamily) -> &'static ureq::Agent {
    static AGENTS: [OnceLock<ureq::Agent>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let (slot, ip_family) = ip_family(family);
    AGENTS[slot].get_or_init(|| {
        ureq::Agent::config_builder()
            .tls_config(
                TlsConfig::builder()
                    .provider(TlsProvider::NativeTls)
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .ip_family(ip_family)
            .user_agent(USER_AGENT)
            .http_status_as_error(false)
            .timeout_connect(Some(PROBE_TIMEOUT))
            .timeout_recv_response(Some(PROBE_TIMEOUT))
            .timeout_recv_body(Some(PROBE_TIMEOUT))
            .build()
            .into()
    })
}

/// One empty request: how long Cloudflare took to start answering, less the time it says it spent
/// itself, and which of its locations answered.
pub(crate) fn probe(family: AddressFamily) -> Result<SpeedProbe, SpeedFailure> {
    let started = Instant::now();
    let mut response = agent(family)
        .get(format!("https://{HOST}/__down?bytes=0"))
        .call()
        .map_err(failure)?;
    let round_trip = started.elapsed();
    check_status(&response)?;

    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    };
    let server_time = header("server-timing")
        .and_then(first_duration)
        .unwrap_or(Duration::ZERO);
    let location = header("colo")
        .or_else(|| header("cf-meta-colo"))
        .map(str::to_string)
        .or_else(|| {
            // "8f1c…-HKG": the last part of CF-RAY is the location.
            header("cf-ray")
                .and_then(|ray| ray.rsplit('-').next())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string());
    // Nothing to read in the body, but drain it so the connection goes back to the pool.
    let _ = response.body_mut().read_to_vec();

    Ok(SpeedProbe {
        round_trip,
        server_time,
        server: SpeedTestServer {
            provider: "Cloudflare".to_string(),
            location,
            address: server_address(family),
        },
    })
}

/// Turns a non-success HTTP status into the failure the Technician should see.
fn check_status(response: &ureq::http::Response<ureq::Body>) -> Result<(), SpeedFailure> {
    let status = response.status();
    if status.as_u16() == 429 {
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|seconds| seconds.trim().parse().ok())
            .map(Duration::from_secs);
        return Err(SpeedFailure::RateLimited { retry_after });
    }
    if !status.is_success() {
        return Err(SpeedFailure::Interrupted {
            reason: format!("{HOST} answered HTTP {status}"),
        });
    }
    Ok(())
}

/// Where a request that never got an answer failed: reaching the server, or part-way through.
fn failure(error: ureq::Error) -> SpeedFailure {
    match error {
        ureq::Error::Timeout(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => {
            SpeedFailure::Unreachable
        }
        ureq::Error::Io(ref io)
            if matches!(
                io.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::NetworkUnreachable
                    | std::io::ErrorKind::HostUnreachable
                    | std::io::ErrorKind::AddrNotAvailable
            ) =>
        {
            SpeedFailure::Unreachable
        }
        other => SpeedFailure::Interrupted {
            reason: other.to_string(),
        },
    }
}

/// The first `dur=` figure (milliseconds) in a `Server-Timing` header, as Cloudflare's own speed
/// test reads it.
fn first_duration(server_timing: &str) -> Option<Duration> {
    let after = server_timing.split("dur=").nth(1)?;
    let digits: String = after
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    let millis: f64 = digits.parse().ok()?;
    Some(Duration::from_secs_f64(millis / 1000.0))
}

/// The address Cloudflare's speed host resolves to for `family`: what the HTTP client will
/// connect to, since it does not report the address it used.
fn server_address(family: AddressFamily) -> Option<IpAddr> {
    let mut addresses = (HOST, 443)
        .to_socket_addrs()
        .ok()?
        .map(|socket| socket.ip());
    match family {
        AddressFamily::Auto => addresses.next(),
        AddressFamily::Ipv4 => addresses.find(IpAddr::is_ipv4),
        AddressFamily::Ipv6 => addresses.find(IpAddr::is_ipv6),
    }
}

/// Sets the flag when dropped, which tells a blocking transfer nobody is waiting for it any more.
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Downloads `bytes` and calls `received` with the size of each piece as it arrives. The read
/// loop runs on a blocking thread and sends piece sizes over a channel; dropping this future
/// stops the thread at its next piece.
pub(crate) async fn download(
    family: AddressFamily,
    bytes: u64,
    received: &(dyn Fn(u64) + Send + Sync),
) -> Result<(), SpeedFailure> {
    let (sender, mut pieces) = mpsc::unbounded_channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel_when_dropped = CancelOnDrop(Arc::clone(&cancelled));
    let worker = tokio::task::spawn_blocking(move || {
        read_download(family, bytes, &cancelled, |piece| {
            let _ = sender.send(piece);
        })
    });
    while let Some(piece) = pieces.recv().await {
        received(piece);
    }
    worker.await.unwrap_or_else(|_| {
        Err(SpeedFailure::Interrupted {
            reason: "the download failed unexpectedly".to_string(),
        })
    })
}

fn read_download(
    family: AddressFamily,
    bytes: u64,
    cancelled: &AtomicBool,
    mut received: impl FnMut(u64),
) -> Result<(), SpeedFailure> {
    let mut response = transfer_agent(family)
        .get(format!("https://{HOST}/__down?bytes={bytes}"))
        .call()
        .map_err(failure)?;
    check_status(&response)?;
    let mut body = response.body_mut().as_reader();
    let mut buffer = [0u8; 64 * 1024];
    while !cancelled.load(Ordering::Relaxed) {
        match body.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(read) => received(read as u64),
            Err(error) => {
                return Err(SpeedFailure::Interrupted {
                    reason: error.to_string(),
                });
            }
        }
    }
    Ok(())
}

/// Uploads `bytes` of zeros and calls `sent` with the size of each piece as it is handed to the
/// connection. Like [`download`], the blocking work runs on its own thread.
pub(crate) async fn upload(
    family: AddressFamily,
    bytes: u64,
    sent: &(dyn Fn(u64) + Send + Sync),
) -> Result<(), SpeedFailure> {
    let (sender, mut pieces) = mpsc::unbounded_channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel_when_dropped = CancelOnDrop(Arc::clone(&cancelled));
    let worker = tokio::task::spawn_blocking(move || {
        send_upload(family, bytes, &cancelled, |piece| {
            let _ = sender.send(piece);
        })
    });
    while let Some(piece) = pieces.recv().await {
        sent(piece);
    }
    worker.await.unwrap_or_else(|_| {
        Err(SpeedFailure::Interrupted {
            reason: "the upload failed unexpectedly".to_string(),
        })
    })
}

/// A body of `left` zero bytes that reports what it hands over and stops early when cancelled.
struct ZeroBody<'a, F: FnMut(u64)> {
    left: u64,
    cancelled: &'a AtomicBool,
    sent: F,
}

impl<F: FnMut(u64)> Read for ZeroBody<'_, F> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("upload abandoned"));
        }
        let count = (buffer.len() as u64).min(self.left) as usize;
        buffer[..count].fill(0);
        self.left -= count as u64;
        if count > 0 {
            (self.sent)(count as u64);
        }
        Ok(count)
    }
}

fn send_upload(
    family: AddressFamily,
    bytes: u64,
    cancelled: &AtomicBool,
    sent: impl FnMut(u64),
) -> Result<(), SpeedFailure> {
    let mut body = ZeroBody {
        left: bytes,
        cancelled,
        sent,
    };
    let response = transfer_agent(family)
        .post(format!("https://{HOST}/__up"))
        .header("Content-Length", bytes.to_string())
        .send(ureq::SendBody::from_reader(&mut body));
    if cancelled.load(Ordering::Relaxed) {
        return Ok(());
    }
    check_status(&response.map_err(failure)?)
}
