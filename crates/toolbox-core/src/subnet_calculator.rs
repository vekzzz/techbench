//! Subnet Calculator: derives network, broadcast, mask and usable range from an address and prefix.
//! It sends no traffic.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub use crate::InputError;

/// Everything the Subnet Calculator reports for one subnet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubnetReport {
    pub network: IpAddr,
    pub prefix: u8,
    pub broadcast: Option<IpAddr>,
    pub first_usable: IpAddr,
    pub last_usable: IpAddr,
    pub usable_count: String,
    pub mask: Option<IpAddr>,
    pub wildcard: Option<IpAddr>,
}

impl SubnetReport {
    /// Whether `address` falls inside this subnet.
    pub fn contains(&self, address: &str) -> Result<bool, InputError> {
        let address = address
            .trim()
            .parse::<IpAddr>()
            .map_err(|_| error("not an IP address"))?;
        match (self.network, address) {
            (IpAddr::V4(network), IpAddr::V4(address)) => {
                let mask = u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0);
                Ok(u32::from(address) & mask == u32::from(network))
            }
            (IpAddr::V6(network), IpAddr::V6(address)) => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.prefix))
                    .unwrap_or(0);
                Ok(u128::from(address) & mask == u128::from(network))
            }
            _ => Err(error(
                "the address and the subnet are different IP versions",
            )),
        }
    }
}

/// Calculates the subnet described by what the Technician typed.
pub fn calculate(input: &str) -> Result<SubnetReport, InputError> {
    let input = input.trim();
    if let Some((address, prefix)) = input.split_once('/') {
        let prefix = prefix
            .trim()
            .parse::<u8>()
            .map_err(|_| error("not a prefix length"))?;
        match address
            .trim()
            .parse::<IpAddr>()
            .map_err(|_| error("not an IP address"))?
        {
            IpAddr::V4(address) => ipv4(address, prefix),
            IpAddr::V6(address) => ipv6(address, prefix),
        }
    } else if let Some((address, mask)) = input.split_once(char::is_whitespace) {
        let address = address
            .parse::<Ipv4Addr>()
            .map_err(|_| error("a dotted mask needs an IPv4 address"))?;
        let mask = u32::from(
            mask.trim()
                .parse::<Ipv4Addr>()
                .map_err(|_| error("not a subnet mask"))?,
        );
        // A valid mask is a run of ones followed only by zeros.
        if mask.leading_ones() + mask.trailing_zeros() != 32 {
            return Err(error("a subnet mask's ones must be contiguous"));
        }
        ipv4(address, mask.leading_ones() as u8)
    } else {
        Err(error("expected address/prefix or address and mask"))
    }
}

fn ipv4(address: Ipv4Addr, prefix: u8) -> Result<SubnetReport, InputError> {
    if prefix > 32 {
        return Err(error("an IPv4 prefix must be between 0 and 32"));
    }

    let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
    let network = u32::from(address) & mask;
    let last = network | !mask;

    // A /31 is a point-to-point link (RFC 3021) and a /32 is a single host:
    // every address is usable and there is no broadcast.
    let (broadcast, first_usable, last_usable) = if prefix >= 31 {
        (None, network, last)
    } else {
        (Some(v4(last)), network + 1, last - 1)
    };

    Ok(SubnetReport {
        network: v4(network),
        prefix,
        broadcast,
        first_usable: v4(first_usable),
        last_usable: v4(last_usable),
        usable_count: (last_usable - first_usable + 1).to_string(),
        mask: Some(v4(mask)),
        wildcard: Some(v4(!mask)),
    })
}

/// IPv6 has no broadcast and no reserved addresses, so the whole prefix is reported.
fn ipv6(address: Ipv6Addr, prefix: u8) -> Result<SubnetReport, InputError> {
    if prefix > 128 {
        return Err(error("an IPv6 prefix must be between 0 and 128"));
    }

    let mask = u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0);
    let network = u128::from(address) & mask;
    let last = network | !mask;

    Ok(SubnetReport {
        network: v6(network),
        prefix,
        broadcast: None,
        first_usable: v6(network),
        last_usable: v6(last),
        // 2^128 (a /0) is one more than u128 can hold, so it is spelled out.
        usable_count: (!mask).checked_add(1).map_or_else(
            || "340282366920938463463374607431768211456".to_string(),
            |count| count.to_string(),
        ),
        mask: None,
        wildcard: None,
    })
}

fn v4(bits: u32) -> IpAddr {
    IpAddr::V4(Ipv4Addr::from(bits))
}

fn v6(bits: u128) -> IpAddr {
    IpAddr::V6(Ipv6Addr::from(bits))
}

fn error(message: &str) -> InputError {
    InputError::new(message)
}
