//! Commandes Tauri : projets docker compose.

use tauri::State;

use crate::compose::ProjectCheck;
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn compose_check(state: State<'_, AppState>) -> Result<Vec<ProjectCheck>, AppError> {
    state.compose_check().await
}

#[tauri::command]
pub async fn compose_forget(state: State<'_, AppState>, project: String) -> Result<(), AppError> {
    state.compose_forget(&project).await
}
