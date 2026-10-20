//! Point d'entrée Tauri 2 : enregistrement des commandes et de l'état.

pub mod commands;
pub mod config;
pub mod docker;
pub mod error;
pub mod run_state;
pub mod scripts;
pub mod secrets;
pub mod sftp;
#[cfg(test)]
mod shared_cases;
pub mod ssh;
pub mod state;
pub mod stream;
pub mod systemd;

use std::sync::Arc;

use tauri::webview::PageLoadEvent;
use tauri::{Emitter, Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Fenêtres de sélection de fichiers (envoi, téléchargement) et
        // « Afficher » dans l'explorateur local.
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let handle = app.handle().clone();
            let on_link_lost: state::LinkLostSink = Arc::new(move |event| {
                let _ = handle.emit(state::LINK_LOST_EVENT, event);
            });
            app.manage(state::AppState::new(&config_dir, on_link_lost));
            Ok(())
        })
        // Page (re)chargée : les canaux de l'ancienne page sont morts, mais
        // leur envoi ne signale pas toujours d'erreur. Leurs flux sont
        // arrêtés pour ne pas occuper la liaison, et leurs transferts annulés
        // (plus personne ne les suit).
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Started {
                let app = webview.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(state) = app.try_state::<state::AppState>() {
                        tokio::join!(state.stop_all_streams(), state.cancel_all_transfers());
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
            commands::systemd::service_unit_preview,
            commands::systemd::service_create,
            commands::systemd::service_delete_info,
            commands::systemd::service_delete,
            commands::systemd::user_linger,
            commands::logs::logs_start,
            commands::logs::logs_stop,
            commands::scripts::scripts_list,
            commands::scripts::scripts_save,
            commands::scripts::scripts_delete,
            commands::scripts::script_run,
            commands::scripts::script_stop,
            commands::files::home_dir,
            commands::files::list_dir,
            commands::files::create_file,
            commands::files::create_dir,
            commands::files::rename,
            commands::files::dir_summary,
            commands::files::delete_path,
            commands::files::upload,
            commands::files::download,
            commands::files::transfer_cancel,
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
