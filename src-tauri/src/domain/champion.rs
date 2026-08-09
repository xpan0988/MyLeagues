#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChampionMastery {
    pub champion_id: i64,
    pub mastery_level: i64,
    pub mastery_points: i64,
    pub last_play_time: Option<i64>,
}
