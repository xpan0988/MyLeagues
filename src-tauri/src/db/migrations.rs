use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::error::AppResult;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("../../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "aggregate_cache",
        sql: include_str!("../../migrations/0002_aggregate_cache.sql"),
    },
    Migration {
        version: 3,
        name: "static_data_cache",
        sql: include_str!("../../migrations/0003_static_data_cache.sql"),
    },
    Migration {
        version: 4,
        name: "sync_laning_timeline",
        sql: include_str!("../../migrations/0004_sync_laning_timeline.sql"),
    },
];

pub fn run(connection: &mut Connection) -> AppResult<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    for migration in MIGRATIONS {
        let applied = connection
            .query_row(
                "SELECT version FROM schema_migrations WHERE version = ?1",
                [migration.version],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();

        if applied {
            continue;
        }

        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            params![migration.version, migration.name],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::run;

    #[test]
    fn migrations_are_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let mut connection = Connection::open_in_memory()?;
        run(&mut connection)?;
        run(&mut connection)?;

        let applied: i64 =
            connection.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })?;

        assert_eq!(applied, 4);
        Ok(())
    }
}
