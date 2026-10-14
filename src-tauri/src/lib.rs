//! The Tauri shell: exposes the toolbox core to the UI. It holds no Tool logic of its own.

mod continuous_ping;
mod dns_lookup;
mod ip_info;
mod ping;
mod port_check;
mod runs;
mod subnet;
mod traceroute;

use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(runs::Runs::default()))
        .invoke_handler(tauri::generate_handler![
            subnet::calculate_subnet,
            subnet::subnet_contains,
            continuous_ping::default_gateway,
            continuous_ping::start_continuous_ping,
            dns_lookup::default_dns_server,
            dns_lookup::start_dns_lookup,
            ip_info::start_ip_info,
            ping::ping_defaults,
            ping::start_ping,
            port_check::port_presets,
            port_check::start_port_check,
            traceroute::start_traceroute,
            runs::run_text,
            runs::stop_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
