use tauri::{AppHandle, State};

use crate::AppState;
use crate::dto::analytics::SyncStateDto;
use crate::error::{AppError, AppResult};
use crate::services::sync::{self, SyncTrigger};

#[tauri::command]
pub async fn start_sync(app: AppHandle, state: State<'_, AppState>) -> AppResult<SyncStateDto> {
    let riot = state.riot.clone().ok_or_else(|| {
        AppError::Configuration("RIOT_API_KEY is not configured in the backend".to_owned())
    })?;
    Ok(sync::start_background(
        state.database.clone(),
        riot,
        state.sync.clone(),
        state.timeline.clone(),
        app,
        SyncTrigger::Manual,
    )
    .await)
}

#[tauri::command]
pub async fn request_freshness_check(
    trigger: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<SyncStateDto> {
    let riot = state.riot.clone().ok_or_else(|| {
        AppError::Configuration("RIOT_API_KEY is not configured in the backend".to_owned())
    })?;
    let trigger = match trigger.as_str() {
        "periodic" => SyncTrigger::Periodic,
        "resume" => SyncTrigger::Resume,
        _ => {
            return Err(AppError::Configuration(
                "invalid automatic sync trigger".to_owned(),
            ));
        }
    };
    sync::start_if_stale(
        state.database.clone(),
        riot,
        state.sync.clone(),
        state.timeline.clone(),
        app,
        trigger,
    )
    .await
}

#[tauri::command]
pub async fn get_sync_state(state: State<'_, AppState>) -> AppResult<SyncStateDto> {
    Ok(state.sync.snapshot().await)
}
