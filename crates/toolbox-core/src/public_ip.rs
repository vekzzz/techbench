//! The public IP lookup: one HTTPS call to ipinfo.io, the app's only external call. It uses
//! Windows' own TLS and certificate store, and blocks, so callers run it off the async runtime.

use std::time::Duration;

use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

use crate::network::PublicIp;

const SERVICE: &str = "https://ipinfo.io/json";
const TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn look_up() -> Result<PublicIp, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .timeout_global(Some(TIMEOUT))
        .build()
        .into();

    let body = agent
        .get(SERVICE)
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .map_err(|error| match error {
            ureq::Error::Timeout(_) => {
                format!("ipinfo.io did not answer within {} s", TIMEOUT.as_secs())
            }
            other => format!("could not reach ipinfo.io ({other})"),
        })?;
    let answer: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| "ipinfo.io sent an answer that could not be read".to_string())?;

    let address = answer["ip"]
        .as_str()
        .and_then(|text| text.parse().ok())
        .ok_or_else(|| "ipinfo.io sent no address".to_string())?;
    let text = |field: &str| answer[field].as_str().filter(|value| !value.is_empty());
    Ok(PublicIp {
        address,
        // ipinfo.io names the ISP by its network, as in "AS9299 Example Telecom".
        isp: text("org").map(|org| match org.split_once(' ') {
            Some((number, name)) if number.starts_with("AS") => name.to_string(),
            _ => org.to_string(),
        }),
        city: text("city").map(str::to_string),
    })
}
