use std::collections::HashMap;
use std::hash::Hash;

use crate::domain::analytics::{AnalyticsFilter, QueueFilter, TimeRangeFilter};
use crate::domain::items::CoreBuild;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackedMatchSample {
    pub match_id: String,
    pub champion_id: i64,
    pub queue_id: i64,
    pub patch: String,
    pub season_key: Option<String>,
    pub game_creation: i64,
    pub duration_seconds: u64,
    pub win: bool,
    pub kills: u64,
    pub deaths: u64,
    pub assists: u64,
    pub double_kills: u64,
    pub triple_kills: u64,
    pub quadra_kills: u64,
    pub penta_kills: u64,
    pub minions: u64,
    pub keystone_id: Option<i64>,
    pub rune_page: Option<RunePageKey>,
    pub summoner_spell_ids: [i64; 2],
    pub core_item_ids: Vec<i64>,
    pub boot_item_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RunePageKey {
    pub primary_style_id: i64,
    pub primary_rune_ids: Vec<i64>,
    pub secondary_style_id: i64,
    pub secondary_rune_ids: Vec<i64>,
    pub stat_shard_ids: Vec<i64>,
}

#[derive(Clone, Copy, Debug)]
pub struct FilterContext<'value> {
    pub filter: AnalyticsFilter,
    pub champion_id: Option<i64>,
    pub current_patch: &'value str,
    pub current_season: &'value str,
    pub recent_games: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OverviewStats {
    pub games: u64,
    pub wins: u64,
    pub losses: u64,
    pub playtime_seconds: u64,
    pub kills: u64,
    pub deaths: u64,
    pub assists: u64,
    pub win_rate: f64,
    pub kda: f64,
    pub average_kills: f64,
    pub average_deaths: f64,
    pub average_assists: f64,
    pub average_cs_per_minute: f64,
    pub average_match_duration_seconds: f64,
    pub highest_kills: u64,
    pub double_kills: u64,
    pub triple_kills: u64,
    pub quadra_kills: u64,
    pub penta_kills: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UsageStats<K> {
    pub key: K,
    pub games: u64,
    pub wins: u64,
    pub usage_rate: f64,
    pub win_rate: f64,
}

pub fn filtered_matches<'sample>(
    samples: &'sample [TrackedMatchSample],
    context: FilterContext<'_>,
) -> Vec<&'sample TrackedMatchSample> {
    let mut matches: Vec<_> = samples
        .iter()
        .filter(|sample| {
            context
                .champion_id
                .is_none_or(|champion_id| sample.champion_id == champion_id)
                && queue_matches(context.filter.queue, sample.queue_id)
                && match context.filter.time_range {
                    TimeRangeFilter::CurrentPatch => sample.patch == context.current_patch,
                    TimeRangeFilter::CurrentSeason => {
                        sample.season_key.as_deref() == Some(context.current_season)
                    }
                    TimeRangeFilter::AllTracked => true,
                }
        })
        .collect();
    matches.sort_by_key(|sample| std::cmp::Reverse(sample.game_creation));
    if let Some(limit) = context.recent_games {
        matches.truncate(limit);
    }
    matches
}

pub fn aggregate_overview(samples: &[&TrackedMatchSample]) -> OverviewStats {
    if samples.is_empty() {
        return OverviewStats::default();
    }

    let games = samples.len() as u64;
    let wins = samples.iter().filter(|sample| sample.win).count() as u64;
    let playtime_seconds = samples.iter().map(|sample| sample.duration_seconds).sum();
    let kills = samples.iter().map(|sample| sample.kills).sum();
    let deaths = samples.iter().map(|sample| sample.deaths).sum();
    let assists = samples.iter().map(|sample| sample.assists).sum();
    let total_minions: u64 = samples.iter().map(|sample| sample.minions).sum();
    let minutes = playtime_seconds as f64 / 60.0;

    OverviewStats {
        games,
        wins,
        losses: games - wins,
        playtime_seconds,
        kills,
        deaths,
        assists,
        win_rate: percentage(wins, games),
        kda: if deaths == 0 {
            (kills + assists) as f64
        } else {
            (kills + assists) as f64 / deaths as f64
        },
        average_kills: kills as f64 / games as f64,
        average_deaths: deaths as f64 / games as f64,
        average_assists: assists as f64 / games as f64,
        average_cs_per_minute: if minutes == 0.0 {
            0.0
        } else {
            total_minions as f64 / minutes
        },
        average_match_duration_seconds: playtime_seconds as f64 / games as f64,
        highest_kills: samples.iter().map(|sample| sample.kills).max().unwrap_or(0),
        double_kills: samples.iter().map(|sample| sample.double_kills).sum(),
        triple_kills: samples.iter().map(|sample| sample.triple_kills).sum(),
        quadra_kills: samples.iter().map(|sample| sample.quadra_kills).sum(),
        penta_kills: samples.iter().map(|sample| sample.penta_kills).sum(),
    }
}

pub fn keystone_usage(samples: &[&TrackedMatchSample]) -> Vec<UsageStats<i64>> {
    usage_groups(samples, |sample| sample.keystone_id, samples.len() as u64)
}

pub fn spell_pair_usage(samples: &[&TrackedMatchSample]) -> Vec<UsageStats<[i64; 2]>> {
    usage_groups(
        samples,
        |sample| {
            let mut pair = sample.summoner_spell_ids;
            pair.sort_unstable();
            Some(pair)
        },
        samples.len() as u64,
    )
}

pub fn core_build_usage(samples: &[&TrackedMatchSample]) -> Vec<UsageStats<Vec<i64>>> {
    usage_groups(
        samples,
        |sample| {
            let build = CoreBuild::first_three(sample.core_item_ids.clone()).item_ids;
            (!build.is_empty()).then_some(build)
        },
        samples.len() as u64,
    )
}

pub fn boots_usage(samples: &[&TrackedMatchSample]) -> Vec<UsageStats<i64>> {
    usage_groups(samples, |sample| sample.boot_item_id, samples.len() as u64)
}

pub fn rune_page_usage(samples: &[&TrackedMatchSample]) -> Vec<UsageStats<RunePageKey>> {
    usage_groups(
        samples,
        |sample| sample.rune_page.clone(),
        samples.len() as u64,
    )
}

fn usage_groups<K, F>(
    samples: &[&TrackedMatchSample],
    key_for: F,
    denominator: u64,
) -> Vec<UsageStats<K>>
where
    K: Clone + Eq + Hash + Ord,
    F: Fn(&TrackedMatchSample) -> Option<K>,
{
    let mut groups: HashMap<K, (u64, u64)> = HashMap::new();
    for sample in samples {
        if let Some(key) = key_for(sample) {
            let entry = groups.entry(key).or_default();
            entry.0 += 1;
            entry.1 += u64::from(sample.win);
        }
    }
    let mut result: Vec<_> = groups
        .into_iter()
        .map(|(key, (games, wins))| UsageStats {
            key,
            games,
            wins,
            usage_rate: percentage(games, denominator),
            win_rate: percentage(wins, games),
        })
        .collect();
    result.sort_by(|left, right| {
        right
            .games
            .cmp(&left.games)
            .then_with(|| left.key.cmp(&right.key))
    });
    result
}

fn queue_matches(filter: QueueFilter, queue_id: i64) -> bool {
    match filter {
        QueueFilter::All => matches!(queue_id, 400 | 420 | 430 | 450),
        QueueFilter::RankedSolo => queue_id == 420,
        QueueFilter::Normal => matches!(queue_id, 400 | 430),
        QueueFilter::Aram => queue_id == 450,
    }
}

fn percentage(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::analytics::{AnalyticsFilter, QueueFilter, TimeRangeFilter};

    use super::*;

    fn sample(
        id: &str,
        champion: i64,
        queue: i64,
        patch: &str,
        created: i64,
        win: bool,
    ) -> TrackedMatchSample {
        TrackedMatchSample {
            match_id: id.to_owned(),
            champion_id: champion,
            queue_id: queue,
            patch: patch.to_owned(),
            season_key: Some("2026".to_owned()),
            game_creation: created,
            duration_seconds: 1_800,
            win,
            kills: 10,
            deaths: 2,
            assists: 8,
            double_kills: 1,
            triple_kills: 0,
            quadra_kills: 0,
            penta_kills: 0,
            minions: 180,
            keystone_id: Some(8010),
            rune_page: Some(RunePageKey {
                primary_style_id: 8000,
                primary_rune_ids: vec![8010, 9111, 9104, 8014],
                secondary_style_id: 8400,
                secondary_rune_ids: vec![8444, 8453],
                stat_shard_ids: vec![5008, 5008, 5002],
            }),
            summoner_spell_ids: [4, 12],
            core_item_ids: vec![4633, 3116, 6653],
            boot_item_id: Some(3047),
        }
    }

    fn context(queue: QueueFilter, time_range: TimeRangeFilter) -> FilterContext<'static> {
        FilterContext {
            filter: AnalyticsFilter { queue, time_range },
            champion_id: None,
            current_patch: "16.15",
            current_season: "2026",
            recent_games: None,
        }
    }

    #[test]
    fn aggregates_win_rate_playtime_career_kda_and_kda() {
        let samples = [
            sample("1", 82, 420, "16.15", 2, true),
            sample("2", 82, 420, "16.15", 1, false),
        ];
        let refs: Vec<_> = samples.iter().collect();
        let stats = aggregate_overview(&refs);
        assert_eq!((stats.games, stats.wins, stats.losses), (2, 1, 1));
        assert_eq!(stats.playtime_seconds, 3_600);
        assert_eq!((stats.kills, stats.deaths, stats.assists), (20, 4, 16));
        assert_eq!(stats.win_rate, 50.0);
        assert_eq!(stats.kda, 9.0);
    }

    #[test]
    fn filters_champion_queue_patch_and_recent_games() {
        let samples = vec![
            sample("new", 82, 420, "16.15", 3, true),
            sample("aram", 82, 450, "16.15", 2, true),
            sample("old-patch", 82, 420, "16.14", 1, false),
            sample("other", 1, 420, "16.15", 4, false),
        ];
        let mut filter = context(QueueFilter::RankedSolo, TimeRangeFilter::CurrentPatch);
        filter.champion_id = Some(82);
        filter.recent_games = Some(1);
        let result = filtered_matches(&samples, filter);
        assert_eq!(
            result
                .iter()
                .map(|m| m.match_id.as_str())
                .collect::<Vec<_>>(),
            vec!["new"]
        );
    }

    #[test]
    fn aggregates_runes_spell_pairs_and_core_builds() {
        let mut second = sample("2", 82, 420, "16.15", 1, false);
        second.summoner_spell_ids = [12, 4];
        let samples = [sample("1", 82, 420, "16.15", 2, true), second];
        let refs: Vec<_> = samples.iter().collect();
        let runes = keystone_usage(&refs);
        let spells = spell_pair_usage(&refs);
        let builds = core_build_usage(&refs);
        assert_eq!((runes[0].key, runes[0].games), (8010, 2));
        assert_eq!((spells[0].key, spells[0].games), ([4, 12], 2));
        assert_eq!(builds[0].key, vec![4633, 3116, 6653]);
        assert_eq!(builds[0].usage_rate, 100.0);
        assert_eq!(boots_usage(&refs)[0].key, 3047);
        assert_eq!(rune_page_usage(&refs)[0].games, 2);
    }

    #[test]
    fn full_rune_pages_distinguish_secondary_runes_and_include_shards() {
        let first = sample("1", 82, 420, "16.15", 2, true);
        let mut second = sample("2", 82, 420, "16.15", 1, false);
        if let Some(page) = second.rune_page.as_mut() {
            page.secondary_rune_ids[1] = 8451;
        }
        let samples = [first, second];
        let refs: Vec<_> = samples.iter().collect();
        let pages = rune_page_usage(&refs);
        assert_eq!(pages.len(), 2);
        assert!(pages.iter().all(|page| page.games == 1));
        assert!(
            pages
                .iter()
                .all(|page| page.key.stat_shard_ids == vec![5008, 5008, 5002])
        );
        assert_eq!(keystone_usage(&refs).len(), 1);
    }

    #[test]
    fn filters_current_season() {
        let mut prior = sample("prior", 82, 420, "15.24", 1, false);
        prior.season_key = Some("2025".to_owned());
        let samples = vec![sample("current", 82, 420, "16.15", 2, true), prior];
        let result = filtered_matches(
            &samples,
            context(QueueFilter::All, TimeRangeFilter::CurrentSeason),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].match_id, "current");
    }
}
