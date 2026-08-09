use crate::domain::match_record::{MatchRecord, PlayerMatch};

pub const ALL_QUEUES: i64 = -1;
pub const NORMAL_QUEUES: i64 = -2;

pub fn queue_scope_for_id(queue_id: i64) -> i64 {
    if matches!(queue_id, 400 | 430) {
        NORMAL_QUEUES
    } else {
        queue_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AggregateScope {
    pub queue_scope: i64,
    pub patch: String,
    pub season: String,
}

impl AggregateScope {
    pub fn for_match(record: &MatchRecord) -> Vec<Self> {
        let mut scopes = Vec::with_capacity(6);
        for queue_scope in [ALL_QUEUES, queue_scope_for_id(record.queue_id)] {
            scopes.push(Self {
                queue_scope,
                patch: String::new(),
                season: String::new(),
            });
            scopes.push(Self {
                queue_scope,
                patch: record.patch.clone(),
                season: String::new(),
            });
            if let Some(season) = record
                .season_key
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                scopes.push(Self {
                    queue_scope,
                    patch: String::new(),
                    season: season.to_owned(),
                });
            }
        }
        scopes
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AggregateCounters {
    pub games: i64,
    pub wins: i64,
    pub losses: i64,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub playtime_seconds: i64,
    pub double_kills: i64,
    pub triple_kills: i64,
    pub quadra_kills: i64,
    pub penta_kills: i64,
    pub total_minions_killed: i64,
    pub neutral_minions_killed: i64,
    pub gold_earned: i64,
    pub highest_kills: i64,
}

impl AggregateCounters {
    pub fn from_match(record: &MatchRecord, player: &PlayerMatch) -> Self {
        Self {
            games: 1,
            wins: i64::from(player.win),
            losses: i64::from(!player.win),
            kills: player.kills,
            deaths: player.deaths,
            assists: player.assists,
            playtime_seconds: record.game_duration_seconds,
            double_kills: player.double_kills,
            triple_kills: player.triple_kills,
            quadra_kills: player.quadra_kills,
            penta_kills: player.penta_kills,
            total_minions_killed: player.total_minions_killed,
            neutral_minions_killed: player.neutral_minions_killed,
            gold_earned: player.gold_earned,
            highest_kills: player.kills,
        }
    }
}
