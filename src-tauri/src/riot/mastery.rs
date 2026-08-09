use crate::error::AppResult;
use crate::riot::client::RiotApiClient;
use crate::riot::types::{MasteryResponse, PlatformRoute};

impl RiotApiClient {
    pub async fn champion_masteries(
        &self,
        route: &PlatformRoute,
        puuid: &str,
    ) -> AppResult<Vec<MasteryResponse>> {
        self.get(
            "champion-mastery-v4",
            &route.host(),
            &[
                "lol",
                "champion-mastery",
                "v4",
                "champion-masteries",
                "by-puuid",
                puuid,
            ],
            "/lol/champion-mastery/v4/champion-masteries/by-puuid/{redacted-puuid}",
            &[],
        )
        .await
    }
}
