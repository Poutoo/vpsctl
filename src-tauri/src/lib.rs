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
pub mod stream;
pub mod systemd;

use tauri::webview::PageLoadEvent;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            app.manage(state::AppState::new(&config_dir));
            Ok(())
        })
        // Page (re)chargée : les canaux de l'ancienne page sont morts, mais
        // leur envoi ne signale pas toujours d'erreur. Leurs flux sont
        // arrêtés pour ne pas occuper la liaison.
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Started {
                let app = webview.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(state) = app.try_state::<state::AppState>() {
                        state.stop_all_streams().await;
                    }
                });
            }
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
            commands::docker::docker_action,
            commands::docker::docker_remove,
            commands::systemd::services_list,
            commands::systemd::service_action,
            commands::logs::logs_start,
            commands::logs::logs_stop,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // Fermeture de l'application : les flux sont arrêtés (aucun
            // processus ne reste sur le serveur) et la liaison fermée.
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<state::AppState>() {
                    tauri::async_runtime::block_on(state.shutdown());
                }
            }
        });
}
