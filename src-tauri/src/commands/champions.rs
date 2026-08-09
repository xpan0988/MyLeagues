use tauri::State;

use crate::AppState;
use crate::dto::analytics::{AnalyticsFilterDto, ChampionProfileDto, ChampionSummaryDto};
use crate::error::AppResult;
use crate::services::aggregation::AggregationService;

#[tauri::command]
pub fn list_champions(
    filter: AnalyticsFilterDto,
    state: State<'_, AppState>,
) -> AppResult<Vec<ChampionSummaryDto>> {
    AggregationService::new(&state.database).champions(filter.into())
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_champion_profile(
    champion_id: i64,
    filter: AnalyticsFilterDto,
    state: State<'_, AppState>,
) -> AppResult<ChampionProfileDto> {
    AggregationService::new(&state.database).champion_profile(champion_id, filter.into())
}
