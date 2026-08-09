use tauri::{AppHandle, State};

use crate::AppState;
use crate::dto::analytics::SyncStateDto;
use crate::error::{AppError, AppResult};
use crate::services::sync;

#[tauri::command]
pub async fn start_sync(app: AppHandle, state: State<'_, AppState>) -> AppResult<SyncStateDto> {
    let riot = state.riot.clone().ok_or_else(|| {
        AppError::Configuration("RIOT_API_KEY is not configured in the backend".to_owned())
    })?;
    Ok(sync::start_background(state.database.clone(), riot, state.sync.clone(), app).await)
}

#[tauri::command]
pub async fn get_sync_state(state: State<'_, AppState>) -> AppResult<SyncStateDto> {
    Ok(state.sync.snapshot().await)
}
