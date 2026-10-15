//! Point d'entrée Tauri 2 : enregistrement des commandes et de l'état.

pub mod commands;
pub mod config;
pub mod docker;
pub mod error;
pub mod run_state;
pub mod scripts;
pub mod secrets;
pub mod sftp;
pub mod ssh;
pub mod state;
pub mod systemd;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            app.manage(state::AppState::new(&config_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connection::list_profiles,
            commands::connection::save_profile,
            commands::connection::delete_profile,
            commands::connection::set_key_passphrase,
            commands::connection::trust_host_key,
            commands::connection::connect,
            commands::connection::disconnect,
            commands::connection::connection_status,
            commands::connection::server_info,
            commands::docker::docker_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
