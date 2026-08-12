use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::error::{AppError, AppResult};

pub const TEN_MINUTE_MS: i64 = 600_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaningSnapshot {
    pub match_id: String,
    pub puuid: String,
    pub frame_timestamp_ms: i64,
    pub lane_minions: i64,
    pub neutral_minions: i64,
    pub total_gold: i64,
    pub experience: i64,
    pub level: i64,
}

pub struct TimelineRepository;

impl TimelineRepository {
    pub fn enqueue_eligible(connection: &Connection, puuid: &str) -> AppResult<usize> {
        Ok(connection.execute(
            "INSERT OR IGNORE INTO timeline_sync_queue (match_id, puuid)
             SELECT m.match_id, pm.puuid
             FROM matches m JOIN player_matches pm ON pm.match_id = m.match_id
             LEFT JOIN match_laning_snapshots snapshot
               ON snapshot.match_id = m.match_id AND snapshot.puuid = pm.puuid AND snapshot.minute = 10
             WHERE pm.puuid = ?1 AND m.queue_id IN (400, 420, 430)
               AND m.game_duration >= 600 AND snapshot.match_id IS NULL",
            [puuid],
        )?)
    }

    pub fn resume_interrupted(connection: &Connection, puuid: &str) -> AppResult<usize> {
        Ok(connection.execute(
            "UPDATE timeline_sync_queue SET status = 'pending', updated_at = CURRENT_TIMESTAMP
             WHERE puuid = ?1 AND status = 'fetching'",
            [puuid],
        )?)
    }

    pub fn claim_next(connection: &mut Connection, puuid: &str) -> AppResult<Option<String>> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let match_id = transaction
            .query_row(
                "SELECT match_id FROM timeline_sync_queue
                 WHERE puuid = ?1 AND status IN ('pending', 'error') AND attempts < 3
                 ORDER BY discovered_at, match_id LIMIT 1",
                [puuid],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(match_id) = &match_id {
            transaction.execute(
                "UPDATE timeline_sync_queue SET status = 'fetching', attempts = attempts + 1,
                 updated_at = CURRENT_TIMESTAMP WHERE puuid = ?1 AND match_id = ?2",
                params![puuid, match_id],
            )?;
        }
        transaction.commit()?;
        Ok(match_id)
    }

    pub fn participant_id(
        connection: &Connection,
        match_id: &str,
        puuid: &str,
    ) -> AppResult<Option<i64>> {
        Ok(connection
            .query_row(
                "SELECT participant_id FROM player_matches WHERE match_id = ?1 AND puuid = ?2",
                params![match_id, puuid],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    pub fn insert_snapshot(
        connection: &mut Connection,
        snapshot: &LaningSnapshot,
    ) -> AppResult<bool> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO match_laning_snapshots (
                match_id, puuid, minute, frame_timestamp_ms, lane_minions, neutral_minions,
                total_gold, experience, level
             ) VALUES (?1, ?2, 10, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                snapshot.match_id,
                snapshot.puuid,
                snapshot.frame_timestamp_ms,
                snapshot.lane_minions,
                snapshot.neutral_minions,
                snapshot.total_gold,
                snapshot.experience,
                snapshot.level,
            ],
        )?;
        transaction.execute(
            "UPDATE timeline_sync_queue SET status = 'complete', last_error = NULL,
             updated_at = CURRENT_TIMESTAMP WHERE puuid = ?1 AND match_id = ?2",
            params![snapshot.puuid, snapshot.match_id],
        )?;
        transaction.commit()?;
        Ok(inserted == 1)
    }

    pub fn mark_error(
        connection: &Connection,
        puuid: &str,
        match_id: &str,
        error: &str,
    ) -> AppResult<()> {
        connection.execute(
            "UPDATE timeline_sync_queue SET status = 'error', last_error = ?3,
             updated_at = CURRENT_TIMESTAMP WHERE puuid = ?1 AND match_id = ?2",
            params![puuid, match_id, error],
        )?;
        Ok(())
    }

    pub fn mark_unsupported(
        connection: &Connection,
        puuid: &str,
        match_id: &str,
        reason: &str,
    ) -> AppResult<()> {
        connection.execute(
            "UPDATE timeline_sync_queue SET status = 'unsupported', last_error = ?3,
             updated_at = CURRENT_TIMESTAMP WHERE puuid = ?1 AND match_id = ?2",
            params![puuid, match_id, reason],
        )?;
        Ok(())
    }

    pub fn coverage(connection: &Connection, puuid: &str) -> AppResult<(u64, u64)> {
        let eligible = connection.query_row(
            "SELECT COUNT(*) FROM matches m JOIN player_matches pm ON pm.match_id = m.match_id
             WHERE pm.puuid = ?1 AND m.queue_id IN (400, 420, 430) AND m.game_duration >= 600",
            [puuid],
            |row| row.get(0),
        )?;
        let covered = connection.query_row(
            "SELECT COUNT(*) FROM match_laning_snapshots WHERE puuid = ?1 AND minute = 10",
            [puuid],
            |row| row.get(0),
        )?;
        Ok((covered, eligible))
    }
}

pub fn validate_snapshot(snapshot: &LaningSnapshot) -> AppResult<()> {
    if snapshot.frame_timestamp_ms < TEN_MINUTE_MS
        || snapshot.lane_minions < 0
        || snapshot.neutral_minions < 0
    {
        return Err(AppError::RiotData(
            "invalid ten-minute timeline fact".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{LaningSnapshot, TimelineRepository};
    use crate::db::Database;
    use crate::db::repositories::account::AccountRepository;
    use crate::db::repositories::matches::MatchRepository;
    use crate::domain::account::Account;
    use crate::domain::match_record::{MatchRecord, PlayerMatch};

    fn seed(database: &Database) -> Result<(), Box<dyn std::error::Error>> {
        let mut connection = database.connection()?;
        AccountRepository::new(&connection).upsert(&Account {
            puuid: "p".into(),
            game_name: "n".into(),
            tag_line: "t".into(),
            summoner_id: None,
            account_region: "americas".into(),
            platform_region: "oc1".into(),
        })?;
        MatchRepository::ingest(
            &mut connection,
            &MatchRecord {
                match_id: "OC1_1".into(),
                game_creation: 1,
                game_end_timestamp: None,
                game_duration_seconds: 700,
                queue_id: 420,
                game_version: "16.1.1".into(),
                patch: "16.1".into(),
                season_key: Some("2026".into()),
            },
            &PlayerMatch {
                match_id: "OC1_1".into(),
                puuid: "p".into(),
                participant_id: Some(1),
                champion_id: 1,
                win: true,
                kills: 0,
                deaths: 0,
                assists: 0,
                double_kills: 0,
                triple_kills: 0,
                quadra_kills: 0,
                penta_kills: 0,
                total_minions_killed: 0,
                neutral_minions_killed: 0,
                gold_earned: 0,
                summoner_spell_ids: [4, 12],
                keystone_id: None,
                primary_style_id: None,
                secondary_style_id: None,
                final_items: vec![],
                rune_selections: vec![],
            },
        )?;
        Ok(())
    }

    #[test]
    fn timeline_facts_are_idempotent_and_coverage_is_honest()
    -> Result<(), Box<dyn std::error::Error>> {
        let database = Database::open_in_memory()?;
        seed(&database)?;
        let mut connection = database.connection()?;
        TimelineRepository::enqueue_eligible(&connection, "p")?;
        let snapshot = LaningSnapshot {
            match_id: "OC1_1".into(),
            puuid: "p".into(),
            frame_timestamp_ms: 600_000,
            lane_minions: 70,
            neutral_minions: 2,
            total_gold: 4000,
            experience: 5000,
            level: 6,
        };
        assert!(TimelineRepository::insert_snapshot(
            &mut connection,
            &snapshot
        )?);
        assert!(!TimelineRepository::insert_snapshot(
            &mut connection,
            &snapshot
        )?);
        assert_eq!(TimelineRepository::coverage(&connection, "p")?, (1, 1));
        Ok(())
    }

    #[test]
    fn interrupted_timeline_work_returns_to_the_persistent_queue()
    -> Result<(), Box<dyn std::error::Error>> {
        let database = Database::open_in_memory()?;
        seed(&database)?;
        let mut connection = database.connection()?;
        TimelineRepository::enqueue_eligible(&connection, "p")?;
        assert_eq!(
            TimelineRepository::claim_next(&mut connection, "p")?.as_deref(),
            Some("OC1_1")
        );
        assert_eq!(TimelineRepository::resume_interrupted(&connection, "p")?, 1);
        assert_eq!(
            TimelineRepository::claim_next(&mut connection, "p")?.as_deref(),
            Some("OC1_1")
        );
        Ok(())
    }
}
