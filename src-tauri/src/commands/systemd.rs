//! Commandes Tauri : services systemd utilisateur et leurs logs.

use tauri::State;

use crate::error::AppError;
use crate::run_state::Action;
use crate::state::AppState;
use crate::systemd::manage::{self, ServiceDeleteInfo, SshHost};
use crate::systemd::unit::{self, ServiceDef};
use crate::systemd::{self, Service};

#[tauri::command]
pub async fn services_list(state: State<'_, AppState>) -> Result<Vec<Service>, AppError> {
    let connection = state.connection().await?;
    systemd::list(&connection).await
}

#[tauri::command]
pub async fn service_action(
    state: State<'_, AppState>,
    unit: String,
    action: Action,
) -> Result<(), AppError> {
    let connection = state.connection().await?;
    systemd::action(&connection, &unit, action).await
}

/// Texte du fichier d'unité, pour l'aperçu du formulaire (sans connexion).
#[tauri::command]
pub fn service_unit_preview(def: ServiceDef) -> Result<String, AppError> {
    unit::render_unit(&def)
}

#[tauri::command]
pub async fn service_create(
    state: State<'_, AppState>,
    def: ServiceDef,
) -> Result<Service, AppError> {
    let host = SshHost::new(state.connection().await?);
    manage::create(&host, &def).await
}

#[tauri::command]
pub async fn service_delete_info(
    state: State<'_, AppState>,
    unit: String,
) -> Result<ServiceDeleteInfo, AppError> {
    let host = SshHost::new(state.connection().await?);
    manage::delete_info(&host, &unit).await
}

#[tauri::command]
pub async fn service_delete(state: State<'_, AppState>, unit: String) -> Result<(), AppError> {
    let host = SshHost::new(state.connection().await?);
    manage::delete(&host, &unit).await
}

/// Lingering du compte ; `None` si son état n'a pas pu être lu.
#[tauri::command]
pub async fn user_linger(state: State<'_, AppState>) -> Result<Option<bool>, AppError> {
    let connection = state.connection().await?;
    Ok(manage::linger(&connection).await)
}
