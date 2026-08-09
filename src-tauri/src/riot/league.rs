use crate::error::AppResult;
use crate::riot::client::RiotApiClient;
use crate::riot::types::{LeagueEntryResponse, PlatformRoute};

impl RiotApiClient {
    pub async fn league_entries(
        &self,
        route: &PlatformRoute,
        puuid: &str,
    ) -> AppResult<Vec<LeagueEntryResponse>> {
        self.get(
            "league-v4",
            &route.host(),
            &["lol", "league", "v4", "entries", "by-puuid", puuid],
            "/lol/league/v4/entries/by-puuid/{redacted-puuid}",
            &[],
        )
        .await
    }
}
