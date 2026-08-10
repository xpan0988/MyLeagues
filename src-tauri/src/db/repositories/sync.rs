use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::error::AppResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedSyncState {
    pub next_match_start: u32,
    pub initial_sync_complete: bool,
    pub last_check_at: Option<String>,
    pub last_successful_sync_at: Option<String>,
    pub last_error: Option<String>,
    pub last_trigger: Option<String>,
}

pub struct SyncRepository;

impl SyncRepository {
    pub fn ensure(connection: &Connection, puuid: &str) -> AppResult<()> {
        connection.execute(
            "INSERT OR IGNORE INTO sync_state (puuid) VALUES (?1)",
            [puuid],
        )?;
        Ok(())
    }

    pub fn get(connection: &Connection, puuid: &str) -> AppResult<PersistedSyncState> {
        let state = connection.query_row(
            "SELECT next_match_start, initial_sync_complete, last_check_at,
                    last_successful_sync_at, last_error, last_trigger
             FROM sync_state WHERE puuid = ?1",
            [puuid],
            |row| {
                Ok(PersistedSyncState {
                    next_match_start: row.get(0)?,
                    initial_sync_complete: row.get(1)?,
                    last_check_at: row.get(2)?,
                    last_successful_sync_at: row.get(3)?,
                    last_error: row.get(4)?,
                    last_trigger: row.get(5)?,
                })
            },
        )?;
        Ok(state)
    }

    pub fn set_status(
        connection: &Connection,
        puuid: &str,
        status: &str,
        error: Option<&str>,
    ) -> AppResult<()> {
        connection.execute(
            "UPDATE sync_state SET status = ?2, last_error = ?3, updated_at = CURRENT_TIMESTAMP
             WHERE puuid = ?1",
            params![puuid, status, error],
        )?;
        Ok(())
    }

    pub fn begin_attempt(connection: &Connection, puuid: &str, trigger: &str) -> AppResult<()> {
        connection.execute(
            "UPDATE sync_state SET status = 'syncing', last_error = NULL,
             last_check_at = CURRENT_TIMESTAMP, last_trigger = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE puuid = ?1",
            params![puuid, trigger],
        )?;
        Ok(())
    }

    pub fn mark_success(connection: &Connection, puuid: &str) -> AppResult<()> {
        connection.execute(
            "UPDATE sync_state SET status = 'success', last_error = NULL,
             last_successful_sync_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE puuid = ?1",
            [puuid],
        )?;
        Ok(())
    }

    pub fn enqueue(
        connection: &mut Connection,
        puuid: &str,
        match_ids: &[String],
    ) -> AppResult<usize> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut inserted = 0;
        for match_id in match_ids {
            inserted += transaction.execute(
                "INSERT OR IGNORE INTO sync_match_queue (match_id, puuid) VALUES (?1, ?2)",
                params![match_id, puuid],
            )?;
        }
        transaction.commit()?;
        Ok(inserted)
    }

    pub fn next_pending(connection: &Connection, puuid: &str) -> AppResult<Option<String>> {
        let match_id = connection
            .query_row(
                "SELECT match_id FROM sync_match_queue
                 WHERE puuid = ?1 AND status IN ('pending', 'error') AND attempts < 3
                 ORDER BY discovered_at, match_id LIMIT 1",
                [puuid],
                |row| row.get(0),
            )
            .optional()?;
        Ok(match_id)
    }

    pub fn claim_pending_batch(
        connection: &mut Connection,
        puuid: &str,
        limit: usize,
    ) -> AppResult<Vec<String>> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let match_ids = {
            let mut statement = transaction.prepare(
                "SELECT match_id FROM sync_match_queue
                 WHERE puuid = ?1 AND status IN ('pending', 'error') AND attempts < 3
                 ORDER BY discovered_at, match_id LIMIT ?2",
            )?;
            statement
                .query_map(params![puuid, limit as i64], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?
        };
        for match_id in &match_ids {
            transaction.execute(
                "UPDATE sync_match_queue SET status = 'fetching', attempts = attempts + 1,
                 updated_at = CURRENT_TIMESTAMP WHERE puuid = ?1 AND match_id = ?2",
                params![puuid, match_id],
            )?;
        }
        transaction.commit()?;
        Ok(match_ids)
    }

    pub fn mark_fetching(connection: &Connection, puuid: &str, match_id: &str) -> AppResult<()> {
        connection.execute(
            "UPDATE sync_match_queue SET status = 'fetching', attempts = attempts + 1,
             updated_at = CURRENT_TIMESTAMP WHERE puuid = ?1 AND match_id = ?2",
            params![puuid, match_id],
        )?;
        Ok(())
    }

    pub fn mark_error(
        connection: &Connection,
        puuid: &str,
        match_id: &str,
        error: &str,
    ) -> AppResult<()> {
        connection.execute(
            "UPDATE sync_match_queue SET status = 'error', last_error = ?3,
             updated_at = CURRENT_TIMESTAMP WHERE puuid = ?1 AND match_id = ?2",
            params![puuid, match_id, error],
        )?;
        Ok(())
    }

    pub fn resume_interrupted(connection: &Connection, puuid: &str) -> AppResult<usize> {
        let changed = connection.execute(
            "UPDATE sync_match_queue SET status = 'pending', updated_at = CURRENT_TIMESTAMP
             WHERE puuid = ?1 AND status = 'fetching'",
            [puuid],
        )?;
        Ok(changed)
    }

    pub fn advance_discovery(
        connection: &Connection,
        puuid: &str,
        next_start: u32,
        complete: bool,
    ) -> AppResult<()> {
        connection.execute(
            "UPDATE sync_state SET next_match_start = ?2, initial_sync_complete = ?3,
             updated_at = CURRENT_TIMESTAMP WHERE puuid = ?1",
            params![puuid, next_start, complete],
        )?;
        Ok(())
    }

    pub fn queue_counts(connection: &Connection, puuid: &str) -> AppResult<(u64, u64)> {
        let total = connection.query_row(
            "SELECT COUNT(*) FROM sync_match_queue WHERE puuid = ?1",
            [puuid],
            |row| row.get(0),
        )?;
        let complete = connection.query_row(
            "SELECT COUNT(*) FROM sync_match_queue WHERE puuid = ?1 AND status = 'complete'",
            [puuid],
            |row| row.get(0),
        )?;
        Ok((complete, total))
    }
}

#[cfg(test)]
mod tests {
    use crate::db::Database;
    use crate::db::repositories::account::AccountRepository;
    use crate::domain::account::Account;

    use super::SyncRepository;

    #[test]
    fn interrupted_fetch_resumes_from_persisted_queue() -> Result<(), Box<dyn std::error::Error>> {
        let database = Database::open_in_memory()?;
        let mut connection = database.connection()?;
        AccountRepository::new(&connection).upsert(&Account {
            puuid: "test-puuid".to_owned(),
            game_name: "Player".to_owned(),
            tag_line: "OC1".to_owned(),
            summoner_id: None,
            account_region: "sea".to_owned(),
            platform_region: "oc1".to_owned(),
        })?;
        SyncRepository::ensure(&connection, "test-puuid")?;
        SyncRepository::enqueue(&mut connection, "test-puuid", &["OC1_1".to_owned()])?;
        SyncRepository::mark_fetching(&connection, "test-puuid", "OC1_1")?;

        assert_eq!(
            SyncRepository::resume_interrupted(&connection, "test-puuid")?,
            1
        );
        assert_eq!(
            SyncRepository::next_pending(&connection, "test-puuid")?.as_deref(),
            Some("OC1_1")
        );
        Ok(())
    }

    #[test]
    fn bounded_claims_are_unique_and_never_exceed_concurrency_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let database = Database::open_in_memory()?;
        let mut connection = database.connection()?;
        AccountRepository::new(&connection).upsert(&Account {
            puuid: "test-puuid".to_owned(),
            game_name: "Player".to_owned(),
            tag_line: "OC1".to_owned(),
            summoner_id: None,
            account_region: "americas".to_owned(),
            platform_region: "oc1".to_owned(),
        })?;
        SyncRepository::ensure(&connection, "test-puuid")?;
        let ids = (0..8)
            .map(|index| format!("OC1_{index}"))
            .collect::<Vec<_>>();
        SyncRepository::enqueue(&mut connection, "test-puuid", &ids)?;
        let first = SyncRepository::claim_pending_batch(&mut connection, "test-puuid", 5)?;
        let second = SyncRepository::claim_pending_batch(&mut connection, "test-puuid", 5)?;
        assert_eq!((first.len(), second.len()), (5, 3));
        let unique = first
            .iter()
            .chain(&second)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), 8);
        Ok(())
    }
}
