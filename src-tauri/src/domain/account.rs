#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub puuid: String,
    pub game_name: String,
    pub tag_line: String,
    pub summoner_id: Option<String>,
    pub account_region: String,
    pub platform_region: String,
}
