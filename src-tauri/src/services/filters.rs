use chrono::{Datelike, Utc};
use rusqlite::{Connection, OptionalExtension};

use crate::domain::aggregates::{ALL_QUEUES, AggregateScope, NORMAL_QUEUES};
use crate::domain::analytics::{AnalyticsFilter, QueueFilter, TimeRangeFilter};
use crate::error::AppResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedFilter {
    pub analytics: AnalyticsFilter,
    pub current_patch: String,
    pub current_season: String,
    pub aggregate_scope: AggregateScope,
}

pub struct FilterResolver;

impl FilterResolver {
    pub fn resolve(connection: &Connection, filter: AnalyticsFilter) -> AppResult<ResolvedFilter> {
        let static_version = connection
            .query_row(
                "SELECT version FROM static_data_versions WHERE is_active = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let current_patch = static_version
            .as_deref()
            .and_then(patch_from_static_version)
            .map(str::to_owned)
            .or(connection
                .query_row(
                    "SELECT patch FROM matches ORDER BY game_creation DESC, match_id DESC LIMIT 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()?)
            .unwrap_or_default();
        let current_season = Utc::now().year().to_string();
        Ok(Self::resolve_with_values(
            filter,
            current_patch,
            current_season,
        ))
    }

    pub fn resolve_with_values(
        filter: AnalyticsFilter,
        current_patch: String,
        current_season: String,
    ) -> ResolvedFilter {
        let queue_scope = match filter.queue {
            QueueFilter::All => ALL_QUEUES,
            QueueFilter::RankedSolo => 420,
            QueueFilter::Normal => NORMAL_QUEUES,
            QueueFilter::Aram => 450,
        };
        let (patch, season) = match filter.time_range {
            TimeRangeFilter::CurrentPatch => (current_patch.clone(), String::new()),
            TimeRangeFilter::CurrentSeason => (String::new(), current_season.clone()),
            TimeRangeFilter::AllTracked => (String::new(), String::new()),
        };
        ResolvedFilter {
            analytics: filter,
            current_patch,
            current_season,
            aggregate_scope: AggregateScope {
                queue_scope,
                patch,
                season,
            },
        }
    }
}

fn patch_from_static_version(version: &str) -> Option<&str> {
    let second_dot = version.match_indices('.').nth(1)?.0;
    Some(&version[..second_dot])
}

#[cfg(test)]
mod tests {
    use crate::domain::aggregates::NORMAL_QUEUES;
    use crate::domain::analytics::{AnalyticsFilter, QueueFilter, TimeRangeFilter};

    use super::FilterResolver;

    #[test]
    fn resolves_queue_and_time_scopes_centrally() {
        let patch = FilterResolver::resolve_with_values(
            AnalyticsFilter {
                queue: QueueFilter::Normal,
                time_range: TimeRangeFilter::CurrentPatch,
            },
            "16.15".to_owned(),
            "2026".to_owned(),
        );
        assert_eq!(patch.aggregate_scope.queue_scope, NORMAL_QUEUES);
        assert_eq!(
            (
                patch.aggregate_scope.patch.as_str(),
                patch.aggregate_scope.season.as_str()
            ),
            ("16.15", "")
        );

        let season = FilterResolver::resolve_with_values(
            AnalyticsFilter {
                queue: QueueFilter::Aram,
                time_range: TimeRangeFilter::CurrentSeason,
            },
            "16.15".to_owned(),
            "2026".to_owned(),
        );
        assert_eq!(
            (
                season.aggregate_scope.queue_scope,
                season.aggregate_scope.patch.as_str(),
                season.aggregate_scope.season.as_str()
            ),
            (450, "", "2026")
        );
    }

    #[test]
    fn derives_current_patch_from_data_dragon_version() {
        assert_eq!(super::patch_from_static_version("16.15.1"), Some("16.15"));
        assert_eq!(super::patch_from_static_version("invalid"), None);
    }
}
