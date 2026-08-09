use serde::{Deserialize, Serialize};

use crate::domain::settings::{LocalSettings, SettingsUpdate};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub game_name: String,
    pub tag_line: String,
    pub account_region: String,
    pub platform_region: String,
    pub riot_client_path: Option<String>,
    pub api_key_configured: bool,
    pub data_dragon_version: Option<String>,
}

impl SettingsDto {
    pub fn from_domain(
        settings: LocalSettings,
        api_key_configured: bool,
        data_dragon_version: Option<String>,
    ) -> Self {
        Self {
            game_name: settings.game_name,
            tag_line: settings.tag_line,
            account_region: settings.account_region,
            platform_region: settings.platform_region,
            riot_client_path: settings.riot_client_path,
            api_key_configured,
            data_dragon_version,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsDto {
    pub game_name: String,
    pub tag_line: String,
    pub account_region: String,
    pub platform_region: String,
    pub riot_client_path: Option<String>,
}

impl From<UpdateSettingsDto> for SettingsUpdate {
    fn from(value: UpdateSettingsDto) -> Self {
        Self {
            game_name: value.game_name.trim().to_owned(),
            tag_line: value.tag_line.trim().to_owned(),
            account_region: value.account_region.trim().to_ascii_lowercase(),
            platform_region: value.platform_region.trim().to_ascii_lowercase(),
            riot_client_path: value
                .riot_client_path
                .map(|path| path.trim().to_owned())
                .filter(|path| !path.is_empty()),
        }
    }
}
