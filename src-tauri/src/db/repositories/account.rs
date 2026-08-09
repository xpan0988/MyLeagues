use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::account::Account;
use crate::error::{AppError, AppResult};

pub struct AccountRepository<'connection> {
    connection: &'connection Connection,
}

impl<'connection> AccountRepository<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn get(&self) -> AppResult<Option<Account>> {
        let account = self
            .connection
            .query_row(
                "SELECT puuid, game_name, tag_line, summoner_id, account_region, platform_region
                 FROM accounts WHERE single_account_guard = 1",
                [],
                |row| {
                    Ok(Account {
                        puuid: row.get(0)?,
                        game_name: row.get(1)?,
                        tag_line: row.get(2)?,
                        summoner_id: row.get(3)?,
                        account_region: row.get(4)?,
                        platform_region: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(account)
    }

    pub fn upsert(&self, account: &Account) -> AppResult<()> {
        if let Some(existing) = self.get()? {
            if existing.puuid != account.puuid {
                return Err(AppError::Configuration(
                    "the local archive already belongs to a different PUUID".to_owned(),
                ));
            }
        }

        self.connection.execute(
            "INSERT INTO accounts (
                puuid, game_name, tag_line, summoner_id, account_region, platform_region
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(puuid) DO UPDATE SET
                game_name = excluded.game_name,
                tag_line = excluded.tag_line,
                summoner_id = excluded.summoner_id,
                account_region = excluded.account_region,
                platform_region = excluded.platform_region,
                updated_at = CURRENT_TIMESTAMP",
            params![
                account.puuid,
                account.game_name,
                account.tag_line,
                account.summoner_id,
                account.account_region,
                account.platform_region,
            ],
        )?;
        Ok(())
    }
}
