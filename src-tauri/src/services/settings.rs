use crate::config::BackendConfig;
use crate::db::Database;
use crate::db::repositories::settings::SettingsRepository;
use crate::db::repositories::static_data::StaticDataRepository;
use crate::domain::settings::SettingsUpdate;
use crate::dto::settings::SettingsDto;
use crate::error::{AppError, AppResult};

pub struct SettingsService<'state> {
    database: &'state Database,
    config: &'state BackendConfig,
}

impl<'state> SettingsService<'state> {
    pub fn new(database: &'state Database, config: &'state BackendConfig) -> Self {
        Self { database, config }
    }

    pub fn get(&self) -> AppResult<SettingsDto> {
        let connection = self.database.connection()?;
        let settings = SettingsRepository::new(&connection).get()?;
        let version = StaticDataRepository::active_version(&connection)?;
        Ok(SettingsDto::from_domain(
            settings,
            self.config.api_key_configured(),
            version,
        ))
    }

    pub fn update(&self, update: SettingsUpdate) -> AppResult<SettingsDto> {
        validate_update(&update)?;
        let connection = self.database.connection()?;
        let settings = SettingsRepository::new(&connection).update(&update)?;
        let version = StaticDataRepository::active_version(&connection)?;
        Ok(SettingsDto::from_domain(
            settings,
            self.config.api_key_configured(),
            version,
        ))
    }
}

fn validate_update(update: &SettingsUpdate) -> AppResult<()> {
    if update.game_name.len() > 64 || update.tag_line.len() > 16 {
        return Err(AppError::Configuration(
            "Riot ID fields exceed the supported length".to_owned(),
        ));
    }

    if update.game_name.is_empty() != update.tag_line.is_empty() {
        return Err(AppError::Configuration(
            "game name and tag line must be configured together".to_owned(),
        ));
    }

    if update.account_region.is_empty() || update.platform_region.is_empty() {
        return Err(AppError::Configuration(
            "account and platform regions are required".to_owned(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_update;
    use crate::domain::settings::SettingsUpdate;

    fn update(game_name: &str, tag_line: &str) -> SettingsUpdate {
        SettingsUpdate {
            game_name: game_name.to_owned(),
            tag_line: tag_line.to_owned(),
            account_region: "americas".to_owned(),
            platform_region: "oc1".to_owned(),
            riot_client_path: None,
        }
    }

    #[test]
    fn riot_id_fields_are_configured_together() {
        assert!(validate_update(&update("Player", "OC1")).is_ok());
        assert!(validate_update(&update("", "")).is_ok());
        assert!(validate_update(&update("Player", "")).is_err());
    }
}
