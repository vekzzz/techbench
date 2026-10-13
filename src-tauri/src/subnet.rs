//! Subnet Calculator commands.

use serde::Serialize;
use toolbox_core::subnet_calculator::{self, SubnetReport};

/// A Subnet Calculator report as the UI receives it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubnetView {
    network: String,
    prefix: u8,
    broadcast: Option<String>,
    first_usable: String,
    last_usable: String,
    usable_count: String,
    mask: Option<String>,
    wildcard: Option<String>,
}

impl From<SubnetReport> for SubnetView {
    fn from(report: SubnetReport) -> Self {
        Self {
            network: report.network.to_string(),
            prefix: report.prefix,
            broadcast: report.broadcast.map(|address| address.to_string()),
            first_usable: report.first_usable.to_string(),
            last_usable: report.last_usable.to_string(),
            usable_count: report.usable_count,
            mask: report.mask.map(|address| address.to_string()),
            wildcard: report.wildcard.map(|address| address.to_string()),
        }
    }
}

#[tauri::command]
pub fn calculate_subnet(input: &str) -> Result<SubnetView, String> {
    subnet_calculator::calculate(input)
        .map(SubnetView::from)
        .map_err(|error| error.message)
}

#[tauri::command]
pub fn subnet_contains(input: &str, address: &str) -> Result<bool, String> {
    let report = subnet_calculator::calculate(input).map_err(|error| error.message)?;
    report.contains(address).map_err(|error| error.message)
}
