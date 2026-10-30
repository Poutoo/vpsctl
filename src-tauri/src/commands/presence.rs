//! Commandes Tauri : présence Discord.

use tauri::State;

use crate::error::AppError;
use crate::preferences::PreferenceStore;
use crate::presence::{Presence, PresenceStatus};

#[tauri::command]
pub fn presence_status(presence: State<'_, Presence>) -> PresenceStatus {
    presence.status()
}

/// Enregistre l'interrupteur puis l'applique ; le statut suit par
/// l'événement `presence-status`.
#[tauri::command]
pub fn presence_set_enabled(
    store: State<'_, PreferenceStore>,
    presence: State<'_, Presence>,
    enabled: bool,
) -> Result<(), AppError> {
    store.set_discord_presence(enabled)?;
    presence.set_enabled(enabled);
    Ok(())
}

#[tauri::command]
pub fn presence_retry(presence: State<'_, Presence>) {
    presence.retry();
}
