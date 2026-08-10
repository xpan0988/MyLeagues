use tauri::{AppHandle, State};

use crate::AppState;
use crate::dto::settings::{SettingsDto, UpdateSettingsDto};
use crate::error::AppResult;
use crate::services::settings::SettingsService;
use crate::services::sync::{self, SyncTrigger};

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<SettingsDto> {
    SettingsService::new(&state.database, &state.config).get()
}

#[tauri::command]
pub async fn update_settings(
    settings: UpdateSettingsDto,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<SettingsDto> {
    let saved = SettingsService::new(&state.database, &state.config).update(settings.into())?;
    if !saved.game_name.is_empty() && !saved.tag_line.is_empty() {
        if let Some(riot) = state.riot.clone() {
            sync::start_background(state.database.clone(), riot, state.sync.clone(), state.timeline.clone(), app, SyncTrigger::SettingsSaved).await;
        }
    }
    Ok(saved)
}
