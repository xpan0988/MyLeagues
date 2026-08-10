use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::db::Database;
use crate::db::repositories::timeline::{LaningSnapshot, TEN_MINUTE_MS, TimelineRepository, validate_snapshot};
use crate::error::AppResult;
use crate::riot::client::RiotApiClient;
use crate::riot::types::{RegionalRoute, TimelineFrameResponse, TimelineResponse};
use crate::services::sync::SyncCoordinator;

pub struct TimelineCoordinator {
    running: Mutex<bool>,
}

impl TimelineCoordinator {
    pub fn new() -> Self { Self { running: Mutex::new(false) } }

    async fn begin(&self) -> bool {
        let mut running = self.running.lock().await;
        if *running { false } else { *running = true; true }
    }

    async fn finish(&self) { *self.running.lock().await = false; }
}

/// Starts a deliberately low-priority one-at-a-time backfill. It shares the
/// same RiotApiClient limiter as summary sync and exits between jobs when a
/// higher-priority summary worker starts.
pub async fn start_background(
    database: Arc<Database>,
    riot: Arc<RiotApiClient>,
    summary_sync: Arc<SyncCoordinator>,
    coordinator: Arc<TimelineCoordinator>,
    app: AppHandle,
    puuid: String,
    route: RegionalRoute,
) {
    if !coordinator.begin().await { return; }
    tauri::async_runtime::spawn(async move {
        let result = run(&database, &riot, &summary_sync, &puuid, &route).await;
        coordinator.finish().await;
        match result {
            Ok(updated) if updated > 0 => { let _ = app.emit("timeline-facts-changed", ()); }
            Ok(_) => {}
            Err(error) => tracing::warn!(target: "timeline", error = %error, "timeline enrichment paused; persistent queue will resume"),
        }
    });
}

async fn run(
    database: &Database,
    riot: &RiotApiClient,
    summary_sync: &SyncCoordinator,
    puuid: &str,
    route: &RegionalRoute,
) -> AppResult<u64> {
    {
        let connection = database.connection()?;
        TimelineRepository::resume_interrupted(&connection, puuid)?;
        TimelineRepository::enqueue_eligible(&connection, puuid)?;
    }
    let mut updated = 0;
    loop {
        if summary_sync.is_running().await { return Ok(updated); }
        let match_id = {
            let mut connection = database.connection()?;
            TimelineRepository::claim_next(&mut connection, puuid)?
        };
        let Some(match_id) = match_id else { return Ok(updated); };
        let response = match riot.match_timeline(route, &match_id).await {
            Ok(response) => response,
            Err(error) => {
                let connection = database.connection()?;
                TimelineRepository::mark_error(&connection, puuid, &match_id, &error.to_string())?;
                continue;
            }
        };
        let participant_id = {
            let connection = database.connection()?;
            TimelineRepository::participant_id(&connection, &match_id, puuid)?
        }.or_else(|| participant_id_from_metadata(&response, puuid));
        let Some(participant_id) = participant_id else {
            let connection = database.connection()?;
            TimelineRepository::mark_unsupported(&connection, puuid, &match_id, "timeline metadata did not map the tracked PUUID to a participant")?;
            continue;
        };
        let Some(frame) = ten_minute_frame(&response.info.frames) else {
            let connection = database.connection()?;
            TimelineRepository::mark_unsupported(&connection, puuid, &match_id, "timeline ended before a ten-minute frame")?;
            continue;
        };
        let Some(participant) = frame.participant_frames.get(&participant_id.to_string()) else {
            let connection = database.connection()?;
            TimelineRepository::mark_unsupported(&connection, puuid, &match_id, "ten-minute frame omitted the tracked participant")?;
            continue;
        };
        let snapshot = LaningSnapshot {
            match_id: match_id.clone(), puuid: puuid.to_owned(), frame_timestamp_ms: frame.timestamp,
            lane_minions: participant.minions_killed, neutral_minions: participant.jungle_minions_killed,
            total_gold: participant.total_gold, experience: participant.xp, level: participant.level,
        };
        if let Err(error) = validate_snapshot(&snapshot) {
            let connection = database.connection()?;
            TimelineRepository::mark_unsupported(&connection, puuid, &match_id, &error.to_string())?;
            continue;
        }
        let mut connection = database.connection()?;
        if TimelineRepository::insert_snapshot(&mut connection, &snapshot)? { updated += 1; }
    }
}

fn participant_id_from_metadata(response: &TimelineResponse, puuid: &str) -> Option<i64> {
    response.metadata.participants.iter().position(|value| value == puuid).map(|index| index as i64 + 1)
}

/// Riot frames normally include 600000 ms. If a payload omits that boundary,
/// use the first later frame rather than reporting a nine-minute fact as CS@10.
fn ten_minute_frame(frames: &[TimelineFrameResponse]) -> Option<&TimelineFrameResponse> {
    frames.iter().find(|frame| frame.timestamp == TEN_MINUTE_MS).or_else(|| {
        frames.iter().filter(|frame| frame.timestamp > TEN_MINUTE_MS).min_by_key(|frame| frame.timestamp)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::riot::types::{TimelineFrameResponse, TimelineInfoResponse, TimelineMetadataResponse, TimelineResponse};
    use super::{participant_id_from_metadata, ten_minute_frame};

    fn frame(timestamp: i64) -> TimelineFrameResponse { TimelineFrameResponse { timestamp, participant_frames: HashMap::new() } }

    #[test]
    fn selects_exact_ten_minute_frame_then_first_later_boundary() {
        let exact = vec![frame(540_000), frame(600_000), frame(660_000)];
        assert_eq!(ten_minute_frame(&exact).unwrap().timestamp, 600_000);
        let absent = vec![frame(540_000), frame(620_000), frame(660_000)];
        assert_eq!(ten_minute_frame(&absent).unwrap().timestamp, 620_000);
        assert!(ten_minute_frame(&[frame(540_000)]).is_none());
    }

    #[test]
    fn maps_historical_local_puuid_from_timeline_metadata_only_as_fallback() {
        let response = TimelineResponse { metadata: TimelineMetadataResponse { participants: vec!["other".into(), "local".into()] }, info: TimelineInfoResponse { frame_interval: 60_000, frames: vec![] } };
        assert_eq!(participant_id_from_metadata(&response, "local"), Some(2));
        assert_eq!(participant_id_from_metadata(&response, "missing"), None);
    }
}
