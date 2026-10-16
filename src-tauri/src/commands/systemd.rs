//! Commandes Tauri : services systemd utilisateur et leurs logs.

use tauri::State;

use crate::error::AppError;
use crate::run_state::Action;
use crate::state::AppState;
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
