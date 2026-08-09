use rusqlite::{Connection, TransactionBehavior, params};

use crate::error::AppResult;
use crate::riot::types::{LeagueEntryResponse, MasteryResponse};

pub struct ProfileRepository;

impl ProfileRepository {
    pub fn replace_mastery(
        connection: &mut Connection,
        puuid: &str,
        masteries: &[MasteryResponse],
    ) -> AppResult<()> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for mastery in masteries {
            transaction.execute(
                "INSERT INTO champion_mastery (
                    puuid, champion_id, mastery_level, mastery_points, last_play_time
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(puuid, champion_id) DO UPDATE SET
                    mastery_level = excluded.mastery_level,
                    mastery_points = excluded.mastery_points,
                    last_play_time = excluded.last_play_time,
                    updated_at = CURRENT_TIMESTAMP",
                params![
                    puuid,
                    mastery.champion_id,
                    mastery.champion_level,
                    mastery.champion_points,
                    mastery.last_play_time,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn add_rank_snapshots(
        connection: &mut Connection,
        puuid: &str,
        entries: &[LeagueEntryResponse],
    ) -> AppResult<()> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for entry in entries {
            transaction.execute(
                "INSERT INTO rank_snapshots (
                    puuid, queue_type, tier, rank_division, league_points, wins, losses
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    puuid,
                    entry.queue_type,
                    entry.tier,
                    entry.rank,
                    entry.league_points,
                    entry.wins,
                    entry.losses,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}
