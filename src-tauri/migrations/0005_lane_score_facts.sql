-- LaneScore fact revision 1 is intentionally independent from Timeline V1.
-- A completed timeline_sync_queue row only proves the legacy local @10 projection.
CREATE TABLE match_participants (
    match_id TEXT NOT NULL,
    participant_id INTEGER NOT NULL CHECK (participant_id BETWEEN 1 AND 10),
    puuid TEXT NOT NULL,
    team_id INTEGER NOT NULL,
    champion_id INTEGER NOT NULL,
    team_position TEXT NOT NULL DEFAULT '',
    individual_position TEXT NOT NULL DEFAULT '',
    captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (match_id, participant_id),
    UNIQUE (match_id, puuid),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE
);

CREATE TABLE lane_timeline_states (
    match_id TEXT NOT NULL,
    participant_id INTEGER NOT NULL CHECK (participant_id BETWEEN 1 AND 10),
    frame_timestamp_ms INTEGER NOT NULL CHECK (frame_timestamp_ms >= 0),
    lane_minions INTEGER NOT NULL CHECK (lane_minions >= 0),
    jungle_minions INTEGER NOT NULL CHECK (jungle_minions >= 0),
    total_gold INTEGER NOT NULL CHECK (total_gold >= 0),
    experience INTEGER NOT NULL CHECK (experience >= 0),
    level INTEGER NOT NULL CHECK (level >= 0),
    PRIMARY KEY (match_id, participant_id, frame_timestamp_ms),
    FOREIGN KEY (match_id, participant_id) REFERENCES match_participants(match_id, participant_id) ON DELETE CASCADE
);

CREATE TABLE lane_timeline_events (
    match_id TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL CHECK (timestamp_ms >= 0),
    event_type TEXT NOT NULL CHECK (event_type IN ('CHAMPION_KILL', 'TURRET_PLATE_DESTROYED', 'BUILDING_KILL', 'ELITE_MONSTER_KILL')),
    killer_participant_id INTEGER,
    victim_participant_id INTEGER,
    monster_type TEXT,
    monster_sub_type TEXT,
    building_type TEXT,
    tower_type TEXT,
    lane_type TEXT,
    x INTEGER,
    y INTEGER,
    PRIMARY KEY (match_id, source_event_id),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE
);

CREATE TABLE lane_timeline_event_participants (
    match_id TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    participant_id INTEGER NOT NULL CHECK (participant_id BETWEEN 1 AND 10),
    relation TEXT NOT NULL CHECK (relation IN ('killer', 'victim', 'assistant')),
    PRIMARY KEY (match_id, source_event_id, participant_id, relation),
    FOREIGN KEY (match_id, source_event_id) REFERENCES lane_timeline_events(match_id, source_event_id) ON DELETE CASCADE
);

CREATE TABLE lane_analysis_queue (
    match_id TEXT NOT NULL,
    puuid TEXT NOT NULL,
    fact_revision INTEGER NOT NULL DEFAULT 1 CHECK (fact_revision = 1),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'fetching', 'complete', 'error', 'unsupported')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    discovered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (match_id, puuid, fact_revision),
    FOREIGN KEY (match_id, puuid) REFERENCES player_matches(match_id, puuid) ON DELETE CASCADE
);

CREATE TABLE lane_opponent_mappings (
    match_id TEXT NOT NULL,
    perspective_participant_id INTEGER NOT NULL,
    opponent_participant_id INTEGER,
    confidence TEXT NOT NULL CHECK (confidence IN ('HIGH', 'MEDIUM', 'LOW', 'UNAVAILABLE')),
    derivation_version TEXT NOT NULL,
    PRIMARY KEY (match_id, perspective_participant_id, derivation_version),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE
);

CREATE TABLE lane_phase_derivations (
    match_id TEXT NOT NULL,
    perspective_participant_id INTEGER NOT NULL,
    derivation_version TEXT NOT NULL,
    end_timestamp_ms INTEGER NOT NULL,
    end_reason TEXT NOT NULL,
    PRIMARY KEY (match_id, perspective_participant_id, derivation_version),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE
);

CREATE TABLE lane_checkpoints (
    match_id TEXT NOT NULL,
    perspective_participant_id INTEGER NOT NULL,
    derivation_version TEXT NOT NULL,
    checkpoint TEXT NOT NULL,
    frame_timestamp_ms INTEGER NOT NULL,
    PRIMARY KEY (match_id, perspective_participant_id, derivation_version, checkpoint),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE
);

CREATE TABLE lane_combat_clusters (
    match_id TEXT NOT NULL,
    perspective_participant_id INTEGER NOT NULL,
    opponent_participant_id INTEGER NOT NULL,
    derivation_version TEXT NOT NULL,
    cluster_id TEXT NOT NULL,
    start_timestamp_ms INTEGER NOT NULL,
    end_timestamp_ms INTEGER NOT NULL,
    classification TEXT NOT NULL,
    signed_strength REAL NOT NULL,
    source_event_ids_json TEXT NOT NULL,
    PRIMARY KEY (match_id, perspective_participant_id, opponent_participant_id, derivation_version, cluster_id),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE
);

CREATE TABLE lane_score_cache (
    match_id TEXT NOT NULL,
    perspective_participant_id INTEGER NOT NULL,
    opponent_participant_id INTEGER NOT NULL,
    model_version TEXT NOT NULL,
    feature_schema_version TEXT NOT NULL,
    derivation_version TEXT NOT NULL,
    ruleset_version TEXT NOT NULL,
    parameter_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('ready', 'insufficient_evidence', 'unsupported')),
    score REAL,
    exp REAL,
    combat REAL,
    farm REAL,
    pressure REAL,
    conversion REAL,
    coverage_json TEXT NOT NULL,
    gold_consistency TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (match_id, perspective_participant_id, opponent_participant_id, model_version, feature_schema_version, derivation_version, ruleset_version, parameter_hash),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE
);

CREATE INDEX idx_lane_analysis_queue_pending ON lane_analysis_queue(puuid, status, discovered_at);
CREATE INDEX idx_lane_timeline_states_match_frame ON lane_timeline_states(match_id, frame_timestamp_ms);
CREATE INDEX idx_lane_timeline_events_match_time ON lane_timeline_events(match_id, timestamp_ms);
