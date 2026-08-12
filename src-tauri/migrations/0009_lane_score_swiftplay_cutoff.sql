-- Queue-specific Swiftplay derivations use an explicit fixed-horizon reason.
-- Rebuild this compact derived table without touching authoritative archive facts.
CREATE TABLE lane_score_eligibility_next (
    match_id TEXT NOT NULL,
    perspective_participant_id INTEGER NOT NULL,
    derivation_version TEXT NOT NULL,
    score_ready INTEGER NOT NULL CHECK (score_ready IN (0, 1)),
    exclusion_reason TEXT
        CHECK (exclusion_reason IS NULL OR exclusion_reason IN (
            'UNSUPPORTED_MAP',
            'UNSUPPORTED_QUEUE',
            'UNSUPPORTED_ROLE',
            'REMAKE',
            'GAME_TOO_SHORT',
            'ABNORMAL_EARLY_END',
            'OPPONENT_UNAVAILABLE',
            'FACTS_INCOMPLETE',
            'RULESET_UNSUPPORTED'
        )),
    cutoff_timestamp_ms INTEGER,
    cutoff_reason TEXT
        CHECK (cutoff_reason IS NULL OR cutoff_reason IN (
            'HERALD',
            'FALLBACK_14',
            'SWIFTPLAY_FIXED_14',
            'RULESET_CAP'
        )),
    evaluated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((score_ready = 1 AND exclusion_reason IS NULL) OR
           (score_ready = 0 AND exclusion_reason IS NOT NULL)),
    PRIMARY KEY (match_id, perspective_participant_id, derivation_version),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE
);

INSERT INTO lane_score_eligibility_next (
    match_id, perspective_participant_id, derivation_version, score_ready,
    exclusion_reason, cutoff_timestamp_ms, cutoff_reason, evaluated_at
)
SELECT
    match_id, perspective_participant_id, derivation_version, score_ready,
    exclusion_reason, cutoff_timestamp_ms, cutoff_reason, evaluated_at
FROM lane_score_eligibility;

DROP TABLE lane_score_eligibility;
ALTER TABLE lane_score_eligibility_next RENAME TO lane_score_eligibility;

CREATE INDEX idx_lane_score_eligibility_summary
    ON lane_score_eligibility(derivation_version, score_ready, exclusion_reason);
