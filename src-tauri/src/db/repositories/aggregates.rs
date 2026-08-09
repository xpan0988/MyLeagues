use rusqlite::{
    Connection, OptionalExtension, Transaction, TransactionBehavior, params, params_from_iter,
};

use crate::domain::aggregates::{AggregateCounters, AggregateScope};
use crate::domain::match_record::{MatchRecord, PlayerMatch};
use crate::error::AppResult;

const COUNTER_COLUMNS: &str = "games, wins, losses, kills, deaths, assists,
    playtime_seconds, double_kills, triple_kills, quadra_kills, penta_kills,
    total_minions_killed, neutral_minions_killed, gold_earned";

pub struct AggregateRepository;

impl AggregateRepository {
    pub fn increment(
        transaction: &Transaction<'_>,
        record: &MatchRecord,
        player: &PlayerMatch,
    ) -> AppResult<()> {
        let counters = AggregateCounters::from_match(record, player);
        for scope in AggregateScope::for_match(record) {
            increment_career(transaction, &player.puuid, &scope, &counters)?;
            increment_champion(
                transaction,
                &player.puuid,
                player.champion_id,
                &scope,
                &counters,
            )?;
        }
        Ok(())
    }

    pub fn rebuild(connection: &mut Connection) -> AppResult<()> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute("DELETE FROM career_aggregates", [])?;
        transaction.execute("DELETE FROM champion_aggregates", [])?;

        let facts = {
            let mut statement = transaction.prepare(
                "SELECT m.match_id, m.game_creation, m.game_end_timestamp, m.game_duration,
                        m.queue_id, m.game_version, m.patch, m.season_key, pm.puuid,
                        pm.champion_id, pm.win, pm.kills, pm.deaths, pm.assists,
                        pm.double_kills, pm.triple_kills, pm.quadra_kills, pm.penta_kills,
                        pm.total_minions_killed, pm.neutral_minions_killed, pm.gold_earned,
                        pm.summoner1_id, pm.summoner2_id, pm.keystone_id,
                        pm.primary_style_id, pm.secondary_style_id
                 FROM matches m JOIN player_matches pm ON pm.match_id = m.match_id
                 ORDER BY m.match_id, pm.puuid",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    MatchRecord {
                        match_id: row.get(0)?,
                        game_creation: row.get(1)?,
                        game_end_timestamp: row.get(2)?,
                        game_duration_seconds: row.get(3)?,
                        queue_id: row.get(4)?,
                        game_version: row.get(5)?,
                        patch: row.get(6)?,
                        season_key: row.get(7)?,
                    },
                    PlayerMatch {
                        match_id: row.get(0)?,
                        puuid: row.get(8)?,
                        champion_id: row.get(9)?,
                        win: row.get(10)?,
                        kills: row.get(11)?,
                        deaths: row.get(12)?,
                        assists: row.get(13)?,
                        double_kills: row.get(14)?,
                        triple_kills: row.get(15)?,
                        quadra_kills: row.get(16)?,
                        penta_kills: row.get(17)?,
                        total_minions_killed: row.get(18)?,
                        neutral_minions_killed: row.get(19)?,
                        gold_earned: row.get(20)?,
                        summoner_spell_ids: [row.get(21)?, row.get(22)?],
                        keystone_id: row.get(23)?,
                        primary_style_id: row.get(24)?,
                        secondary_style_id: row.get(25)?,
                        final_items: Vec::new(),
                        rune_selections: Vec::new(),
                    },
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        for (record, player) in facts {
            Self::increment(&transaction, &record, &player)?;
        }
        transaction.commit()?;
        tracing::info!(target: "aggregate_rebuild", "rebuilt aggregate cache from normalized facts");
        Ok(())
    }

    pub fn career(
        connection: &Connection,
        puuid: &str,
        scope: &AggregateScope,
    ) -> AppResult<AggregateCounters> {
        let sql = format!(
            "SELECT {COUNTER_COLUMNS}, 0 FROM career_aggregates
             WHERE puuid = ?1 AND queue_scope = ?2 AND patch = ?3 AND season = ?4"
        );
        Ok(connection
            .query_row(
                &sql,
                params![puuid, scope.queue_scope, scope.patch, scope.season],
                counters_from_row,
            )
            .optional()?
            .unwrap_or_default())
    }

    pub fn champion(
        connection: &Connection,
        puuid: &str,
        champion_id: i64,
        scope: &AggregateScope,
    ) -> AppResult<AggregateCounters> {
        let sql = format!(
            "SELECT {COUNTER_COLUMNS}, highest_kills FROM champion_aggregates
             WHERE puuid = ?1 AND champion_id = ?2 AND queue_scope = ?3
               AND patch = ?4 AND season = ?5"
        );
        Ok(connection
            .query_row(
                &sql,
                params![
                    puuid,
                    champion_id,
                    scope.queue_scope,
                    scope.patch,
                    scope.season
                ],
                counters_from_row,
            )
            .optional()?
            .unwrap_or_default())
    }

    pub fn champions(
        connection: &Connection,
        puuid: &str,
        scope: &AggregateScope,
    ) -> AppResult<Vec<(i64, AggregateCounters)>> {
        let sql = format!(
            "SELECT champion_id, {COUNTER_COLUMNS}, highest_kills
             FROM champion_aggregates
             WHERE puuid = ?1 AND queue_scope = ?2 AND patch = ?3 AND season = ?4
             ORDER BY games DESC, champion_id"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(
            params![puuid, scope.queue_scope, scope.patch, scope.season],
            |row| Ok((row.get(0)?, counters_from_row_offset(row, 1)?)),
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

fn increment_career(
    transaction: &Transaction<'_>,
    puuid: &str,
    scope: &AggregateScope,
    value: &AggregateCounters,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO career_aggregates (
            puuid, queue_scope, patch, season, games, wins, losses, kills, deaths,
            assists, playtime_seconds, double_kills, triple_kills, quadra_kills,
            penta_kills, total_minions_killed, neutral_minions_killed, gold_earned
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                   ?14, ?15, ?16, ?17, ?18)
         ON CONFLICT(puuid, queue_scope, patch, season) DO UPDATE SET
            games = games + excluded.games, wins = wins + excluded.wins,
            losses = losses + excluded.losses, kills = kills + excluded.kills,
            deaths = deaths + excluded.deaths, assists = assists + excluded.assists,
            playtime_seconds = playtime_seconds + excluded.playtime_seconds,
            double_kills = double_kills + excluded.double_kills,
            triple_kills = triple_kills + excluded.triple_kills,
            quadra_kills = quadra_kills + excluded.quadra_kills,
            penta_kills = penta_kills + excluded.penta_kills,
            total_minions_killed = total_minions_killed + excluded.total_minions_killed,
            neutral_minions_killed = neutral_minions_killed + excluded.neutral_minions_killed,
            gold_earned = gold_earned + excluded.gold_earned, updated_at = CURRENT_TIMESTAMP",
        params_from_iter(counter_params(puuid, None, scope, value)),
    )?;
    Ok(())
}

fn increment_champion(
    transaction: &Transaction<'_>,
    puuid: &str,
    champion_id: i64,
    scope: &AggregateScope,
    value: &AggregateCounters,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO champion_aggregates (
            puuid, champion_id, queue_scope, patch, season, games, wins, losses,
            kills, deaths, assists, playtime_seconds, double_kills, triple_kills,
            quadra_kills, penta_kills, total_minions_killed, neutral_minions_killed,
            gold_earned, highest_kills
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                   ?14, ?15, ?16, ?17, ?18, ?19, ?20)
         ON CONFLICT(puuid, champion_id, queue_scope, patch, season) DO UPDATE SET
            games = games + excluded.games, wins = wins + excluded.wins,
            losses = losses + excluded.losses, kills = kills + excluded.kills,
            deaths = deaths + excluded.deaths, assists = assists + excluded.assists,
            playtime_seconds = playtime_seconds + excluded.playtime_seconds,
            double_kills = double_kills + excluded.double_kills,
            triple_kills = triple_kills + excluded.triple_kills,
            quadra_kills = quadra_kills + excluded.quadra_kills,
            penta_kills = penta_kills + excluded.penta_kills,
            total_minions_killed = total_minions_killed + excluded.total_minions_killed,
            neutral_minions_killed = neutral_minions_killed + excluded.neutral_minions_killed,
            gold_earned = gold_earned + excluded.gold_earned,
            highest_kills = MAX(highest_kills, excluded.highest_kills),
            updated_at = CURRENT_TIMESTAMP",
        params_from_iter(counter_params(puuid, Some(champion_id), scope, value)),
    )?;
    Ok(())
}

fn counter_params(
    puuid: &str,
    champion_id: Option<i64>,
    scope: &AggregateScope,
    value: &AggregateCounters,
) -> Vec<rusqlite::types::Value> {
    let mut values = vec![puuid.to_owned().into()];
    if let Some(champion_id) = champion_id {
        values.push(champion_id.into());
    }
    values.extend([
        scope.queue_scope.into(),
        scope.patch.clone().into(),
        scope.season.clone().into(),
        value.games.into(),
        value.wins.into(),
        value.losses.into(),
        value.kills.into(),
        value.deaths.into(),
        value.assists.into(),
        value.playtime_seconds.into(),
        value.double_kills.into(),
        value.triple_kills.into(),
        value.quadra_kills.into(),
        value.penta_kills.into(),
        value.total_minions_killed.into(),
        value.neutral_minions_killed.into(),
        value.gold_earned.into(),
    ]);
    if champion_id.is_some() {
        values.push(value.highest_kills.into());
    }
    values
}

fn counters_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AggregateCounters> {
    counters_from_row_offset(row, 0)
}

fn counters_from_row_offset(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<AggregateCounters> {
    Ok(AggregateCounters {
        games: row.get(offset)?,
        wins: row.get(offset + 1)?,
        losses: row.get(offset + 2)?,
        kills: row.get(offset + 3)?,
        deaths: row.get(offset + 4)?,
        assists: row.get(offset + 5)?,
        playtime_seconds: row.get(offset + 6)?,
        double_kills: row.get(offset + 7)?,
        triple_kills: row.get(offset + 8)?,
        quadra_kills: row.get(offset + 9)?,
        penta_kills: row.get(offset + 10)?,
        total_minions_killed: row.get(offset + 11)?,
        neutral_minions_killed: row.get(offset + 12)?,
        gold_earned: row.get(offset + 13)?,
        highest_kills: row.get(offset + 14)?,
    })
}
