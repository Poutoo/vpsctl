//! Commandes Tauri : scripts personnalisés et leur exécution.

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::State;

use crate::error::AppError;
use crate::scripts::Script;
use crate::state::AppState;
use crate::stream::{EventSink, LogEvent};

#[tauri::command]
pub fn scripts_list(state: State<'_, AppState>) -> Result<Vec<Script>, AppError> {
    state.list_scripts()
}

#[tauri::command]
pub fn scripts_save(state: State<'_, AppState>, script: Script) -> Result<Script, AppError> {
    state.save_script(script)
}

#[tauri::command]
pub fn scripts_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.delete_script(&id)
}

#[tauri::command]
pub async fn script_run(
    state: State<'_, AppState>,
    id: String,
    on_event: Channel<LogEvent>,
) -> Result<String, AppError> {
    let sink: EventSink = Arc::new(move |event| on_event.send(event).is_ok());
    state.run_script(&id, sink).await
}

#[tauri::command]
pub async fn script_stop(state: State<'_, AppState>, run_id: String) -> Result<(), AppError> {
    state.stop_stream(&run_id).await;
    Ok(())
}
