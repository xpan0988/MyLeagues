//! Conservative generic matchup derivation for Match presentation. This is
//! independent from TOP-only LaneScore opponent mapping.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchupParticipant {
    pub participant_id: i64,
    pub team_id: i64,
    pub champion_id: i64,
    pub team_position: String,
    pub individual_position: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchRole {
    Top,
    Jungle,
    Middle,
    Bottom,
    Utility,
}

impl MatchRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Top => "TOP",
            Self::Jungle => "JUNGLE",
            Self::Middle => "MIDDLE",
            Self::Bottom => "BOTTOM",
            Self::Utility => "UTILITY",
        }
    }

    pub fn lane_score_in_scope(self) -> bool {
        matches!(self, Self::Top)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchupConfidence {
    High,
    Medium,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchupOpponent {
    pub participant_id: i64,
    pub champion_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchupMapping {
    pub local_role: Option<MatchRole>,
    pub opponent: Option<MatchupOpponent>,
    pub confidence: MatchupConfidence,
}

fn normalized_role(value: &str) -> Option<MatchRole> {
    match value.trim().to_ascii_uppercase().as_str() {
        "TOP" => Some(MatchRole::Top),
        "JUNGLE" | "JG" => Some(MatchRole::Jungle),
        "MIDDLE" | "MID" => Some(MatchRole::Middle),
        "BOTTOM" | "BOT" | "ADC" => Some(MatchRole::Bottom),
        "UTILITY" | "SUPPORT" | "SUP" => Some(MatchRole::Utility),
        _ => None,
    }
}

fn resolved_role(participant: &MatchupParticipant) -> Option<(MatchRole, MatchupConfidence)> {
    let team = normalized_role(&participant.team_position);
    let individual = normalized_role(&participant.individual_position);
    match (team, individual) {
        (Some(team), Some(individual)) if team == individual => {
            Some((team, MatchupConfidence::High))
        }
        (Some(team), None) => Some((team, MatchupConfidence::Medium)),
        (None, Some(individual)) => Some((individual, MatchupConfidence::Medium)),
        _ => None,
    }
}

/// Map a match-context opponent only when the local role and exactly one
/// opposite-team counterpart are resolved from stored Match-V5 role facts.
pub fn derive_matchup(participants: &[MatchupParticipant], local_id: i64) -> MatchupMapping {
    let Some(local) = participants
        .iter()
        .find(|value| value.participant_id == local_id)
    else {
        return MatchupMapping {
            local_role: None,
            opponent: None,
            confidence: MatchupConfidence::Unavailable,
        };
    };
    let Some((local_role, local_confidence)) = resolved_role(local) else {
        return MatchupMapping {
            local_role: None,
            opponent: None,
            confidence: MatchupConfidence::Unavailable,
        };
    };
    let candidates: Vec<_> = participants
        .iter()
        .filter_map(|candidate| {
            (candidate.team_id != local.team_id)
                .then(|| resolved_role(candidate).map(|resolved| (candidate, resolved)))
                .flatten()
        })
        .filter(|(_, (role, _))| *role == local_role)
        .collect();
    if candidates.len() != 1 {
        return MatchupMapping {
            local_role: Some(local_role),
            opponent: None,
            confidence: MatchupConfidence::Unavailable,
        };
    }
    let (opponent, (_, opponent_confidence)) = candidates[0];
    MatchupMapping {
        local_role: Some(local_role),
        opponent: Some(MatchupOpponent {
            participant_id: opponent.participant_id,
            champion_id: opponent.champion_id,
        }),
        confidence: if local_confidence == MatchupConfidence::High
            && opponent_confidence == MatchupConfidence::High
        {
            MatchupConfidence::High
        } else {
            MatchupConfidence::Medium
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn participant(id: i64, team: i64, role: &str) -> MatchupParticipant {
        MatchupParticipant {
            participant_id: id,
            team_id: team,
            champion_id: id,
            team_position: role.into(),
            individual_position: role.into(),
        }
    }

    fn roster(local_role: &str) -> Vec<MatchupParticipant> {
        vec![
            participant(1, 100, local_role),
            participant(2, 200, local_role),
            participant(3, 200, if local_role == "TOP" { "MIDDLE" } else { "TOP" }),
        ]
    }

    #[test]
    fn maps_each_supported_role_to_its_opposite_team_counterpart() {
        for (role, expected) in [
            ("TOP", MatchRole::Top),
            ("JUNGLE", MatchRole::Jungle),
            ("MIDDLE", MatchRole::Middle),
            ("BOTTOM", MatchRole::Bottom),
            ("UTILITY", MatchRole::Utility),
        ] {
            let mapping = derive_matchup(&roster(role), 1);
            assert_eq!(mapping.local_role, Some(expected));
            assert_eq!(mapping.opponent.unwrap().participant_id, 2);
        }
    }

    #[test]
    fn ambiguous_or_conflicting_role_facts_do_not_fabricate_an_opponent() {
        let mut participants = roster("MIDDLE");
        participants[0].individual_position = "TOP".into();
        let mapping = derive_matchup(&participants, 1);
        assert_eq!(mapping.local_role, None);
        assert_eq!(mapping.opponent, None);

        let mut participants = roster("MIDDLE");
        participants.push(participant(4, 200, "MIDDLE"));
        let mapping = derive_matchup(&participants, 1);
        assert_eq!(mapping.local_role, Some(MatchRole::Middle));
        assert_eq!(mapping.opponent, None);
    }

    #[test]
    fn only_top_is_in_current_lane_score_scope() {
        assert!(MatchRole::Top.lane_score_in_scope());
        assert!(!MatchRole::Middle.lane_score_in_scope());
    }
}
