//! Commandes Tauri : services systemd utilisateur et leurs logs.

use tauri::State;

use crate::error::AppError;
use crate::state::AppState;
use crate::systemd::{self, Service};

#[tauri::command]
pub async fn services_list(state: State<'_, AppState>) -> Result<Vec<Service>, AppError> {
    let connection = state.connection().await?;
    systemd::list(&connection).await
}
