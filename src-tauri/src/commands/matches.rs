use tauri::State;

use crate::AppState;
use crate::dto::analytics::{MatchDetailDto, MatchQueryDto, MatchSummaryDto, PageDto};
use crate::error::AppResult;
use crate::services::aggregation::AggregationService;

#[tauri::command]
pub fn list_matches(
    query: MatchQueryDto,
    state: State<'_, AppState>,
) -> AppResult<PageDto<MatchSummaryDto>> {
    AggregationService::new(&state.database).matches(query)
}

#[tauri::command]
pub fn get_match_detail(match_id: String, state: State<'_, AppState>) -> AppResult<MatchDetailDto> {
    AggregationService::new(&state.database).match_detail(&match_id)
}
