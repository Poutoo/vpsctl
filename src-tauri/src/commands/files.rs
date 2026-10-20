//! Commandes Tauri : explorateur de fichiers (SFTP), progression via `Channel`.

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::State;

use crate::error::AppError;
use crate::sftp::fs::SftpFs;
use crate::sftp::ops::{self, FileEntry};
use crate::sftp::transfer::{Progress, ProgressSink};
use crate::sftp::walk::DirSummary;
use crate::state::AppState;

#[tauri::command]
pub async fn home_dir(state: State<'_, AppState>) -> Result<String, AppError> {
    state
        .remote(|connection| async move {
            let fs = SftpFs::shared(connection);
            ops::home_dir(&*fs).await
        })
        .await
}

#[tauri::command]
pub async fn list_dir(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<FileEntry>, AppError> {
    state
        .remote(|connection| async move {
            let fs = SftpFs::shared(connection);
            ops::list_dir(&fs, &path).await
        })
        .await
}

#[tauri::command]
pub async fn create_file(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
    state
        .remote(|connection| async move {
            let fs = SftpFs::shared(connection);
            ops::create_file(&*fs, &path).await
        })
        .await
}

#[tauri::command]
pub async fn create_dir(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
    state
        .remote(|connection| async move {
            let fs = SftpFs::shared(connection);
            ops::create_dir(&*fs, &path).await
        })
        .await
}

#[tauri::command]
pub async fn rename(
    state: State<'_, AppState>,
    from_path: String,
    to_path: String,
) -> Result<(), AppError> {
    state
        .remote(|connection| async move {
            let fs = SftpFs::shared(connection);
            ops::rename(&*fs, &from_path, &to_path).await
        })
        .await
}

#[tauri::command]
pub async fn dir_summary(state: State<'_, AppState>, path: String) -> Result<DirSummary, AppError> {
    state
        .remote(|connection| async move {
            let fs = SftpFs::shared(connection);
            ops::dir_summary(&fs, &path).await
        })
        .await
}

#[tauri::command]
pub async fn delete_path(
    state: State<'_, AppState>,
    path: String,
    recursive: bool,
) -> Result<(), AppError> {
    state
        .remote(|connection| async move {
            let fs = SftpFs::shared(connection);
            ops::delete_path(&fs, &path, recursive).await
        })
        .await
}

#[tauri::command]
pub async fn upload(
    state: State<'_, AppState>,
    local_path: String,
    remote_path: String,
    overwrite: bool,
    on_progress: Channel<Progress>,
) -> Result<String, AppError> {
    let sink: ProgressSink = Arc::new(move |event| on_progress.send(event).is_ok());
    state
        .upload(&local_path, &remote_path, overwrite, sink)
        .await
}

#[tauri::command]
pub async fn download(
    state: State<'_, AppState>,
    remote_path: String,
    local_path: String,
    on_progress: Channel<Progress>,
) -> Result<String, AppError> {
    let sink: ProgressSink = Arc::new(move |event| on_progress.send(event).is_ok());
    state.download(&remote_path, &local_path, sink).await
}

#[tauri::command]
pub async fn transfer_cancel(
    state: State<'_, AppState>,
    transfer_id: String,
) -> Result<(), AppError> {
    state.cancel_transfer(&transfer_id).await;
    Ok(())
}
