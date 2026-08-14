use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::domain::items::{ItemMetadata, ItemTimelineEvent, first_completed_core_items};
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
                    pm.primary_style_id, pm.secondary_style_id, pm.participant_id
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
                row.get::<_, Option<i64>>(21)?,
            ))
        })?;
        let loaded: Vec<_> = rows.collect::<Result<_, _>>()?;
        let mut styles = HashMap::new();
        let mut participant_ids = HashMap::new();
        let mut matches = Vec::with_capacity(loaded.len());
        for (sample, primary, secondary, participant_id) in loaded {
            styles.insert(sample.match_id.clone(), (primary, secondary));
            // Historical summary rows legitimately predate participant roster
            // enrichment. Preserve their missing identity as None; a build
            // path can only use Timeline events bound to the local player.
            participant_ids.insert(sample.match_id.clone(), participant_id);
            matches.push(sample);
        }

        let mut boots: HashMap<String, i64> = HashMap::new();
        let mut item_statement = self.connection.prepare(
            "SELECT pmi.match_id, pmi.item_id, si.classification
             FROM player_match_items pmi
             JOIN static_data_versions version ON version.is_active = 1
             JOIN static_items si ON si.version = version.version AND si.item_id = pmi.item_id
             WHERE pmi.puuid = ?1 AND si.classification = 'boot'
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
            }
        }

        // Completion paths come only from normalized Timeline item events. Do
        // not fall back to final Match-V5 inventory slots: their order is not
        // purchase order and would silently change the product meaning.
        let mut item_events_by_match: HashMap<String, Vec<ItemTimelineEvent>> = HashMap::new();
        let mut item_event_statement = self.connection.prepare(
            "SELECT event.match_id,event.source_event_id,event.timestamp_ms,event.participant_id,
                    event.event_type,event.item_id,event.before_item_id,event.after_item_id
             FROM timeline_item_events event
             JOIN player_matches player ON player.match_id=event.match_id
               AND player.puuid=?1 AND player.participant_id=event.participant_id
             ORDER BY event.match_id,event.timestamp_ms,event.source_event_id",
        )?;
        let item_event_rows = item_event_statement.query_map([puuid], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ItemTimelineEvent {
                    source_id: row.get(1)?,
                    timestamp_ms: row.get(2)?,
                    participant_id: row.get(3)?,
                    event_type: row.get(4)?,
                    item_id: row.get(5)?,
                    before_item_id: row.get(6)?,
                    after_item_id: row.get(7)?,
                },
            ))
        })?;
        for row in item_event_rows {
            let (match_id, event) = row?;
            item_events_by_match
                .entry(match_id)
                .or_default()
                .push(event);
        }
        let mut core_item_ids = HashSet::new();
        let mut core_metadata = self.connection.prepare(
            "SELECT item_id,name,description,icon,gold,purchasable,tags_json,from_json,into_json,maps_json
             FROM static_items item
             JOIN static_data_versions version ON version.version=item.version AND version.is_active=1
             ORDER BY item_id",
        )?;
        for row in core_metadata.query_map([], |row| {
            Ok(ItemMetadata {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                icon: row.get(3)?,
                gold: row.get(4)?,
                purchasable: row.get(5)?,
                tags: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                from: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                into: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
                maps: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
            })
        })? {
            let item = row?;
            if item.is_valid_core_item() {
                core_item_ids.insert(item.id);
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
            // Historical summary rows can legitimately predate roster
            // enrichment and therefore have no participant identity. They
            // cannot be joined to a player-bound Timeline item event; leave
            // their build path unavailable rather than inventing participant 0.
            sample.core_item_ids = participant_ids
                .remove(&sample.match_id)
                .flatten()
                .map(|participant_id| {
                    first_completed_core_items(
                        item_events_by_match
                            .remove(&sample.match_id)
                            .unwrap_or_default(),
                        participant_id,
                        |item_id| core_item_ids.contains(&item_id),
                    )
                })
                .unwrap_or_default();
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

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::StatisticsRepository;
    use crate::db::Database;

    fn insert_match_with_player(
        connection: &rusqlite::Connection,
        match_id: &str,
        participant_id: Option<i64>,
        game_creation: i64,
    ) {
        connection
            .execute(
                "INSERT INTO matches(match_id,game_creation,game_duration,queue_id,game_version,patch)
                 VALUES(?1,?2,1800,420,'16.16.1','16.16')",
                params![match_id, game_creation],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO player_matches(
                    match_id,puuid,champion_id,win,kills,deaths,assists,
                    summoner1_id,summoner2_id,participant_id
                 ) VALUES(?1,'player',1,1,1,0,1,4,14,?2)",
                params![match_id, participant_id],
            )
            .unwrap();
    }

    #[test]
    fn historical_missing_participant_identity_never_becomes_participant_zero() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        connection
            .execute(
                "INSERT INTO accounts(puuid,game_name,tag_line,account_region,platform_region)
                 VALUES('player','Name','TAG','americas','oc1')",
                [],
            )
            .unwrap();
        insert_match_with_player(&connection, "missing-participant", None, 2);
        insert_match_with_player(&connection, "player-bound", Some(1), 1);
        connection
            .execute(
                "INSERT INTO static_data_versions(version,is_active) VALUES('16.16.1',1)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO static_items(
                    version,item_id,name,description,icon,gold,purchasable,
                    tags_json,from_json,into_json,maps_json,classification
                 ) VALUES('16.16.1',4000,'Core','', '',3000,1,'[]','[]','[]','{}','core')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO timeline_item_events(
                    match_id,source_event_id,timestamp_ms,participant_id,event_type,item_id
                 ) VALUES('player-bound','purchase',1000,1,'ITEM_PURCHASED',4000)",
                [],
            )
            .unwrap();

        let samples = StatisticsRepository::new(&connection)
            .load_matches("player")
            .unwrap();
        let missing = samples
            .iter()
            .find(|sample| sample.match_id == "missing-participant")
            .unwrap();
        let bound = samples
            .iter()
            .find(|sample| sample.match_id == "player-bound")
            .unwrap();

        // The nullable roster identity decodes safely and cannot join a
        // participant-less row to local-player Timeline facts.
        assert!(missing.core_item_ids.is_empty());
        assert_eq!(bound.core_item_ids, vec![4000]);
    }
}
