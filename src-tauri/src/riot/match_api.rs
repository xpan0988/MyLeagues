use crate::error::AppResult;
use crate::riot::client::{RiotApiClient, TimedRiotResponse};
use crate::riot::types::{MatchResponse, RegionalRoute};

impl RiotApiClient {
    pub async fn match_ids(
        &self,
        route: &RegionalRoute,
        puuid: &str,
        start: u32,
        count: u32,
    ) -> AppResult<Vec<String>> {
        self.get(
            "match-v5",
            &route.host(),
            &["lol", "match", "v5", "matches", "by-puuid", puuid, "ids"],
            "/lol/match/v5/matches/by-puuid/{redacted-puuid}/ids",
            &[
                ("start", start.to_string()),
                ("count", count.clamp(1, 100).to_string()),
            ],
        )
        .await
    }

    pub async fn match_by_id(
        &self,
        route: &RegionalRoute,
        match_id: &str,
    ) -> AppResult<MatchResponse> {
        self.get(
            "match-v5",
            &route.host(),
            &["lol", "match", "v5", "matches", match_id],
            &format!("/lol/match/v5/matches/{match_id}"),
            &[],
        )
        .await
    }

    pub async fn match_by_id_timed(
        &self,
        route: &RegionalRoute,
        match_id: &str,
    ) -> AppResult<TimedRiotResponse<MatchResponse>> {
        self.get_timed(
            "match-v5",
            &route.host(),
            &["lol", "match", "v5", "matches", match_id],
            &format!("/lol/match/v5/matches/{match_id}"),
            &[],
        )
        .await
    }
}
