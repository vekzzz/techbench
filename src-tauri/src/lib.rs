//! The Tauri shell: exposes the toolbox core to the UI. It holds no Tool logic of its own.

mod continuous_ping;
mod ip_info;
mod ping;
mod port_check;
mod runs;
mod subnet;

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
            ip_info::start_ip_info,
            ping::ping_defaults,
            ping::start_ping,
            port_check::port_presets,
            port_check::start_port_check,
            runs::run_text,
            runs::stop_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
