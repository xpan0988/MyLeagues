use std::collections::HashMap;

use rusqlite::Connection;

use crate::domain::stats::{RunePageKey, TrackedMatchSample};
use crate::error::AppResult;

pub struct StatisticsRepository<'connection> {
    connection: &'connection Connection,
}

impl<'connection> StatisticsRepository<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn load_matches(&self, puuid: &str) -> AppResult<Vec<TrackedMatchSample>> {
        let mut statement = self.connection.prepare(
            "SELECT m.match_id, pm.champion_id, m.queue_id, m.patch, m.season_key,
                    m.game_creation, m.game_duration, pm.win, pm.kills, pm.deaths, pm.assists,
                    pm.double_kills, pm.triple_kills, pm.quadra_kills, pm.penta_kills,
                    pm.total_minions_killed + pm.neutral_minions_killed,
                    pm.keystone_id, pm.summoner1_id, pm.summoner2_id,
                    pm.primary_style_id, pm.secondary_style_id
             FROM player_matches pm
             JOIN matches m ON m.match_id = pm.match_id
             WHERE pm.puuid = ?1
             ORDER BY m.game_creation DESC, m.match_id",
        )?;
        let rows = statement.query_map([puuid], |row| {
            Ok((
                TrackedMatchSample {
                    match_id: row.get(0)?,
                    champion_id: row.get(1)?,
                    queue_id: row.get(2)?,
                    patch: row.get(3)?,
                    season_key: row.get(4)?,
                    game_creation: row.get(5)?,
                    duration_seconds: nonnegative(row.get(6)?),
                    win: row.get(7)?,
                    kills: nonnegative(row.get(8)?),
                    deaths: nonnegative(row.get(9)?),
                    assists: nonnegative(row.get(10)?),
                    double_kills: nonnegative(row.get(11)?),
                    triple_kills: nonnegative(row.get(12)?),
                    quadra_kills: nonnegative(row.get(13)?),
                    penta_kills: nonnegative(row.get(14)?),
                    minions: nonnegative(row.get(15)?),
                    keystone_id: row.get(16)?,
                    rune_page: None,
                    summoner_spell_ids: [row.get(17)?, row.get(18)?],
                    core_item_ids: Vec::new(),
                    boot_item_id: None,
                },
                row.get::<_, Option<i64>>(19)?,
                row.get::<_, Option<i64>>(20)?,
            ))
        })?;
        let loaded: Vec<_> = rows.collect::<Result<_, _>>()?;
        let mut styles = HashMap::new();
        let mut matches = Vec::with_capacity(loaded.len());
        for (sample, primary, secondary) in loaded {
            styles.insert(sample.match_id.clone(), (primary, secondary));
            matches.push(sample);
        }

        let mut core_items: HashMap<String, Vec<i64>> = HashMap::new();
        let mut boots: HashMap<String, i64> = HashMap::new();
        let mut item_statement = self.connection.prepare(
            "SELECT pmi.match_id, pmi.item_id, si.classification
             FROM player_match_items pmi
             JOIN static_data_versions version ON version.is_active = 1
             JOIN static_items si ON si.version = version.version AND si.item_id = pmi.item_id
             WHERE pmi.puuid = ?1 AND si.classification IN ('core', 'boot')
             ORDER BY pmi.match_id, pmi.slot",
        )?;
        let item_rows = item_statement.query_map([puuid], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in item_rows {
            let (match_id, item_id, classification) = row?;
            if classification == "boot" {
                boots.entry(match_id).or_insert(item_id);
            } else {
                core_items.entry(match_id).or_default().push(item_id);
            }
        }

        #[derive(Default)]
        struct RuneParts {
            primary: Vec<(i64, i64)>,
            secondary: Vec<(i64, i64)>,
            shards: Vec<(i64, i64)>,
        }
        let mut rune_parts: HashMap<String, RuneParts> = HashMap::new();
        let mut rune_statement = self.connection.prepare(
            "SELECT match_id, selection_type, slot, rune_id FROM player_match_runes
             WHERE puuid = ?1 ORDER BY match_id, selection_type, slot",
        )?;
        let rune_rows = rune_statement.query_map([puuid], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        for row in rune_rows {
            let (match_id, kind, slot, rune_id) = row?;
            let parts = rune_parts.entry(match_id).or_default();
            match kind.as_str() {
                "primary" => parts.primary.push((slot, rune_id)),
                "secondary" => parts.secondary.push((slot, rune_id)),
                "stat_shard" => parts.shards.push((slot, rune_id)),
                _ => {}
            }
        }
        for sample in &mut matches {
            sample.core_item_ids = core_items.remove(&sample.match_id).unwrap_or_default();
            sample.boot_item_id = boots.remove(&sample.match_id);
            if let (Some((Some(primary_style_id), Some(secondary_style_id))), Some(mut parts)) = (
                styles.remove(&sample.match_id),
                rune_parts.remove(&sample.match_id),
            ) {
                parts.primary.sort_by_key(|value| value.0);
                parts.secondary.sort_by_key(|value| value.0);
                parts.shards.sort_by_key(|value| value.0);
                sample.rune_page = Some(RunePageKey {
                    primary_style_id,
                    primary_rune_ids: parts.primary.into_iter().map(|value| value.1).collect(),
                    secondary_style_id,
                    secondary_rune_ids: parts.secondary.into_iter().map(|value| value.1).collect(),
                    stat_shard_ids: parts.shards.into_iter().map(|value| value.1).collect(),
                });
            }
        }
        Ok(matches)
    }

    pub fn mastery(&self, puuid: &str) -> AppResult<HashMap<i64, (i64, i64)>> {
        let mut statement = self.connection.prepare(
            "SELECT champion_id, mastery_points, mastery_level
             FROM champion_mastery WHERE puuid = ?1",
        )?;
        let rows = statement.query_map([puuid], |row| {
            Ok((row.get::<_, i64>(0)?, (row.get(1)?, row.get(2)?)))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

fn nonnegative(value: i64) -> u64 {
    value.max(0) as u64
}
