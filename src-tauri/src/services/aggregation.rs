use std::collections::HashMap;

use chrono::{Datelike, TimeZone, Utc};
use rusqlite::{OptionalExtension, params};

use crate::db::Database;
use crate::db::repositories::account::AccountRepository;
use crate::db::repositories::aggregates::AggregateRepository;
use crate::db::repositories::matches::MatchRepository;
use crate::db::repositories::static_data::StaticDataRepository;
use crate::db::repositories::statistics::StatisticsRepository;
use crate::domain::account::Account;
use crate::domain::aggregates::AggregateCounters;
use crate::domain::analytics::{AnalyticsFilter, QueueFilter, TimeRangeFilter};
use crate::domain::runes::RuneSelectionType;
use crate::domain::static_data::StaticCatalog;
use crate::domain::stats::{
    FilterContext, RunePageKey, TrackedMatchSample, UsageStats, boots_usage, core_build_usage,
    filtered_matches, keystone_usage, rune_page_usage, spell_pair_usage,
};
use crate::dto::analytics::{
    AccountDto, AnalyticsFilterDto, CareerDto, CareerQueuesDto, ChampionMasteryDto,
    ChampionProfileDto, ChampionSummaryDto, ClientStateDto, CoreBuildDto, CoreBuildStatsDto,
    EntityUsageDto, HistoricalSyncDto, HomeDto, LaningAtTenDto, MatchDetailDto, MatchItemDto, MatchQueryDto, MatchSummaryDto, PageDto,
    PerformanceDto, PreferenceDto, RankDto, ResolvedFilterDto, RunePageDto, RunePageStatsDto,
    SpellPairStatsDto, SyncStateDto, TrackedOverviewDto,
};
use crate::error::{AppError, AppResult};
use crate::services::filters::{FilterResolver, ResolvedFilter};

pub struct AggregationService<'state> {
    database: &'state Database,
}

impl<'state> AggregationService<'state> {
    pub fn new(database: &'state Database) -> Self {
        Self { database }
    }

    pub fn home(&self) -> AppResult<HomeDto> {
        let (account, samples, mastery, catalog) = self.load()?;
        let filter = AnalyticsFilter {
            queue: QueueFilter::All,
            time_range: TimeRangeFilter::AllTracked,
        };
        let recent = select(&samples, filter, None, Some(20));

        let (overview, ranked_games, top_champions, rank, sync_state, historical_sync, configured_executable_found) = {
            let connection = self.database.connection()?;
            let Some(account_ref) = account.as_ref() else {
                return Ok(empty_home(configured_client_path(&connection)?));
            };
            let resolved = FilterResolver::resolve(&connection, filter)?;
            let overview = AggregateRepository::career(
                &connection,
                &account_ref.puuid,
                &resolved.aggregate_scope,
            )?;
            let ranked_scope = FilterResolver::resolve_with_values(
                AnalyticsFilter {
                    queue: QueueFilter::RankedSolo,
                    time_range: TimeRangeFilter::AllTracked,
                },
                resolved.current_patch.clone(),
                resolved.current_season.clone(),
            );
            let ranked_games = AggregateRepository::career(
                &connection,
                &account_ref.puuid,
                &ranked_scope.aggregate_scope,
            )?
            .games
            .max(0) as u64;
            let mut top_champions: Vec<_> = AggregateRepository::champions(
                &connection,
                &account_ref.puuid,
                &resolved.aggregate_scope,
            )?
            .into_iter()
            .take(3)
            .map(|(champion_id, counters)| {
                champion_summary(&samples, &mastery, &catalog, champion_id, filter, &counters)
            })
            .collect();
            let sync_state = if let Some(account) = &account {
                connection.query_row(
                    "SELECT status, last_error, last_successful_sync_at, last_check_at, last_trigger FROM sync_state WHERE puuid = ?1",
                    [&account.puuid],
                    |row| Ok(SyncStateDto { status: row.get(0)?, currently_running: false, trigger: row.get(4)?, completed: 0, total: None,
                        message: row.get(1)?, last_successful_sync_at: row.get(2)?, last_check_at: row.get(3)? }),
                ).unwrap_or_else(|_| idle_sync())
            } else {
                idle_sync()
            };
            let historical_sync = historical_sync_diagnostics(&connection, &account_ref.puuid)?;
            let rank = connection
                .query_row(
                    "SELECT tier, rank_division, league_points, wins, losses
                 FROM rank_snapshots WHERE puuid = ?1 AND queue_type = 'RANKED_SOLO_5x5'
                 ORDER BY captured_at DESC, id DESC LIMIT 1",
                    [&account_ref.puuid],
                    |row| {
                        let wins: i64 = row.get(3)?;
                        let losses: i64 = row.get(4)?;
                        Ok(RankDto {
                            tier: row.get(0)?,
                            division: row.get(1)?,
                            league_points: row.get(2)?,
                            wins,
                            losses,
                            win_rate: percentage(wins, wins + losses),
                        })
                    },
                )
                .optional()?;
            let configured = configured_client_path(&connection)?;
            top_champions.truncate(3);
            (
                overview,
                ranked_games,
                top_champions,
                rank,
                sync_state,
                historical_sync,
                configured,
            )
        };

        Ok(HomeDto {
            account: account.map(account_dto),
            rank,
            client_state: ClientStateDto {
                riot_client_running: false,
                league_client_running: false,
                game_running: false,
                configured_executable_found,
            },
            sync_state,
            historical_sync,
            tracked_career: counters_overview_dto(&overview),
            ranked_games,
            recent_form: recent.iter().map(|sample| sample.win).collect(),
            top_champions,
        })
    }

    pub fn champions(&self, filter: AnalyticsFilter) -> AppResult<Vec<ChampionSummaryDto>> {
        let (_, samples, mastery, catalog) = self.load()?;
        let connection = self.database.connection()?;
        let Some(account) = AccountRepository::new(&connection).get()? else {
            return Ok(Vec::new());
        };
        let scope = FilterResolver::resolve(&connection, filter)?.aggregate_scope;
        AggregateRepository::champions(&connection, &account.puuid, &scope).map(|rows| {
            rows.into_iter()
                .map(|(id, counters)| {
                    champion_summary(&samples, &mastery, &catalog, id, filter, &counters)
                })
                .collect()
        })
    }

    pub fn champion_profile(
        &self,
        champion_id: i64,
        filter: AnalyticsFilter,
    ) -> AppResult<ChampionProfileDto> {
        let (_, samples, mastery, catalog) = self.load()?;
        let (counters, resolved, laning_at_ten) = {
            let connection = self.database.connection()?;
            let resolved = FilterResolver::resolve(&connection, filter)?;
            match AccountRepository::new(&connection).get()? {
                Some(account) => {
                    let counters = AggregateRepository::champion(
                        &connection,
                        &account.puuid,
                        champion_id,
                        &resolved.aggregate_scope,
                    )?;
                    let laning = laning_at_ten(&connection, &account.puuid, champion_id, &resolved.aggregate_scope)?;
                    (counters, resolved, laning)
                }
                None => (AggregateCounters::default(), resolved, LaningAtTenDto::default()),
            }
        };
        let selected = select_resolved(&samples, &resolved, Some(champion_id), None);
        let mastery = mastery.get(&champion_id).copied();
        Ok(ChampionProfileDto {
            champion: catalog.champion(champion_id).into(),
            mastery: ChampionMasteryDto {
                points: mastery.map(|m| m.0),
                level: mastery.map(|m| m.1),
            },
            filter_context: resolved_filter_dto(&resolved),
            overview: counters_overview_dto(&counters),
            performance: counters_performance_dto(&counters),
            laning_at_ten,
            core_builds: core_build_usage(&selected)
                .into_iter()
                .map(|usage| core_build_stats_dto(usage, &catalog))
                .collect(),
            boots: boots_usage(&selected)
                .into_iter()
                .map(|usage| EntityUsageDto {
                    entity: catalog.item(usage.key).into(),
                    games: usage.games,
                    wins: usage.wins,
                    usage_rate: usage.usage_rate,
                    win_rate: usage.win_rate,
                })
                .collect(),
            rune_pages: rune_page_usage(&selected)
                .into_iter()
                .map(|usage| rune_page_stats_dto(usage, &catalog))
                .collect(),
            keystone_usage: keystone_usage(&selected)
                .into_iter()
                .map(|usage| EntityUsageDto {
                    entity: catalog.rune(usage.key).into(),
                    games: usage.games,
                    wins: usage.wins,
                    usage_rate: usage.usage_rate,
                    win_rate: usage.win_rate,
                })
                .collect(),
            summoner_spell_pairs: spell_pair_usage(&selected)
                .into_iter()
                .map(|usage| SpellPairStatsDto {
                    spells: usage
                        .key
                        .into_iter()
                        .map(|id| catalog.spell(id).into())
                        .collect(),
                    games: usage.games,
                    wins: usage.wins,
                    usage_rate: usage.usage_rate,
                    win_rate: usage.win_rate,
                })
                .collect(),
        })
    }

    pub fn matches(&self, query: MatchQueryDto) -> AppResult<PageDto<MatchSummaryDto>> {
        let filter: AnalyticsFilter = AnalyticsFilterDto {
            queue: query.queue,
            time_range: query.time_range,
        }
        .into();
        let offset = query
            .cursor
            .as_deref()
            .unwrap_or("0")
            .parse::<u32>()
            .map_err(|_| AppError::Configuration("invalid match cursor".to_owned()))?;
        let limit = query.limit.unwrap_or(50).clamp(1, 100);
        let connection = self.database.connection()?;
        let Some(account) = AccountRepository::new(&connection).get()? else {
            return Ok(PageDto {
                items: Vec::new(),
                next_cursor: None,
            });
        };
        let resolved = FilterResolver::resolve(&connection, filter)?;
        let catalog = StaticDataRepository::catalog(&connection)?;
        let mut rows = MatchRepository::page(
            &connection,
            &account.puuid,
            query.champion_id,
            &resolved.aggregate_scope,
            offset,
            limit + 1,
        )?;
        let has_more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let items = rows
            .into_iter()
            .map(|sample| MatchSummaryDto {
                match_id: sample.match_id,
                champion: catalog.champion(sample.champion_id).into(),
                win: sample.win,
                queue_id: sample.queue_id,
                kills: sample.kills,
                deaths: sample.deaths,
                assists: sample.assists,
                duration_seconds: sample.duration_seconds,
                keystone: sample.keystone_id.map(|id| catalog.rune(id).into()),
                summoner_spells: sample
                    .summoner_spell_ids
                    .into_iter()
                    .map(|id| catalog.spell(id).into())
                    .collect(),
                game_creation: Utc
                    .timestamp_millis_opt(sample.game_creation)
                    .single()
                    .map(|value| value.to_rfc3339())
                    .unwrap_or_else(|| sample.game_creation.to_string()),
                patch: sample.patch.clone(),
            })
            .collect();
        Ok(PageDto {
            items,
            next_cursor: has_more.then(|| offset.saturating_add(limit).to_string()),
        })
    }

    pub fn match_detail(&self, match_id: &str) -> AppResult<MatchDetailDto> {
        let connection = self.database.connection()?;
        let account = AccountRepository::new(&connection).get()?.ok_or_else(|| {
            AppError::Configuration(
                "configure a Riot account before opening match details".to_owned(),
            )
        })?;
        let (record, player) = MatchRepository::detail(&connection, &account.puuid, match_id)?
            .ok_or_else(|| {
                AppError::Configuration("match was not found in local history".to_owned())
            })?;
        let catalog = StaticDataRepository::catalog(&connection)?;
        let mut primary = Vec::new();
        let mut secondary = Vec::new();
        let mut shards = Vec::new();
        for rune in &player.rune_selections {
            match rune.selection_type {
                RuneSelectionType::Primary => {
                    primary.push((rune.slot, catalog.rune(rune.rune_id).into()))
                }
                RuneSelectionType::Secondary => {
                    secondary.push((rune.slot, catalog.rune(rune.rune_id).into()))
                }
                RuneSelectionType::StatShard => {
                    shards.push((rune.slot, catalog.stat_shard(rune.rune_id).into()))
                }
            }
        }
        primary.sort_by_key(|entry| entry.0);
        secondary.sort_by_key(|entry| entry.0);
        shards.sort_by_key(|entry| entry.0);
        Ok(MatchDetailDto {
            match_id: record.match_id,
            champion: catalog.champion(player.champion_id).into(),
            win: player.win,
            queue_id: record.queue_id,
            game_creation: Utc
                .timestamp_millis_opt(record.game_creation)
                .single()
                .map(|value| value.to_rfc3339())
                .unwrap_or_else(|| record.game_creation.to_string()),
            duration_seconds: record.game_duration_seconds,
            patch: record.patch,
            kills: player.kills,
            deaths: player.deaths,
            assists: player.assists,
            total_cs: player.total_minions_killed + player.neutral_minions_killed,
            gold_earned: player.gold_earned,
            summoner_spells: player
                .summoner_spell_ids
                .into_iter()
                .map(|id| catalog.spell(id).into())
                .collect(),
            rune_page: RunePageDto {
                primary_style: player
                    .primary_style_id
                    .map(|id| catalog.rune_style(id).into()),
                primary_runes: primary.into_iter().map(|entry| entry.1).collect(),
                secondary_style: player
                    .secondary_style_id
                    .map(|id| catalog.rune_style(id).into()),
                secondary_runes: secondary.into_iter().map(|entry| entry.1).collect(),
                stat_shards: shards.into_iter().map(|entry| entry.1).collect(),
            },
            final_items: player
                .final_items
                .into_iter()
                .filter(|item| item.item_id != 0)
                .map(|item| MatchItemDto {
                    item: catalog.item(item.item_id).into(),
                    slot: item.slot,
                })
                .collect(),
            double_kills: player.double_kills,
            triple_kills: player.triple_kills,
            quadra_kills: player.quadra_kills,
            penta_kills: player.penta_kills,
        })
    }

    pub fn career(&self, filter: AnalyticsFilter) -> AppResult<CareerDto> {
        let connection = self.database.connection()?;
        let Some(account) = AccountRepository::new(&connection).get()? else {
            return Ok(empty_career());
        };
        let resolved = FilterResolver::resolve(&connection, filter)?;
        let overall =
            AggregateRepository::career(&connection, &account.puuid, &resolved.aggregate_scope)?;
        let queue_overview = |queue| -> AppResult<AggregateCounters> {
            let scoped = FilterResolver::resolve_with_values(
                AnalyticsFilter {
                    queue,
                    time_range: filter.time_range,
                },
                resolved.current_patch.clone(),
                resolved.current_season.clone(),
            );
            AggregateRepository::career(&connection, &account.puuid, &scoped.aggregate_scope)
        };
        let champions =
            AggregateRepository::champions(&connection, &account.puuid, &resolved.aggregate_scope)?;
        let most_played_champion_id = champions.first().map(|(id, _)| *id);
        let champion_pool = champions.len() as u64;
        Ok(CareerDto {
            overall: counters_overview_dto(&overall),
            by_queue: CareerQueuesDto {
                ranked_solo: counters_overview_dto(&queue_overview(QueueFilter::RankedSolo)?),
                normal: counters_overview_dto(&queue_overview(QueueFilter::Normal)?),
                aram: counters_overview_dto(&queue_overview(QueueFilter::Aram)?),
            },
            average_match_duration_seconds: average(overall.playtime_seconds, overall.games).round()
                as u64,
            most_played_champion_id,
            champion_pool,
        })
    }

    fn load(
        &self,
    ) -> AppResult<(
        Option<Account>,
        Vec<TrackedMatchSample>,
        HashMap<i64, (i64, i64)>,
        StaticCatalog,
    )> {
        let connection = self.database.connection()?;
        let account = AccountRepository::new(&connection).get()?;
        let Some(account) = account else {
            return Ok((
                None,
                Vec::new(),
                HashMap::new(),
                StaticDataRepository::catalog(&connection)?,
            ));
        };
        let repository = StatisticsRepository::new(&connection);
        let matches = repository.load_matches(&account.puuid)?;
        let mastery = repository.mastery(&account.puuid)?;
        let catalog = StaticDataRepository::catalog(&connection)?;
        Ok((Some(account), matches, mastery, catalog))
    }
}

fn select<'a>(
    samples: &'a [TrackedMatchSample],
    filter: AnalyticsFilter,
    champion_id: Option<i64>,
    recent_games: Option<usize>,
) -> Vec<&'a TrackedMatchSample> {
    let current_patch = samples.first().map(|m| m.patch.as_str()).unwrap_or("");
    let current_season = Utc::now().year().to_string();
    filtered_matches(
        samples,
        FilterContext {
            filter,
            champion_id,
            current_patch,
            current_season: &current_season,
            recent_games,
        },
    )
}

fn select_resolved<'a>(
    samples: &'a [TrackedMatchSample],
    resolved: &ResolvedFilter,
    champion_id: Option<i64>,
    recent_games: Option<usize>,
) -> Vec<&'a TrackedMatchSample> {
    filtered_matches(
        samples,
        FilterContext {
            filter: resolved.analytics,
            champion_id,
            current_patch: &resolved.current_patch,
            current_season: &resolved.current_season,
            recent_games,
        },
    )
}

fn resolved_filter_dto(resolved: &ResolvedFilter) -> ResolvedFilterDto {
    ResolvedFilterDto {
        queue: resolved.analytics.queue.into(),
        time_range: resolved.analytics.time_range.into(),
        current_patch: resolved.current_patch.clone(),
        current_season: resolved.current_season.clone(),
    }
}

fn champion_summary(
    samples: &[TrackedMatchSample],
    mastery: &HashMap<i64, (i64, i64)>,
    catalog: &StaticCatalog,
    champion_id: i64,
    filter: AnalyticsFilter,
    counters: &AggregateCounters,
) -> ChampionSummaryDto {
    let selected = select(samples, filter, Some(champion_id), None);
    let mastery = mastery.get(&champion_id).copied();
    ChampionSummaryDto {
        champion: catalog.champion(champion_id).into(),
        mastery_points: mastery.map(|m| m.0),
        mastery_level: mastery.map(|m| m.1),
        tracked_games: counters.games.max(0) as u64,
        wins: counters.wins.max(0) as u64,
        losses: counters.losses.max(0) as u64,
        win_rate: percentage(counters.wins, counters.games),
        playtime_seconds: counters.playtime_seconds.max(0) as u64,
        kills: counters.kills.max(0) as u64,
        deaths: counters.deaths.max(0) as u64,
        assists: counters.assists.max(0) as u64,
        kda: kda(counters),
        most_used_core_build: core_build_usage(&selected).into_iter().next().map(|u| {
            CoreBuildDto {
                items: u
                    .key
                    .into_iter()
                    .map(|id| catalog.item(id).into())
                    .collect(),
                games: u.games,
                usage_rate: u.usage_rate,
                win_rate: u.win_rate,
            }
        }),
        most_used_keystone: keystone_usage(&selected)
            .into_iter()
            .next()
            .map(|u| PreferenceDto {
                ids: vec![u.key],
                games: u.games,
                usage_rate: u.usage_rate,
                win_rate: u.win_rate,
            }),
    }
}

fn core_build_stats_dto(usage: UsageStats<Vec<i64>>, catalog: &StaticCatalog) -> CoreBuildStatsDto {
    CoreBuildStatsDto {
        items: usage
            .key
            .into_iter()
            .map(|id| catalog.item(id).into())
            .collect(),
        games: usage.games,
        wins: usage.wins,
        usage_rate: usage.usage_rate,
        win_rate: usage.win_rate,
    }
}

fn rune_page_stats_dto(
    usage: UsageStats<RunePageKey>,
    catalog: &StaticCatalog,
) -> RunePageStatsDto {
    RunePageStatsDto {
        primary_style: catalog.rune_style(usage.key.primary_style_id).into(),
        primary_runes: usage
            .key
            .primary_rune_ids
            .into_iter()
            .map(|id| catalog.rune(id).into())
            .collect(),
        secondary_style: catalog.rune_style(usage.key.secondary_style_id).into(),
        secondary_runes: usage
            .key
            .secondary_rune_ids
            .into_iter()
            .map(|id| catalog.rune(id).into())
            .collect(),
        stat_shards: usage
            .key
            .stat_shard_ids
            .into_iter()
            .map(|id| catalog.stat_shard(id).into())
            .collect(),
        games: usage.games,
        wins: usage.wins,
        usage_rate: usage.usage_rate,
        win_rate: usage.win_rate,
    }
}

fn counters_overview_dto(counters: &AggregateCounters) -> TrackedOverviewDto {
    TrackedOverviewDto {
        games: counters.games.max(0) as u64,
        wins: counters.wins.max(0) as u64,
        losses: counters.losses.max(0) as u64,
        win_rate: percentage(counters.wins, counters.games),
        playtime_seconds: counters.playtime_seconds.max(0) as u64,
        kills: counters.kills.max(0) as u64,
        deaths: counters.deaths.max(0) as u64,
        assists: counters.assists.max(0) as u64,
        kda: kda(counters),
    }
}

fn counters_performance_dto(counters: &AggregateCounters) -> PerformanceDto {
    let minutes = counters.playtime_seconds.max(0) as f64 / 60.0;
    PerformanceDto {
        average_kills: average(counters.kills, counters.games),
        average_deaths: average(counters.deaths, counters.games),
        average_assists: average(counters.assists, counters.games),
        average_cs_per_minute: if minutes == 0.0 {
            0.0
        } else {
            (counters.total_minions_killed + counters.neutral_minions_killed).max(0) as f64
                / minutes
        },
        average_match_duration_seconds: average(counters.playtime_seconds, counters.games),
        highest_kills: counters.highest_kills.max(0) as u64,
        double_kills: counters.double_kills.max(0) as u64,
        triple_kills: counters.triple_kills.max(0) as u64,
        quadra_kills: counters.quadra_kills.max(0) as u64,
        penta_kills: counters.penta_kills.max(0) as u64,
    }
}

fn laning_at_ten(
    connection: &rusqlite::Connection,
    puuid: &str,
    champion_id: i64,
    scope: &crate::domain::aggregates::AggregateScope,
) -> AppResult<LaningAtTenDto> {
    connection.query_row(
        "SELECT COUNT(m.match_id), COUNT(snapshot.match_id),
                AVG(snapshot.lane_minions * 1.0), AVG(snapshot.lane_minions * 1.0 / 10.0),
                AVG(snapshot.total_gold * 1.0), AVG(snapshot.experience * 1.0), AVG(snapshot.level * 1.0)
         FROM matches m JOIN player_matches pm ON pm.match_id = m.match_id
         LEFT JOIN match_laning_snapshots snapshot
           ON snapshot.match_id = m.match_id AND snapshot.puuid = pm.puuid AND snapshot.minute = 10
         WHERE pm.puuid = ?1 AND pm.champion_id = ?2
           AND m.queue_id IN (400, 420, 430) AND m.game_duration >= 600
           AND (?3 = ?4 OR (?3 = ?5 AND m.queue_id IN (400, 430)) OR m.queue_id = ?3)
           AND (?6 = '' OR m.patch = ?6)
           AND (?7 = '' OR COALESCE(m.season_key, '') = ?7)",
        params![puuid, champion_id, scope.queue_scope, crate::domain::aggregates::ALL_QUEUES,
            crate::domain::aggregates::NORMAL_QUEUES, scope.patch, scope.season],
        |row| Ok(LaningAtTenDto {
            eligible_games: row.get::<_, i64>(0)?.max(0) as u64,
            covered_games: row.get::<_, i64>(1)?.max(0) as u64,
            average_cs_at_ten: row.get(2)?,
            average_cs_per_minute_at_ten: row.get(3)?,
            average_gold_at_ten: row.get(4)?,
            average_xp_at_ten: row.get(5)?,
            average_level_at_ten: row.get(6)?,
        }),
    ).map_err(Into::into)
}

fn historical_sync_diagnostics(
    connection: &rusqlite::Connection,
    puuid: &str,
) -> AppResult<HistoricalSyncDto> {
    let (matches_tracked, oldest_tracked_at, tracked_playtime_seconds): (i64, Option<i64>, i64) = connection.query_row(
        "SELECT COUNT(*), MIN(m.game_creation), COALESCE(SUM(m.game_duration), 0)
         FROM matches m JOIN player_matches pm ON pm.match_id = m.match_id WHERE pm.puuid = ?1",
        [puuid],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let state = connection.query_row(
        "SELECT initial_sync_complete, status, next_match_start FROM sync_state WHERE puuid = ?1",
        [puuid],
        |row| Ok((row.get::<_, bool>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)),
    ).optional()?;
    let (active_queue_rows, errored_queue_rows): (i64, i64) = connection.query_row(
        "SELECT
             COALESCE(SUM(CASE WHEN status IN ('pending', 'fetching') THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END), 0)
         FROM sync_match_queue WHERE puuid = ?1",
        [puuid],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let (history_status, next_match_start) = match state {
        Some((initial_complete, status, cursor)) => (
            historical_sync_status(
                initial_complete,
                &status,
                active_queue_rows,
                errored_queue_rows,
            ),
            cursor,
        ),
        None => ("Still backfilling", 0),
    };
    Ok(HistoricalSyncDto {
        matches_tracked: matches_tracked.max(0) as u64,
        oldest_tracked_at: oldest_tracked_at.and_then(|value| Utc.timestamp_millis_opt(value).single()).map(|value| value.to_rfc3339()),
        tracked_playtime_seconds: tracked_playtime_seconds.max(0) as u64,
        history_status: history_status.to_owned(),
        next_match_start: next_match_start.max(0) as u64,
    })
}

fn historical_sync_status(
    initial_sync_complete: bool,
    sync_status: &str,
    active_queue_rows: i64,
    errored_queue_rows: i64,
) -> &'static str {
    if sync_status == "error" || errored_queue_rows > 0 {
        "Interrupted"
    } else if !initial_sync_complete || active_queue_rows > 0 {
        "Still backfilling"
    } else {
        "Complete"
    }
}

fn average(value: i64, games: i64) -> f64 {
    if games <= 0 {
        0.0
    } else {
        value.max(0) as f64 / games as f64
    }
}

fn percentage(value: i64, total: i64) -> f64 {
    average(value, total) * 100.0
}

fn kda(counters: &AggregateCounters) -> f64 {
    let numerator = (counters.kills + counters.assists).max(0) as f64;
    if counters.deaths <= 0 {
        numerator
    } else {
        numerator / counters.deaths as f64
    }
}

fn configured_client_path(connection: &rusqlite::Connection) -> AppResult<bool> {
    let path: Option<String> = connection.query_row(
        "SELECT riot_client_path FROM app_settings WHERE id = 1",
        [],
        |row| row.get(0),
    )?;
    Ok(path.is_some_and(|path| std::path::Path::new(&path).is_file()))
}

fn empty_home(configured_executable_found: bool) -> HomeDto {
    HomeDto {
        account: None,
        rank: None,
        client_state: ClientStateDto {
            riot_client_running: false,
            league_client_running: false,
            game_running: false,
            configured_executable_found,
        },
        sync_state: idle_sync(),
        historical_sync: HistoricalSyncDto {
            matches_tracked: 0,
            oldest_tracked_at: None,
            tracked_playtime_seconds: 0,
            history_status: "Not configured".to_owned(),
            next_match_start: 0,
        },
        tracked_career: counters_overview_dto(&AggregateCounters::default()),
        ranked_games: 0,
        recent_form: Vec::new(),
        top_champions: Vec::new(),
    }
}

fn empty_career() -> CareerDto {
    let empty = counters_overview_dto(&AggregateCounters::default());
    CareerDto {
        overall: empty.clone(),
        by_queue: CareerQueuesDto {
            ranked_solo: empty.clone(),
            normal: empty.clone(),
            aram: empty,
        },
        average_match_duration_seconds: 0,
        most_played_champion_id: None,
        champion_pool: 0,
    }
}

fn account_dto(account: Account) -> AccountDto {
    AccountDto {
        puuid: account.puuid,
        game_name: account.game_name,
        tag_line: account.tag_line,
        account_region: account.account_region,
        platform_region: account.platform_region,
    }
}

fn idle_sync() -> SyncStateDto {
    SyncStateDto {
        status: "idle".to_owned(),
        currently_running: false,
        trigger: None,
        completed: 0,
        total: None,
        message: None,
        last_check_at: None,
        last_successful_sync_at: None,
    }
}

#[cfg(test)]
mod enrichment_tests {
    use super::{
        champion_summary, core_build_stats_dto, historical_sync_status, rune_page_stats_dto,
    };
    use crate::domain::aggregates::AggregateCounters;
    use crate::domain::analytics::{AnalyticsFilter, QueueFilter, TimeRangeFilter};
    use crate::domain::static_data::{GameEntity, StaticCatalog};
    use crate::domain::stats::{RunePageKey, TrackedMatchSample, UsageStats};
    use std::collections::HashMap;

    fn catalog() -> StaticCatalog {
        let mut catalog = StaticCatalog::default();
        for (id, name) in [
            (6610, "Sundered Sky"),
            (6692, "Eclipse"),
            (3075, "Thornmail"),
        ] {
            catalog.items.insert(
                id,
                GameEntity {
                    id,
                    name: name.to_owned(),
                    icon: format!("https://example.test/{id}.png"),
                },
            );
        }
        catalog
    }

    fn sample() -> TrackedMatchSample {
        TrackedMatchSample {
            match_id: "OC1_1".to_owned(),
            champion_id: 82,
            queue_id: 420,
            patch: "16.15".to_owned(),
            season_key: Some("2026".to_owned()),
            game_creation: 1,
            duration_seconds: 1800,
            win: true,
            kills: 10,
            deaths: 2,
            assists: 8,
            double_kills: 1,
            triple_kills: 0,
            quadra_kills: 0,
            penta_kills: 0,
            minions: 200,
            keystone_id: Some(8010),
            rune_page: None,
            summoner_spell_ids: [4, 12],
            core_item_ids: vec![6610, 6692, 3075],
            boot_item_id: None,
        }
    }

    #[test]
    fn champion_summary_and_profile_core_builds_are_enriched() {
        let catalog = catalog();
        let samples = vec![sample()];
        let summary = champion_summary(
            &samples,
            &HashMap::new(),
            &catalog,
            82,
            AnalyticsFilter {
                queue: QueueFilter::All,
                time_range: TimeRangeFilter::AllTracked,
            },
            &AggregateCounters {
                games: 1,
                wins: 1,
                ..Default::default()
            },
        );
        let summary_build = summary.most_used_core_build.unwrap();
        assert_eq!(summary_build.items[0].name, "Thornmail");
        assert!(summary_build.items.iter().all(|item| !item.icon.is_empty()));

        let profile_build = core_build_stats_dto(
            UsageStats {
                key: vec![3075, 6610, 6692],
                games: 1,
                wins: 1,
                usage_rate: 100.0,
                win_rate: 100.0,
            },
            &catalog,
        );
        assert_eq!(profile_build.items.len(), 3);
        assert!(
            profile_build
                .items
                .iter()
                .all(|item| !item.name.starts_with("Unknown"))
        );
    }

    #[test]
    fn rune_page_dto_contains_enriched_stat_shards() {
        let dto = rune_page_stats_dto(
            UsageStats {
                key: RunePageKey {
                    primary_style_id: 8000,
                    primary_rune_ids: vec![],
                    secondary_style_id: 8400,
                    secondary_rune_ids: vec![],
                    stat_shard_ids: vec![5008, 5005, 5001],
                },
                games: 1,
                wins: 1,
                usage_rate: 100.0,
                win_rate: 100.0,
            },
            &catalog(),
        );
        assert_eq!(
            dto.stat_shards
                .iter()
                .map(|shard| shard.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Adaptive Force", "Attack Speed", "Health Scaling"]
        );
        assert!(dto.stat_shards.iter().all(|shard| !shard.icon.is_empty()));
    }

    #[test]
    fn historical_sync_status_distinguishes_completion_backfill_and_interruption() {
        assert_eq!(historical_sync_status(true, "success", 0, 0), "Complete");
        assert_eq!(
            historical_sync_status(false, "syncing", 0, 0),
            "Still backfilling"
        );
        assert_eq!(
            historical_sync_status(true, "success", 3, 0),
            "Still backfilling"
        );
        assert_eq!(historical_sync_status(true, "error", 0, 0), "Interrupted");
        assert_eq!(
            historical_sync_status(true, "success", 0, 1),
            "Interrupted"
        );
    }
}
