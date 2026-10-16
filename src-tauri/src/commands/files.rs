//! Commandes Tauri : explorateur de fichiers (SFTP), progression via `Channel`.

use tauri::State;

use crate::error::AppError;
use crate::sftp::fs::SftpFs;
use crate::sftp::ops::{self, FileEntry};
use crate::state::AppState;

#[tauri::command]
pub async fn home_dir(state: State<'_, AppState>) -> Result<String, AppError> {
    let fs = SftpFs::shared(state.connection().await?);
    ops::home_dir(&*fs).await
}

#[tauri::command]
pub async fn list_dir(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<FileEntry>, AppError> {
    let fs = SftpFs::shared(state.connection().await?);
    ops::list_dir(&fs, &path).await
}

#[tauri::command]
pub async fn create_file(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
    let fs = SftpFs::shared(state.connection().await?);
    ops::create_file(&*fs, &path).await
}

#[tauri::command]
pub async fn create_dir(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
    let fs = SftpFs::shared(state.connection().await?);
    ops::create_dir(&*fs, &path).await
}

#[tauri::command]
pub async fn rename(
    state: State<'_, AppState>,
    from_path: String,
    to_path: String,
) -> Result<(), AppError> {
    let fs = SftpFs::shared(state.connection().await?);
    ops::rename(&*fs, &from_path, &to_path).await
}
