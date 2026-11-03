//! Commandes Tauri : édition de fichiers texte (SFTP).

use tauri::State;

use crate::editor::authorized_keys::AuthorizedKeysCheck;
use crate::editor::{self, OpenedFile, SaveOutcome, SaveRequest};
use crate::error::AppError;
use crate::sftp::fs::SftpFs;
use crate::state::AppState;

#[tauri::command]
pub async fn file_open(state: State<'_, AppState>, path: String) -> Result<OpenedFile, AppError> {
    state
        .remote(|connection| async move {
            let user = connection.user().to_owned();
            let fs = SftpFs::shared(connection);
            editor::open(&*fs, &path, &user).await
        })
        .await
}

#[tauri::command]
pub async fn file_save(
    state: State<'_, AppState>,
    request: SaveRequest,
) -> Result<SaveOutcome, AppError> {
    state
        .remote(|connection| async move {
            let user = connection.user().to_owned();
            let fs = SftpFs::shared(connection);
            editor::save(&*fs, &request, &user).await
        })
        .await
}

#[tauri::command]
pub async fn authorized_keys_check(
    state: State<'_, AppState>,
    path: String,
    before: String,
    after: String,
) -> Result<AuthorizedKeysCheck, AppError> {
    state
        .remote(|connection| async move {
            let key = connection.client_key().cloned();
            let fs = SftpFs::shared(connection);
            editor::check_authorized_keys(&*fs, &path, &before, &after, key.as_ref()).await
        })
        .await
}
