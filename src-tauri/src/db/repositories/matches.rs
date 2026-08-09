use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use std::time::{Duration, Instant};

use crate::db::repositories::aggregates::AggregateRepository;
use crate::domain::aggregates::{ALL_QUEUES, AggregateScope, NORMAL_QUEUES};
use crate::domain::items::FinalItem;
use crate::domain::match_record::{MatchRecord, PlayerMatch};
use crate::domain::runes::{RuneSelection, RuneSelectionType};
use crate::error::AppResult;

#[derive(Clone, Debug)]
pub struct MatchListRow {
    pub match_id: String,
    pub champion_id: i64,
    pub win: bool,
    pub queue_id: i64,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub duration_seconds: i64,
    pub keystone_id: Option<i64>,
    pub summoner_spell_ids: [i64; 2],
    pub game_creation: i64,
    pub patch: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct IngestTiming {
    pub total: Duration,
    pub aggregate: Duration,
    pub queue_update: Duration,
}

pub struct MatchRepository;

impl MatchRepository {
    pub fn page(
        connection: &Connection,
        puuid: &str,
        champion_id: Option<i64>,
        scope: &AggregateScope,
        offset: u32,
        limit: u32,
    ) -> AppResult<Vec<MatchListRow>> {
        let mut statement = connection.prepare(
            "SELECT m.match_id, pm.champion_id, pm.win, m.queue_id,
                    pm.kills, pm.deaths, pm.assists, m.game_duration,
                    pm.keystone_id, pm.summoner1_id, pm.summoner2_id,
                    m.game_creation, m.patch
             FROM matches m JOIN player_matches pm ON pm.match_id = m.match_id
             WHERE pm.puuid = ?1
               AND (?2 IS NULL OR pm.champion_id = ?2)
               AND (?3 = ?4 OR (?3 = ?5 AND m.queue_id IN (400, 430)) OR m.queue_id = ?3)
               AND (?6 = '' OR m.patch = ?6)
               AND (?7 = '' OR COALESCE(m.season_key, '') = ?7)
             ORDER BY m.game_creation DESC, m.match_id DESC
             LIMIT ?8 OFFSET ?9",
        )?;
        statement
            .query_map(
                params![
                    puuid,
                    champion_id,
                    scope.queue_scope,
                    ALL_QUEUES,
                    NORMAL_QUEUES,
                    scope.patch,
                    scope.season,
                    limit,
                    offset,
                ],
                |row| {
                    Ok(MatchListRow {
                        match_id: row.get(0)?,
                        champion_id: row.get(1)?,
                        win: row.get(2)?,
                        queue_id: row.get(3)?,
                        kills: row.get(4)?,
                        deaths: row.get(5)?,
                        assists: row.get(6)?,
                        duration_seconds: row.get(7)?,
                        keystone_id: row.get(8)?,
                        summoner_spell_ids: [row.get(9)?, row.get(10)?],
                        game_creation: row.get(11)?,
                        patch: row.get(12)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn exists(connection: &Connection, match_id: &str) -> AppResult<bool> {
        let found = connection
            .query_row(
                "SELECT 1 FROM matches WHERE match_id = ?1",
                [match_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        Ok(found)
    }

    pub fn detail(
        connection: &Connection,
        puuid: &str,
        match_id: &str,
    ) -> AppResult<Option<(MatchRecord, PlayerMatch)>> {
        let result = connection
            .query_row(
                "SELECT m.match_id, m.game_creation, m.game_end_timestamp, m.game_duration,
                    m.queue_id, m.game_version, m.patch, m.season_key,
                    pm.champion_id, pm.win, pm.kills, pm.deaths, pm.assists,
                    pm.double_kills, pm.triple_kills, pm.quadra_kills, pm.penta_kills,
                    pm.total_minions_killed, pm.neutral_minions_killed, pm.gold_earned,
                    pm.summoner1_id, pm.summoner2_id, pm.keystone_id,
                    pm.primary_style_id, pm.secondary_style_id
             FROM matches m JOIN player_matches pm ON pm.match_id = m.match_id
             WHERE m.match_id = ?1 AND pm.puuid = ?2",
                params![match_id, puuid],
                |row| {
                    Ok((
                        MatchRecord {
                            match_id: row.get(0)?,
                            game_creation: row.get(1)?,
                            game_end_timestamp: row.get(2)?,
                            game_duration_seconds: row.get(3)?,
                            queue_id: row.get(4)?,
                            game_version: row.get(5)?,
                            patch: row.get(6)?,
                            season_key: row.get(7)?,
                        },
                        PlayerMatch {
                            match_id: row.get(0)?,
                            puuid: puuid.to_owned(),
                            champion_id: row.get(8)?,
                            win: row.get(9)?,
                            kills: row.get(10)?,
                            deaths: row.get(11)?,
                            assists: row.get(12)?,
                            double_kills: row.get(13)?,
                            triple_kills: row.get(14)?,
                            quadra_kills: row.get(15)?,
                            penta_kills: row.get(16)?,
                            total_minions_killed: row.get(17)?,
                            neutral_minions_killed: row.get(18)?,
                            gold_earned: row.get(19)?,
                            summoner_spell_ids: [row.get(20)?, row.get(21)?],
                            keystone_id: row.get(22)?,
                            primary_style_id: row.get(23)?,
                            secondary_style_id: row.get(24)?,
                            final_items: Vec::new(),
                            rune_selections: Vec::new(),
                        },
                    ))
                },
            )
            .optional()?;
        let Some((record, mut player)) = result else {
            return Ok(None);
        };
        let mut item_statement = connection.prepare(
            "SELECT item_id, slot, classification FROM player_match_items
             WHERE match_id = ?1 AND puuid = ?2 ORDER BY slot",
        )?;
        player.final_items = item_statement
            .query_map(params![match_id, puuid], |row| {
                Ok(FinalItem {
                    item_id: row.get(0)?,
                    slot: row.get(1)?,
                    classification: row.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        let mut rune_statement = connection.prepare(
            "SELECT selection_type, slot, rune_id, style_id FROM player_match_runes
             WHERE match_id = ?1 AND puuid = ?2 ORDER BY selection_type, slot",
        )?;
        player.rune_selections = rune_statement
            .query_map(params![match_id, puuid], |row| {
                let kind: String = row.get(0)?;
                let selection_type = match kind.as_str() {
                    "primary" => RuneSelectionType::Primary,
                    "secondary" => RuneSelectionType::Secondary,
                    _ => RuneSelectionType::StatShard,
                };
                Ok(RuneSelection {
                    selection_type,
                    slot: row.get(1)?,
                    rune_id: row.get(2)?,
                    style_id: row.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(Some((record, player)))
    }

    pub fn ingest(
        connection: &mut Connection,
        match_record: &MatchRecord,
        player: &PlayerMatch,
    ) -> AppResult<bool> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = ingest_facts(&transaction, match_record, player)?;
        if inserted {
            AggregateRepository::increment(&transaction, match_record, player)?;
        }
        transaction.commit()?;
        Ok(inserted)
    }

    pub fn ingest_synced(
        connection: &mut Connection,
        match_record: &MatchRecord,
        player: &PlayerMatch,
    ) -> AppResult<bool> {
        Ok(Self::ingest_synced_timed(connection, match_record, player)?.0)
    }

    pub fn ingest_synced_timed(
        connection: &mut Connection,
        match_record: &MatchRecord,
        player: &PlayerMatch,
    ) -> AppResult<(bool, IngestTiming)> {
        let total_started = Instant::now();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = ingest_facts(&transaction, match_record, player)?;
        let mut aggregate = Duration::ZERO;
        if inserted {
            let aggregate_started = Instant::now();
            AggregateRepository::increment(&transaction, match_record, player)?;
            aggregate = aggregate_started.elapsed();
        }
        let queue_started = Instant::now();
        let updated = transaction.execute(
            "UPDATE sync_match_queue SET status = 'complete', last_error = NULL,
             updated_at = CURRENT_TIMESTAMP WHERE puuid = ?1 AND match_id = ?2",
            params![player.puuid, match_record.match_id],
        )?;
        if updated != 1 {
            return Err(rusqlite::Error::QueryReturnedNoRows.into());
        }
        let queue_update = queue_started.elapsed();
        transaction.commit()?;
        Ok((
            inserted,
            IngestTiming {
                total: total_started.elapsed(),
                aggregate,
                queue_update,
            },
        ))
    }
}

fn ingest_facts(
    transaction: &Transaction<'_>,
    match_record: &MatchRecord,
    player: &PlayerMatch,
) -> AppResult<bool> {
    let inserted = transaction.execute(
        "INSERT OR IGNORE INTO matches (
                match_id, game_creation, game_end_timestamp, game_duration, queue_id,
                game_version, patch, season_key
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            match_record.match_id,
            match_record.game_creation,
            match_record.game_end_timestamp,
            match_record.game_duration_seconds,
            match_record.queue_id,
            match_record.game_version,
            match_record.patch,
            match_record.season_key,
        ],
    )?;

    if inserted == 0 {
        return Ok(false);
    }

    transaction.execute(
        "INSERT INTO player_matches (
                match_id, puuid, champion_id, win, kills, deaths, assists,
                double_kills, triple_kills, quadra_kills, penta_kills,
                total_minions_killed, neutral_minions_killed, gold_earned,
                summoner1_id, summoner2_id, keystone_id, primary_style_id, secondary_style_id
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19
             )",
        params![
            player.match_id,
            player.puuid,
            player.champion_id,
            player.win,
            player.kills,
            player.deaths,
            player.assists,
            player.double_kills,
            player.triple_kills,
            player.quadra_kills,
            player.penta_kills,
            player.total_minions_killed,
            player.neutral_minions_killed,
            player.gold_earned,
            player.summoner_spell_ids[0],
            player.summoner_spell_ids[1],
            player.keystone_id,
            player.primary_style_id,
            player.secondary_style_id,
        ],
    )?;

    for item in &player.final_items {
        transaction.execute(
            "INSERT INTO player_match_items (
                    match_id, puuid, item_id, slot, classification
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                player.match_id,
                player.puuid,
                item.item_id,
                item.slot,
                item.classification,
            ],
        )?;
    }

    for rune in &player.rune_selections {
        let selection_type = match rune.selection_type {
            RuneSelectionType::Primary => "primary",
            RuneSelectionType::Secondary => "secondary",
            RuneSelectionType::StatShard => "stat_shard",
        };
        transaction.execute(
            "INSERT INTO player_match_runes (
                    match_id, puuid, selection_type, slot, rune_id, style_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                player.match_id,
                player.puuid,
                selection_type,
                rune.slot,
                rune.rune_id,
                rune.style_id,
            ],
        )?;
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::db::Database;
    use crate::db::repositories::account::AccountRepository;
    use crate::db::repositories::aggregates::AggregateRepository;
    use crate::db::repositories::sync::SyncRepository;
    use crate::domain::account::Account;
    use crate::domain::aggregates::{ALL_QUEUES, AggregateScope};
    use crate::domain::match_record::{MatchRecord, PlayerMatch};

    use super::MatchRepository;

    fn account() -> Account {
        Account {
            puuid: "test-puuid".to_owned(),
            game_name: "Player".to_owned(),
            tag_line: "OC1".to_owned(),
            summoner_id: Some("summoner-id".to_owned()),
            account_region: "sea".to_owned(),
            platform_region: "oc1".to_owned(),
        }
    }

    fn match_record(id: &str, queue: i64, patch: &str) -> MatchRecord {
        MatchRecord {
            match_id: id.to_owned(),
            game_creation: 1_700_000_000_000,
            game_end_timestamp: Some(1_700_001_800_000),
            game_duration_seconds: 1_800,
            queue_id: queue,
            game_version: "16.15.1".to_owned(),
            patch: patch.to_owned(),
            season_key: Some("2026".to_owned()),
        }
    }

    fn player(id: &str, champion_id: i64, kills: i64) -> PlayerMatch {
        PlayerMatch {
            match_id: id.to_owned(),
            puuid: "test-puuid".to_owned(),
            champion_id,
            win: true,
            kills,
            deaths: 2,
            assists: 8,
            double_kills: 1,
            triple_kills: 0,
            quadra_kills: 0,
            penta_kills: 0,
            total_minions_killed: 200,
            neutral_minions_killed: 8,
            gold_earned: 12_000,
            summoner_spell_ids: [4, 12],
            keystone_id: Some(8010),
            primary_style_id: Some(8000),
            secondary_style_id: Some(8400),
            final_items: Vec::new(),
            rune_selections: Vec::new(),
        }
    }

    fn setup() -> Result<Database, Box<dyn std::error::Error>> {
        let database = Database::open_in_memory()?;
        {
            let connection = database.connection()?;
            AccountRepository::new(&connection).upsert(&account())?;
        }
        Ok(database)
    }

    fn all_scope() -> AggregateScope {
        AggregateScope {
            queue_scope: ALL_QUEUES,
            patch: String::new(),
            season: String::new(),
        }
    }

    #[test]
    fn duplicate_match_ingestion_increments_aggregate_once()
    -> Result<(), Box<dyn std::error::Error>> {
        let database = setup()?;
        let mut connection = database.connection()?;
        let match_record = match_record("OC1_1", 420, "16.15");
        let player = player("OC1_1", 82, 10);

        assert!(MatchRepository::ingest(
            &mut connection,
            &match_record,
            &player
        )?);
        assert!(!MatchRepository::ingest(
            &mut connection,
            &match_record,
            &player
        )?);
        let count: i64 =
            connection.query_row("SELECT COUNT(*) FROM matches", [], |row| row.get(0))?;
        assert_eq!(count, 1);
        let aggregate = AggregateRepository::career(&connection, "test-puuid", &all_scope())?;
        assert_eq!(
            (aggregate.games, aggregate.kills, aggregate.playtime_seconds),
            (1, 10, 1_800)
        );
        Ok(())
    }

    #[test]
    fn synced_ingestion_rolls_back_when_queue_completion_fails()
    -> Result<(), Box<dyn std::error::Error>> {
        let database = setup()?;
        let mut connection = database.connection()?;
        let result = MatchRepository::ingest_synced(
            &mut connection,
            &match_record("OC1_rollback", 420, "16.15"),
            &player("OC1_rollback", 82, 7),
        );
        assert!(result.is_err());
        let facts: i64 =
            connection.query_row("SELECT COUNT(*) FROM matches", [], |row| row.get(0))?;
        let aggregates: i64 =
            connection.query_row("SELECT COUNT(*) FROM career_aggregates", [], |row| {
                row.get(0)
            })?;
        assert_eq!((facts, aggregates), (0, 0));
        Ok(())
    }

    #[test]
    fn scopes_champions_and_highest_kills_are_isolated_and_rebuildable()
    -> Result<(), Box<dyn std::error::Error>> {
        let database = setup()?;
        let mut connection = database.connection()?;
        for (record, participant) in [
            (match_record("OC1_1", 420, "16.15"), player("OC1_1", 82, 10)),
            (match_record("OC1_2", 420, "16.15"), player("OC1_2", 82, 18)),
            (match_record("OC1_3", 450, "16.14"), player("OC1_3", 1, 6)),
        ] {
            assert!(MatchRepository::ingest(
                &mut connection,
                &record,
                &participant
            )?);
        }
        let ranked_patch = AggregateScope {
            queue_scope: 420,
            patch: "16.15".to_owned(),
            season: String::new(),
        };
        let aram_patch = AggregateScope {
            queue_scope: 450,
            patch: "16.14".to_owned(),
            season: String::new(),
        };
        assert_eq!(
            AggregateRepository::career(&connection, "test-puuid", &ranked_patch)?.games,
            2
        );
        assert_eq!(
            AggregateRepository::career(&connection, "test-puuid", &aram_patch)?.games,
            1
        );
        let champion = AggregateRepository::champion(&connection, "test-puuid", 82, &all_scope())?;
        assert_eq!(
            (champion.games, champion.kills, champion.highest_kills),
            (2, 28, 18)
        );
        assert_eq!(
            AggregateRepository::champion(&connection, "test-puuid", 1, &all_scope())?.games,
            1
        );

        let before = AggregateRepository::career(&connection, "test-puuid", &all_scope())?;
        AggregateRepository::rebuild(&mut connection)?;
        let after = AggregateRepository::career(&connection, "test-puuid", &all_scope())?;
        assert_eq!(before, after);
        let dynamic: (i64, i64, i64, i64, i64, i64, i64) = connection.query_row(
            "SELECT COUNT(*), SUM(win), SUM(kills), SUM(deaths), SUM(assists),
                    SUM(game_duration), SUM(penta_kills)
             FROM player_matches JOIN matches USING(match_id)",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )?;
        assert_eq!(
            dynamic,
            (
                after.games,
                after.wins,
                after.kills,
                after.deaths,
                after.assists,
                after.playtime_seconds,
                after.penta_kills
            )
        );
        Ok(())
    }

    #[test]
    fn synced_duplicate_completes_queue_without_incrementing_twice()
    -> Result<(), Box<dyn std::error::Error>> {
        let database = setup()?;
        let mut connection = database.connection()?;
        SyncRepository::ensure(&connection, "test-puuid")?;
        let record = match_record("OC1_sync", 420, "16.15");
        let participant = player("OC1_sync", 82, 9);
        SyncRepository::enqueue(&mut connection, "test-puuid", &[record.match_id.clone()])?;
        assert!(MatchRepository::ingest_synced(
            &mut connection,
            &record,
            &participant
        )?);
        connection.execute(
            "UPDATE sync_match_queue SET status = 'pending' WHERE match_id = ?1 AND puuid = ?2",
            rusqlite::params![record.match_id, participant.puuid],
        )?;
        assert!(!MatchRepository::ingest_synced(
            &mut connection,
            &record,
            &participant
        )?);
        let aggregate = AggregateRepository::career(&connection, "test-puuid", &all_scope())?;
        assert_eq!(aggregate.games, 1);
        let status: String = connection.query_row(
            "SELECT status FROM sync_match_queue WHERE match_id = ?1",
            [record.match_id],
            |row| row.get(0),
        )?;
        assert_eq!(status, "complete");
        Ok(())
    }
}
