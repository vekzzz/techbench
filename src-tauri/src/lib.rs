//! The Tauri shell: exposes the toolbox core to the UI. It holds no Tool logic of its own.

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
            ping::ping_defaults,
            ping::start_ping,
            port_check::port_presets,
            port_check::start_port_check,
            runs::stop_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
