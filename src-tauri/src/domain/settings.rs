#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalSettings {
    pub game_name: String,
    pub tag_line: String,
    pub account_region: String,
    pub platform_region: String,
    pub riot_client_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsUpdate {
    pub game_name: String,
    pub tag_line: String,
    pub account_region: String,
    pub platform_region: String,
    pub riot_client_path: Option<String>,
}
