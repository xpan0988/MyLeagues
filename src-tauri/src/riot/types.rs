use serde::Deserialize;
use std::collections::HashMap;

use crate::error::{AppError, AppResult};

const PLATFORM_ROUTES: &[&str] = &[
    "br1", "eun1", "euw1", "jp1", "kr", "la1", "la2", "na1", "oc1", "tr1", "ru", "ph2", "sg2",
    "th2", "tw2", "vn2",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionalRoute(String);

impl RegionalRoute {
    pub fn host(&self) -> String {
        format!("{}.api.riotgames.com", self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformRoute(String);

impl PlatformRoute {
    pub fn parse(value: &str) -> AppResult<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        if !PLATFORM_ROUTES.contains(&normalized.as_str()) {
            return Err(AppError::Configuration(format!(
                "unsupported Riot platform route: {value}"
            )));
        }
        Ok(Self(normalized))
    }

    pub fn host(&self) -> String {
        format!("{}.api.riotgames.com", self.0)
    }

    pub fn account_route(&self) -> RegionalRoute {
        let route = match self.0.as_str() {
            "br1" | "la1" | "la2" | "na1" | "oc1" => "americas",
            "jp1" | "kr" => "asia",
            "eun1" | "euw1" | "tr1" | "ru" => "europe",
            "ph2" | "sg2" | "th2" | "tw2" | "vn2" => "sea",
            _ => unreachable!("validated platform route"),
        };
        RegionalRoute(route.to_owned())
    }

    pub fn match_route(&self) -> RegionalRoute {
        let route = match self.0.as_str() {
            "br1" | "la1" | "la2" | "na1" => "americas",
            "jp1" | "kr" => "asia",
            "eun1" | "euw1" | "tr1" | "ru" => "europe",
            "oc1" | "ph2" | "sg2" | "th2" | "tw2" | "vn2" => "sea",
            _ => unreachable!("validated platform route"),
        };
        RegionalRoute(route.to_owned())
    }
}

#[cfg(test)]
mod routing_tests {
    use super::PlatformRoute;

    #[test]
    fn account_americas_route_has_expected_host() {
        let platform = PlatformRoute::parse("OC1").unwrap();
        assert_eq!(
            platform.account_route().host(),
            "americas.api.riotgames.com"
        );
    }

    #[test]
    fn oce_resolves_to_oc1_platform_americas_account_and_sea_match() {
        let platform = PlatformRoute::parse("OC1").unwrap();
        assert_eq!(platform.host(), "oc1.api.riotgames.com");
        assert_eq!(
            platform.account_route().host(),
            "americas.api.riotgames.com"
        );
        assert_eq!(platform.match_route().host(), "sea.api.riotgames.com");
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiotAccountResponse {
    pub puuid: String,
    pub game_name: String,
    pub tag_line: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummonerResponse {
    pub puuid: String,
    #[serde(default)]
    pub profile_icon_id: Option<i64>,
    #[serde(default)]
    pub revision_date: Option<i64>,
    #[serde(default)]
    pub summoner_level: Option<i64>,
}

#[cfg(test)]
mod summoner_response_tests {
    use super::SummonerResponse;

    #[test]
    fn deserializes_live_summoner_shape_without_summoner_id() {
        let response: SummonerResponse = serde_json::from_str(
            r#"{
              "puuid": "test-puuid",
              "profileIconId": 7143,
              "revisionDate": 1786249896687,
              "summonerLevel": 89
            }"#,
        )
        .unwrap();

        assert_eq!(response.puuid, "test-puuid");
        assert_eq!(response.profile_icon_id, Some(7143));
        assert_eq!(response.revision_date, Some(1786249896687));
        assert_eq!(response.summoner_level, Some(89));
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeagueEntryResponse {
    pub queue_type: String,
    pub tier: String,
    pub rank: String,
    pub league_points: i64,
    pub wins: i64,
    pub losses: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasteryResponse {
    pub champion_id: i64,
    pub champion_level: i64,
    pub champion_points: i64,
    pub last_play_time: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResponse {
    pub metadata: MatchMetadataResponse,
    pub info: MatchInfoResponse,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchMetadataResponse {
    pub match_id: String,
    pub participants: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchInfoResponse {
    pub game_creation: i64,
    pub game_end_timestamp: Option<i64>,
    pub game_duration: i64,
    pub game_version: String,
    pub queue_id: i64,
    pub participants: Vec<ParticipantResponse>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantResponse {
    #[serde(default)]
    pub participant_id: Option<i64>,
    pub puuid: String,
    pub champion_id: i64,
    #[serde(default)]
    pub team_id: Option<i64>,
    #[serde(default)]
    pub team_position: String,
    #[serde(default)]
    pub individual_position: String,
    pub win: bool,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub double_kills: i64,
    pub triple_kills: i64,
    pub quadra_kills: i64,
    pub penta_kills: i64,
    pub total_minions_killed: i64,
    pub neutral_minions_killed: i64,
    pub gold_earned: i64,
    pub summoner1_id: i64,
    pub summoner2_id: i64,
    pub item0: i64,
    pub item1: i64,
    pub item2: i64,
    pub item3: i64,
    pub item4: i64,
    pub item5: i64,
    pub item6: i64,
    pub perks: PerksResponse,
}

/// Match-V5 timeline payload is converted immediately into normalized facts.
/// Raw JSON is deliberately not retained as the authoritative model.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineResponse {
    pub metadata: TimelineMetadataResponse,
    pub info: TimelineInfoResponse,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineMetadataResponse {
    pub participants: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineInfoResponse {
    pub frame_interval: i64,
    pub frames: Vec<TimelineFrameResponse>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineFrameResponse {
    pub timestamp: i64,
    #[serde(default)]
    pub participant_frames: HashMap<String, TimelineParticipantFrameResponse>,
    #[serde(default)]
    pub events: Vec<TimelineEventResponse>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineParticipantFrameResponse {
    pub total_gold: i64,
    pub xp: i64,
    pub level: i64,
    pub minions_killed: i64,
    pub jungle_minions_killed: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEventResponse {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub killer_id: Option<i64>,
    #[serde(default)]
    pub victim_id: Option<i64>,
    #[serde(default)]
    pub team_id: Option<i64>,
    #[serde(default)]
    pub assisting_participant_ids: Vec<i64>,
    #[serde(default)]
    pub monster_type: Option<String>,
    #[serde(default)]
    pub monster_sub_type: Option<String>,
    #[serde(default)]
    pub building_type: Option<String>,
    #[serde(default)]
    pub tower_type: Option<String>,
    #[serde(default)]
    pub lane_type: Option<String>,
    #[serde(default)]
    pub position: Option<TimelinePositionResponse>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TimelinePositionResponse {
    pub x: i64,
    pub y: i64,
}

#[cfg(test)]
mod timeline_tests {
    use super::TimelineResponse;

    #[test]
    fn deserializes_current_match_v5_timeline_frame_shape() {
        let value: TimelineResponse = serde_json::from_str(
            r#"{
                "metadata":{"participants":["local-puuid"]},
                "info":{"frameInterval":60000,"frames":[{
                    "timestamp":600000,
                    "participantFrames":{"1":{"totalGold":4123,"xp":5040,"level":6,"minionsKilled":71,"jungleMinionsKilled":3}},
                    "events":[]
                }]}
            }"#,
        )
        .unwrap();
        let frame = &value.info.frames[0];
        let player = &frame.participant_frames["1"];
        assert_eq!(
            (value.info.frame_interval, frame.timestamp),
            (60_000, 600_000)
        );
        assert_eq!(
            (
                player.total_gold,
                player.xp,
                player.level,
                player.minions_killed,
                player.jungle_minions_killed
            ),
            (4123, 5040, 6, 71, 3)
        );
    }

    #[test]
    fn deserializes_sanitized_lane_score_event_variants() {
        let value: TimelineResponse = serde_json::from_str(
            r#"{
                "metadata":{"participants":["local-puuid"]},
                "info":{"frameInterval":60000,"frames":[{
                    "timestamp":600000,
                    "participantFrames":{},
                    "events":[
                        {"type":"CHAMPION_KILL","timestamp":601000,"killerId":1,"victimId":6,"assistingParticipantIds":[2],"position":{"x":3100,"y":12200}},
                        {"type":"TURRET_PLATE_DESTROYED","timestamp":602000,"teamId":200,"killerId":0,"laneType":"TOP_LANE","position":{"x":4318,"y":13875}},
                        {"type":"BUILDING_KILL","timestamp":603000,"teamId":200,"killerId":1,"buildingType":"TOWER_BUILDING","towerType":"OUTER_TURRET","laneType":"TOP_LANE","position":{"x":4318,"y":13875}},
                        {"type":"ELITE_MONSTER_KILL","timestamp":604000,"killerId":1,"monsterType":"VOID_GRUB","monsterSubType":"VOID_GRUB"},
                        {"type":"ELITE_MONSTER_KILL","timestamp":605000,"killerId":1,"monsterType":"RIFTHERALD","monsterSubType":"RIFTHERALD"}
                    ]
                }]}
            }"#,
        )
        .unwrap();

        let events = &value.info.frames[0].events;
        assert_eq!(events[0].event_type, "CHAMPION_KILL");
        assert_eq!(events[0].assisting_participant_ids, vec![2]);
        assert_eq!(
            events[0].position.as_ref().map(|p| (p.x, p.y)),
            Some((3100, 12200))
        );
        assert_eq!(events[1].killer_id, Some(0));
        assert_eq!(events[1].team_id, Some(200));
        assert_eq!(events[2].building_type.as_deref(), Some("TOWER_BUILDING"));
        assert_eq!(events[2].tower_type.as_deref(), Some("OUTER_TURRET"));
        assert_eq!(events[2].lane_type.as_deref(), Some("TOP_LANE"));
        assert_eq!(events[3].monster_type.as_deref(), Some("VOID_GRUB"));
        assert_eq!(events[3].monster_sub_type.as_deref(), Some("VOID_GRUB"));
        assert_eq!(events[4].monster_type.as_deref(), Some("RIFTHERALD"));
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerksResponse {
    pub stat_perks: StatPerksResponse,
    pub styles: Vec<PerkStyleResponse>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatPerksResponse {
    pub defense: i64,
    pub flex: i64,
    pub offense: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerkStyleResponse {
    pub description: String,
    pub selections: Vec<PerkSelectionResponse>,
    pub style: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerkSelectionResponse {
    pub perk: i64,
}
