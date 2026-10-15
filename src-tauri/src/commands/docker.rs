//! Commandes Tauri : conteneurs Docker et leurs logs.

use tauri::State;

use crate::docker::{self, ContainerList};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn docker_list(state: State<'_, AppState>) -> Result<ContainerList, AppError> {
    let connection = state.connection().await?;
    docker::list(&connection).await
}
