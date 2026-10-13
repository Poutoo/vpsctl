//! Point d'entrée Tauri 2 : enregistrement des commandes et de l'état.

pub mod commands;
pub mod config;
pub mod docker;
pub mod error;
pub mod scripts;
pub mod secrets;
pub mod sftp;
pub mod ssh;
pub mod state;
pub mod systemd;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
