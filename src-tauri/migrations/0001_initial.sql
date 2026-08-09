PRAGMA foreign_keys = ON;

CREATE TABLE accounts (
    puuid TEXT PRIMARY KEY NOT NULL,
    single_account_guard INTEGER NOT NULL DEFAULT 1 UNIQUE CHECK (single_account_guard = 1),
    game_name TEXT NOT NULL,
    tag_line TEXT NOT NULL,
    summoner_id TEXT,
    account_region TEXT NOT NULL,
    platform_region TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE matches (
    match_id TEXT PRIMARY KEY NOT NULL,
    game_creation INTEGER NOT NULL,
    game_end_timestamp INTEGER,
    game_duration INTEGER NOT NULL CHECK (game_duration >= 0),
    queue_id INTEGER NOT NULL,
    game_version TEXT NOT NULL,
    patch TEXT NOT NULL,
    season_key TEXT,
    ingested_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE player_matches (
    match_id TEXT NOT NULL,
    puuid TEXT NOT NULL,
    champion_id INTEGER NOT NULL,
    win INTEGER NOT NULL CHECK (win IN (0, 1)),
    kills INTEGER NOT NULL CHECK (kills >= 0),
    deaths INTEGER NOT NULL CHECK (deaths >= 0),
    assists INTEGER NOT NULL CHECK (assists >= 0),
    double_kills INTEGER NOT NULL DEFAULT 0 CHECK (double_kills >= 0),
    triple_kills INTEGER NOT NULL DEFAULT 0 CHECK (triple_kills >= 0),
    quadra_kills INTEGER NOT NULL DEFAULT 0 CHECK (quadra_kills >= 0),
    penta_kills INTEGER NOT NULL DEFAULT 0 CHECK (penta_kills >= 0),
    total_minions_killed INTEGER NOT NULL DEFAULT 0 CHECK (total_minions_killed >= 0),
    neutral_minions_killed INTEGER NOT NULL DEFAULT 0 CHECK (neutral_minions_killed >= 0),
    gold_earned INTEGER NOT NULL DEFAULT 0 CHECK (gold_earned >= 0),
    summoner1_id INTEGER NOT NULL,
    summoner2_id INTEGER NOT NULL,
    keystone_id INTEGER,
    primary_style_id INTEGER,
    secondary_style_id INTEGER,
    PRIMARY KEY (match_id, puuid),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE,
    FOREIGN KEY (puuid) REFERENCES accounts(puuid) ON DELETE CASCADE
);

CREATE TABLE player_match_items (
    match_id TEXT NOT NULL,
    puuid TEXT NOT NULL,
    item_id INTEGER NOT NULL,
    slot INTEGER NOT NULL CHECK (slot BETWEEN 0 AND 6),
    classification TEXT,
    PRIMARY KEY (match_id, puuid, slot),
    FOREIGN KEY (match_id, puuid) REFERENCES player_matches(match_id, puuid) ON DELETE CASCADE
);

CREATE TABLE player_match_runes (
    match_id TEXT NOT NULL,
    puuid TEXT NOT NULL,
    selection_type TEXT NOT NULL CHECK (selection_type IN ('primary', 'secondary', 'stat_shard')),
    slot INTEGER NOT NULL CHECK (slot >= 0),
    rune_id INTEGER NOT NULL,
    style_id INTEGER,
    PRIMARY KEY (match_id, puuid, selection_type, slot),
    FOREIGN KEY (match_id, puuid) REFERENCES player_matches(match_id, puuid) ON DELETE CASCADE
);

CREATE TABLE champion_mastery (
    puuid TEXT NOT NULL,
    champion_id INTEGER NOT NULL,
    mastery_level INTEGER NOT NULL CHECK (mastery_level >= 0),
    mastery_points INTEGER NOT NULL CHECK (mastery_points >= 0),
    last_play_time INTEGER,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (puuid, champion_id),
    FOREIGN KEY (puuid) REFERENCES accounts(puuid) ON DELETE CASCADE
);

CREATE TABLE rank_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    puuid TEXT NOT NULL,
    queue_type TEXT NOT NULL,
    tier TEXT NOT NULL,
    rank_division TEXT NOT NULL,
    league_points INTEGER NOT NULL,
    wins INTEGER NOT NULL,
    losses INTEGER NOT NULL,
    captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (puuid) REFERENCES accounts(puuid) ON DELETE CASCADE
);

CREATE TABLE sync_state (
    puuid TEXT PRIMARY KEY NOT NULL,
    status TEXT NOT NULL DEFAULT 'idle' CHECK (status IN ('idle', 'syncing', 'success', 'error')),
    next_match_start INTEGER NOT NULL DEFAULT 0 CHECK (next_match_start >= 0),
    initial_sync_complete INTEGER NOT NULL DEFAULT 0 CHECK (initial_sync_complete IN (0, 1)),
    newest_match_id TEXT,
    oldest_match_id TEXT,
    last_successful_sync_at TEXT,
    last_error TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (puuid) REFERENCES accounts(puuid) ON DELETE CASCADE
);

CREATE TABLE sync_match_queue (
    match_id TEXT NOT NULL,
    puuid TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'fetching', 'complete', 'error')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    discovered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (match_id, puuid),
    FOREIGN KEY (puuid) REFERENCES accounts(puuid) ON DELETE CASCADE
);

CREATE TABLE app_settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    game_name TEXT NOT NULL DEFAULT '',
    tag_line TEXT NOT NULL DEFAULT '',
    account_region TEXT NOT NULL DEFAULT 'sea',
    platform_region TEXT NOT NULL DEFAULT 'oc1',
    riot_client_path TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO app_settings (id) VALUES (1);

CREATE INDEX idx_matches_queue_patch_creation
    ON matches(queue_id, patch, game_creation DESC);
CREATE INDEX idx_player_matches_puuid_champion
    ON player_matches(puuid, champion_id);
CREATE INDEX idx_player_matches_puuid_win
    ON player_matches(puuid, win);
CREATE INDEX idx_player_match_items_item
    ON player_match_items(puuid, item_id);
CREATE INDEX idx_player_match_runes_rune
    ON player_match_runes(puuid, rune_id);
CREATE INDEX idx_rank_snapshots_latest
    ON rank_snapshots(puuid, queue_type, captured_at DESC);
CREATE INDEX idx_sync_match_queue_pending
    ON sync_match_queue(puuid, status, discovered_at);
