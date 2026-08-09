use tauri::State;

use crate::AppState;
use crate::dto::analytics::{AnalyticsFilterDto, CareerDto};
use crate::error::AppResult;
use crate::services::aggregation::AggregationService;

#[tauri::command]
pub fn get_career(filter: AnalyticsFilterDto, state: State<'_, AppState>) -> AppResult<CareerDto> {
    AggregationService::new(&state.database).career(filter.into())
}
