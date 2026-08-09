use tauri::State;

use crate::AppState;
use crate::dto::settings::{SettingsDto, UpdateSettingsDto};
use crate::error::AppResult;
use crate::services::settings::SettingsService;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<SettingsDto> {
    SettingsService::new(&state.database, &state.config).get()
}

#[tauri::command]
pub fn update_settings(
    settings: UpdateSettingsDto,
    state: State<'_, AppState>,
) -> AppResult<SettingsDto> {
    SettingsService::new(&state.database, &state.config).update(settings.into())
}
