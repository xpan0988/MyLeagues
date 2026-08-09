use tauri::State;

use crate::AppState;
use crate::error::AppResult;
use crate::services::maintenance::MaintenanceService;

#[tauri::command]
pub fn rebuild_aggregates(state: State<'_, AppState>) -> AppResult<()> {
    MaintenanceService::new(&state.database).rebuild_aggregates()
}

#[tauri::command]
pub fn clear_static_cache(state: State<'_, AppState>) -> AppResult<()> {
    MaintenanceService::new(&state.database).clear_static_cache()
}
