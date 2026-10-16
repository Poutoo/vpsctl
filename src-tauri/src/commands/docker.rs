//! Commandes Tauri : conteneurs Docker et leurs logs.

use tauri::State;

use crate::docker::{self, ContainerList};
use crate::error::AppError;
use crate::run_state::Action;
use crate::state::AppState;

#[tauri::command]
pub async fn docker_list(state: State<'_, AppState>) -> Result<ContainerList, AppError> {
    let connection = state.connection().await?;
    docker::list(&connection).await
}

#[tauri::command]
pub async fn docker_action(
    state: State<'_, AppState>,
    id: String,
    action: Action,
) -> Result<(), AppError> {
    let connection = state.connection().await?;
    docker::action(&connection, &id, action).await
}

#[tauri::command]
pub async fn docker_remove(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let connection = state.connection().await?;
    docker::remove(&connection, &id).await
}
