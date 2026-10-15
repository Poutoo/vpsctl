//! Commandes Tauri : profils, passphrase, clé hôte, connexion et déconnexion.

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::State;
use zeroize::Zeroizing;

use crate::config::Profile;
use crate::error::AppError;
use crate::ssh::{ConnectStep, ServerInfo};
use crate::state::{AppState, ConnectionInfo};

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, AppError> {
    state.list_profiles()
}

#[tauri::command]
pub fn save_profile(state: State<'_, AppState>, profile: Profile) -> Result<Profile, AppError> {
    state.save_profile(profile)
}

#[tauri::command]
pub async fn delete_profile(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.delete_profile(&id).await
}

#[tauri::command]
pub async fn set_key_passphrase(
    state: State<'_, AppState>,
    profile_id: String,
    passphrase: String,
) -> Result<(), AppError> {
    state
        .set_key_passphrase(&profile_id, Zeroizing::new(passphrase))
        .await
}

#[tauri::command]
pub fn trust_host_key(
    state: State<'_, AppState>,
    profile_id: String,
    fingerprint: String,
) -> Result<(), AppError> {
    state.trust_host_key(&profile_id, &fingerprint)
}

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    profile_id: String,
    passphrase: Option<String>,
    on_step: Channel<ConnectStep>,
) -> Result<ConnectionInfo, AppError> {
    let sink = Arc::new(move |step: ConnectStep| {
        let _ = on_step.send(step);
    });
    state
        .connect(&profile_id, passphrase.map(Zeroizing::new), sink)
        .await
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), AppError> {
    state.disconnect().await;
    Ok(())
}

#[tauri::command]
pub async fn connection_status(
    state: State<'_, AppState>,
) -> Result<Option<ConnectionInfo>, AppError> {
    Ok(state.connection_status().await)
}

#[tauri::command]
pub async fn server_info(state: State<'_, AppState>) -> Result<ServerInfo, AppError> {
    state.server_info().await
}
