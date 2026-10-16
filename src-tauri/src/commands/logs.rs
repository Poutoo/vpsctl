//! Commandes Tauri : flux de logs Docker et systemd, événements via `Channel`.

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;
use crate::stream::{EventSink, LogEvent, LogSource};

#[tauri::command]
pub async fn logs_start(
    state: State<'_, AppState>,
    source: LogSource,
    tail: u32,
    on_event: Channel<LogEvent>,
) -> Result<String, AppError> {
    let sink: EventSink = Arc::new(move |event| on_event.send(event).is_ok());
    state.start_logs(&source, tail, sink).await
}

#[tauri::command]
pub async fn logs_stop(state: State<'_, AppState>, stream_id: String) -> Result<(), AppError> {
    state.stop_stream(&stream_id).await;
    Ok(())
}
