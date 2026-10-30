//! Commandes Tauri : préférences de l'interface et image de fond.

use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::error::AppError;
use crate::preferences::{BackgroundChoice, PreferenceStore, Preferences};

#[tauri::command]
pub fn preferences_get(store: State<'_, PreferenceStore>) -> Preferences {
    store.get()
}

#[tauri::command]
pub fn preferences_set_dimming(
    store: State<'_, PreferenceStore>,
    dimming: u8,
) -> Result<Preferences, AppError> {
    store.set_dimming(dimming)
}

#[tauri::command]
pub fn preferences_set_original_colors(
    store: State<'_, PreferenceStore>,
    enabled: bool,
) -> Result<Preferences, AppError> {
    store.set_original_colors(enabled)
}

/// Ouvre la fenêtre de sélection du système puis contrôle l'image choisie.
/// Le chemin reste dans le backend : l'interface ne le reçoit jamais.
#[tauri::command]
pub async fn background_choose(app: AppHandle) -> Result<BackgroundChoice, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app
            .dialog()
            .file()
            .set_title("Image de fond")
            .add_filter("Images PNG, JPEG ou WebP", &["png", "jpg", "jpeg", "webp"])
            .add_filter("Tous les fichiers", &["*"]);
        if let Some(window) = app.get_webview_window("main") {
            dialog = dialog.set_parent(&window);
        }
        let Some(picked) = dialog.blocking_pick_file() else {
            return Ok(BackgroundChoice::Cancelled);
        };
        let path = picked
            .into_path()
            .map_err(|e| AppError::invalid_input(format!("fichier inutilisable : {e}")))?;
        app.state::<PreferenceStore>().choose(&path)
    })
    .await
    .map_err(|e| AppError::io(format!("choix de l'image interrompu : {e}")))?
}

#[tauri::command]
pub fn background_pending_image(
    store: State<'_, PreferenceStore>,
    token: String,
) -> Result<Response, AppError> {
    store.pending_image(&token).map(Response::new)
}

#[tauri::command]
pub fn background_commit(
    store: State<'_, PreferenceStore>,
    token: String,
) -> Result<Preferences, AppError> {
    store.commit(&token)
}

#[tauri::command]
pub fn background_discard(store: State<'_, PreferenceStore>, token: String) {
    store.discard(&token);
}

#[tauri::command]
pub fn background_image(store: State<'_, PreferenceStore>) -> Result<Response, AppError> {
    store.background_image().map(Response::new)
}

#[tauri::command]
pub fn background_reset(store: State<'_, PreferenceStore>) -> Result<Preferences, AppError> {
    store.reset()
}

#[tauri::command]
pub fn background_lost_at_startup(store: State<'_, PreferenceStore>) -> bool {
    store.take_background_lost()
}
