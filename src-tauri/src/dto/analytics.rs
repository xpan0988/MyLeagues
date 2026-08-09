use serde::{Deserialize, Serialize};

use crate::domain::analytics::{AnalyticsFilter, QueueFilter, TimeRangeFilter};
use crate::domain::static_data::GameEntity;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QueueFilterDto {
    All,
    RankedSolo,
    Normal,
    Aram,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TimeRangeFilterDto {
    CurrentPatch,
    CurrentSeason,
    AllTracked,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsFilterDto {
    pub queue: QueueFilterDto,
    pub time_range: TimeRangeFilterDto,
}

impl From<AnalyticsFilterDto> for AnalyticsFilter {
    fn from(value: AnalyticsFilterDto) -> Self {
        let queue = match value.queue {
            QueueFilterDto::All => QueueFilter::All,
            QueueFilterDto::RankedSolo => QueueFilter::RankedSolo,
            QueueFilterDto::Normal => QueueFilter::Normal,
            QueueFilterDto::Aram => QueueFilter::Aram,
        };
        let time_range = match value.time_range {
            TimeRangeFilterDto::CurrentPatch => TimeRangeFilter::CurrentPatch,
            TimeRangeFilterDto::CurrentSeason => TimeRangeFilter::CurrentSeason,
            TimeRangeFilterDto::AllTracked => TimeRangeFilter::AllTracked,
        };

        Self { queue, time_range }
    }
}

impl From<QueueFilter> for QueueFilterDto {
    fn from(value: QueueFilter) -> Self {
        match value {
            QueueFilter::All => Self::All,
            QueueFilter::RankedSolo => Self::RankedSolo,
            QueueFilter::Normal => Self::Normal,
            QueueFilter::Aram => Self::Aram,
        }
    }
}

impl From<TimeRangeFilter> for TimeRangeFilterDto {
    fn from(value: TimeRangeFilter) -> Self {
        match value {
            TimeRangeFilter::CurrentPatch => Self::CurrentPatch,
            TimeRangeFilter::CurrentSeason => Self::CurrentSeason,
            TimeRangeFilter::AllTracked => Self::AllTracked,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub puuid: String,
    pub game_name: String,
    pub tag_line: String,
    pub account_region: String,
    pub platform_region: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStateDto {
    pub riot_client_running: bool,
    pub league_client_running: bool,
    pub game_running: bool,
    pub configured_executable_found: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStateDto {
    pub status: String,
    pub completed: u64,
    pub total: Option<u64>,
    pub message: Option<String>,
    pub last_successful_sync_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedOverviewDto {
    pub games: u64,
    pub wins: u64,
    pub losses: u64,
    pub win_rate: f64,
    pub playtime_seconds: u64,
    pub kills: u64,
    pub deaths: u64,
    pub assists: u64,
    pub kda: f64,
}

impl Default for TrackedOverviewDto {
    fn default() -> Self {
        Self {
            games: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.0,
            playtime_seconds: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            kda: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceDto {
    pub ids: Vec<i64>,
    pub games: u64,
    pub usage_rate: f64,
    pub win_rate: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameEntityDto {
    pub id: i64,
    pub name: String,
    pub icon: String,
}

impl From<GameEntity> for GameEntityDto {
    fn from(value: GameEntity) -> Self {
        Self {
            id: value.id,
            name: value.name,
            icon: value.icon,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedFilterDto {
    pub queue: QueueFilterDto,
    pub time_range: TimeRangeFilterDto,
    pub current_patch: String,
    pub current_season: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreBuildDto {
    pub items: Vec<GameEntityDto>,
    pub games: u64,
    pub usage_rate: f64,
    pub win_rate: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampionSummaryDto {
    pub champion: GameEntityDto,
    pub mastery_points: Option<i64>,
    pub mastery_level: Option<i64>,
    pub tracked_games: u64,
    pub wins: u64,
    pub losses: u64,
    pub win_rate: f64,
    pub playtime_seconds: u64,
    pub kills: u64,
    pub deaths: u64,
    pub assists: u64,
    pub kda: f64,
    pub most_used_core_build: Option<CoreBuildDto>,
    pub most_used_keystone: Option<PreferenceDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampionMasteryDto {
    pub points: Option<i64>,
    pub level: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampionProfileDto {
    pub champion: GameEntityDto,
    pub mastery: ChampionMasteryDto,
    pub filter_context: ResolvedFilterDto,
    pub overview: TrackedOverviewDto,
    pub performance: PerformanceDto,
    pub core_builds: Vec<CoreBuildStatsDto>,
    pub boots: Vec<EntityUsageDto>,
    pub rune_pages: Vec<RunePageStatsDto>,
    pub keystone_usage: Vec<EntityUsageDto>,
    pub summoner_spell_pairs: Vec<SpellPairStatsDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreBuildStatsDto {
    pub items: Vec<GameEntityDto>,
    pub games: u64,
    pub wins: u64,
    pub usage_rate: f64,
    pub win_rate: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityUsageDto {
    pub entity: GameEntityDto,
    pub games: u64,
    pub wins: u64,
    pub usage_rate: f64,
    pub win_rate: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunePageStatsDto {
    pub primary_style: GameEntityDto,
    pub primary_runes: Vec<GameEntityDto>,
    pub secondary_style: GameEntityDto,
    pub secondary_runes: Vec<GameEntityDto>,
    pub stat_shards: Vec<GameEntityDto>,
    pub games: u64,
    pub wins: u64,
    pub usage_rate: f64,
    pub win_rate: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellPairStatsDto {
    pub spells: Vec<GameEntityDto>,
    pub games: u64,
    pub wins: u64,
    pub usage_rate: f64,
    pub win_rate: f64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceDto {
    pub average_kills: f64,
    pub average_deaths: f64,
    pub average_assists: f64,
    pub average_cs_per_minute: f64,
    pub average_match_duration_seconds: f64,
    pub highest_kills: u64,
    pub double_kills: u64,
    pub triple_kills: u64,
    pub quadra_kills: u64,
    pub penta_kills: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchQueryDto {
    pub queue: QueueFilterDto,
    pub time_range: TimeRangeFilterDto,
    pub champion_id: Option<i64>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchSummaryDto {
    pub match_id: String,
    pub champion: GameEntityDto,
    pub win: bool,
    pub queue_id: i64,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub duration_seconds: i64,
    pub keystone: Option<GameEntityDto>,
    pub summoner_spells: Vec<GameEntityDto>,
    pub game_creation: String,
    pub patch: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunePageDto {
    pub primary_style: Option<GameEntityDto>,
    pub primary_runes: Vec<GameEntityDto>,
    pub secondary_style: Option<GameEntityDto>,
    pub secondary_runes: Vec<GameEntityDto>,
    pub stat_shards: Vec<GameEntityDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchItemDto {
    pub item: GameEntityDto,
    pub slot: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDetailDto {
    pub match_id: String,
    pub champion: GameEntityDto,
    pub win: bool,
    pub queue_id: i64,
    pub game_creation: String,
    pub duration_seconds: i64,
    pub patch: String,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub total_cs: i64,
    pub gold_earned: i64,
    pub summoner_spells: Vec<GameEntityDto>,
    pub rune_page: RunePageDto,
    pub final_items: Vec<MatchItemDto>,
    pub double_kills: i64,
    pub triple_kills: i64,
    pub quadra_kills: i64,
    pub penta_kills: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageDto<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeDto {
    pub account: Option<AccountDto>,
    pub rank: Option<RankDto>,
    pub client_state: ClientStateDto,
    pub sync_state: SyncStateDto,
    pub tracked_career: TrackedOverviewDto,
    pub ranked_games: u64,
    pub recent_form: Vec<bool>,
    pub top_champions: Vec<ChampionSummaryDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankDto {
    pub tier: String,
    pub division: String,
    pub league_points: i64,
    pub wins: i64,
    pub losses: i64,
    pub win_rate: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerDto {
    pub overall: TrackedOverviewDto,
    pub by_queue: CareerQueuesDto,
    pub average_match_duration_seconds: u64,
    pub most_played_champion_id: Option<i64>,
    pub champion_pool: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerQueuesDto {
    pub ranked_solo: TrackedOverviewDto,
    pub normal: TrackedOverviewDto,
    pub aram: TrackedOverviewDto,
}
