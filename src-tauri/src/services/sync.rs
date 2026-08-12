use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use crate::db::Database;
use crate::db::repositories::account::AccountRepository;
use crate::db::repositories::matches::MatchRepository;
use crate::db::repositories::profile::ProfileRepository;
use crate::db::repositories::settings::SettingsRepository;
use crate::db::repositories::sync::SyncRepository;
use crate::db::repositories::timeline::TimelineRepository;
use crate::domain::account::Account;
use crate::dto::analytics::SyncStateDto;
use crate::error::{AppError, AppResult};
use crate::riot::client::RiotApiClient;
use crate::riot::parser::parse_match;
use crate::riot::types::{PlatformRoute, RegionalRoute};
use crate::services::timeline::{self, TimelineCoordinator};

const MATCH_PAGE_SIZE: u32 = 100;
const MATCH_FETCH_CONCURRENCY: usize = 5;
const PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(500);
pub const FRESHNESS_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncTrigger {
    Startup,
    SettingsSaved,
    Periodic,
    Resume,
    Manual,
    ArchiveReset,
}

impl SyncTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::SettingsSaved => "settings_saved",
            Self::Periodic => "periodic",
            Self::Resume => "resume",
            Self::Manual => "manual",
            Self::ArchiveReset => "archive_reset",
        }
    }
}

pub struct SyncCoordinator {
    state: Mutex<SyncStateDto>,
    last_progress_emit: Mutex<Option<Instant>>,
}

impl SyncCoordinator {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(SyncStateDto {
                status: "idle".to_owned(),
                currently_running: false,
                trigger: None,
                completed: 0,
                total: None,
                message: None,
                last_check_at: None,
                last_successful_sync_at: None,
            }),
            last_progress_emit: Mutex::new(None),
        }
    }

    pub async fn begin(&self, trigger: SyncTrigger) -> bool {
        let mut state = self.state.lock().await;
        if state.currently_running {
            return false;
        }
        state.status = "checking".to_owned();
        state.currently_running = true;
        state.trigger = Some(trigger.as_str().to_owned());
        state.message = Some("Checking local archive freshness".to_owned());
        state.completed = 0;
        state.total = None;
        true
    }

    pub async fn snapshot(&self) -> SyncStateDto {
        self.state.lock().await.clone()
    }

    pub async fn is_running(&self) -> bool {
        self.state.lock().await.currently_running
    }

    async fn syncing(&self, app: &AppHandle) {
        let snapshot = {
            let mut state = self.state.lock().await;
            state.status = "syncing".to_owned();
            state.message = Some("Syncing match history".to_owned());
            state.clone()
        };
        let _ = app.emit("sync-state-changed", snapshot);
    }

    async fn progress(
        &self,
        app: &AppHandle,
        completed: u64,
        total: u64,
        message: impl Into<String>,
    ) -> bool {
        let snapshot = {
            let mut state = self.state.lock().await;
            state.completed = completed;
            state.total = Some(total);
            state.message = Some(message.into());
            state.clone()
        };
        let emitted = {
            let now = Instant::now();
            let mut last = self.last_progress_emit.lock().await;
            if should_emit_progress(*last, now, false) {
                *last = Some(now);
                true
            } else {
                false
            }
        };
        if emitted {
            let _ = app.emit("sync-state-changed", snapshot);
        }
        emitted
    }

    pub async fn finish(&self, app: &AppHandle, result: &AppResult<()>) {
        let snapshot = self.complete(result).await;
        let _ = app.emit("sync-state-changed", snapshot);
    }

    async fn complete(&self, result: &AppResult<()>) -> SyncStateDto {
        {
            let mut state = self.state.lock().await;
            match result {
                Ok(()) => {
                    state.status = "success".to_owned();
                    state.currently_running = false;
                    state.message = Some("Local archive is up to date".to_owned());
                    let now = chrono::Utc::now().to_rfc3339();
                    state.last_check_at = Some(now.clone());
                    state.last_successful_sync_at = Some(now);
                }
                Err(error) => {
                    state.status = "error".to_owned();
                    state.currently_running = false;
                    state.message = Some(error.to_string());
                    state.last_check_at = Some(chrono::Utc::now().to_rfc3339());
                }
            }
            state.clone()
        }
    }
}

pub async fn start_background(
    database: Arc<Database>,
    riot: Arc<RiotApiClient>,
    coordinator: Arc<SyncCoordinator>,
    timeline_coordinator: Arc<TimelineCoordinator>,
    app: AppHandle,
    trigger: SyncTrigger,
) -> SyncStateDto {
    if coordinator.begin(trigger).await {
        tracing::info!(target: "sync", trigger = trigger.as_str(), "starting synchronization attempt");
        let checking = coordinator.snapshot().await;
        let _ = app.emit("sync-state-changed", checking);
        let task_coordinator = Arc::clone(&coordinator);
        tauri::async_runtime::spawn(async move {
            let result = run(
                Arc::clone(&database),
                Arc::clone(&riot),
                Arc::clone(&task_coordinator),
                app.clone(),
                trigger,
            )
            .await;
            task_coordinator.finish(&app, &result).await;
            if result.is_ok() {
                let candidate = (|| -> AppResult<(String, RegionalRoute)> {
                    let connection = database.connection()?;
                    let account = AccountRepository::new(&connection).get()?.ok_or_else(|| {
                        AppError::Configuration(
                            "configured account disappeared during synchronization".to_owned(),
                        )
                    })?;
                    Ok((
                        account.puuid,
                        PlatformRoute::parse(&account.platform_region)?.match_route(),
                    ))
                })();
                if let Ok((puuid, route)) = candidate {
                    timeline::start_background(
                        database,
                        riot,
                        task_coordinator.clone(),
                        timeline_coordinator,
                        app.clone(),
                        puuid,
                        route,
                    )
                    .await;
                }
            }
            if let Err(error) = result {
                tracing::error!(error = %error, "background synchronization failed");
            }
        });
    } else {
        tracing::info!(target: "sync", trigger = trigger.as_str(), "coalesced synchronization attempt because a worker is already running");
    }
    coordinator.snapshot().await
}

pub async fn start_if_stale(
    database: Arc<Database>,
    riot: Arc<RiotApiClient>,
    coordinator: Arc<SyncCoordinator>,
    timeline_coordinator: Arc<TimelineCoordinator>,
    app: AppHandle,
    trigger: SyncTrigger,
) -> AppResult<SyncStateDto> {
    if coordinator.is_running().await {
        return Ok(coordinator.snapshot().await);
    }
    let should_start = {
        let connection = database.connection()?;
        let settings = SettingsRepository::new(&connection).get()?;
        if settings.game_name.is_empty() || settings.tag_line.is_empty() {
            false
        } else if let Some(account) = AccountRepository::new(&connection).get()? {
            SyncRepository::ensure(&connection, &account.puuid)?;
            SyncRepository::get(&connection, &account.puuid)?
                .last_check_at
                .as_deref()
                .map(is_stale)
                .unwrap_or(true)
        } else {
            true
        }
    };
    Ok(if should_start {
        start_background(
            database,
            riot,
            coordinator,
            timeline_coordinator,
            app,
            trigger,
        )
        .await
    } else {
        coordinator.snapshot().await
    })
}

fn is_stale(value: &str) -> bool {
    let parsed = chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .map(|date| date.and_utc())
        });
    parsed
        .map(|date| {
            chrono::Utc::now()
                .signed_duration_since(date)
                .to_std()
                .map(|age| age >= FRESHNESS_INTERVAL)
                .unwrap_or(true)
        })
        .unwrap_or(true)
}

pub async fn run(
    database: Arc<Database>,
    riot: Arc<RiotApiClient>,
    coordinator: Arc<SyncCoordinator>,
    app: AppHandle,
    trigger: SyncTrigger,
) -> AppResult<()> {
    let settings = {
        let connection = database.connection()?;
        SettingsRepository::new(&connection).get()?
    };
    if settings.game_name.is_empty() || settings.tag_line.is_empty() {
        return Err(AppError::Configuration(
            "configure a Riot ID before synchronizing".to_owned(),
        ));
    }

    let platform_route = PlatformRoute::parse(&settings.platform_region)?;
    let account_route = platform_route.account_route();
    let match_route = platform_route.match_route();
    let riot_account = riot
        .account_by_riot_id(&account_route, &settings.game_name, &settings.tag_line)
        .await?;
    let summoner = riot
        .summoner_by_puuid(&platform_route, &riot_account.puuid)
        .await?;
    if summoner.puuid != riot_account.puuid {
        return Err(AppError::RiotData(
            "Summoner-V4 returned a PUUID that did not match Account-V1".to_owned(),
        ));
    }
    let account = Account {
        puuid: riot_account.puuid,
        game_name: riot_account.game_name,
        tag_line: riot_account.tag_line,
        summoner_id: None,
        account_region: account_route.as_str().to_owned(),
        platform_region: settings.platform_region,
    };

    {
        let connection = database.connection()?;
        AccountRepository::new(&connection).upsert(&account)?;
        SyncRepository::ensure(&connection, &account.puuid)?;
        SyncRepository::resume_interrupted(&connection, &account.puuid)?;
        SyncRepository::begin_attempt(&connection, &account.puuid, trigger.as_str())?;
    }
    coordinator.syncing(&app).await;

    let sync_result = synchronize_matches(
        &database,
        &riot,
        &coordinator,
        &app,
        &match_route,
        &account.puuid,
    )
    .await;

    if let Err(error) = &sync_result {
        let connection = database.connection()?;
        SyncRepository::set_status(
            &connection,
            &account.puuid,
            "error",
            Some(&error.to_string()),
        )?;
        return sync_result;
    }

    let masteries = riot
        .champion_masteries(&platform_route, &account.puuid)
        .await?;
    let league_entries = riot.league_entries(&platform_route, &account.puuid).await?;
    {
        let mut connection = database.connection()?;
        ProfileRepository::replace_mastery(&mut connection, &account.puuid, &masteries)?;
        ProfileRepository::add_rank_snapshots(&mut connection, &account.puuid, &league_entries)?;
        SyncRepository::mark_success(&connection, &account.puuid)?;
        TimelineRepository::enqueue_eligible(&connection, &account.puuid)?;
    }
    Ok(())
}

async fn synchronize_matches(
    database: &Database,
    riot: &RiotApiClient,
    coordinator: &SyncCoordinator,
    app: &AppHandle,
    route: &RegionalRoute,
    puuid: &str,
) -> AppResult<()> {
    process_pending(database, riot, coordinator, app, route, puuid).await?;

    let persisted = {
        let connection = database.connection()?;
        SyncRepository::get(&connection, puuid)?
    };

    if persisted.initial_sync_complete {
        discover_incremental(database, riot, coordinator, app, route, puuid).await
    } else {
        discover_initial(
            database,
            riot,
            coordinator,
            app,
            route,
            puuid,
            persisted.next_match_start,
        )
        .await
    }
}

async fn discover_initial(
    database: &Database,
    riot: &RiotApiClient,
    coordinator: &SyncCoordinator,
    app: &AppHandle,
    route: &RegionalRoute,
    puuid: &str,
    mut start: u32,
) -> AppResult<()> {
    loop {
        let fetch_started = Instant::now();
        let match_ids = riot.match_ids(route, puuid, start, MATCH_PAGE_SIZE).await?;
        tracing::info!(target: "sync", operation = "match-id-fetch", start, count = match_ids.len(), elapsed_ms = fetch_started.elapsed().as_millis(), "fetched Match-V5 ID page");
        let page_len = match_ids.len() as u32;
        {
            let mut connection = database.connection()?;
            SyncRepository::enqueue(&mut connection, puuid, &match_ids)?;
            start = start.saturating_add(page_len);
            SyncRepository::advance_discovery(
                &connection,
                puuid,
                start,
                page_len < MATCH_PAGE_SIZE,
            )?;
        }
        process_pending(database, riot, coordinator, app, route, puuid).await?;
        if page_len < MATCH_PAGE_SIZE {
            return Ok(());
        }
    }
}

async fn discover_incremental(
    database: &Database,
    riot: &RiotApiClient,
    coordinator: &SyncCoordinator,
    app: &AppHandle,
    route: &RegionalRoute,
    puuid: &str,
) -> AppResult<()> {
    let mut start = 0;
    loop {
        let fetch_started = Instant::now();
        let match_ids = riot.match_ids(route, puuid, start, MATCH_PAGE_SIZE).await?;
        tracing::info!(target: "sync", operation = "match-id-fetch", start, count = match_ids.len(), elapsed_ms = fetch_started.elapsed().as_millis(), "fetched Match-V5 ID page");
        let mut unknown = Vec::new();
        let mut reached_known = false;
        {
            let connection = database.connection()?;
            for match_id in &match_ids {
                if MatchRepository::exists(&connection, match_id)? {
                    reached_known = true;
                    break;
                }
                unknown.push(match_id.clone());
            }
        }
        {
            let mut connection = database.connection()?;
            SyncRepository::enqueue(&mut connection, puuid, &unknown)?;
        }
        process_pending(database, riot, coordinator, app, route, puuid).await?;

        if reached_known || match_ids.len() < MATCH_PAGE_SIZE as usize {
            return Ok(());
        }
        start = start.saturating_add(MATCH_PAGE_SIZE);
    }
}

async fn process_pending(
    database: &Database,
    riot: &RiotApiClient,
    coordinator: &SyncCoordinator,
    app: &AppHandle,
    route: &RegionalRoute,
    puuid: &str,
) -> AppResult<()> {
    loop {
        let match_ids = {
            let mut connection = database.connection()?;
            SyncRepository::claim_pending_batch(&mut connection, puuid, MATCH_FETCH_CONCURRENCY)?
        };
        if match_ids.is_empty() {
            return Ok(());
        }
        let batch_started = Instant::now();
        let mut tasks = JoinSet::new();
        for match_id in match_ids {
            let client = riot.clone();
            let route = route.clone();
            tasks.spawn(async move {
                let result = client.match_by_id_timed(&route, &match_id).await;
                (match_id, result)
            });
        }
        let mut metrics = SyncBatchMetrics::default();
        let mut first_error = None;
        while let Some(joined) = tasks.join_next().await {
            let (match_id, response) = match joined {
                Ok(value) => value,
                Err(error) => {
                    first_error.get_or_insert_with(|| {
                        AppError::Unavailable(format!("match fetch task failed: {error}"))
                    });
                    continue;
                }
            };
            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    let connection = database.connection()?;
                    SyncRepository::mark_error(&connection, puuid, &match_id, &error.to_string())?;
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    continue;
                }
            };
            metrics.network += response.timing.network;
            metrics.rate_limit_wait += response.timing.rate_limit_wait;
            metrics.retry_backoff += response.timing.retry_backoff;
            metrics.deserialize += response.timing.deserialize;
            let parse_started = Instant::now();
            let parsed = match parse_match(response.data, puuid) {
                Ok(parsed) => parsed,
                Err(error) => {
                    metrics.parse += parse_started.elapsed();
                    let connection = database.connection()?;
                    SyncRepository::mark_error(&connection, puuid, &match_id, &error.to_string())?;
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    continue;
                }
            };
            metrics.parse += parse_started.elapsed();
            let timing = {
                let mut connection = database.connection()?;
                let timing = MatchRepository::ingest_synced_timed(
                    &mut connection,
                    &parsed.match_record,
                    &parsed.player_match,
                )?
                .1;
                MatchRepository::upsert_participant_roster(
                    &mut connection,
                    &parsed.match_record.match_id,
                    &parsed.participant_roster,
                )?;
                timing
            };
            metrics.db += timing.total;
            metrics.aggregate += timing.aggregate;
            metrics.queue_update += timing.queue_update;
            metrics.matches += 1;
        }
        let (complete, total) = {
            let connection = database.connection()?;
            SyncRepository::queue_counts(&connection, puuid)?
        };
        let event_emitted = coordinator
            .progress(
                app,
                complete,
                total,
                format!("Stored {complete} of {total} matches"),
            )
            .await;
        metrics.log(batch_started.elapsed(), event_emitted);
        if let Some(error) = first_error {
            return Err(error);
        }
    }
}

#[derive(Default)]
struct SyncBatchMetrics {
    matches: u64,
    network: Duration,
    rate_limit_wait: Duration,
    retry_backoff: Duration,
    deserialize: Duration,
    parse: Duration,
    db: Duration,
    aggregate: Duration,
    queue_update: Duration,
}

impl SyncBatchMetrics {
    fn log(&self, elapsed: Duration, event_emitted: bool) {
        let throughput = if elapsed.is_zero() {
            0.0
        } else {
            self.matches as f64 / elapsed.as_secs_f64()
        };
        let request_time = self.network + self.rate_limit_wait + self.retry_backoff;
        let wait_percent = if request_time.is_zero() {
            0.0
        } else {
            self.rate_limit_wait.as_secs_f64() / request_time.as_secs_f64() * 100.0
        };
        tracing::info!(target: "sync", operation = "match-batch", matches = self.matches,
            elapsed_ms = elapsed.as_millis(), network_ms = self.network.as_millis(),
            rate_limit_wait_ms = self.rate_limit_wait.as_millis(), rate_limit_wait_percent = wait_percent,
            retry_backoff_ms = self.retry_backoff.as_millis(), deserialize_ms = self.deserialize.as_millis(),
            parse_ms = self.parse.as_millis(), db_ms = self.db.as_millis(), aggregate_ms = self.aggregate.as_millis(),
            queue_update_ms = self.queue_update.as_millis(), static_metadata_ms = 0_u64,
            throughput_matches_per_sec = throughput, progress_event_emitted = event_emitted,
            "sync batch diagnostics");
    }
}

fn should_emit_progress(last: Option<Instant>, now: Instant, force: bool) -> bool {
    force || last.is_none_or(|last| now.duration_since(last) >= PROGRESS_EVENT_INTERVAL)
}

#[cfg(test)]
mod progress_tests {
    use super::{SyncCoordinator, SyncTrigger, is_stale, should_emit_progress};
    use crate::error::{AppError, AppResult};
    use std::time::{Duration, Instant};

    #[test]
    fn throttles_progress_but_never_suppresses_forced_final_state() {
        let start = Instant::now();
        assert!(should_emit_progress(None, start, false));
        assert!(!should_emit_progress(
            Some(start),
            start + Duration::from_millis(100),
            false
        ));
        assert!(should_emit_progress(
            Some(start),
            start + Duration::from_millis(100),
            true
        ));
    }

    #[test]
    fn every_automatic_and_manual_source_has_a_non_secret_diagnostic_trigger() {
        assert_eq!(
            [
                SyncTrigger::Startup,
                SyncTrigger::SettingsSaved,
                SyncTrigger::Periodic,
                SyncTrigger::Resume,
                SyncTrigger::Manual,
                SyncTrigger::ArchiveReset
            ]
            .map(SyncTrigger::as_str),
            [
                "startup",
                "settings_saved",
                "periodic",
                "resume",
                "manual",
                "archive_reset"
            ],
        );
    }

    #[tokio::test]
    async fn coordinator_coalesces_overlapping_sync_requests() {
        let coordinator = SyncCoordinator::new();
        assert!(coordinator.begin(SyncTrigger::Startup).await);
        assert!(!coordinator.begin(SyncTrigger::Manual).await);
        let state = coordinator.snapshot().await;
        assert!(state.currently_running);
        assert_eq!(state.status, "checking");
        assert_eq!(state.trigger.as_deref(), Some("startup"));
    }

    #[tokio::test]
    async fn failed_startup_transitions_to_error_and_a_later_automatic_check_recovers() {
        let coordinator = SyncCoordinator::new();
        assert!(coordinator.begin(SyncTrigger::Startup).await);
        let failed: AppResult<()> = Err(AppError::Configuration("temporary failure".into()));
        let error = coordinator.complete(&failed).await;
        assert_eq!(error.status, "error");
        assert!(!error.currently_running);
        assert!(coordinator.begin(SyncTrigger::Periodic).await);
        let succeeded: AppResult<()> = Ok(());
        let success = coordinator.complete(&succeeded).await;
        assert_eq!(success.status, "success");
        assert!(!success.currently_running);
        assert!(success.last_successful_sync_at.is_some());
    }

    #[test]
    fn stale_checks_allow_a_failed_startup_to_be_retried_later() {
        assert!(is_stale("2000-01-01 00:00:00"));
        assert!(!is_stale(&chrono::Utc::now().to_rfc3339()));
    }
}
