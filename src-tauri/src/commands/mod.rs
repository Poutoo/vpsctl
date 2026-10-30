//! Commandes `#[tauri::command]` : adaptateurs très fins qui récupèrent l'état,
//! appellent un module métier et renvoient le résultat. Aucune logique propre.

pub mod connection;
pub mod docker;
pub mod files;
pub mod logs;
pub mod preferences;
pub mod presence;
pub mod scripts;
pub mod systemd;
