-- Derivation revision 1 changes only rebuildable products. Authoritative fact
-- revision 1 remains valid and can be re-derived locally without Riot refetches.
CREATE TABLE lane_derivation_queue (
    match_id TEXT NOT NULL,
    puuid TEXT NOT NULL,
    derivation_version TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'deriving', 'complete', 'error')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    discovered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (match_id, puuid, derivation_version),
    FOREIGN KEY (match_id, puuid) REFERENCES player_matches(match_id, puuid) ON DELETE CASCADE
);

CREATE TABLE lane_score_eligibility (
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
        CHECK (cutoff_reason IS NULL OR cutoff_reason IN ('HERALD', 'FALLBACK_14', 'RULESET_CAP')),
    evaluated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((score_ready = 1 AND exclusion_reason IS NULL) OR
           (score_ready = 0 AND exclusion_reason IS NOT NULL)),
    PRIMARY KEY (match_id, perspective_participant_id, derivation_version),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE
);

CREATE INDEX idx_lane_derivation_queue_pending
    ON lane_derivation_queue(puuid, derivation_version, status, discovered_at);
CREATE INDEX idx_lane_score_eligibility_summary
    ON lane_score_eligibility(derivation_version, score_ready, exclusion_reason);

INSERT OR IGNORE INTO lane_derivation_queue (match_id, puuid, derivation_version)
SELECT queue.match_id, queue.puuid, 'lane-derivation-v1-herald-cutoff'
FROM lane_analysis_queue queue
WHERE queue.fact_revision = 1
  AND queue.status = 'complete'
  AND EXISTS (SELECT 1 FROM match_participants participant WHERE participant.match_id = queue.match_id)
  AND EXISTS (SELECT 1 FROM lane_timeline_states state WHERE state.match_id = queue.match_id);
