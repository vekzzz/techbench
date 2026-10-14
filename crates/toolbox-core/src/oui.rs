//! Vendor lookup from a MAC address, using a bundled copy of the IEEE OUI registry (the MA-L
//! assignments), so it works offline. `scripts/update-oui.py` rebuilds the data.

use std::sync::OnceLock;

/// One `PREFIX<TAB>Vendor name` line per assignment, sorted by prefix.
const REGISTRY: &str = include_str!("../data/oui.tsv");

/// The vendor the registry assigns the first three bytes of `mac` to, if it names one.
pub fn vendor(mac: [u8; 6]) -> Option<&'static str> {
    static TABLE: OnceLock<Vec<(u32, &'static str)>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        REGISTRY
            .lines()
            .filter_map(|line| {
                let (prefix, name) = line.split_once('\t')?;
                Some((u32::from_str_radix(prefix, 16).ok()?, name))
            })
            .collect()
    });
    let prefix = u32::from_be_bytes([0, mac[0], mac[1], mac[2]]);
    let at = table
        .binary_search_by_key(&prefix, |(known, _)| *known)
        .ok()?;
    Some(table[at].1)
}
