use tauri::State;

use crate::AppState;
use crate::dto::analytics::HomeDto;
use crate::error::AppResult;
use crate::services::aggregation::AggregationService;

#[tauri::command]
pub fn get_home(state: State<'_, AppState>) -> AppResult<HomeDto> {
    AggregationService::new(&state.database).home()
}
