//! Persistent LaneScore fact revision worker. It is intentionally run after
//! normal match sync and Timeline V1, shares the Riot client limiter, and uses
//! its own queue so neither older V1 completion nor a restart loses work.
use crate::db::Database;
use crate::db::repositories::lane_analysis::LaneAnalysisRepository;
use crate::domain::items::ItemTimelineEvent;
use crate::domain::lane_score::{LaneState, TimelineEvent};
use crate::error::AppResult;
use crate::riot::client::RiotApiClient;
use crate::riot::parser::parse_match;
use crate::riot::types::{RegionalRoute, TimelineResponse};

pub async fn run(
    database: &Database,
    riot: &RiotApiClient,
    puuid: &str,
    route: &RegionalRoute,
) -> AppResult<u64> {
    let mut updated = rederive_local(database, puuid)?;
    {
        let c = database.connection()?;
        LaneAnalysisRepository::resume_interrupted(&c, puuid)?;
        LaneAnalysisRepository::enqueue_eligible(&c, puuid)?;
    }
    loop {
        let match_id = {
            let mut c = database.connection()?;
            LaneAnalysisRepository::claim_next(&mut c, puuid)?
        };
        let Some(match_id) = match_id else {
            return Ok(updated);
        };
        let match_response = match riot.match_by_id(route, &match_id).await {
            Ok(value) => value,
            Err(error) => {
                let c = database.connection()?;
                LaneAnalysisRepository::mark_error(&c, puuid, &match_id, &error.to_string())?;
                continue;
            }
        };
        let parsed = match parse_match(match_response, puuid) {
            Ok(value) => value,
            Err(error) => {
                let c = database.connection()?;
                LaneAnalysisRepository::mark_unsupported(&c, puuid, &match_id, &error.to_string())?;
                continue;
            }
        };
        if parsed.participant_roster.len() != 10 {
            let c = database.connection()?;
            LaneAnalysisRepository::mark_unsupported(
                &c,
                puuid,
                &match_id,
                "Match-V5 did not supply a complete ten-participant roster",
            )?;
            continue;
        }
        let timeline = match riot.match_timeline(route, &match_id).await {
            Ok(value) => value,
            Err(error) => {
                let c = database.connection()?;
                LaneAnalysisRepository::mark_error(&c, puuid, &match_id, &error.to_string())?;
                continue;
            }
        };
        let (states, events, item_events) = normalize_timeline(&timeline);
        if states.is_empty() {
            let c = database.connection()?;
            LaneAnalysisRepository::mark_unsupported(
                &c,
                puuid,
                &match_id,
                "Timeline-V5 did not contain participant state frames",
            )?;
            continue;
        }
        let mut c = database.connection()?;
        LaneAnalysisRepository::store_facts(
            &mut c,
            puuid,
            &match_id,
            &parsed.participant_roster,
            &states,
            &events,
            &item_events,
        )?;
        updated += 1;
        drop(c);
        updated += run_pending_derivations(database, puuid)?;
    }
}

pub fn rederive_local(database: &Database, puuid: &str) -> AppResult<u64> {
    {
        let connection = database.connection()?;
        LaneAnalysisRepository::record_static_exclusions(&connection, puuid)?;
        LaneAnalysisRepository::resume_interrupted_derivations(&connection, puuid)?;
        LaneAnalysisRepository::enqueue_rederivations(&connection, puuid)?;
    }
    run_pending_derivations(database, puuid)
}

fn run_pending_derivations(database: &Database, puuid: &str) -> AppResult<u64> {
    let mut rebuilt = 0;
    loop {
        let match_id = {
            let mut connection = database.connection()?;
            LaneAnalysisRepository::claim_next_derivation(&mut connection, puuid)?
        };
        let Some(match_id) = match_id else {
            return Ok(rebuilt);
        };
        let result = {
            let mut connection = database.connection()?;
            LaneAnalysisRepository::rebuild_score(&mut connection, &match_id, puuid, true)
        };
        let connection = database.connection()?;
        match result {
            Ok(_) => {
                LaneAnalysisRepository::complete_derivation(&connection, puuid, &match_id)?;
                rebuilt += 1;
            }
            Err(error) => {
                LaneAnalysisRepository::fail_derivation(
                    &connection,
                    puuid,
                    &match_id,
                    &error.to_string(),
                )?;
            }
        }
    }
}

fn normalize_timeline(
    response: &TimelineResponse,
) -> (Vec<LaneState>, Vec<TimelineEvent>, Vec<ItemTimelineEvent>) {
    let mut states = Vec::new();
    let mut events = Vec::new();
    let mut item_events = Vec::new();
    for (frame_index, frame) in response.info.frames.iter().enumerate() {
        for (id, value) in &frame.participant_frames {
            if let Ok(participant_id) = id.parse::<i64>() {
                states.push(LaneState {
                    participant_id,
                    timestamp_ms: frame.timestamp,
                    lane_cs: value.minions_killed,
                    jungle_cs: value.jungle_minions_killed,
                    gold: value.total_gold,
                    xp: value.xp,
                    level: value.level,
                });
            }
        }
        for (event_index, event) in frame.events.iter().enumerate() {
            let source_id = format!("{frame_index}:{event_index}");
            if matches!(
                event.event_type.as_str(),
                "ITEM_PURCHASED" | "ITEM_SOLD" | "ITEM_UNDO" | "ITEM_DESTROYED"
            ) {
                if let Some(participant_id) = participant_id(event.participant_id) {
                    item_events.push(ItemTimelineEvent {
                        source_id: source_id.clone(),
                        timestamp_ms: event.timestamp.unwrap_or(frame.timestamp),
                        participant_id,
                        event_type: event.event_type.clone(),
                        item_id: event.item_id,
                        before_item_id: event.before_id,
                        after_item_id: event.after_id,
                    });
                }
            }
            if !matches!(
                event.event_type.as_str(),
                "CHAMPION_KILL" | "TURRET_PLATE_DESTROYED" | "BUILDING_KILL" | "ELITE_MONSTER_KILL"
            ) {
                continue;
            }
            events.push(TimelineEvent {
                source_id,
                timestamp_ms: event.timestamp.unwrap_or(frame.timestamp),
                kind: event.event_type.clone(),
                killer: participant_id(event.killer_id),
                victim: participant_id(event.victim_id),
                team_id: event.team_id,
                assistants: event
                    .assisting_participant_ids
                    .iter()
                    .copied()
                    .filter_map(|id| participant_id(Some(id)))
                    .collect(),
                monster_type: event.monster_type.clone(),
                monster_sub_type: event.monster_sub_type.clone(),
                building_type: event.building_type.clone(),
                tower_type: event.tower_type.clone(),
                lane_type: event.lane_type.clone(),
                position: event.position.as_ref().map(|p| (p.x, p.y)),
            });
        }
    }
    states.sort_by_key(|s| (s.timestamp_ms, s.participant_id));
    events.sort_by_key(|e| (e.timestamp_ms, e.source_id.clone()));
    item_events.sort_by_key(|event| (event.timestamp_ms, event.source_id.clone()));
    (states, events, item_events)
}

fn participant_id(value: Option<i64>) -> Option<i64> {
    value.filter(|id| (1..=10).contains(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::riot::types::{
        TimelineFrameResponse, TimelineInfoResponse, TimelineMetadataResponse,
        TimelineParticipantFrameResponse,
    };
    use std::collections::HashMap;
    #[test]
    fn normalizes_non_participant_zero_as_missing() {
        assert_eq!(participant_id(Some(0)), None);
        assert_eq!(participant_id(Some(1)), Some(1));
        assert_eq!(participant_id(Some(11)), None);
    }

    #[test]
    fn normalizes_selected_events_without_raw_json() {
        let mut frames = HashMap::new();
        frames.insert(
            "1".into(),
            TimelineParticipantFrameResponse {
                total_gold: 100,
                xp: 200,
                level: 2,
                minions_killed: 3,
                jungle_minions_killed: 4,
            },
        );
        let response = TimelineResponse {
            metadata: TimelineMetadataResponse {
                participants: vec![],
            },
            info: TimelineInfoResponse {
                frame_interval: 60_000,
                frames: vec![TimelineFrameResponse {
                    timestamp: 60_000,
                    participant_frames: frames,
                    events: vec![],
                }],
            },
        };
        let (states, events, item_events) = normalize_timeline(&response);
        assert_eq!(states.len(), 1);
        assert!(events.is_empty());
        assert!(item_events.is_empty());
        assert_eq!(states[0].lane_cs, 3);
        assert_eq!(states[0].jungle_cs, 4);
    }
}
