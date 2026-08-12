use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::domain::aggregates::AggregateScope;
use crate::domain::lane_score::{
    self, CombatCluster, LaneCutoff, LanePair, LaneState, ParticipantFact, ScoreResult,
    TimelineEvent,
};
use crate::error::AppResult;
use crate::riot::parser::ParsedParticipant;

pub const LANE_FACT_REVISION: i64 = 1;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LanePerformanceAggregate {
    pub tracked_matches: u64,
    pub scored_matches: u64,
    pub excluded_matches: u64,
    pub average_lane_score: Option<f64>,
    pub history_start_utc: Option<String>,
    pub history_end_utc: Option<String>,
    pub compatible_ruleset_versions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaneCheckpointRecord {
    pub label: String,
    pub timestamp_ms: i64,
    pub level_difference: i64,
    pub xp_difference: i64,
    pub lane_cs_difference: i64,
    pub gold_difference: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaneCombatRecord {
    pub classification: String,
    pub start_timestamp_ms: i64,
    pub end_timestamp_ms: i64,
    pub signed_strength: f64,
    pub attributions: Vec<lane_score::CombatEventAttribution>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaneEventRecord {
    pub event_type: String,
    pub timestamp_ms: i64,
    pub team_id: Option<i64>,
    pub killer_participant_id: Option<i64>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaneMatchRecord {
    pub opponent_participant_id: Option<i64>,
    pub opponent_champion_id: Option<i64>,
    pub confidence: String,
    pub score: Option<f64>,
    pub exclusion_reason: Option<String>,
    pub cutoff_timestamp_ms: Option<i64>,
    pub cutoff_reason: Option<String>,
    pub exp: Option<f64>,
    pub combat: Option<f64>,
    pub farm: Option<f64>,
    pub pressure: Option<f64>,
    pub conversion: Option<f64>,
    pub coverage_json: Option<String>,
    pub gold_consistency: Option<String>,
    pub model_version: String,
    pub derivation_version: String,
    pub ruleset_version: String,
    pub checkpoints: Vec<LaneCheckpointRecord>,
    pub combat_clusters: Vec<LaneCombatRecord>,
    pub pressure_events: Vec<LaneEventRecord>,
    pub objective_events: Vec<LaneEventRecord>,
}

pub struct LaneAnalysisRepository;

impl LaneAnalysisRepository {
    pub fn record_static_exclusions(connection: &Connection, puuid: &str) -> AppResult<usize> {
        let fallback_seconds = lane_score::ExperimentalManifest::initial()
            .parameters
            .lane_fallback_ms
            / 1_000;
        let patch_14 = format!("{}.%", lane_score::COMPATIBLE_RULESETS[0].raw_patch_major);
        let patch_15 = format!("{}.%", lane_score::COMPATIBLE_RULESETS[1].raw_patch_major);
        let patch_16 = format!("{}.%", lane_score::COMPATIBLE_RULESETS[3].raw_patch_major);
        Ok(connection.execute(
            "INSERT OR REPLACE INTO lane_score_eligibility(
                match_id,perspective_participant_id,derivation_version,score_ready,
                exclusion_reason,cutoff_timestamp_ms,cutoff_reason,evaluated_at)
             SELECT m.match_id,pm.participant_id,?2,0,
                    CASE
                      WHEN m.queue_id NOT IN (400,420,430,480,490) THEN 'UNSUPPORTED_QUEUE'
                      WHEN m.patch NOT LIKE ?4 AND m.patch NOT LIKE ?5 AND m.patch NOT LIKE ?6
                        THEN 'RULESET_UNSUPPORTED'
                      ELSE 'GAME_TOO_SHORT'
                    END,
                    NULL,NULL,CURRENT_TIMESTAMP
             FROM matches m JOIN player_matches pm ON pm.match_id=m.match_id
             WHERE pm.puuid=?1 AND pm.participant_id IS NOT NULL
               AND (m.queue_id NOT IN (400,420,430,480,490)
                    OR (m.patch NOT LIKE ?4 AND m.patch NOT LIKE ?5 AND m.patch NOT LIKE ?6)
                    OR m.game_duration<?3)",
            params![
                puuid,
                lane_score::DERIVATION_VERSION,
                fallback_seconds,
                patch_14,
                patch_15,
                patch_16,
            ],
        )?)
    }

    pub fn enqueue_eligible(connection: &Connection, puuid: &str) -> AppResult<usize> {
        let fallback_seconds = lane_score::ExperimentalManifest::initial()
            .parameters
            .lane_fallback_ms
            / 1_000;
        let patch_14 = format!("{}.%", lane_score::COMPATIBLE_RULESETS[0].raw_patch_major);
        let patch_15 = format!("{}.%", lane_score::COMPATIBLE_RULESETS[1].raw_patch_major);
        let patch_16 = format!("{}.%", lane_score::COMPATIBLE_RULESETS[3].raw_patch_major);
        Ok(connection.execute(
            "INSERT OR IGNORE INTO lane_analysis_queue (match_id, puuid, fact_revision)
             SELECT m.match_id, pm.puuid, ?2 FROM matches m JOIN player_matches pm ON pm.match_id = m.match_id
             WHERE pm.puuid = ?1 AND m.queue_id IN (400, 420, 430, 480, 490)
               AND m.game_duration >= ?3
               AND (m.patch LIKE ?4 OR m.patch LIKE ?5 OR m.patch LIKE ?6)",
            params![
                puuid,
                LANE_FACT_REVISION,
                fallback_seconds,
                patch_14,
                patch_15,
                patch_16,
            ],
        )?)
    }
    pub fn resume_interrupted(connection: &Connection, puuid: &str) -> AppResult<usize> {
        Ok(connection.execute("UPDATE lane_analysis_queue SET status='pending', updated_at=CURRENT_TIMESTAMP WHERE puuid=?1 AND fact_revision=?2 AND status='fetching'", params![puuid, LANE_FACT_REVISION])?)
    }
    pub fn claim_next(connection: &mut Connection, puuid: &str) -> AppResult<Option<String>> {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let match_id=tx.query_row("SELECT match_id FROM lane_analysis_queue WHERE puuid=?1 AND fact_revision=?2 AND status IN ('pending','error') AND attempts < 3 ORDER BY discovered_at,match_id LIMIT 1",params![puuid,LANE_FACT_REVISION],|r|r.get(0)).optional()?;
        if let Some(id) = &match_id {
            tx.execute("UPDATE lane_analysis_queue SET status='fetching',attempts=attempts+1,updated_at=CURRENT_TIMESTAMP WHERE puuid=?1 AND match_id=?2 AND fact_revision=?3",params![puuid,id,LANE_FACT_REVISION])?;
        }
        tx.commit()?;
        Ok(match_id)
    }

    pub fn enqueue_rederivations(connection: &Connection, puuid: &str) -> AppResult<usize> {
        Ok(connection.execute(
            "INSERT OR IGNORE INTO lane_derivation_queue(match_id,puuid,derivation_version)
             SELECT queue.match_id,queue.puuid,?2 FROM lane_analysis_queue queue
             WHERE queue.puuid=?1 AND queue.fact_revision=?3 AND queue.status='complete'
               AND EXISTS (SELECT 1 FROM match_participants participant WHERE participant.match_id=queue.match_id)
               AND EXISTS (SELECT 1 FROM lane_timeline_states state WHERE state.match_id=queue.match_id)",
            params![puuid, lane_score::DERIVATION_VERSION, LANE_FACT_REVISION],
        )?)
    }

    pub fn resume_interrupted_derivations(
        connection: &Connection,
        puuid: &str,
    ) -> AppResult<usize> {
        Ok(connection.execute(
            "UPDATE lane_derivation_queue SET status='pending',updated_at=CURRENT_TIMESTAMP
             WHERE puuid=?1 AND derivation_version=?2 AND status='deriving'",
            params![puuid, lane_score::DERIVATION_VERSION],
        )?)
    }

    pub fn claim_next_derivation(
        connection: &mut Connection,
        puuid: &str,
    ) -> AppResult<Option<String>> {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let match_id = tx
            .query_row(
                "SELECT match_id FROM lane_derivation_queue
                 WHERE puuid=?1 AND derivation_version=?2 AND status IN ('pending','error')
                   AND attempts < 3 ORDER BY discovered_at,match_id LIMIT 1",
                params![puuid, lane_score::DERIVATION_VERSION],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(match_id) = &match_id {
            tx.execute(
                "UPDATE lane_derivation_queue SET status='deriving',attempts=attempts+1,
                 updated_at=CURRENT_TIMESTAMP WHERE match_id=?1 AND puuid=?2 AND derivation_version=?3",
                params![match_id, puuid, lane_score::DERIVATION_VERSION],
            )?;
        }
        tx.commit()?;
        Ok(match_id)
    }

    pub fn complete_derivation(
        connection: &Connection,
        puuid: &str,
        match_id: &str,
    ) -> AppResult<()> {
        connection.execute(
            "UPDATE lane_derivation_queue SET status='complete',last_error=NULL,updated_at=CURRENT_TIMESTAMP
             WHERE match_id=?1 AND puuid=?2 AND derivation_version=?3",
            params![match_id, puuid, lane_score::DERIVATION_VERSION],
        )?;
        Ok(())
    }

    pub fn fail_derivation(
        connection: &Connection,
        puuid: &str,
        match_id: &str,
        reason: &str,
    ) -> AppResult<()> {
        connection.execute(
            "UPDATE lane_derivation_queue SET status='error',last_error=?3,updated_at=CURRENT_TIMESTAMP
             WHERE match_id=?1 AND puuid=?2 AND derivation_version=?4",
            params![match_id, puuid, reason, lane_score::DERIVATION_VERSION],
        )?;
        Ok(())
    }
    pub fn mark_error(
        connection: &Connection,
        puuid: &str,
        match_id: &str,
        reason: &str,
    ) -> AppResult<()> {
        connection.execute("UPDATE lane_analysis_queue SET status='error',last_error=?3,updated_at=CURRENT_TIMESTAMP WHERE puuid=?1 AND match_id=?2 AND fact_revision=?4",params![puuid,match_id,reason,LANE_FACT_REVISION])?;
        Ok(())
    }
    pub fn mark_unsupported(
        connection: &Connection,
        puuid: &str,
        match_id: &str,
        reason: &str,
    ) -> AppResult<()> {
        connection.execute("UPDATE lane_analysis_queue SET status='unsupported',last_error=?3,updated_at=CURRENT_TIMESTAMP WHERE puuid=?1 AND match_id=?2 AND fact_revision=?4",params![puuid,match_id,reason,LANE_FACT_REVISION])?;
        Ok(())
    }

    pub fn store_facts(
        connection: &mut Connection,
        puuid: &str,
        match_id: &str,
        participants: &[ParsedParticipant],
        states: &[LaneState],
        events: &[TimelineEvent],
    ) -> AppResult<()> {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for p in participants {
            let fact = &p.fact;
            tx.execute("INSERT INTO match_participants(match_id,participant_id,puuid,team_id,champion_id,team_position,individual_position) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(match_id,participant_id) DO UPDATE SET puuid=excluded.puuid,team_id=excluded.team_id,champion_id=excluded.champion_id,team_position=excluded.team_position,individual_position=excluded.individual_position",params![match_id,fact.participant_id,p.puuid,fact.team_id,fact.champion_id,fact.team_position,fact.individual_position])?;
        }
        for s in states {
            tx.execute("INSERT OR REPLACE INTO lane_timeline_states(match_id,participant_id,frame_timestamp_ms,lane_minions,jungle_minions,total_gold,experience,level) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![match_id,s.participant_id,s.timestamp_ms,s.lane_cs,s.jungle_cs,s.gold,s.xp,s.level])?;
        }
        for e in events {
            tx.execute("INSERT OR REPLACE INTO lane_timeline_events(match_id,source_event_id,timestamp_ms,event_type,killer_participant_id,victim_participant_id,team_id,monster_type,monster_sub_type,building_type,tower_type,lane_type,x,y) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",params![match_id,e.source_id,e.timestamp_ms,e.kind,e.killer,e.victim,e.team_id,e.monster_type,e.monster_sub_type,e.building_type,e.tower_type,e.lane_type,e.position.map(|p|p.0),e.position.map(|p|p.1)])?;
            tx.execute("DELETE FROM lane_timeline_event_participants WHERE match_id=?1 AND source_event_id=?2",params![match_id,e.source_id])?;
            for (id, relation) in e
                .killer
                .into_iter()
                .map(|id| (id, "killer"))
                .chain(e.victim.map(|id| (id, "victim")))
                .chain(e.assistants.iter().map(|id| (*id, "assistant")))
            {
                tx.execute("INSERT OR IGNORE INTO lane_timeline_event_participants(match_id,source_event_id,participant_id,relation) VALUES(?1,?2,?3,?4)",params![match_id,e.source_id,id,relation])?;
            }
        }
        tx.execute("UPDATE lane_analysis_queue SET status='complete',last_error=NULL,updated_at=CURRENT_TIMESTAMP WHERE puuid=?1 AND match_id=?2 AND fact_revision=?3",params![puuid,match_id,LANE_FACT_REVISION])?;
        tx.execute(
            "INSERT OR IGNORE INTO lane_derivation_queue(match_id,puuid,derivation_version) VALUES(?1,?2,?3)",
            params![match_id, puuid, lane_score::DERIVATION_VERSION],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn rebuild_score(
        connection: &mut Connection,
        match_id: &str,
        puuid: &str,
        kill_coverage_complete: bool,
    ) -> AppResult<Option<ScoreResult>> {
        let (queue_id, patch, game_end_ms) = connection.query_row(
            "SELECT queue_id,patch,game_duration * 1000 FROM matches WHERE match_id=?1",
            [match_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?;
        let participants = load_participants(connection, match_id)?;
        let a = connection
            .query_row(
                "SELECT participant_id FROM match_participants WHERE match_id=?1 AND puuid=?2",
                params![match_id, puuid],
                |r| r.get(0),
            )
            .optional()?;
        let Some(a) = a else { return Ok(None) };
        let pair = lane_score::derive_opponent(&participants, a);
        let states = load_states(connection, match_id)?;
        let events = load_events(connection, match_id)?;
        let (result, clusters, lane_end) = lane_score::score(
            &pair,
            &participants,
            &states,
            &events,
            queue_id,
            &patch,
            game_end_ms,
            kill_coverage_complete,
        );
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        persist_derivations(
            &tx, match_id, &pair, lane_end, &states, &events, &clusters, queue_id,
        )?;
        persist_eligibility(&tx, match_id, &pair, &result, lane_end)?;
        if pair.b != 0 {
            persist_score(&tx, match_id, &pair, &result)?;
        }
        tx.commit()?;
        Ok(Some(result))
    }

    pub fn performance_summary(
        connection: &Connection,
        puuid: &str,
        champion_id: Option<i64>,
        _scope: &AggregateScope,
    ) -> AppResult<LanePerformanceAggregate> {
        let manifest = lane_score::ExperimentalManifest::initial();
        let rulesets = &lane_score::COMPATIBLE_RULESETS;
        connection
            .query_row(
                "WITH compatible_rulesets(raw_major,minor_from,minor_to,ruleset_version) AS (
                    VALUES (?7,?8,?9,?10), (?11,?12,?13,?14),
                           (?15,?16,?17,?18), (?19,?20,?21,?22)
                 ), population AS (
                    SELECT m.match_id,m.game_creation,player.champion_id,
                           local.participant_id,ruleset.ruleset_version
                    FROM matches m
                    JOIN player_matches player ON player.match_id=m.match_id AND player.puuid=?1
                    JOIN compatible_rulesets ruleset
                      ON CAST(substr(m.patch,1,instr(m.patch,'.')-1) AS INTEGER)=ruleset.raw_major
                     AND CAST(substr(m.patch,instr(m.patch,'.')+1) AS INTEGER)
                         BETWEEN ruleset.minor_from AND ruleset.minor_to
                    JOIN match_participants local
                      ON local.match_id=m.match_id AND local.puuid=player.puuid
                    WHERE m.queue_id IN (400,420,430,480,490)
                      AND UPPER(local.team_position)='TOP'
                      AND UPPER(local.individual_position)='TOP'
                      AND (?6 IS NULL OR player.champion_id=?6)
                 )
                 SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN eligibility.score_ready=1 AND score.status='ready' AND score.score IS NOT NULL THEN 1 ELSE 0 END),0),
                    COUNT(*) - COALESCE(SUM(CASE WHEN eligibility.score_ready=1 AND score.status='ready' AND score.score IS NOT NULL THEN 1 ELSE 0 END),0),
                    AVG(CASE WHEN eligibility.score_ready=1 AND score.status='ready' THEN score.score END),
                    MIN(datetime(population.game_creation/1000,'unixepoch')),
                    MAX(datetime(population.game_creation/1000,'unixepoch'))
                 FROM population
                 LEFT JOIN lane_score_eligibility eligibility
                   ON eligibility.match_id=population.match_id
                  AND eligibility.perspective_participant_id=population.participant_id
                  AND eligibility.derivation_version=?4
                 LEFT JOIN lane_score_cache score
                   ON score.match_id=population.match_id
                  AND score.perspective_participant_id=population.participant_id
                  AND score.model_version=?2
                  AND score.feature_schema_version=?3
                  AND score.derivation_version=?4
                  AND score.ruleset_version=population.ruleset_version
                  AND score.parameter_hash=?5",
                params![
                    puuid,
                    manifest.model_version,
                    manifest.feature_schema_version,
                    manifest.derivation_version,
                    manifest.parameter_hash,
                    champion_id,
                    rulesets[0].raw_patch_major,
                    rulesets[0].raw_patch_minor_from,
                    rulesets[0].raw_patch_minor_to,
                    rulesets[0].ruleset_version,
                    rulesets[1].raw_patch_major,
                    rulesets[1].raw_patch_minor_from,
                    rulesets[1].raw_patch_minor_to,
                    rulesets[1].ruleset_version,
                    rulesets[2].raw_patch_major,
                    rulesets[2].raw_patch_minor_from,
                    rulesets[2].raw_patch_minor_to,
                    rulesets[2].ruleset_version,
                    rulesets[3].raw_patch_major,
                    rulesets[3].raw_patch_minor_from,
                    rulesets[3].raw_patch_minor_to,
                    rulesets[3].ruleset_version,
                ],
                |row| {
                    Ok(LanePerformanceAggregate {
                        tracked_matches: row.get::<_, i64>(0)?.max(0) as u64,
                        scored_matches: row.get::<_, i64>(1)?.max(0) as u64,
                        excluded_matches: row.get::<_, i64>(2)?.max(0) as u64,
                        average_lane_score: row.get(3)?,
                        history_start_utc: row.get(4)?,
                        history_end_utc: row.get(5)?,
                        compatible_ruleset_versions: lane_score::compatible_ruleset_versions()
                            .into_iter()
                            .map(str::to_owned)
                            .collect(),
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn match_lane(
        connection: &Connection,
        puuid: &str,
        match_id: &str,
        include_evidence: bool,
    ) -> AppResult<Option<LaneMatchRecord>> {
        let patch = connection
            .query_row(
                "SELECT patch FROM matches WHERE match_id=?1",
                [match_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let manifest = patch
            .as_deref()
            .and_then(lane_score::ExperimentalManifest::for_patch)
            .unwrap_or_else(lane_score::ExperimentalManifest::initial);
        let base = connection
            .query_row(
                "SELECT opponent.champion_id,COALESCE(mapping.confidence,'UNAVAILABLE'),
                        score.score,eligibility.exclusion_reason,
                        eligibility.cutoff_timestamp_ms,eligibility.cutoff_reason,
                        score.exp,score.combat,score.farm,score.pressure,score.conversion,
                        score.coverage_json,score.gold_consistency,mapping.opponent_participant_id,
                        local.participant_id,COALESCE(score.model_version,?4),
                        COALESCE(score.derivation_version,?3),COALESCE(score.ruleset_version,?6)
                 FROM match_participants local
                 JOIN lane_score_eligibility eligibility
                   ON eligibility.match_id=local.match_id
                  AND eligibility.perspective_participant_id=local.participant_id
                  AND eligibility.derivation_version=?3
                 LEFT JOIN lane_opponent_mappings mapping
                   ON mapping.match_id=local.match_id
                  AND mapping.perspective_participant_id=local.participant_id
                  AND mapping.derivation_version=?3
                 LEFT JOIN match_participants opponent
                   ON opponent.match_id=local.match_id
                  AND opponent.participant_id=mapping.opponent_participant_id
                 LEFT JOIN lane_score_cache score
                   ON score.match_id=local.match_id
                  AND score.perspective_participant_id=local.participant_id
                  AND score.model_version=?4
                  AND score.feature_schema_version=?5
                  AND score.derivation_version=?3
                  AND score.ruleset_version=?6
                  AND score.parameter_hash=?7
                 WHERE local.match_id=?1 AND local.puuid=?2",
                params![
                    match_id,
                    puuid,
                    manifest.derivation_version,
                    manifest.model_version,
                    manifest.feature_schema_version,
                    manifest.ruleset_version,
                    manifest.parameter_hash,
                ],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<f64>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<f64>>(6)?,
                        row.get::<_, Option<f64>>(7)?,
                        row.get::<_, Option<f64>>(8)?,
                        row.get::<_, Option<f64>>(9)?,
                        row.get::<_, Option<f64>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, Option<i64>>(13)?,
                        row.get::<_, i64>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, String>(16)?,
                        row.get::<_, String>(17)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            opponent_champion_id,
            confidence,
            score,
            exclusion_reason,
            cutoff_timestamp_ms,
            cutoff_reason,
            exp,
            combat,
            farm,
            pressure,
            conversion,
            coverage_json,
            gold_consistency,
            opponent_id,
            local_id,
            model_version,
            derivation_version,
            ruleset_version,
        )) = base
        else {
            return Ok(None);
        };

        if !include_evidence {
            return Ok(Some(LaneMatchRecord {
                opponent_participant_id: opponent_id,
                opponent_champion_id,
                confidence,
                score,
                exclusion_reason,
                cutoff_timestamp_ms,
                cutoff_reason,
                exp,
                combat,
                farm,
                pressure,
                conversion,
                coverage_json,
                gold_consistency,
                model_version,
                derivation_version,
                ruleset_version,
                checkpoints: Vec::new(),
                combat_clusters: Vec::new(),
                pressure_events: Vec::new(),
                objective_events: Vec::new(),
            }));
        }

        let mut checkpoints = Vec::new();
        let mut combat_clusters = Vec::new();
        if let Some(opponent_id) = opponent_id {
            let mut statement = connection.prepare(
                "SELECT checkpoint.checkpoint,checkpoint.frame_timestamp_ms,
                        local.level-opponent.level,local.experience-opponent.experience,
                        local.lane_minions-opponent.lane_minions,local.total_gold-opponent.total_gold
                 FROM lane_checkpoints checkpoint
                 JOIN lane_timeline_states local
                   ON local.match_id=checkpoint.match_id
                  AND local.participant_id=checkpoint.perspective_participant_id
                  AND local.frame_timestamp_ms=checkpoint.frame_timestamp_ms
                 JOIN lane_timeline_states opponent
                   ON opponent.match_id=checkpoint.match_id
                  AND opponent.participant_id=?3
                  AND opponent.frame_timestamp_ms=checkpoint.frame_timestamp_ms
                 WHERE checkpoint.match_id=?1 AND checkpoint.perspective_participant_id=?2
                   AND checkpoint.derivation_version=?4
                 ORDER BY checkpoint.frame_timestamp_ms,checkpoint.checkpoint",
            )?;
            checkpoints = statement
                .query_map(
                    params![
                        match_id,
                        local_id,
                        opponent_id,
                        lane_score::DERIVATION_VERSION
                    ],
                    |row| {
                        Ok(LaneCheckpointRecord {
                            label: row.get(0)?,
                            timestamp_ms: row.get(1)?,
                            level_difference: row.get(2)?,
                            xp_difference: row.get(3)?,
                            lane_cs_difference: row.get(4)?,
                            gold_difference: row.get(5)?,
                        })
                    },
                )?
                .collect::<Result<_, _>>()?;

            let mut statement = connection.prepare(
                "SELECT classification,start_timestamp_ms,end_timestamp_ms,signed_strength,attribution_json
                 FROM lane_combat_clusters
                 WHERE match_id=?1 AND perspective_participant_id=?2
                   AND opponent_participant_id=?3 AND derivation_version=?4
                 ORDER BY start_timestamp_ms,cluster_id",
            )?;
            combat_clusters = statement
                .query_map(
                    params![
                        match_id,
                        local_id,
                        opponent_id,
                        lane_score::DERIVATION_VERSION
                    ],
                    |row| {
                        Ok(LaneCombatRecord {
                            classification: row.get(0)?,
                            start_timestamp_ms: row.get(1)?,
                            end_timestamp_ms: row.get(2)?,
                            signed_strength: row.get(3)?,
                            attributions: serde_json::from_str::<
                                Vec<lane_score::CombatEventAttribution>,
                            >(&row.get::<_, String>(4)?)
                            .unwrap_or_default(),
                        })
                    },
                )?
                .collect::<Result<_, _>>()?;
        }

        let load_events = |predicate: &str| -> AppResult<Vec<LaneEventRecord>> {
            let sql = format!(
                "SELECT event_type,timestamp_ms,team_id,killer_participant_id,
                        COALESCE(monster_type,tower_type,building_type,lane_type)
                 FROM lane_timeline_events
                 WHERE match_id=?1 AND (?2 IS NULL OR timestamp_ms<=?2) AND ({predicate})
                 ORDER BY timestamp_ms,source_event_id"
            );
            let mut statement = connection.prepare(&sql)?;
            Ok(statement
                .query_map(params![match_id, cutoff_timestamp_ms], |row| {
                    Ok(LaneEventRecord {
                        event_type: row.get(0)?,
                        timestamp_ms: row.get(1)?,
                        team_id: row.get(2)?,
                        killer_participant_id: row.get(3)?,
                        detail: row.get(4)?,
                    })
                })?
                .collect::<Result<_, _>>()?)
        };
        let pressure_events = load_events(
            "event_type IN ('TURRET_PLATE_DESTROYED','BUILDING_KILL') AND UPPER(COALESCE(lane_type,'')) LIKE '%TOP%'",
        )?;
        let objective_events = load_events(
            "event_type='ELITE_MONSTER_KILL' AND (UPPER(COALESCE(monster_type,'')) LIKE '%RIFTHERALD%' OR UPPER(COALESCE(monster_type,'')) LIKE '%RIFT_HERALD%' OR UPPER(COALESCE(monster_type,''))='HORDE' OR UPPER(COALESCE(monster_type,'')) LIKE '%VOIDGRUB%' OR UPPER(COALESCE(monster_type,'')) LIKE '%VOID_GRUB%')",
        )?;

        Ok(Some(LaneMatchRecord {
            opponent_participant_id: opponent_id,
            opponent_champion_id,
            confidence,
            score,
            exclusion_reason,
            cutoff_timestamp_ms,
            cutoff_reason,
            exp,
            combat,
            farm,
            pressure,
            conversion,
            coverage_json,
            gold_consistency,
            model_version,
            derivation_version,
            ruleset_version,
            checkpoints,
            combat_clusters,
            pressure_events,
            objective_events,
        }))
    }
}

fn load_participants(c: &Connection, match_id: &str) -> AppResult<Vec<ParticipantFact>> {
    let mut s=c.prepare("SELECT participant_id,team_id,champion_id,team_position,individual_position FROM match_participants WHERE match_id=?1 ORDER BY participant_id")?;
    Ok(s.query_map([match_id], |r| {
        Ok(ParticipantFact {
            participant_id: r.get(0)?,
            team_id: r.get(1)?,
            champion_id: r.get(2)?,
            team_position: r.get(3)?,
            individual_position: r.get(4)?,
        })
    })?
    .collect::<Result<_, _>>()?)
}
fn load_states(c: &Connection, match_id: &str) -> AppResult<Vec<LaneState>> {
    let mut s=c.prepare("SELECT participant_id,frame_timestamp_ms,lane_minions,jungle_minions,total_gold,experience,level FROM lane_timeline_states WHERE match_id=?1 ORDER BY frame_timestamp_ms,participant_id")?;
    Ok(s.query_map([match_id], |r| {
        Ok(LaneState {
            participant_id: r.get(0)?,
            timestamp_ms: r.get(1)?,
            lane_cs: r.get(2)?,
            jungle_cs: r.get(3)?,
            gold: r.get(4)?,
            xp: r.get(5)?,
            level: r.get(6)?,
        })
    })?
    .collect::<Result<_, _>>()?)
}
fn load_events(c: &Connection, match_id: &str) -> AppResult<Vec<TimelineEvent>> {
    let mut s=c.prepare("SELECT source_event_id,timestamp_ms,event_type,killer_participant_id,victim_participant_id,team_id,monster_type,monster_sub_type,building_type,tower_type,lane_type,x,y FROM lane_timeline_events WHERE match_id=?1 ORDER BY timestamp_ms,source_event_id")?;
    let mut out = Vec::new();
    for row in s.query_map([match_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get(7)?,
            r.get(8)?,
            r.get(9)?,
            r.get(10)?,
            r.get::<_, Option<i64>>(11)?,
            r.get::<_, Option<i64>>(12)?,
        ))
    })? {
        let (
            id,
            timestamp,
            kind,
            killer,
            victim,
            team_id,
            monster,
            monster_sub_type,
            building,
            tower,
            lane,
            x,
            y,
        ) = row?;
        let mut assists=c.prepare("SELECT participant_id FROM lane_timeline_event_participants WHERE match_id=?1 AND source_event_id=?2 AND relation='assistant' ORDER BY participant_id")?;
        let assistants = assists
            .query_map(params![match_id, id], |r| r.get(0))?
            .collect::<Result<Vec<i64>, _>>()?;
        out.push(TimelineEvent {
            source_id: id,
            timestamp_ms: timestamp,
            kind,
            killer,
            victim,
            team_id,
            assistants,
            monster_type: monster,
            monster_sub_type,
            building_type: building,
            tower_type: tower,
            lane_type: lane,
            position: x.zip(y),
        });
    }
    Ok(out)
}
fn persist_derivations(
    tx: &rusqlite::Transaction<'_>,
    match_id: &str,
    pair: &LanePair,
    cutoff: Option<LaneCutoff>,
    states: &[LaneState],
    events: &[TimelineEvent],
    clusters: &[CombatCluster],
    queue_id: i64,
) -> AppResult<()> {
    let confidence = pair.confidence.as_str();
    tx.execute(
        "DELETE FROM lane_checkpoints WHERE match_id=?1 AND perspective_participant_id=?2 AND derivation_version=?3",
        params![match_id, pair.a, lane_score::DERIVATION_VERSION],
    )?;
    tx.execute(
        "DELETE FROM lane_phase_derivations WHERE match_id=?1 AND perspective_participant_id=?2 AND derivation_version=?3",
        params![match_id, pair.a, lane_score::DERIVATION_VERSION],
    )?;
    tx.execute("DELETE FROM lane_combat_clusters WHERE match_id=?1 AND perspective_participant_id=?2 AND derivation_version=?3",params![match_id,pair.a,lane_score::DERIVATION_VERSION])?;
    tx.execute("INSERT OR REPLACE INTO lane_opponent_mappings(match_id,perspective_participant_id,opponent_participant_id,confidence,derivation_version) VALUES(?1,?2,?3,?4,?5)",params![match_id,pair.a,(pair.b!=0).then_some(pair.b),confidence,lane_score::DERIVATION_VERSION])?;
    let Some(cutoff) = cutoff else {
        return Ok(());
    };
    let lane_end = cutoff.timestamp_ms;
    tx.execute("INSERT OR REPLACE INTO lane_phase_derivations(match_id,perspective_participant_id,derivation_version,end_timestamp_ms,end_reason) VALUES(?1,?2,?3,?4,?5)",params![match_id,pair.a,lane_score::DERIVATION_VERSION,lane_end,cutoff.reason.as_str()])?;
    for (label, anchor) in [
        ("@6", 360000),
        ("@8", 480000),
        ("@10", 600000),
        ("@12", 720000),
        ("@14", 840000),
    ] {
        if let Some(frame) = lane_score::nominal_checkpoint(states, pair.a, anchor, lane_end) {
            tx.execute("INSERT OR REPLACE INTO lane_checkpoints(match_id,perspective_participant_id,derivation_version,checkpoint,frame_timestamp_ms) VALUES(?1,?2,?3,?4,?5)",params![match_id,pair.a,lane_score::DERIVATION_VERSION,label,frame.timestamp_ms])?;
        }
    }
    let lane_end_frame = if cutoff.state_strictly_before {
        lane_score::latest_before(states, pair.a, lane_end)
    } else {
        states
            .iter()
            .filter(|state| state.participant_id == pair.a && state.timestamp_ms <= lane_end)
            .max_by_key(|state| state.timestamp_ms)
    };
    if let Some(frame) = lane_end_frame {
        tx.execute("INSERT OR REPLACE INTO lane_checkpoints(match_id,perspective_participant_id,derivation_version,checkpoint,frame_timestamp_ms) VALUES(?1,?2,?3,'LANE_PHASE_END',?4)",params![match_id,pair.a,lane_score::DERIVATION_VERSION,frame.timestamp_ms])?;
    }
    for event in events.iter().filter(|event| {
        event.timestamp_ms <= lane_end && !(queue_id == 480 && event.kind == "ELITE_MONSTER_KILL")
    }) {
        let checkpoint = if event.kind == "ELITE_MONSTER_KILL" {
            match event
                .monster_type
                .as_deref()
                .unwrap_or("")
                .to_ascii_uppercase()
                .as_str()
            {
                value
                    if value == "HORDE"
                        || value.contains("VOIDGRUB")
                        || value.contains("VOID_GRUB") =>
                {
                    Some("PRE_GRUBS")
                }
                value if value.contains("RIFTHERALD") || value.contains("RIFT_HERALD") => {
                    Some("PRE_HERALD")
                }
                _ => None,
            }
        } else if event.kind == "BUILDING_KILL"
            && event
                .lane_type
                .as_deref()
                .is_some_and(|v| v.to_ascii_uppercase().contains("TOP"))
        {
            Some("PRE_TOP_OUTER_TURRET")
        } else {
            None
        };
        if let Some(label) = checkpoint {
            if let Some(frame) = lane_score::latest_before(states, pair.a, event.timestamp_ms) {
                tx.execute("INSERT OR REPLACE INTO lane_checkpoints(match_id,perspective_participant_id,derivation_version,checkpoint,frame_timestamp_ms) VALUES(?1,?2,?3,?4,?5)",params![match_id,pair.a,lane_score::DERIVATION_VERSION,format!("{label}:{}",event.source_id),frame.timestamp_ms])?;
            }
        }
    }
    for c in clusters {
        tx.execute("INSERT INTO lane_combat_clusters(match_id,perspective_participant_id,opponent_participant_id,derivation_version,cluster_id,start_timestamp_ms,end_timestamp_ms,classification,signed_strength,source_event_ids_json,attribution_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![match_id,pair.a,pair.b,lane_score::DERIVATION_VERSION,c.id,c.start_ms,c.end_ms,c.classification.as_str(),c.signed_strength,serde_json::to_string(&c.source_event_ids)?,serde_json::to_string(&c.attributions)?])?;
    }
    Ok(())
}

fn persist_eligibility(
    tx: &rusqlite::Transaction<'_>,
    match_id: &str,
    pair: &LanePair,
    result: &ScoreResult,
    cutoff: Option<LaneCutoff>,
) -> AppResult<()> {
    tx.execute(
        "INSERT OR REPLACE INTO lane_score_eligibility(
            match_id,perspective_participant_id,derivation_version,score_ready,
            exclusion_reason,cutoff_timestamp_ms,cutoff_reason,evaluated_at
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,CURRENT_TIMESTAMP)",
        params![
            match_id,
            pair.a,
            lane_score::DERIVATION_VERSION,
            i64::from(result.score.is_some() && result.exclusion_reason.is_none()),
            result.exclusion_reason.map(|reason| reason.as_str()),
            cutoff.map(|value| value.timestamp_ms),
            cutoff.map(|value| value.reason.as_str()),
        ],
    )?;
    Ok(())
}
fn persist_score(
    tx: &rusqlite::Transaction<'_>,
    match_id: &str,
    pair: &LanePair,
    result: &ScoreResult,
) -> AppResult<()> {
    tx.execute("INSERT OR IGNORE INTO lane_score_model_manifests(model_version,feature_schema_version,derivation_version,ruleset_version,parameter_hash,valid_patch_from,valid_patch_to,calibration_dataset_id,status) VALUES(?1,?2,?3,?4,?5,?6,?7,NULL,'EXPERIMENTAL_INITIAL_HYPOTHESIS')",params![result.manifest.model_version,result.manifest.feature_schema_version,result.manifest.derivation_version,result.manifest.ruleset_version,result.manifest.parameter_hash,result.manifest.valid_patch_from,result.manifest.valid_patch_to])?;
    tx.execute("INSERT OR REPLACE INTO lane_score_cache(match_id,perspective_participant_id,opponent_participant_id,model_version,feature_schema_version,derivation_version,ruleset_version,parameter_hash,status,score,exp,combat,farm,pressure,conversion,coverage_json,gold_consistency) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",params![match_id,pair.a,pair.b,result.manifest.model_version,result.manifest.feature_schema_version,result.manifest.derivation_version,result.manifest.ruleset_version,result.manifest.parameter_hash,result.status,result.score,result.exp.value,result.combat.value,result.farm.value,result.pressure.value,result.conversion.value,serde_json::json!({"exp":result.exp.coverage,"combat":result.combat.coverage,"farm":result.farm.coverage,"pressure":result.pressure.coverage,"conversion":result.conversion.coverage}).to_string(),result.gold_consistency])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::db::repositories::account::AccountRepository;
    use crate::db::repositories::matches::MatchRepository;
    use crate::domain::account::Account;
    use crate::domain::aggregates::{ALL_QUEUES, AggregateScope};
    use crate::domain::match_record::{MatchRecord, PlayerMatch};
    fn seed(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
        let mut c = db.connection()?;
        AccountRepository::new(&c).upsert(&Account {
            puuid: "p".into(),
            game_name: "n".into(),
            tag_line: "t".into(),
            summoner_id: None,
            account_region: "sea".into(),
            platform_region: "oc1".into(),
        })?;
        MatchRepository::ingest(
            &mut c,
            &MatchRecord {
                match_id: "M".into(),
                game_creation: 0,
                game_end_timestamp: None,
                game_duration_seconds: 900,
                queue_id: 420,
                game_version: "16.15.802.4387".into(),
                patch: "16.15".into(),
                season_key: None,
            },
            &PlayerMatch {
                match_id: "M".into(),
                puuid: "p".into(),
                participant_id: Some(1),
                champion_id: 1,
                win: true,
                kills: 0,
                deaths: 0,
                assists: 0,
                double_kills: 0,
                triple_kills: 0,
                quadra_kills: 0,
                penta_kills: 0,
                total_minions_killed: 0,
                neutral_minions_killed: 0,
                gold_earned: 0,
                summoner_spell_ids: [1, 2],
                keystone_id: None,
                primary_style_id: None,
                secondary_style_id: None,
                final_items: vec![],
                rune_selections: vec![],
            },
        )?;
        Ok(())
    }
    #[test]
    fn lane_revision_queue_is_independent_and_idempotent() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = Database::open_in_memory()?;
        seed(&db)?;
        let mut c = db.connection()?;
        assert_eq!(LaneAnalysisRepository::enqueue_eligible(&c, "p")?, 1);
        assert_eq!(LaneAnalysisRepository::enqueue_eligible(&c, "p")?, 0);
        assert_eq!(
            LaneAnalysisRepository::claim_next(&mut c, "p")?.as_deref(),
            Some("M")
        );
        assert_eq!(LaneAnalysisRepository::resume_interrupted(&c, "p")?, 1);
        Ok(())
    }

    #[test]
    fn swiftplay_enters_the_persistent_fact_queue_for_missing_authoritative_facts()
    -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::open_in_memory()?;
        seed(&db)?;
        let c = db.connection()?;
        c.execute("UPDATE matches SET queue_id=480 WHERE match_id='M'", [])?;
        assert_eq!(LaneAnalysisRepository::enqueue_eligible(&c, "p")?, 1);
        assert_eq!(
            c.query_row(
                "SELECT status FROM lane_analysis_queue WHERE match_id='M' AND puuid='p'",
                [],
                |row| row.get::<_, String>(0),
            )?,
            "pending"
        );
        Ok(())
    }

    #[test]
    fn corrected_ruleset_identity_requeues_completed_old_derivation_locally()
    -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::open_in_memory()?;
        seed(&db)?;
        let c = db.connection()?;
        LaneAnalysisRepository::enqueue_eligible(&c, "p")?;
        c.execute(
            "UPDATE lane_analysis_queue SET status='complete' WHERE match_id='M' AND puuid='p'",
            [],
        )?;
        c.execute(
            "INSERT INTO match_participants(
                match_id,participant_id,puuid,team_id,champion_id,team_position,individual_position)
             VALUES('M',1,'p',100,1,'TOP','TOP')",
            [],
        )?;
        c.execute(
            "INSERT INTO lane_timeline_states(
                match_id,participant_id,frame_timestamp_ms,lane_minions,jungle_minions,total_gold,experience,level)
             VALUES('M',1,600000,80,0,4000,6000,8)",
            [],
        )?;
        c.execute(
            "INSERT INTO lane_derivation_queue(match_id,puuid,derivation_version,status)
             VALUES('M','p','lane-derivation-v1-herald-cutoff','complete')",
            [],
        )?;

        assert_eq!(LaneAnalysisRepository::enqueue_rederivations(&c, "p")?, 1);
        assert_eq!(
            c.query_row(
                "SELECT status FROM lane_derivation_queue
                 WHERE match_id='M' AND puuid='p' AND derivation_version=?1",
                [lane_score::DERIVATION_VERSION],
                |row| row.get::<_, String>(0),
            )?,
            "pending"
        );
        Ok(())
    }

    #[test]
    fn normalized_facts_rebuild_the_same_versioned_score() -> Result<(), Box<dyn std::error::Error>>
    {
        use crate::riot::parser::ParsedParticipant;
        let db = Database::open_in_memory()?;
        seed(&db)?;
        let mut c = db.connection()?;
        c.execute("UPDATE matches SET queue_id=480 WHERE match_id='M'", [])?;
        LaneAnalysisRepository::enqueue_eligible(&c, "p")?;
        let roster = vec![
            ParsedParticipant {
                puuid: "p".into(),
                fact: ParticipantFact {
                    participant_id: 1,
                    team_id: 100,
                    champion_id: 1,
                    team_position: "TOP".into(),
                    individual_position: "TOP".into(),
                },
            },
            ParsedParticipant {
                puuid: "opponent".into(),
                fact: ParticipantFact {
                    participant_id: 2,
                    team_id: 200,
                    champion_id: 2,
                    team_position: "TOP".into(),
                    individual_position: "TOP".into(),
                },
            },
        ];
        let states = vec![
            LaneState {
                participant_id: 1,
                timestamp_ms: 600_000,
                lane_cs: 80,
                jungle_cs: 0,
                gold: 4_000,
                xp: 6_000,
                level: 8,
            },
            LaneState {
                participant_id: 2,
                timestamp_ms: 600_000,
                lane_cs: 80,
                jungle_cs: 0,
                gold: 4_000,
                xp: 6_000,
                level: 8,
            },
        ];
        let herald = TimelineEvent {
            source_id: "herald".into(),
            timestamp_ms: 600_000,
            kind: "ELITE_MONSTER_KILL".into(),
            killer: Some(1),
            victim: None,
            team_id: Some(100),
            assistants: vec![],
            monster_type: Some("RIFTHERALD".into()),
            monster_sub_type: None,
            building_type: None,
            tower_type: None,
            lane_type: None,
            position: Some((5_000, 10_000)),
        };
        LaneAnalysisRepository::store_facts(&mut c, "p", "M", &roster, &states, &[herald])?;
        let first = LaneAnalysisRepository::rebuild_score(&mut c, "M", "p", true)?.unwrap();
        let second = LaneAnalysisRepository::rebuild_score(&mut c, "M", "p", true)?.unwrap();
        assert_eq!(first, second);
        assert_eq!(first.score, Some(0.0));
        assert_eq!(
            c.query_row(
                "SELECT COUNT(*) FROM lane_score_model_manifests",
                [],
                |row| row.get::<_, i64>(0)
            )?,
            1
        );
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM lane_score_cache", [], |row| row
                .get::<_, i64>(0))?,
            1
        );
        let scope = AggregateScope {
            queue_scope: ALL_QUEUES,
            patch: String::new(),
            season: String::new(),
        };
        let career = LaneAnalysisRepository::performance_summary(&c, "p", None, &scope)?;
        assert_eq!(career.scored_matches, 1);
        assert_eq!(career.excluded_matches, 0);
        assert_eq!(career.average_lane_score, Some(0.0));
        let champion = LaneAnalysisRepository::performance_summary(&c, "p", Some(1), &scope)?;
        assert_eq!(champion.scored_matches, 1);
        let other_champion =
            LaneAnalysisRepository::performance_summary(&c, "p", Some(999), &scope)?;
        assert_eq!(other_champion.scored_matches, 0);
        let diagnostic = LaneAnalysisRepository::match_lane(&c, "p", "M", true)?.unwrap();
        assert_eq!(diagnostic.opponent_champion_id, Some(2));
        assert_eq!(diagnostic.confidence, "HIGH");
        assert_eq!(diagnostic.score, Some(0.0));
        assert_eq!(diagnostic.cutoff_timestamp_ms, Some(840_000));
        assert_eq!(
            diagnostic.cutoff_reason.as_deref(),
            Some("SWIFTPLAY_FIXED_14")
        );
        assert_eq!(
            c.query_row(
                "SELECT COUNT(*) FROM lane_checkpoints
                 WHERE match_id='M' AND derivation_version=?1 AND checkpoint LIKE 'PRE_HERALD:%'",
                [lane_score::DERIVATION_VERSION],
                |row| row.get::<_, i64>(0),
            )?,
            0
        );

        c.execute(
            "UPDATE lane_score_cache SET derivation_version='lane-derivation-v0'",
            [],
        )?;
        let stale = LaneAnalysisRepository::performance_summary(&c, "p", None, &scope)?;
        assert_eq!(stale.scored_matches, 0);
        assert_eq!(stale.excluded_matches, 1);
        Ok(())
    }
    #[test]
    fn stale_swiftplay_unsupported_queue_result_cannot_satisfy_current_summary()
    -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::open_in_memory()?;
        seed(&db)?;
        let c = db.connection()?;
        let manifest = lane_score::ExperimentalManifest::initial();
        c.execute("UPDATE matches SET queue_id=480 WHERE match_id='M'", [])?;
        c.execute(
            "INSERT INTO match_participants(
                match_id,participant_id,puuid,team_id,champion_id,team_position,individual_position)
             VALUES('M',1,'p',100,1,'TOP','TOP'),('M',2,'opponent',200,2,'TOP','TOP')",
            [],
        )?;
        c.execute(
            "INSERT INTO lane_score_eligibility(
                match_id,perspective_participant_id,derivation_version,score_ready,exclusion_reason)
             VALUES('M',1,'lane-derivation-v4-historical-rulesets-horde',0,'UNSUPPORTED_QUEUE')",
            [],
        )?;
        c.execute(
            "INSERT INTO lane_score_cache(
                match_id,perspective_participant_id,opponent_participant_id,
                model_version,feature_schema_version,derivation_version,ruleset_version,
                parameter_hash,status,score,coverage_json,gold_consistency)
             VALUES('M',1,2,?1,?2,'lane-derivation-v4-historical-rulesets-horde',
                'riot-2026-sr-lane-v0',?3,'unsupported',NULL,'{}','unavailable')",
            params![
                manifest.model_version,
                manifest.feature_schema_version,
                manifest.parameter_hash,
            ],
        )?;
        let scope = AggregateScope {
            queue_scope: ALL_QUEUES,
            patch: String::new(),
            season: String::new(),
        };
        let summary = LaneAnalysisRepository::performance_summary(&c, "p", None, &scope)?;
        assert_eq!(summary.tracked_matches, 1);
        assert_eq!(summary.scored_matches, 0);
        assert_eq!(summary.excluded_matches, 1);
        Ok(())
    }
    #[test]
    fn compatible_historical_rulesets_aggregate_without_erasing_provenance()
    -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::open_in_memory()?;
        seed(&db)?;
        let c = db.connection()?;
        let manifest = lane_score::ExperimentalManifest::initial();
        c.execute(
            "INSERT INTO match_participants(
                match_id,participant_id,puuid,team_id,champion_id,team_position,individual_position)
             VALUES('M',1,'p',100,1,'TOP','TOP'),('M',2,'opponent',200,2,'TOP','TOP')",
            [],
        )?;
        c.execute(
            "INSERT INTO lane_score_eligibility(
                match_id,perspective_participant_id,derivation_version,score_ready)
             VALUES('M',1,?1,1)",
            [lane_score::DERIVATION_VERSION],
        )?;
        c.execute(
            "INSERT INTO lane_score_cache(
                match_id,perspective_participant_id,opponent_participant_id,
                model_version,feature_schema_version,derivation_version,ruleset_version,
                parameter_hash,status,score,coverage_json,gold_consistency)
             VALUES('M',1,2,?1,?2,?3,'riot-2026-sr-lane-v0',?4,'ready',0.25,'{}','diagnostic_only')",
            params![
                manifest.model_version,
                manifest.feature_schema_version,
                manifest.derivation_version,
                manifest.parameter_hash,
            ],
        )?;
        for (match_id, patch, ruleset, queue_id) in [
            ("H14", "14.23", "riot-2024-late-sr-lane-v0", 490),
            ("H15A", "15.8", "riot-2025-s1-sr-lane-v0", 420),
            ("H15B", "15.9", "riot-2025-s2-sr-lane-v0", 420),
            ("Swiftplay", "16.15", "riot-2026-sr-lane-v0", 480),
        ] {
            c.execute(
                "INSERT INTO matches SELECT ?1,game_creation-1,NULL,game_duration,?3,?2,?2,NULL,CURRENT_TIMESTAMP FROM matches WHERE match_id='M'",
                params![match_id, patch, queue_id],
            )?;
            c.execute(
                "INSERT INTO player_matches SELECT ?1,puuid,champion_id,win,kills,deaths,assists,double_kills,triple_kills,quadra_kills,penta_kills,total_minions_killed,neutral_minions_killed,gold_earned,summoner1_id,summoner2_id,keystone_id,primary_style_id,secondary_style_id,participant_id FROM player_matches WHERE match_id='M'",
                [match_id],
            )?;
            c.execute(
                "INSERT INTO match_participants SELECT ?1,participant_id,puuid,team_id,champion_id,team_position,individual_position,CURRENT_TIMESTAMP FROM match_participants WHERE match_id='M'",
                [match_id],
            )?;
            c.execute(
                "INSERT INTO lane_score_eligibility(
                    match_id,perspective_participant_id,derivation_version,score_ready)
                 VALUES(?1,1,?2,1)",
                params![match_id, lane_score::DERIVATION_VERSION],
            )?;
            c.execute(
                "INSERT INTO lane_score_cache(
                    match_id,perspective_participant_id,opponent_participant_id,
                    model_version,feature_schema_version,derivation_version,ruleset_version,
                    parameter_hash,status,score,coverage_json,gold_consistency)
                 VALUES(?1,1,2,?2,?3,?4,?5,?6,'ready',0.25,'{}','diagnostic_only')",
                params![
                    match_id,
                    manifest.model_version,
                    manifest.feature_schema_version,
                    manifest.derivation_version,
                    ruleset,
                    manifest.parameter_hash,
                ],
            )?;
        }
        let scope = AggregateScope {
            queue_scope: ALL_QUEUES,
            patch: "16.15".into(),
            season: "2026".into(),
        };
        let summary = LaneAnalysisRepository::performance_summary(&c, "p", None, &scope)?;
        assert_eq!(summary.tracked_matches, 5);
        assert_eq!(summary.scored_matches, 5);
        assert_eq!(summary.average_lane_score, Some(0.25));
        assert_eq!(summary.compatible_ruleset_versions.len(), 4);
        let champion_summary =
            LaneAnalysisRepository::performance_summary(&c, "p", Some(1), &scope)?;
        assert_eq!(champion_summary.tracked_matches, 5);
        assert_eq!(champion_summary.scored_matches, 5);

        c.execute(
            "UPDATE lane_score_cache SET ruleset_version='incompatible-test-ruleset' WHERE match_id='H14'",
            [],
        )?;
        let guarded = LaneAnalysisRepository::performance_summary(&c, "p", None, &scope)?;
        assert_eq!(guarded.tracked_matches, 5);
        assert_eq!(guarded.scored_matches, 4);
        assert_eq!(guarded.excluded_matches, 1);
        Ok(())
    }
}
