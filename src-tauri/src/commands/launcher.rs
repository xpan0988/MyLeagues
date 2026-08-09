use tauri::State;

use crate::AppState;
use crate::dto::analytics::ClientStateDto;
use crate::error::AppResult;
use crate::services::launcher::LauncherService;

#[tauri::command]
pub fn get_client_state(state: State<'_, AppState>) -> AppResult<ClientStateDto> {
    LauncherService::new(&state.database).state()
}

#[tauri::command]
pub fn launch_client(state: State<'_, AppState>) -> AppResult<ClientStateDto> {
    LauncherService::new(&state.database).launch()
}
