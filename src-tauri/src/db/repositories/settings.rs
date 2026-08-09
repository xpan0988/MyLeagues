use rusqlite::{Connection, params};

use crate::domain::settings::{LocalSettings, SettingsUpdate};
use crate::error::AppResult;

pub struct SettingsRepository<'connection> {
    connection: &'connection Connection,
}

impl<'connection> SettingsRepository<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn get(&self) -> AppResult<LocalSettings> {
        let settings = self.connection.query_row(
            "SELECT game_name, tag_line, account_region, platform_region, riot_client_path
             FROM app_settings WHERE id = 1",
            [],
            |row| {
                Ok(LocalSettings {
                    game_name: row.get(0)?,
                    tag_line: row.get(1)?,
                    account_region: row.get(2)?,
                    platform_region: row.get(3)?,
                    riot_client_path: row.get(4)?,
                })
            },
        )?;

        Ok(settings)
    }

    pub fn update(&self, update: &SettingsUpdate) -> AppResult<LocalSettings> {
        self.connection.execute(
            "UPDATE app_settings
             SET game_name = ?1,
                 tag_line = ?2,
                 account_region = ?3,
                 platform_region = ?4,
                 riot_client_path = ?5,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = 1",
            params![
                update.game_name,
                update.tag_line,
                update.account_region,
                update.platform_region,
                update.riot_client_path,
            ],
        )?;

        self.get()
    }
}
