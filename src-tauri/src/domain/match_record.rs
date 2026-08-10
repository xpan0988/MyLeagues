use super::items::FinalItem;
use super::runes::RuneSelection;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchRecord {
    pub match_id: String,
    pub game_creation: i64,
    pub game_end_timestamp: Option<i64>,
    pub game_duration_seconds: i64,
    pub queue_id: i64,
    pub game_version: String,
    pub patch: String,
    pub season_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerMatch {
    pub match_id: String,
    pub puuid: String,
    pub participant_id: Option<i64>,
    pub champion_id: i64,
    pub win: bool,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub double_kills: i64,
    pub triple_kills: i64,
    pub quadra_kills: i64,
    pub penta_kills: i64,
    pub total_minions_killed: i64,
    pub neutral_minions_killed: i64,
    pub gold_earned: i64,
    pub summoner_spell_ids: [i64; 2],
    pub keystone_id: Option<i64>,
    pub primary_style_id: Option<i64>,
    pub secondary_style_id: Option<i64>,
    pub final_items: Vec<FinalItem>,
    pub rune_selections: Vec<RuneSelection>,
}
