//! Deterministic LaneScore V0 derivations.  This module deliberately consumes
//! normalized facts; it never knows about Riot JSON or UI presentation.

use std::collections::{BTreeMap, BTreeSet};

// Queue eligibility, objective applicability, and lane-cutoff semantics are
// derivations rather than model parameters. Keep the parameter hash stable
// while making stale Swiftplay exclusions ineligible for current queries.
pub const DERIVATION_VERSION: &str = "lane-derivation-v6-combat-contributor-sharing";
pub const FEATURE_SCHEMA_VERSION: &str = "lane-features-v0";
pub const MODEL_VERSION: &str = "lane-score-v0-experimental";
pub const HISTORICAL_COMPATIBILITY_VERSION: &str = "riot-sr-2024-late-through-2026-compatible-v0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RulesetManifest {
    pub ruleset_version: &'static str,
    pub raw_patch_major: i64,
    pub raw_patch_minor_from: i64,
    pub raw_patch_minor_to: i64,
    /// Active semantic rulesets accept later ordinary minors until a newer
    /// explicit mechanic-boundary ruleset takes precedence.
    pub open_ended: bool,
    pub valid_patch_from: &'static str,
    pub valid_patch_to: &'static str,
    pub herald_anchor_supported: bool,
    pub void_grub_conversion_supported: bool,
    pub maximum_grub_encounters: i64,
    pub turret_plate_semantics: &'static str,
}

pub const COMPATIBLE_RULESETS: [RulesetManifest; 4] = [
    RulesetManifest {
        ruleset_version: "riot-2024-late-sr-lane-v0",
        raw_patch_major: 14,
        raw_patch_minor_from: 22,
        raw_patch_minor_to: 23,
        open_ended: false,
        valid_patch_from: "14.22",
        valid_patch_to: "14.23",
        herald_anchor_supported: true,
        void_grub_conversion_supported: true,
        maximum_grub_encounters: 2,
        turret_plate_semantics: "OUTER_PLATES_EXPIRE_AT_14",
    },
    RulesetManifest {
        ruleset_version: "riot-2025-s1-sr-lane-v0",
        raw_patch_major: 15,
        raw_patch_minor_from: 4,
        raw_patch_minor_to: 8,
        open_ended: false,
        valid_patch_from: "15.4",
        valid_patch_to: "15.8",
        herald_anchor_supported: true,
        void_grub_conversion_supported: true,
        maximum_grub_encounters: 2,
        turret_plate_semantics: "OUTER_PLATES_EXPIRE_AT_14",
    },
    RulesetManifest {
        ruleset_version: "riot-2025-s2-sr-lane-v0",
        raw_patch_major: 15,
        raw_patch_minor_from: 9,
        raw_patch_minor_to: 23,
        open_ended: false,
        valid_patch_from: "15.9",
        valid_patch_to: "15.23",
        herald_anchor_supported: true,
        void_grub_conversion_supported: true,
        maximum_grub_encounters: 1,
        turret_plate_semantics: "OUTER_PLATES_EXPIRE_AT_14",
    },
    RulesetManifest {
        ruleset_version: "riot-2026-sr-lane-v0",
        raw_patch_major: 16,
        raw_patch_minor_from: 1,
        raw_patch_minor_to: 16,
        open_ended: true,
        valid_patch_from: "16.1",
        valid_patch_to: "16.x-active",
        herald_anchor_supported: true,
        void_grub_conversion_supported: true,
        maximum_grub_encounters: 1,
        turret_plate_semantics: "ALL_TURRET_PLATES_PERMANENT",
    },
];

fn parsed_patch(patch: &str) -> Option<(i64, i64)> {
    let mut components = patch.split('.');
    Some((
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
    ))
}

pub fn ruleset_for_patch(patch: &str) -> Option<&'static RulesetManifest> {
    ruleset_for_patch_in(&COMPATIBLE_RULESETS, patch)
}

pub fn ruleset_for_patch_in<'a>(
    rulesets: &'a [RulesetManifest],
    patch: &str,
) -> Option<&'a RulesetManifest> {
    let (major, minor) = parsed_patch(patch)?;
    rulesets
        .iter()
        .filter(|ruleset| {
            ruleset.raw_patch_major == major
                && minor >= ruleset.raw_patch_minor_from
                && (ruleset.open_ended || minor <= ruleset.raw_patch_minor_to)
        })
        .max_by_key(|ruleset| ruleset.raw_patch_minor_from)
}

pub fn compatible_ruleset_versions() -> Vec<&'static str> {
    COMPATIBLE_RULESETS
        .iter()
        .map(|ruleset| ruleset.ruleset_version)
        .collect()
}

pub fn sql_patch_minor_to(ruleset: &RulesetManifest) -> i64 {
    if ruleset.open_ended {
        i64::MAX
    } else {
        ruleset.raw_patch_minor_to
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentalParameters {
    pub level_table: [f64; 5],
    pub level_extension_step: f64,
    pub level_lambda: f64,
    pub expected_xp_per_level: f64,
    pub xp_residual_scale: f64,
    pub exp_level_weight: f64,
    pub exp_xp_weight: f64,
    pub farm_absolute_scale: f64,
    pub farm_relative_scale: f64,
    pub farm_absolute_weight: f64,
    pub farm_relative_weight: f64,
    pub farm_interaction_weight: f64,
    pub combat_window_ms: i64,
    pub combat_solo: f64,
    pub combat_assisted: f64,
    pub combat_jungle: f64,
    pub combat_jungle_mid: f64,
    pub combat_ambiguous: f64,
    pub core_exp_weight: f64,
    pub core_combat_weight: f64,
    pub core_farm_weight: f64,
    pub pressure_cap: f64,
    pub pressure_tower_units: f64,
    pub objective_cap: f64,
    /// INITIAL_HYPOTHESIS_REQUIRES_CALIBRATION.
    pub lane_fallback_ms: i64,
    /// INITIAL_HYPOTHESIS_REQUIRES_CALIBRATION.
    pub ruleset_lane_cap_ms: i64,
    pub lane_cs_baseline_floor: f64,
    pub top_region_max_x: i64,
    pub top_region_min_y: i64,
    pub teamfight_participant_count: usize,
    pub teamfight_kill_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpponentConfidence {
    High,
    Medium,
    Low,
    Unavailable,
}
impl OpponentConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Low => "LOW",
            Self::Unavailable => "UNAVAILABLE",
        }
    }
    pub fn score_eligible(self) -> bool {
        matches!(self, Self::High)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParticipantFact {
    pub participant_id: i64,
    pub team_id: i64,
    pub champion_id: i64,
    pub team_position: String,
    pub individual_position: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanePair {
    pub a: i64,
    pub b: i64,
    pub confidence: OpponentConfidence,
}

fn role(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}
fn is_top(value: &ParticipantFact) -> bool {
    role(&value.team_position) == "TOP" || role(&value.individual_position) == "TOP"
}
fn strong_top(value: &ParticipantFact) -> bool {
    role(&value.team_position) == "TOP" && role(&value.individual_position) == "TOP"
}

/// V0 only maps a unique opposite-team TOP candidate. A role conflict may be
/// MEDIUM; multiple candidates are deliberately UNAVAILABLE rather than guessed.
pub fn derive_opponent(participants: &[ParticipantFact], a: i64) -> LanePair {
    let Some(local) = participants.iter().find(|p| p.participant_id == a) else {
        return LanePair {
            a,
            b: 0,
            confidence: OpponentConfidence::Unavailable,
        };
    };
    let opponents: Vec<_> = participants
        .iter()
        .filter(|p| p.team_id != local.team_id && is_top(p))
        .collect();
    let local_candidates = participants
        .iter()
        .filter(|p| p.team_id == local.team_id && is_top(p))
        .count();
    if opponents.len() != 1 || local_candidates != 1 {
        return LanePair {
            a,
            b: 0,
            confidence: OpponentConfidence::Unavailable,
        };
    }
    let b = opponents[0].participant_id;
    let confidence = if strong_top(local) && strong_top(opponents[0]) {
        OpponentConfidence::High
    } else if role(&local.team_position) != "TOP" || role(&opponents[0].team_position) != "TOP" {
        OpponentConfidence::Low
    } else {
        OpponentConfidence::Medium
    };
    LanePair { a, b, confidence }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaneState {
    pub participant_id: i64,
    pub timestamp_ms: i64,
    pub lane_cs: i64,
    pub jungle_cs: i64,
    pub gold: i64,
    pub xp: i64,
    pub level: i64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineEvent {
    pub source_id: String,
    pub timestamp_ms: i64,
    pub kind: String,
    pub killer: Option<i64>,
    pub victim: Option<i64>,
    pub team_id: Option<i64>,
    pub assistants: Vec<i64>,
    pub monster_type: Option<String>,
    pub monster_sub_type: Option<String>,
    pub building_type: Option<String>,
    pub tower_type: Option<String>,
    pub lane_type: Option<String>,
    pub position: Option<(i64, i64)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneCutoffReason {
    Herald,
    Fallback14,
    SwiftplayFixed14,
    RulesetCap,
}

impl LaneCutoffReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Herald => "HERALD",
            Self::Fallback14 => "FALLBACK_14",
            Self::SwiftplayFixed14 => "SWIFTPLAY_FIXED_14",
            Self::RulesetCap => "RULESET_CAP",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneCutoff {
    pub timestamp_ms: i64,
    pub reason: LaneCutoffReason,
    pub state_strictly_before: bool,
}

fn is_rift_herald(event: &TimelineEvent) -> bool {
    event.kind == "ELITE_MONSTER_KILL"
        && event.monster_type.as_deref().is_some_and(|value| {
            let value = role(value);
            value.contains("RIFTHERALD") || value.contains("RIFT_HERALD")
        })
}

fn is_void_grub(event: &TimelineEvent) -> bool {
    event.kind == "ELITE_MONSTER_KILL"
        && event.monster_type.as_deref().is_some_and(|value| {
            let value = role(value);
            value == "HORDE" || value.contains("VOIDGRUB") || value.contains("VOID_GRUB")
        })
}

/// Ruleset-aware V0 lane-analysis cutoff. Herald anchors state strictly before
/// the event; fallback and cap are time boundaries and may use an exact frame.
pub fn resolve_lane_cutoff(
    queue_id: i64,
    events: &[TimelineEvent],
    game_end_ms: i64,
    parameters: &ExperimentalParameters,
) -> LaneCutoff {
    if queue_id == 480 {
        return LaneCutoff {
            timestamp_ms: parameters.lane_fallback_ms,
            reason: LaneCutoffReason::SwiftplayFixed14,
            state_strictly_before: false,
        };
    }
    let herald = events
        .iter()
        .filter(|event| event.timestamp_ms <= game_end_ms && is_rift_herald(event))
        .map(|event| event.timestamp_ms)
        .min();
    let (candidate, reason, strict) = herald.map_or(
        (
            parameters.lane_fallback_ms,
            LaneCutoffReason::Fallback14,
            false,
        ),
        |timestamp| (timestamp, LaneCutoffReason::Herald, true),
    );
    if candidate > parameters.ruleset_lane_cap_ms {
        LaneCutoff {
            timestamp_ms: parameters.ruleset_lane_cap_ms,
            reason: LaneCutoffReason::RulesetCap,
            state_strictly_before: false,
        }
    } else {
        LaneCutoff {
            timestamp_ms: candidate,
            reason,
            state_strictly_before: strict,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExclusionReason {
    UnsupportedQueue,
    UnsupportedRole,
    GameTooShort,
    OpponentUnavailable,
    FactsIncomplete,
    RulesetUnsupported,
}

impl ExclusionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedQueue => "UNSUPPORTED_QUEUE",
            Self::UnsupportedRole => "UNSUPPORTED_ROLE",
            Self::GameTooShort => "GAME_TOO_SHORT",
            Self::OpponentUnavailable => "OPPONENT_UNAVAILABLE",
            Self::FactsIncomplete => "FACTS_INCOMPLETE",
            Self::RulesetUnsupported => "RULESET_UNSUPPORTED",
        }
    }
}

fn supported_queue(queue_id: i64) -> bool {
    matches!(queue_id, 400 | 420 | 430 | 480 | 490)
}

fn is_swiftplay_queue(queue_id: i64) -> bool {
    queue_id == 480
}

pub fn latest_before<'a>(
    states: &'a [LaneState],
    participant_id: i64,
    anchor_ms: i64,
) -> Option<&'a LaneState> {
    states
        .iter()
        .filter(|s| s.participant_id == participant_id && s.timestamp_ms < anchor_ms)
        .max_by_key(|s| s.timestamp_ms)
}
pub fn nominal_checkpoint<'a>(
    states: &'a [LaneState],
    participant_id: i64,
    nominal_ms: i64,
    lane_end_ms: i64,
) -> Option<&'a LaneState> {
    states
        .iter()
        .filter(|s| {
            s.participant_id == participant_id
                && s.timestamp_ms <= lane_end_ms
                && s.timestamp_ms >= nominal_ms
        })
        .min_by_key(|s| s.timestamp_ms)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatClassification {
    LaneSoloKill,
    AssistedLaneKill,
    ReinforcementReversal,
    ReinforcementTriple,
    AmbiguousTopSkirmish,
    NonLaneTeamfight,
}
impl CombatClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LaneSoloKill => "LANE_SOLO_KILL",
            Self::AssistedLaneKill => "ASSISTED_LANE_KILL",
            Self::ReinforcementReversal => "REINFORCEMENT_REVERSAL",
            Self::ReinforcementTriple => "REINFORCEMENT_TRIPLE",
            Self::AmbiguousTopSkirmish => "AMBIGUOUS_TOP_SKIRMISH",
            Self::NonLaneTeamfight => "NON_LANE_TEAMFIGHT",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct CombatCluster {
    pub id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub classification: CombatClassification,
    pub signed_strength: f64,
    pub source_event_ids: Vec<String>,
    pub attributions: Vec<CombatEventAttribution>,
}

/// The lane-pair share carried by a source CHAMPION_KILL. Contributors are
/// normalized Riot participant IDs only: valid killer plus unique valid assists,
/// excluding the victim. `signed_lane_pair_share` is from `LanePair::a`'s
/// perspective, so swapping the pair reverses it exactly.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CombatEventAttribution {
    pub source_event_id: String,
    pub contributor_count: usize,
    pub lane_pair_contributor_id: Option<i64>,
    pub lane_opponent_involved: bool,
    pub lane_opponent_share: f64,
    pub signed_lane_pair_share: f64,
}

fn valid_contributors(event: &TimelineEvent) -> BTreeSet<i64> {
    event
        .killer
        .into_iter()
        .chain(event.assistants.iter().copied())
        .filter(|id| (1..=10).contains(id) && Some(*id) != event.victim)
        .collect()
}

/// Equal-share V0 attribution for the direct lane-pair responsibility in a
/// normalized kill event. This intentionally makes no attempt to infer damage,
/// crowd control, killer priority, or other hidden contribution.
pub fn combat_event_attribution(event: &TimelineEvent, pair: &LanePair) -> CombatEventAttribution {
    let contributors = valid_contributors(event);
    let count = contributors.len();
    let share = (count > 0).then_some(1.0 / count as f64).unwrap_or(0.0);
    let (contributor, signed) = if event.victim == Some(pair.b) && contributors.contains(&pair.a) {
        (Some(pair.a), share)
    } else if event.victim == Some(pair.a) && contributors.contains(&pair.b) {
        (Some(pair.b), -share)
    } else {
        (None, 0.0)
    };
    CombatEventAttribution {
        source_event_id: event.source_id.clone(),
        contributor_count: count,
        lane_pair_contributor_id: contributor,
        lane_opponent_involved: contributors.contains(&pair.b),
        lane_opponent_share: contributors
            .contains(&pair.b)
            .then_some(share)
            .unwrap_or(0.0),
        signed_lane_pair_share: signed,
    }
}

fn participant_role(participants: &[ParticipantFact], id: i64) -> String {
    participants
        .iter()
        .find(|p| p.participant_id == id)
        .map(|p| role(&p.team_position))
        .unwrap_or_default()
}
fn top_side(position: Option<(i64, i64)>, p: &ExperimentalParameters) -> bool {
    position.is_some_and(|(x, y)| y > x || (x < p.top_region_max_x && y > p.top_region_min_y))
}

/// Groups chronological source kills exactly once. A cluster is eligible only
/// in lane phase and with lane-pair involvement; large groups remain teamfights.
pub fn combat_clusters(
    events: &[TimelineEvent],
    pair: &LanePair,
    participants: &[ParticipantFact],
    lane_end_ms: i64,
    p: &ExperimentalParameters,
) -> Vec<CombatCluster> {
    let mut kills: Vec<_> = events
        .iter()
        .filter(|e| {
            e.kind == "CHAMPION_KILL"
                && e.timestamp_ms <= lane_end_ms
                && (e.killer == Some(pair.a)
                    || e.killer == Some(pair.b)
                    || e.victim == Some(pair.a)
                    || e.victim == Some(pair.b)
                    || e.assistants.contains(&pair.a)
                    || e.assistants.contains(&pair.b))
        })
        .collect();
    kills.sort_by_key(|e| (e.timestamp_ms, &e.source_id));
    let mut groups: Vec<Vec<&TimelineEvent>> = Vec::new();
    for event in kills {
        if groups.last().is_some_and(|g| {
            event.timestamp_ms - g.last().unwrap().timestamp_ms <= p.combat_window_ms
        }) {
            groups.last_mut().unwrap().push(event);
        } else {
            groups.push(vec![event]);
        }
    }
    groups
        .into_iter()
        .enumerate()
        .map(|(index, group)| {
            let attributions = group
                .iter()
                .map(|event| combat_event_attribution(event, pair))
                .collect::<Vec<_>>();
            let ids: BTreeSet<i64> = group
                .iter()
                .flat_map(|e| valid_contributors(e).into_iter().chain(e.victim))
                .collect();
            let a_wins = attributions
                .iter()
                .any(|attribution| attribution.signed_lane_pair_share > 0.0);
            let b_wins = attributions
                .iter()
                .any(|attribution| attribution.signed_lane_pair_share < 0.0);
            let sign = if a_wins && !b_wins {
                1.0
            } else if b_wins && !a_wins {
                -1.0
            } else {
                0.0
            };
            let jungler = ids.iter().any(|id| {
                participant_role(participants, *id) == "JUNGLE"
                    && group.iter().any(|e| e.victim == Some(*id))
            });
            let mid = ids.iter().any(|id| {
                participant_role(participants, *id) == "MIDDLE"
                    && group.iter().any(|e| e.victim == Some(*id))
            });
            let teamfight =
                ids.len() >= p.teamfight_participant_count || group.len() >= p.teamfight_kill_count;
            let top_context = group.iter().all(|e| top_side(e.position, p));
            let direct_share = attributions
                .iter()
                .filter(|attribution| attribution.signed_lane_pair_share * sign > 0.0)
                .map(|attribution| attribution.signed_lane_pair_share.abs())
                .fold(0.0_f64, f64::max);
            let classification = if sign == 0.0 || teamfight {
                CombatClassification::NonLaneTeamfight
            } else if !top_context {
                CombatClassification::AmbiguousTopSkirmish
            } else if jungler && mid {
                CombatClassification::ReinforcementTriple
            } else if jungler {
                CombatClassification::ReinforcementReversal
            } else if (direct_share - 1.0).abs() < f64::EPSILON {
                CombatClassification::LaneSoloKill
            } else {
                CombatClassification::AssistedLaneKill
            };
            let magnitude = match classification {
                CombatClassification::LaneSoloKill => p.combat_solo,
                CombatClassification::AssistedLaneKill => p.combat_assisted,
                CombatClassification::ReinforcementReversal => p.combat_jungle,
                CombatClassification::ReinforcementTriple => p.combat_jungle_mid,
                CombatClassification::AmbiguousTopSkirmish => p.combat_ambiguous,
                CombatClassification::NonLaneTeamfight => 0.0,
            };
            // The cluster remains atomic. Equal contributor sharing applies to
            // the direct lane-pair kill, while reinforcement classes retain
            // their existing context strength and are never scaled by the
            // number of enemy reinforcements defeated.
            CombatCluster {
                id: format!("cluster-{index}"),
                start_ms: group[0].timestamp_ms,
                end_ms: group.last().unwrap().timestamp_ms,
                classification,
                signed_strength: sign * magnitude * direct_share,
                source_event_ids: group.iter().map(|e| e.source_id.clone()).collect(),
                attributions,
            }
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentalManifest {
    pub model_version: &'static str,
    pub feature_schema_version: &'static str,
    pub derivation_version: &'static str,
    pub ruleset_version: &'static str,
    pub valid_patch_from: &'static str,
    pub valid_patch_to: &'static str,
    pub parameter_hash: String,
    pub parameters: ExperimentalParameters,
}
impl ExperimentalManifest {
    pub fn initial() -> Self {
        Self::for_ruleset(&COMPATIBLE_RULESETS[3])
    }

    pub fn for_patch(patch: &str) -> Option<Self> {
        ruleset_for_patch(patch).map(Self::for_ruleset)
    }

    fn for_ruleset(ruleset: &'static RulesetManifest) -> Self {
        let parameters = ExperimentalParameters {
            level_table: [0.0, 1.0, 2.25, 3.75, 5.5],
            level_extension_step: 2.0,
            level_lambda: 3.0,
            expected_xp_per_level: 500.0,
            xp_residual_scale: 1200.0,
            exp_level_weight: 0.65,
            exp_xp_weight: 0.35,
            farm_absolute_scale: 25.0,
            farm_relative_scale: 0.20,
            farm_absolute_weight: 0.60,
            farm_relative_weight: 0.25,
            farm_interaction_weight: 0.15,
            combat_window_ms: 45_000,
            combat_solo: 0.35,
            combat_assisted: 0.25,
            combat_jungle: 0.50,
            combat_jungle_mid: 0.65,
            combat_ambiguous: 0.10,
            core_exp_weight: 0.45,
            core_combat_weight: 0.30,
            core_farm_weight: 0.25,
            pressure_cap: 0.12,
            pressure_tower_units: 2.0,
            objective_cap: 0.10,
            lane_fallback_ms: 840_000,
            ruleset_lane_cap_ms: 1_020_000,
            lane_cs_baseline_floor: 10.0,
            top_region_max_x: 7_000,
            top_region_min_y: 7_000,
            teamfight_participant_count: 7,
            teamfight_kill_count: 4,
        };
        Self {
            model_version: MODEL_VERSION,
            feature_schema_version: FEATURE_SCHEMA_VERSION,
            derivation_version: DERIVATION_VERSION,
            ruleset_version: ruleset.ruleset_version,
            valid_patch_from: ruleset.valid_patch_from,
            valid_patch_to: ruleset.valid_patch_to,
            parameter_hash: parameter_hash(&parameters),
            parameters,
        }
    }
}

/// This is an identity checksum for immutable model manifests, not a security
/// primitive. The field order is explicit so historical identities cannot be
/// changed by Rust struct layout or map iteration.
fn parameter_hash(parameters: &ExperimentalParameters) -> String {
    format!(
        "fnv1a64:{:016x}",
        fnv1a(canonical_parameter_serialization(parameters).as_bytes())
    )
}

fn canonical_parameter_serialization(parameters: &ExperimentalParameters) -> String {
    format!(
        "v1|level={},{},{},{},{};extension={};lambda={};expected_xp={};xp_scale={};exp={},{};farm={},{},{},{},{};combat={},{},{},{},{},{};core={},{},{};pressure={},{};objective={};lane={},{},{};region={},{};teamfight={},{}",
        parameters.level_table[0],
        parameters.level_table[1],
        parameters.level_table[2],
        parameters.level_table[3],
        parameters.level_table[4],
        parameters.level_extension_step,
        parameters.level_lambda,
        parameters.expected_xp_per_level,
        parameters.xp_residual_scale,
        parameters.exp_level_weight,
        parameters.exp_xp_weight,
        parameters.farm_absolute_scale,
        parameters.farm_relative_scale,
        parameters.farm_absolute_weight,
        parameters.farm_relative_weight,
        parameters.farm_interaction_weight,
        parameters.combat_window_ms,
        parameters.combat_solo,
        parameters.combat_assisted,
        parameters.combat_jungle,
        parameters.combat_jungle_mid,
        parameters.combat_ambiguous,
        parameters.core_exp_weight,
        parameters.core_combat_weight,
        parameters.core_farm_weight,
        parameters.pressure_cap,
        parameters.pressure_tower_units,
        parameters.objective_cap,
        parameters.lane_fallback_ms,
        parameters.ruleset_lane_cap_ms,
        parameters.lane_cs_baseline_floor,
        parameters.top_region_max_x,
        parameters.top_region_min_y,
        parameters.teamfight_participant_count,
        parameters.teamfight_kill_count,
    )
}
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn raw_level_evidence_with(parameters: &ExperimentalParameters, level_difference: i64) -> f64 {
    let value = parameters
        .level_table
        .get(level_difference.unsigned_abs() as usize)
        .copied()
        .unwrap_or(
            parameters.level_table[4]
                + (level_difference.unsigned_abs() as f64 - 4.0) * parameters.level_extension_step,
        );
    value.copysign(level_difference as f64)
}
fn sat(value: f64) -> f64 {
    value.tanh()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dimension {
    pub value: Option<f64>,
    pub coverage: &'static str,
}

/// Objective conversion is deliberately not a missing fact for Swiftplay.
/// It contributes neutral zero without changing the fixed core composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectiveConversionStatus {
    Observed,
    ObservedZero,
    Unavailable,
    NotApplicableByQueue,
}

impl ObjectiveConversionStatus {
    fn coverage(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::ObservedZero => "complete_zero",
            Self::Unavailable => "unavailable",
            Self::NotApplicableByQueue => "not_applicable_by_queue",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreResult {
    pub status: &'static str,
    pub exclusion_reason: Option<ExclusionReason>,
    pub score: Option<f64>,
    pub exp: Dimension,
    pub combat: Dimension,
    pub farm: Dimension,
    pub pressure: Dimension,
    pub conversion: Dimension,
    pub gold_consistency: &'static str,
    pub manifest: ExperimentalManifest,
}

fn pair_frames<'a>(
    states: &'a [LaneState],
    pair: &LanePair,
    cutoff: LaneCutoff,
) -> Vec<(&'a LaneState, &'a LaneState)> {
    let mut a: BTreeMap<i64, &LaneState> = BTreeMap::new();
    let mut b: BTreeMap<i64, &LaneState> = BTreeMap::new();
    for state in states.iter().filter(|state| {
        if cutoff.state_strictly_before {
            state.timestamp_ms < cutoff.timestamp_ms
        } else {
            state.timestamp_ms <= cutoff.timestamp_ms
        }
    }) {
        if state.participant_id == pair.a {
            a.insert(state.timestamp_ms, state);
        }
        if state.participant_id == pair.b {
            b.insert(state.timestamp_ms, state);
        }
    }
    a.into_iter()
        .filter_map(|(time, left)| b.get(&time).copied().map(|right| (left, right)))
        .collect()
}
fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let values: Vec<_> = values.collect();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}
fn exp_dimension(frames: &[(&LaneState, &LaneState)], p: &ExperimentalParameters) -> Dimension {
    let value = mean(frames.iter().map(|(a, b)| {
        let dl = a.level - b.level;
        let level = sat(raw_level_evidence_with(p, dl) / p.level_lambda);
        let residual = (a.xp - b.xp) as f64 - dl as f64 * p.expected_xp_per_level;
        p.exp_level_weight * level + p.exp_xp_weight * sat(residual / p.xp_residual_scale)
    }));
    Dimension {
        value,
        coverage: if value.is_some() {
            "observed"
        } else {
            "unavailable"
        },
    }
}
fn farm_dimension(frames: &[(&LaneState, &LaneState)], p: &ExperimentalParameters) -> Dimension {
    let value = mean(frames.iter().map(|(a, b)| {
        let delta = (a.lane_cs - b.lane_cs) as f64;
        let rel = delta / (((a.lane_cs + b.lane_cs) as f64 / 2.0).max(p.lane_cs_baseline_floor));
        let abs = sat(delta / p.farm_absolute_scale);
        let relative = sat(rel / p.farm_relative_scale);
        sat(p.farm_absolute_weight * abs
            + p.farm_relative_weight * relative
            + p.farm_interaction_weight * abs * relative.abs())
    }));
    Dimension {
        value,
        coverage: if value.is_some() {
            "observed"
        } else {
            "unavailable"
        },
    }
}
fn combat_dimension(clusters: &[CombatCluster], complete: bool) -> Dimension {
    let value = complete.then(|| sat(clusters.iter().map(|c| c.signed_strength).sum::<f64>()));
    Dimension {
        value,
        coverage: if complete { "complete" } else { "unavailable" },
    }
}

fn event_participants(event: &TimelineEvent) -> impl Iterator<Item = i64> + '_ {
    event
        .killer
        .into_iter()
        .chain(event.assistants.iter().copied())
}
fn pressure_dimension(
    events: &[TimelineEvent],
    participants: &[ParticipantFact],
    pair: &LanePair,
    lane_end_ms: i64,
    p: &ExperimentalParameters,
) -> Dimension {
    let team_a = participants
        .iter()
        .find(|p| p.participant_id == pair.a)
        .map(|p| p.team_id);
    let team_b = participants
        .iter()
        .find(|p| p.participant_id == pair.b)
        .map(|p| p.team_id);
    let Some((team_a, team_b)) = team_a.zip(team_b) else {
        return Dimension {
            value: None,
            coverage: "unavailable",
        };
    };
    let mut diff = 0.0;
    let mut observed = false;
    for e in events.iter().filter(|e| {
        e.timestamp_ms <= lane_end_ms
            && (e.kind == "TURRET_PLATE_DESTROYED" || e.kind == "BUILDING_KILL")
            && e.lane_type
                .as_deref()
                .is_some_and(|lane| role(lane).contains("TOP"))
    }) {
        if let Some(killer) = event_participants(e).next() {
            observed = true;
            let team = participants
                .iter()
                .find(|p| p.participant_id == killer)
                .map(|p| p.team_id);
            if team == Some(team_a) {
                diff += if e.kind == "BUILDING_KILL" {
                    p.pressure_tower_units
                } else {
                    1.0
                };
            }
            if team == Some(team_b) {
                diff -= if e.kind == "BUILDING_KILL" {
                    p.pressure_tower_units
                } else {
                    1.0
                };
            }
        }
    }
    Dimension {
        value: observed.then(|| sat(diff / (p.pressure_tower_units * 2.0)) * p.pressure_cap),
        coverage: if observed {
            "team_side_attribution"
        } else {
            "unavailable"
        },
    }
}
fn objective_dimension(
    events: &[TimelineEvent],
    states: &[LaneState],
    pair: &LanePair,
    lane_end_ms: i64,
    p: &ExperimentalParameters,
    queue_id: i64,
) -> Dimension {
    if is_swiftplay_queue(queue_id) {
        return Dimension {
            value: Some(0.0),
            coverage: ObjectiveConversionStatus::NotApplicableByQueue.coverage(),
        };
    }
    let objectives: Vec<_> = events
        .iter()
        .filter(|e| {
            e.timestamp_ms <= lane_end_ms
                && e.kind == "ELITE_MONSTER_KILL"
                && (is_rift_herald(e) || is_void_grub(e))
        })
        .collect();
    if objectives.is_empty() {
        return Dimension {
            value: Some(0.0),
            coverage: ObjectiveConversionStatus::ObservedZero.coverage(),
        };
    }
    let mut value = 0.0;
    let mut observed = false;
    for event in objectives {
        let a = latest_before(states, pair.a, event.timestamp_ms);
        let b = latest_before(states, pair.b, event.timestamp_ms);
        if let (Some(a), Some(b)) = (a, b) {
            observed = true;
            let priority_a = ((a.xp - b.xp) as f64 / p.xp_residual_scale
                + (a.lane_cs - b.lane_cs) as f64 / p.farm_absolute_scale)
                .max(0.0);
            let priority_b = (-((a.xp - b.xp) as f64 / p.xp_residual_scale
                + (a.lane_cs - b.lane_cs) as f64 / p.farm_absolute_scale))
                .max(0.0);
            let participation_a = event_participants(event).any(|id| id == pair.a) as i32 as f64;
            let participation_b = event_participants(event).any(|id| id == pair.b) as i32 as f64;
            value += participation_a * sat(priority_a) - participation_b * sat(priority_b);
        }
    }
    Dimension {
        value: observed.then(|| sat(value) * p.objective_cap),
        coverage: if observed {
            ObjectiveConversionStatus::Observed.coverage()
        } else {
            ObjectiveConversionStatus::Unavailable.coverage()
        },
    }
}

pub fn score(
    pair: &LanePair,
    participants: &[ParticipantFact],
    states: &[LaneState],
    events: &[TimelineEvent],
    queue_id: i64,
    patch: &str,
    game_end_ms: i64,
    kill_coverage_complete: bool,
) -> (ScoreResult, Vec<CombatCluster>, Option<LaneCutoff>) {
    let selected_manifest = ExperimentalManifest::for_patch(patch);
    let manifest = selected_manifest
        .clone()
        .unwrap_or_else(ExperimentalManifest::initial);
    let p = &manifest.parameters;
    let cutoff = resolve_lane_cutoff(queue_id, events, game_end_ms, p);
    let frames = pair_frames(states, pair, cutoff);
    let exp = exp_dimension(&frames, p);
    let farm = farm_dimension(&frames, p);
    let clusters = combat_clusters(events, pair, participants, cutoff.timestamp_ms, p);
    let combat = combat_dimension(&clusters, kill_coverage_complete);
    let pressure = pressure_dimension(events, participants, pair, cutoff.timestamp_ms, p);
    let conversion = objective_dimension(events, states, pair, cutoff.timestamp_ms, p, queue_id);
    let core = exp
        .value
        .zip(combat.value)
        .zip(farm.value)
        .map(|((e, c), f)| {
            p.core_exp_weight * e + p.core_combat_weight * c + p.core_farm_weight * f
        });
    let local = participants
        .iter()
        .find(|participant| participant.participant_id == pair.a);
    let exclusion_reason = if !supported_queue(queue_id) {
        Some(ExclusionReason::UnsupportedQueue)
    } else if selected_manifest.is_none() {
        Some(ExclusionReason::RulesetUnsupported)
    } else if game_end_ms < p.lane_fallback_ms {
        Some(ExclusionReason::GameTooShort)
    } else if !local.is_some_and(strong_top) {
        Some(ExclusionReason::UnsupportedRole)
    } else if !pair.confidence.score_eligible() {
        Some(ExclusionReason::OpponentUnavailable)
    } else if core.is_none() {
        Some(ExclusionReason::FactsIncomplete)
    } else {
        None
    };
    let score = exclusion_reason
        .is_none()
        .then_some(())
        .and(core)
        .map(|z| sat(z + pressure.value.unwrap_or(0.0) + conversion.value.unwrap_or(0.0)));
    let status = match exclusion_reason {
        None => "ready",
        Some(ExclusionReason::FactsIncomplete) => "insufficient_evidence",
        Some(_) => "unsupported",
    };
    let gold_consistency = if frames.is_empty() {
        "unavailable"
    } else {
        "diagnostic_only"
    };
    let persisted_cutoff = (game_end_ms >= p.lane_fallback_ms).then_some(cutoff);
    (
        ScoreResult {
            status,
            exclusion_reason,
            score,
            exp,
            combat,
            farm,
            pressure,
            conversion,
            gold_consistency,
            manifest,
        },
        clusters,
        persisted_cutoff,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn participants() -> Vec<ParticipantFact> {
        vec![
            ParticipantFact {
                participant_id: 1,
                team_id: 100,
                champion_id: 1,
                team_position: "TOP".into(),
                individual_position: "TOP".into(),
            },
            ParticipantFact {
                participant_id: 2,
                team_id: 200,
                champion_id: 2,
                team_position: "TOP".into(),
                individual_position: "TOP".into(),
            },
            ParticipantFact {
                participant_id: 3,
                team_id: 200,
                champion_id: 3,
                team_position: "JUNGLE".into(),
                individual_position: "JUNGLE".into(),
            },
            ParticipantFact {
                participant_id: 4,
                team_id: 200,
                champion_id: 4,
                team_position: "MIDDLE".into(),
                individual_position: "MIDDLE".into(),
            },
        ]
    }
    fn state(id: i64, time: i64, xp: i64, level: i64, cs: i64) -> LaneState {
        LaneState {
            participant_id: id,
            timestamp_ms: time,
            lane_cs: cs,
            jungle_cs: 0,
            gold: 0,
            xp,
            level,
        }
    }
    fn base_states() -> Vec<LaneState> {
        vec![
            state(1, 360000, 4000, 6, 50),
            state(2, 360000, 4000, 6, 50),
            state(1, 600000, 6000, 8, 80),
            state(2, 600000, 6000, 8, 80),
        ]
    }
    fn kill(id: &str, time: i64, killer: i64, victim: i64, assistants: Vec<i64>) -> TimelineEvent {
        TimelineEvent {
            source_id: id.into(),
            timestamp_ms: time,
            kind: "CHAMPION_KILL".into(),
            killer: Some(killer),
            victim: Some(victim),
            team_id: None,
            assistants,
            monster_type: None,
            monster_sub_type: None,
            building_type: None,
            tower_type: None,
            lane_type: None,
            position: Some((1000, 9000)),
        }
    }
    fn herald(time: i64) -> TimelineEvent {
        TimelineEvent {
            source_id: "herald".into(),
            timestamp_ms: time,
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
            position: Some((5000, 10000)),
        }
    }
    #[test]
    fn mapping_is_conservative() {
        let pair = derive_opponent(&participants(), 1);
        assert_eq!(pair.confidence, OpponentConfidence::High);
        let generic_participants = participants()
            .into_iter()
            .map(|participant| crate::domain::matchup::MatchupParticipant {
                participant_id: participant.participant_id,
                team_id: participant.team_id,
                champion_id: participant.champion_id,
                team_position: participant.team_position,
                individual_position: participant.individual_position,
            })
            .collect::<Vec<_>>();
        let generic = crate::domain::matchup::derive_matchup(&generic_participants, 1);
        assert_eq!(
            generic.local_role,
            Some(crate::domain::matchup::MatchRole::Top)
        );
        assert_eq!(generic.opponent.unwrap().participant_id, pair.b);
        let mut bad = participants();
        bad.push(ParticipantFact {
            participant_id: 5,
            team_id: 200,
            champion_id: 5,
            team_position: "TOP".into(),
            individual_position: "".into(),
        });
        assert_eq!(
            derive_opponent(&bad, 1).confidence,
            OpponentConfidence::Unavailable
        );
    }
    #[test]
    fn raw_game_versions_map_to_explicit_historical_and_active_rulesets() {
        assert_eq!(
            ruleset_for_patch("14.22").unwrap().ruleset_version,
            "riot-2024-late-sr-lane-v0"
        );
        assert_eq!(
            ruleset_for_patch("15.8").unwrap().ruleset_version,
            "riot-2025-s1-sr-lane-v0"
        );
        assert_eq!(
            ruleset_for_patch("15.9").unwrap().ruleset_version,
            "riot-2025-s2-sr-lane-v0"
        );
        assert_eq!(
            ruleset_for_patch("16.1").unwrap().ruleset_version,
            "riot-2026-sr-lane-v0"
        );
        assert_eq!(
            ruleset_for_patch("16.15.802.4387").unwrap().ruleset_version,
            "riot-2026-sr-lane-v0"
        );
        assert_eq!(
            ruleset_for_patch("16.16.804.9184").unwrap().ruleset_version,
            "riot-2026-sr-lane-v0"
        );
        assert_eq!(
            ruleset_for_patch("16.17.1.1").unwrap().ruleset_version,
            "riot-2026-sr-lane-v0"
        );
        assert_ne!(
            ruleset_for_patch("15.23").unwrap().ruleset_version,
            "riot-2026-sr-lane-v0"
        );
        assert!(ruleset_for_patch("14.21").is_none());
        assert!(ruleset_for_patch("26.15").is_none());
    }

    #[test]
    fn later_mechanic_boundary_overrides_an_open_active_ruleset() {
        let mut split = COMPATIBLE_RULESETS[3];
        split.ruleset_version = "riot-2026-sr-lane-v1";
        split.raw_patch_minor_from = 20;
        split.raw_patch_minor_to = 20;
        split.open_ended = true;
        let rulesets = [COMPATIBLE_RULESETS[3], split];
        assert_eq!(
            ruleset_for_patch_in(&rulesets, "16.19")
                .unwrap()
                .ruleset_version,
            "riot-2026-sr-lane-v0"
        );
        assert_eq!(
            ruleset_for_patch_in(&rulesets, "16.20")
                .unwrap()
                .ruleset_version,
            "riot-2026-sr-lane-v1"
        );
    }
    #[test]
    fn live_horde_monster_type_is_void_grub_evidence() {
        let mut event = herald(500_000);
        event.monster_type = Some("HORDE".into());
        assert!(is_void_grub(&event));
        assert!(!is_rift_herald(&event));
    }
    #[test]
    fn bounded_neutral_and_swap_antisymmetry() {
        let pair = derive_opponent(&participants(), 1);
        let (even, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            420,
            "16.1",
            900000,
            true,
        );
        assert_eq!(even.score, Some(0.0));
        let states = vec![
            state(1, 600000, 8000, 10, 110),
            state(2, 600000, 5000, 7, 70),
        ];
        let (forward, ..) = score(
            &pair,
            &participants(),
            &states,
            &[],
            420,
            "16.1",
            900000,
            true,
        );
        let reverse = LanePair {
            a: 2,
            b: 1,
            confidence: OpponentConfidence::High,
        };
        let (backward, ..) = score(
            &reverse,
            &participants(),
            &states,
            &[],
            420,
            "16.1",
            900000,
            true,
        );
        assert!(forward.score.unwrap() <= 1.0 && forward.score.unwrap() >= -1.0);
        assert_eq!(forward.score.unwrap(), -backward.score.unwrap());
    }
    #[test]
    fn raw_levels_are_superlinear_before_saturation() {
        let parameters = ExperimentalManifest::initial().parameters;
        assert_eq!(raw_level_evidence_with(&parameters, 0), 0.0);
        assert_eq!(raw_level_evidence_with(&parameters, 1), 1.0);
        for i in 2..=4 {
            assert!(
                raw_level_evidence_with(&parameters, i)
                    > i as f64 * raw_level_evidence_with(&parameters, 1)
            );
        }
        assert!(sat(raw_level_evidence_with(&parameters, 20) / parameters.level_lambda) < 1.0);
    }
    #[test]
    fn parameter_hash_is_stable_ordered_and_changes_with_parameters() {
        let manifest = ExperimentalManifest::initial();
        assert_eq!(manifest.parameter_hash, "fnv1a64:7d243b2899fb5092");
        assert_eq!(
            parameter_hash(&manifest.parameters),
            manifest.parameter_hash
        );
        assert_eq!(
            canonical_parameter_serialization(&manifest.parameters),
            canonical_parameter_serialization(&manifest.parameters)
        );
        let mut changed = manifest.parameters.clone();
        changed.objective_cap = 0.11;
        assert_ne!(parameter_hash(&changed), manifest.parameter_hash);
    }
    #[test]
    fn cs_xp_win_and_levels_are_monotone() {
        let pair = derive_opponent(&participants(), 1);
        let neutral = vec![state(1, 600000, 6000, 8, 80), state(2, 600000, 6000, 8, 80)];
        let ahead = vec![
            state(1, 600000, 7600, 10, 105),
            state(2, 600000, 5500, 7, 70),
        ];
        let (n, ..) = score(
            &pair,
            &participants(),
            &neutral,
            &[],
            420,
            "16.1",
            900000,
            true,
        );
        let (a, ..) = score(
            &pair,
            &participants(),
            &ahead,
            &[],
            420,
            "16.1",
            900000,
            true,
        );
        assert!(a.score.unwrap() > n.score.unwrap());
    }
    #[test]
    fn missing_kill_coverage_is_not_neutral() {
        let pair = derive_opponent(&participants(), 1);
        let (missing, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            420,
            "16.1",
            900000,
            false,
        );
        let (zero, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            420,
            "16.1",
            900000,
            true,
        );
        assert_eq!(missing.status, "insufficient_evidence");
        assert_eq!(missing.combat.value, None);
        assert_eq!(zero.combat.value, Some(0.0));
    }
    #[test]
    fn strict_pre_event_checkpoint_never_uses_future_state() {
        let states = vec![state(1, 100, 1, 1, 1), state(1, 200, 2, 1, 2)];
        assert_eq!(latest_before(&states, 1, 200).unwrap().timestamp_ms, 100);
    }

    #[test]
    fn pre_herald_uses_the_latest_real_state_not_a_nominal_checkpoint() {
        let states = vec![
            state(1, 900_000, 1, 1, 1),
            state(1, 960_000, 2, 1, 2),
            state(1, 1_020_000, 3, 1, 3),
        ];
        assert_eq!(
            latest_before(&states, 1, 983_000).unwrap().timestamp_ms,
            960_000
        );
    }

    #[test]
    fn pre_grubs_uses_the_latest_real_state_and_never_a_post_event_frame() {
        let states = vec![
            state(1, 300_000, 1, 1, 1),
            state(1, 345_000, 2, 1, 2),
            state(1, 360_000, 3, 1, 3),
        ];
        assert_eq!(
            latest_before(&states, 1, 360_000).unwrap().timestamp_ms,
            345_000
        );
    }
    #[test]
    fn no_herald_uses_fourteen_minute_fallback() {
        let parameters = ExperimentalManifest::initial().parameters;
        let cutoff = resolve_lane_cutoff(420, &[], 1_800_000, &parameters);
        assert_eq!(cutoff.timestamp_ms, 840_000);
        assert_eq!(cutoff.reason, LaneCutoffReason::Fallback14);
        assert!(!cutoff.state_strictly_before);
    }
    #[test]
    fn herald_anchor_uses_only_strictly_prior_state() {
        let parameters = ExperimentalManifest::initial().parameters;
        let cutoff = resolve_lane_cutoff(420, &[herald(942_000)], 1_800_000, &parameters);
        assert_eq!(cutoff.timestamp_ms, 942_000);
        assert_eq!(cutoff.reason, LaneCutoffReason::Herald);
        let pair = derive_opponent(&participants(), 1);
        let states = vec![
            state(1, 900_000, 8_000, 10, 120),
            state(2, 900_000, 8_000, 10, 120),
            state(1, 942_000, 20_000, 18, 300),
            state(2, 942_000, 8_000, 10, 120),
            state(1, 960_000, 21_000, 18, 310),
            state(2, 960_000, 8_000, 10, 120),
        ];
        let frames = pair_frames(&states, &pair, cutoff);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0.timestamp_ms, 900_000);
    }
    #[test]
    fn late_herald_uses_ruleset_cap() {
        let parameters = ExperimentalManifest::initial().parameters;
        let cutoff = resolve_lane_cutoff(420, &[herald(1_080_000)], 1_800_000, &parameters);
        assert_eq!(cutoff.timestamp_ms, 1_020_000);
        assert_eq!(cutoff.reason, LaneCutoffReason::RulesetCap);
    }
    #[test]
    fn swiftplay_uses_fixed_fourteen_minute_cutoff_without_herald_checkpoints() {
        let parameters = ExperimentalManifest::initial().parameters;
        for herald_time in [600_000, 720_000, 900_000, 1_080_000] {
            let cutoff = resolve_lane_cutoff(480, &[herald(herald_time)], 1_800_000, &parameters);
            assert_eq!(cutoff.timestamp_ms, 840_000);
            assert_eq!(cutoff.reason, LaneCutoffReason::SwiftplayFixed14);
            assert!(!cutoff.state_strictly_before);
        }
    }
    #[test]
    fn swiftplay_objective_conversion_is_policy_na_without_core_inflation() {
        let pair = derive_opponent(&participants(), 1);
        let mut grub = herald(600_000);
        grub.source_id = "grubs".into();
        grub.monster_type = Some("HORDE".into());
        let objective_events = vec![grub, herald(720_000)];
        let (with_objectives, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &objective_events,
            480,
            "16.1",
            1_200_000,
            true,
        );
        let (without_objectives, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            480,
            "16.1",
            1_200_000,
            true,
        );
        assert_eq!(with_objectives.status, "ready");
        assert_eq!(with_objectives.conversion.value, Some(0.0));
        assert_eq!(
            with_objectives.conversion.coverage,
            ObjectiveConversionStatus::NotApplicableByQueue.coverage()
        );
        assert_eq!(with_objectives.score, without_objectives.score);
        assert_eq!(with_objectives.exp, without_objectives.exp);
        assert_eq!(with_objectives.combat, without_objectives.combat);
        assert_eq!(with_objectives.farm, without_objectives.farm);
    }
    #[test]
    fn swiftplay_keeps_valid_lane_combat_and_pressure() {
        let pair = derive_opponent(&participants(), 1);
        let pressure = TimelineEvent {
            source_id: "top-plate".into(),
            timestamp_ms: 700_000,
            kind: "TURRET_PLATE_DESTROYED".into(),
            killer: Some(1),
            victim: None,
            team_id: Some(100),
            assistants: vec![],
            monster_type: None,
            monster_sub_type: None,
            building_type: None,
            tower_type: None,
            lane_type: Some("TOP_LANE".into()),
            position: Some((1_000, 9_000)),
        };
        let (result, clusters, cutoff) = score(
            &pair,
            &participants(),
            &base_states(),
            &[
                herald(600_000),
                kill("near-herald", 605_000, 1, 2, vec![]),
                pressure,
            ],
            480,
            "16.1",
            1_200_000,
            true,
        );
        assert_eq!(cutoff.unwrap().timestamp_ms, 840_000);
        assert_eq!(result.combat.coverage, "complete");
        assert!(result.combat.value.unwrap() > 0.0);
        assert!(
            clusters
                .iter()
                .any(|cluster| cluster.classification == CombatClassification::LaneSoloKill)
        );
        assert_eq!(result.pressure.coverage, "team_side_attribution");
        assert!(result.pressure.value.unwrap() > 0.0);
    }
    #[test]
    fn eligibility_excludes_short_and_aram_but_allows_post_horizon_end() {
        let pair = derive_opponent(&participants(), 1);
        let (short, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            420,
            "16.1",
            839_000,
            true,
        );
        assert_eq!(short.score, None);
        assert_eq!(short.exclusion_reason, Some(ExclusionReason::GameTooShort));
        let (aram, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            450,
            "16.1",
            1_800_000,
            true,
        );
        assert_eq!(aram.score, None);
        assert_eq!(
            aram.exclusion_reason,
            Some(ExclusionReason::UnsupportedQueue)
        );
        let (normal_surrender_horizon, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            420,
            "16.1",
            930_000,
            true,
        );
        assert_eq!(normal_surrender_horizon.exclusion_reason, None);
        assert!(normal_surrender_horizon.score.is_some());
        let (swiftplay, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            480,
            "16.1",
            1_200_000,
            true,
        );
        assert_eq!(swiftplay.status, "ready");
        let (short_swiftplay, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            480,
            "16.1",
            839_000,
            true,
        );
        assert_eq!(
            short_swiftplay.exclusion_reason,
            Some(ExclusionReason::GameTooShort)
        );
    }
    #[test]
    fn short_remake_like_game_is_unavailable_and_excluded() {
        let pair = derive_opponent(&participants(), 1);
        let (result, clusters, cutoff) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            420,
            "16.1",
            300_000,
            true,
        );
        assert_eq!(result.score, None);
        assert_eq!(result.exclusion_reason, Some(ExclusionReason::GameTooShort));
        assert!(clusters.is_empty());
        assert_eq!(cutoff, None);
    }
    #[test]
    fn combat_contributor_sharing_is_equal_and_antisymmetric() {
        let pair = derive_opponent(&participants(), 1);
        let solo = combat_event_attribution(&kill("solo", 100_000, 1, 2, vec![]), &pair);
        assert_eq!(solo.contributor_count, 1);
        assert_eq!(solo.signed_lane_pair_share, 1.0);

        let two = kill("two", 100_000, 1, 2, vec![3]);
        let two_forward = combat_event_attribution(&two, &pair);
        let two_reverse = combat_event_attribution(
            &two,
            &LanePair {
                a: 2,
                b: 1,
                confidence: OpponentConfidence::High,
            },
        );
        assert_eq!(two_forward.contributor_count, 2);
        assert_eq!(two_forward.signed_lane_pair_share, 0.5);
        assert_eq!(
            two_forward.signed_lane_pair_share,
            -two_reverse.signed_lane_pair_share
        );
        let manifest = ExperimentalManifest::initial();
        let forward_cluster = combat_clusters(
            &[two.clone()],
            &pair,
            &participants(),
            800_000,
            &manifest.parameters,
        );
        let reverse_cluster = combat_clusters(
            &[two],
            &LanePair {
                a: 2,
                b: 1,
                confidence: OpponentConfidence::High,
            },
            &participants(),
            800_000,
            &manifest.parameters,
        );
        assert_eq!(
            forward_cluster[0].signed_strength,
            -reverse_cluster[0].signed_strength
        );

        let three = combat_event_attribution(&kill("three", 100_000, 1, 2, vec![3, 4]), &pair);
        assert_eq!(three.contributor_count, 3);
        assert_eq!(three.signed_lane_pair_share, 1.0 / 3.0);
    }
    #[test]
    fn combat_contributors_ignore_invalid_ids_deduplicate_and_exclude_victim() {
        let pair = derive_opponent(&participants(), 1);
        let invalid =
            combat_event_attribution(&kill("invalid", 100_000, 0, 1, vec![2, 2, 3, 0, 1]), &pair);
        // {2, 3}: zero is invalid, duplicate 2 is collapsed, victim 1 is excluded.
        assert_eq!(invalid.contributor_count, 2);
        assert_eq!(invalid.signed_lane_pair_share, -0.5);
        assert!(invalid.lane_opponent_involved);
        assert_eq!(invalid.lane_opponent_share, 0.5);
    }
    #[test]
    fn jungler_only_kill_has_no_lane_pair_combat_credit() {
        let pair = derive_opponent(&participants(), 1);
        let event = kill("jg-only", 100_000, 3, 1, vec![]);
        let attribution = combat_event_attribution(&event, &pair);
        assert_eq!(attribution.contributor_count, 1);
        assert_eq!(attribution.signed_lane_pair_share, 0.0);
        assert!(!attribution.lane_opponent_involved);
        let manifest = ExperimentalManifest::initial();
        let clusters = combat_clusters(
            &[event],
            &pair,
            &participants(),
            800_000,
            &manifest.parameters,
        );
        assert_eq!(clusters[0].signed_strength, 0.0);
    }
    #[test]
    fn direct_solo_cluster_is_stronger_than_assisted_clusters() {
        let pair = derive_opponent(&participants(), 1);
        let manifest = ExperimentalManifest::initial();
        let solo = combat_clusters(
            &[kill("solo", 100_000, 1, 2, vec![])],
            &pair,
            &participants(),
            800_000,
            &manifest.parameters,
        );
        let two = combat_clusters(
            &[kill("two", 100_000, 1, 2, vec![3])],
            &pair,
            &participants(),
            800_000,
            &manifest.parameters,
        );
        let three = combat_clusters(
            &[kill("three", 100_000, 1, 2, vec![3, 4])],
            &pair,
            &participants(),
            800_000,
            &manifest.parameters,
        );
        assert_eq!(solo[0].classification, CombatClassification::LaneSoloKill);
        assert_eq!(
            two[0].classification,
            CombatClassification::AssistedLaneKill
        );
        assert_eq!(
            three[0].classification,
            CombatClassification::AssistedLaneKill
        );
        assert!(solo[0].signed_strength.abs() > two[0].signed_strength.abs());
        assert!(two[0].signed_strength.abs() > three[0].signed_strength.abs());
    }
    #[test]
    fn gank_combat_sharing_does_not_change_state_dimensions() {
        let pair = derive_opponent(&participants(), 1);
        let states = vec![
            state(1, 600_000, 5_000, 7, 60),
            state(2, 600_000, 7_000, 9, 90),
        ];
        let (with_gank, ..) = score(
            &pair,
            &participants(),
            &states,
            &[kill("gank", 500_000, 2, 1, vec![3])],
            420,
            "16.1",
            900_000,
            true,
        );
        let (without_gank, ..) = score(
            &pair,
            &participants(),
            &states,
            &[],
            420,
            "16.1",
            900_000,
            true,
        );
        assert_eq!(with_gank.exp, without_gank.exp);
        assert_eq!(with_gank.farm, without_gank.farm);
        assert_eq!(with_gank.pressure, without_gank.pressure);
        assert!(with_gank.combat.value.unwrap() < without_gank.combat.value.unwrap());
    }
    #[test]
    fn clusters_are_atomic_and_upgrade_reinforcements() {
        let pair = derive_opponent(&participants(), 1);
        let events = vec![
            kill("a", 100000, 1, 2, vec![]),
            kill("b", 110000, 1, 3, vec![]),
            kill("c", 120000, 1, 4, vec![]),
        ];
        let manifest = ExperimentalManifest::initial();
        let clusters = combat_clusters(
            &events,
            &pair,
            &participants(),
            800000,
            &manifest.parameters,
        );
        assert_eq!(clusters.len(), 1);
        assert_eq!(
            clusters[0].classification,
            CombatClassification::ReinforcementTriple
        );
        assert_eq!(clusters[0].source_event_ids.len(), 3);
        let unique = clusters[0].source_event_ids.iter().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), clusters[0].source_event_ids.len());
    }
    #[test]
    fn solo_and_jungle_reversal_have_single_strength() {
        let pair = derive_opponent(&participants(), 1);
        let manifest = ExperimentalManifest::initial();
        let solo = combat_clusters(
            &[kill("a", 100000, 1, 2, vec![])],
            &pair,
            &participants(),
            800000,
            &manifest.parameters,
        );
        assert_eq!(solo[0].classification, CombatClassification::LaneSoloKill);
        let anti = combat_clusters(
            &[
                kill("a", 100000, 1, 2, vec![]),
                kill("b", 110000, 1, 3, vec![]),
            ],
            &pair,
            &participants(),
            800000,
            &manifest.parameters,
        );
        assert_eq!(
            anti[0].classification,
            CombatClassification::ReinforcementReversal
        );
        assert_eq!(anti[0].attributions[0].signed_lane_pair_share, 1.0);
        assert!(anti[0].signed_strength > solo[0].signed_strength);
    }
    #[test]
    fn manifest_is_deterministic_and_optional_missing_does_not_change_core() {
        let pair = derive_opponent(&participants(), 1);
        let (one, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            420,
            "16.1",
            900000,
            true,
        );
        let (two, ..) = score(
            &pair,
            &participants(),
            &base_states(),
            &[],
            420,
            "16.1",
            900000,
            true,
        );
        assert_eq!(one, two);
        assert_eq!(one.pressure.value, None);
        assert_eq!(one.score, Some(0.0));
    }
}
