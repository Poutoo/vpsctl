//! Commandes Tauri : ressources du serveur.

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::State;

use crate::error::AppError;
use crate::resources::analysis::{AnalysisEvent, AnalysisKind, AnalysisSink};
use crate::resources::settings::Thresholds;
use crate::resources::ResourceUpdate;
use crate::state::AppState;

#[tauri::command]
pub fn resources_watch(state: State<'_, AppState>, foreground: bool) {
    state.resources_watch(foreground);
}

#[tauri::command]
pub fn resources_state(state: State<'_, AppState>) -> Option<ResourceUpdate> {
    state.resources_state()
}

#[tauri::command]
pub fn resource_thresholds_get(state: State<'_, AppState>, profile_id: String) -> Thresholds {
    state.resource_thresholds(&profile_id)
}

#[tauri::command]
pub fn resource_thresholds_save(
    state: State<'_, AppState>,
    profile_id: String,
    thresholds: Thresholds,
) -> Result<Thresholds, AppError> {
    state.save_resource_thresholds(&profile_id, thresholds)
}

#[tauri::command]
pub fn resource_thresholds_reset(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<Thresholds, AppError> {
    state.reset_resource_thresholds(&profile_id)
}

#[tauri::command]
pub async fn docker_analyze(
    state: State<'_, AppState>,
    kind: AnalysisKind,
    on_event: Channel<AnalysisEvent>,
) -> Result<String, AppError> {
    let sink: AnalysisSink = Arc::new(move |event| on_event.send(event).is_ok());
    state.docker_analyze(kind, sink).await
}

#[tauri::command]
pub async fn docker_analyze_stop(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<(), AppError> {
    state.stop_stream(&run_id).await;
    Ok(())
}
