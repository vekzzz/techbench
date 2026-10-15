//! The Tauri shell: exposes the toolbox core to the UI. It holds no Tool logic of its own.

mod continuous_ping;
mod dns_lookup;
mod ip_info;
mod lan_scan;
mod ping;
mod port_check;
mod runs;
mod saved_targets;
mod speed_test;
mod subnet;
mod traceroute;

use std::sync::Arc;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(runs::Runs::default()))
        .setup(|app| {
            let folder = app.path().app_data_dir()?;
            app.manage(saved_targets::SavedTargets::open(folder));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            subnet::calculate_subnet,
            subnet::subnet_contains,
            continuous_ping::default_gateway,
            continuous_ping::start_continuous_ping,
            dns_lookup::default_dns_server,
            dns_lookup::start_dns_lookup,
            ip_info::start_ip_info,
            lan_scan::default_lan_range,
            lan_scan::lan_range_info,
            lan_scan::start_lan_scan,
            ping::ping_defaults,
            ping::start_ping,
            port_check::port_presets,
            port_check::start_port_check,
            traceroute::start_traceroute,
            speed_test::ookla_installed,
            speed_test::speed_test_cost_warning,
            speed_test::start_speed_test,
            saved_targets::list_saved_targets,
            saved_targets::create_saved_target,
            saved_targets::update_saved_target,
            saved_targets::delete_saved_target,
            saved_targets::export_saved_targets,
            saved_targets::import_saved_targets,
            runs::run_text,
            runs::stop_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
