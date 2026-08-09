use chrono::{Datelike, TimeZone, Utc};

use crate::domain::items::FinalItem;
use crate::domain::match_record::{MatchRecord, PlayerMatch};
use crate::domain::runes::{RuneSelection, RuneSelectionType};
use crate::error::{AppError, AppResult};
use crate::riot::types::{MatchResponse, ParticipantResponse};

pub struct ParsedMatch {
    pub match_record: MatchRecord,
    pub player_match: PlayerMatch,
}

pub fn parse_match(response: MatchResponse, puuid: &str) -> AppResult<ParsedMatch> {
    if response.metadata.match_id.trim().is_empty() {
        return Err(AppError::RiotData("match ID is empty".to_owned()));
    }

    let participant = response
        .info
        .participants
        .iter()
        .find(|participant| participant.puuid == puuid)
        .ok_or_else(|| {
            AppError::RiotData(format!(
                "PUUID was not present in match {}",
                response.metadata.match_id
            ))
        })?;

    let duration_seconds = normalize_duration(response.info.game_duration);
    let patch = patch_from_version(&response.info.game_version)?;
    let season_key = Utc
        .timestamp_millis_opt(response.info.game_creation)
        .single()
        .map(|timestamp| timestamp.year().to_string());

    let match_record = MatchRecord {
        match_id: response.metadata.match_id.clone(),
        game_creation: response.info.game_creation,
        game_end_timestamp: response.info.game_end_timestamp,
        game_duration_seconds: duration_seconds,
        queue_id: response.info.queue_id,
        game_version: response.info.game_version,
        patch,
        season_key,
    };
    let player_match = parse_participant(response.metadata.match_id, participant);

    Ok(ParsedMatch {
        match_record,
        player_match,
    })
}

fn parse_participant(match_id: String, participant: &ParticipantResponse) -> PlayerMatch {
    let item_ids = [
        participant.item0,
        participant.item1,
        participant.item2,
        participant.item3,
        participant.item4,
        participant.item5,
        participant.item6,
    ];
    let final_items = item_ids
        .into_iter()
        .enumerate()
        .filter(|(_, item_id)| *item_id > 0)
        .map(|(slot, item_id)| FinalItem {
            item_id,
            slot: slot as i64,
            classification: None,
        })
        .collect();

    let primary_style = participant
        .perks
        .styles
        .iter()
        .find(|style| style.description == "primaryStyle")
        .or_else(|| participant.perks.styles.first());
    let secondary_style = participant
        .perks
        .styles
        .iter()
        .find(|style| style.description == "subStyle")
        .or_else(|| participant.perks.styles.get(1));

    let mut rune_selections = Vec::new();
    if let Some(style) = primary_style {
        rune_selections.extend(
            style
                .selections
                .iter()
                .enumerate()
                .map(|(slot, selection)| RuneSelection {
                    selection_type: RuneSelectionType::Primary,
                    slot: slot as i64,
                    rune_id: selection.perk,
                    style_id: Some(style.style),
                }),
        );
    }
    if let Some(style) = secondary_style {
        rune_selections.extend(
            style
                .selections
                .iter()
                .enumerate()
                .map(|(slot, selection)| RuneSelection {
                    selection_type: RuneSelectionType::Secondary,
                    slot: slot as i64,
                    rune_id: selection.perk,
                    style_id: Some(style.style),
                }),
        );
    }
    let shards = [
        participant.perks.stat_perks.offense,
        participant.perks.stat_perks.flex,
        participant.perks.stat_perks.defense,
    ];
    rune_selections.extend(
        shards
            .into_iter()
            .enumerate()
            .map(|(slot, rune_id)| RuneSelection {
                selection_type: RuneSelectionType::StatShard,
                slot: slot as i64,
                rune_id,
                style_id: None,
            }),
    );

    PlayerMatch {
        match_id,
        puuid: participant.puuid.clone(),
        champion_id: participant.champion_id,
        win: participant.win,
        kills: participant.kills,
        deaths: participant.deaths,
        assists: participant.assists,
        double_kills: participant.double_kills,
        triple_kills: participant.triple_kills,
        quadra_kills: participant.quadra_kills,
        penta_kills: participant.penta_kills,
        total_minions_killed: participant.total_minions_killed,
        neutral_minions_killed: participant.neutral_minions_killed,
        gold_earned: participant.gold_earned,
        summoner_spell_ids: [participant.summoner1_id, participant.summoner2_id],
        keystone_id: primary_style
            .and_then(|style| style.selections.first())
            .map(|selection| selection.perk),
        primary_style_id: primary_style.map(|style| style.style),
        secondary_style_id: secondary_style.map(|style| style.style),
        final_items,
        rune_selections,
    }
}

fn normalize_duration(value: i64) -> i64 {
    if value > 86_400 { value / 1_000 } else { value }
}

fn patch_from_version(version: &str) -> AppResult<String> {
    let mut parts = version.split('.');
    let major = parts.next().filter(|part| !part.is_empty());
    let minor = parts.next().filter(|part| !part.is_empty());
    match (major, minor) {
        (Some(major), Some(minor)) => Ok(format!("{major}.{minor}")),
        _ => Err(AppError::RiotData(format!(
            "invalid game version: {version}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_duration, patch_from_version};

    #[test]
    fn extracts_major_minor_patch() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(patch_from_version("16.15.702.123")?, "16.15");
        assert!(patch_from_version("16").is_err());
        Ok(())
    }

    #[test]
    fn normalizes_legacy_millisecond_duration() {
        assert_eq!(normalize_duration(1_800), 1_800);
        assert_eq!(normalize_duration(1_800_000), 1_800);
    }
}
