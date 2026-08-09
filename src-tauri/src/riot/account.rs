use crate::error::AppResult;
use crate::riot::client::RiotApiClient;
use crate::riot::types::{PlatformRoute, RegionalRoute, RiotAccountResponse, SummonerResponse};

impl RiotApiClient {
    pub async fn account_by_riot_id(
        &self,
        route: &RegionalRoute,
        game_name: &str,
        tag_line: &str,
    ) -> AppResult<RiotAccountResponse> {
        self.get(
            "account-v1",
            &route.host(),
            &[
                "riot",
                "account",
                "v1",
                "accounts",
                "by-riot-id",
                game_name,
                tag_line,
            ],
            "/riot/account/v1/accounts/by-riot-id/{redacted-riot-id}",
            &[],
        )
        .await
    }

    pub async fn summoner_by_puuid(
        &self,
        route: &PlatformRoute,
        puuid: &str,
    ) -> AppResult<SummonerResponse> {
        self.get(
            "summoner-v4",
            &route.host(),
            &["lol", "summoner", "v4", "summoners", "by-puuid", puuid],
            "/lol/summoner/v4/summoners/by-puuid/{redacted-puuid}",
            &[],
        )
        .await
    }
}
