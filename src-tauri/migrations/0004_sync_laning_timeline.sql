-- Sync attempts are retained independently of the in-memory worker so a restart
-- can still report freshness from the local archive.
ALTER TABLE sync_state ADD COLUMN last_check_at TEXT;
ALTER TABLE sync_state ADD COLUMN last_trigger TEXT;

-- Participant IDs are the authoritative bridge between Match-V5 participants
-- and Timeline-V5 participantFrames. Older archived matches leave this NULL;
-- their timeline metadata is used as a conservative fallback.
ALTER TABLE player_matches ADD COLUMN participant_id INTEGER;

CREATE TABLE match_laning_snapshots (
    match_id TEXT NOT NULL,
    puuid TEXT NOT NULL,
    minute INTEGER NOT NULL CHECK (minute = 10),
    frame_timestamp_ms INTEGER NOT NULL CHECK (frame_timestamp_ms >= 600000),
    lane_minions INTEGER NOT NULL CHECK (lane_minions >= 0),
    neutral_minions INTEGER NOT NULL CHECK (neutral_minions >= 0),
    total_gold INTEGER NOT NULL CHECK (total_gold >= 0),
    experience INTEGER NOT NULL CHECK (experience >= 0),
    level INTEGER NOT NULL CHECK (level >= 0),
    captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (match_id, puuid, minute),
    FOREIGN KEY (match_id, puuid) REFERENCES player_matches(match_id, puuid) ON DELETE CASCADE
);

CREATE TABLE timeline_sync_queue (
    match_id TEXT NOT NULL,
    puuid TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'fetching', 'complete', 'error', 'unsupported')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    discovered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (match_id, puuid),
    FOREIGN KEY (match_id, puuid) REFERENCES player_matches(match_id, puuid) ON DELETE CASCADE
);

CREATE INDEX idx_timeline_sync_queue_pending
    ON timeline_sync_queue(puuid, status, discovered_at);
CREATE INDEX idx_match_laning_snapshots_puuid
    ON match_laning_snapshots(puuid, match_id);
