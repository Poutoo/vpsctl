//! Commandes Tauri : préférences de l'interface.

use tauri::State;

use crate::error::AppError;
use crate::preferences::{PreferenceStore, Preferences};

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
