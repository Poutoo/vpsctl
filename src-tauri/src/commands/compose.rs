//! Commandes Tauri : projets docker compose.

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::State;

use crate::compose::{ComposeAction, ComposeRun, DownInfo, ProjectCheck};
use crate::error::AppError;
use crate::state::AppState;
use crate::stream::{EventSink, LogEvent};

#[tauri::command]
pub async fn compose_run(
    state: State<'_, AppState>,
    project: String,
    action: ComposeAction,
    on_event: Channel<LogEvent>,
) -> Result<ComposeRun, AppError> {
    let sink: EventSink = Arc::new(move |event| on_event.send(event).is_ok());
    state.compose_run(&project, action, sink).await
}

#[tauri::command]
pub async fn compose_stop(state: State<'_, AppState>, run_id: String) -> Result<(), AppError> {
    state.stop_stream(&run_id).await;
    Ok(())
}

#[tauri::command]
pub async fn compose_down_info(
    state: State<'_, AppState>,
    project: String,
) -> Result<DownInfo, AppError> {
    state.compose_down_info(&project).await
}

#[tauri::command]
pub async fn compose_check(state: State<'_, AppState>) -> Result<Vec<ProjectCheck>, AppError> {
    state.compose_check().await
}

#[tauri::command]
pub async fn compose_forget(state: State<'_, AppState>, project: String) -> Result<(), AppError> {
    state.compose_forget(&project).await
}
