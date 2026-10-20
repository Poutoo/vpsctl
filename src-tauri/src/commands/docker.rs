//! Commandes Tauri : conteneurs Docker et leurs logs.

use tauri::State;

use crate::docker::{self, ContainerList};
use crate::error::AppError;
use crate::run_state::Action;
use crate::state::AppState;

#[tauri::command]
pub async fn docker_list(state: State<'_, AppState>) -> Result<ContainerList, AppError> {
    state
        .remote(|connection| async move { docker::list(&connection).await })
        .await
}

#[tauri::command]
pub async fn docker_action(
    state: State<'_, AppState>,
    id: String,
    action: Action,
) -> Result<(), AppError> {
    state
        .remote(|connection| async move { docker::action(&connection, &id, action).await })
        .await
}

#[tauri::command]
pub async fn docker_remove(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state
        .remote(|connection| async move { docker::remove(&connection, &id).await })
        .await
}
