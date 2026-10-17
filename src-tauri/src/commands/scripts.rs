//! Commandes Tauri : scripts personnalisés et leur exécution.

use tauri::State;

use crate::error::AppError;
use crate::scripts::Script;
use crate::state::AppState;

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
