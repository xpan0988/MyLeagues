use crate::db::Database;
use crate::db::repositories::aggregates::AggregateRepository;
use crate::error::AppResult;

pub struct MaintenanceService<'state> {
    database: &'state Database,
}

impl<'state> MaintenanceService<'state> {
    pub fn new(database: &'state Database) -> Self {
        Self { database }
    }

    pub fn rebuild_aggregates(&self) -> AppResult<()> {
        let mut connection = self.database.connection()?;
        AggregateRepository::rebuild(&mut connection)
    }

    pub fn clear_static_cache(&self) -> AppResult<()> {
        let connection = self.database.connection()?;
        connection.execute("DELETE FROM static_data_versions", [])?;
        tracing::info!(target: "data_dragon", "cleared static data cache");
        Ok(())
    }

    /// Removes only reconstructable Riot-derived archive material. `accounts`
    /// and `app_settings` intentionally remain: they are configuration/identity,
    /// not downloaded match history.
    pub fn reset_local_archive(&self) -> AppResult<()> {
        let mut connection = self.database.connection()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        transaction.execute("DELETE FROM timeline_sync_queue", [])?;
        transaction.execute("DELETE FROM sync_match_queue", [])?;
        transaction.execute("DELETE FROM match_laning_snapshots", [])?;
        transaction.execute("DELETE FROM player_match_items", [])?;
        transaction.execute("DELETE FROM player_match_runes", [])?;
        transaction.execute("DELETE FROM player_matches", [])?;
        transaction.execute("DELETE FROM matches", [])?;
        transaction.execute("DELETE FROM champion_mastery", [])?;
        transaction.execute("DELETE FROM rank_snapshots", [])?;
        transaction.execute("DELETE FROM career_aggregates", [])?;
        transaction.execute("DELETE FROM champion_aggregates", [])?;
        transaction.execute("DELETE FROM sync_state", [])?;
        transaction.commit()?;
        tracing::info!(target: "maintenance", "reset reconstructable local Riot archive while preserving account and application settings");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MaintenanceService;
    use crate::db::Database;
    use crate::db::repositories::account::AccountRepository;
    use crate::db::repositories::matches::MatchRepository;
    use crate::db::repositories::settings::SettingsRepository;
    use crate::db::repositories::sync::SyncRepository;
    use crate::db::repositories::timeline::{LaningSnapshot, TimelineRepository};
    use crate::domain::account::Account;
    use crate::domain::match_record::{MatchRecord, PlayerMatch};
    use crate::domain::settings::SettingsUpdate;

    #[test]
    fn reset_removes_reconstructable_archive_but_preserves_configuration()
    -> Result<(), Box<dyn std::error::Error>> {
        let database = Database::open_in_memory()?;
        let mut connection = database.connection()?;
        SettingsRepository::new(&connection).update(&SettingsUpdate {
            game_name: "Player".into(),
            tag_line: "OCE".into(),
            account_region: "americas".into(),
            platform_region: "oc1".into(),
            riot_client_path: Some(
                "/Applications/Riot Client.app/Contents/MacOS/RiotClientServices".into(),
            ),
        })?;
        AccountRepository::new(&connection).upsert(&Account {
            puuid: "p".into(),
            game_name: "Player".into(),
            tag_line: "OCE".into(),
            summoner_id: None,
            account_region: "americas".into(),
            platform_region: "oc1".into(),
        })?;
        let record = MatchRecord {
            match_id: "OC1_1".into(),
            game_creation: 1,
            game_end_timestamp: None,
            game_duration_seconds: 700,
            queue_id: 420,
            game_version: "16.1.1".into(),
            patch: "16.1".into(),
            season_key: Some("2026".into()),
        };
        let player = PlayerMatch {
            match_id: "OC1_1".into(),
            puuid: "p".into(),
            participant_id: Some(1),
            champion_id: 1,
            win: true,
            kills: 1,
            deaths: 1,
            assists: 1,
            double_kills: 0,
            triple_kills: 0,
            quadra_kills: 0,
            penta_kills: 0,
            total_minions_killed: 70,
            neutral_minions_killed: 2,
            gold_earned: 4000,
            summoner_spell_ids: [4, 12],
            keystone_id: None,
            primary_style_id: None,
            secondary_style_id: None,
            final_items: vec![],
            rune_selections: vec![],
        };
        MatchRepository::ingest(&mut connection, &record, &player)?;
        SyncRepository::ensure(&connection, "p")?;
        SyncRepository::enqueue(&mut connection, "p", &["OC1_1".into()])?;
        TimelineRepository::enqueue_eligible(&connection, "p")?;
        TimelineRepository::insert_snapshot(
            &mut connection,
            &LaningSnapshot {
                match_id: "OC1_1".into(),
                puuid: "p".into(),
                frame_timestamp_ms: 600_000,
                lane_minions: 70,
                neutral_minions: 2,
                total_gold: 4000,
                experience: 5000,
                level: 6,
            },
        )?;
        drop(connection);

        MaintenanceService::new(&database).reset_local_archive()?;
        let connection = database.connection()?;
        for table in [
            "matches",
            "player_matches",
            "match_laning_snapshots",
            "sync_match_queue",
            "timeline_sync_queue",
            "sync_state",
            "career_aggregates",
            "champion_aggregates",
            "rank_snapshots",
            "champion_mastery",
        ] {
            let count: i64 =
                connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })?;
            assert_eq!(count, 0, "{table} should be reset");
        }
        assert_eq!(
            AccountRepository::new(&connection).get()?.unwrap().puuid,
            "p"
        );
        let settings = SettingsRepository::new(&connection).get()?;
        assert_eq!(
            (
                settings.game_name.as_str(),
                settings.tag_line.as_str(),
                settings.platform_region.as_str(),
                settings.riot_client_path.as_deref()
            ),
            (
                "Player",
                "OCE",
                "oc1",
                Some("/Applications/Riot Client.app/Contents/MacOS/RiotClientServices")
            )
        );
        Ok(())
    }
}
