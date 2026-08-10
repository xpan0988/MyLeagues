use tauri::{AppHandle, State};

use crate::AppState;
use crate::error::AppResult;
use crate::services::maintenance::MaintenanceService;
use crate::services::sync::{self, SyncTrigger};

#[tauri::command]
pub fn rebuild_aggregates(state: State<'_, AppState>) -> AppResult<()> {
    MaintenanceService::new(&state.database).rebuild_aggregates()
}

#[tauri::command]
pub fn clear_static_cache(state: State<'_, AppState>) -> AppResult<()> {
    MaintenanceService::new(&state.database).clear_static_cache()
}

#[tauri::command]
pub async fn reset_local_archive(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    MaintenanceService::new(&state.database).reset_local_archive()?;
    if let Some(riot) = state.riot.clone() {
        sync::start_background(state.database.clone(), riot, state.sync.clone(), state.timeline.clone(), app, SyncTrigger::ArchiveReset).await;
    }
    Ok(())
}
