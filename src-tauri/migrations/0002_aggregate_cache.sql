CREATE TABLE career_aggregates (
    puuid TEXT NOT NULL,
    queue_scope INTEGER NOT NULL,
    patch TEXT NOT NULL DEFAULT '',
    season TEXT NOT NULL DEFAULT '',
    games INTEGER NOT NULL DEFAULT 0 CHECK (games >= 0),
    wins INTEGER NOT NULL DEFAULT 0 CHECK (wins >= 0),
    losses INTEGER NOT NULL DEFAULT 0 CHECK (losses >= 0),
    kills INTEGER NOT NULL DEFAULT 0 CHECK (kills >= 0),
    deaths INTEGER NOT NULL DEFAULT 0 CHECK (deaths >= 0),
    assists INTEGER NOT NULL DEFAULT 0 CHECK (assists >= 0),
    playtime_seconds INTEGER NOT NULL DEFAULT 0 CHECK (playtime_seconds >= 0),
    double_kills INTEGER NOT NULL DEFAULT 0 CHECK (double_kills >= 0),
    triple_kills INTEGER NOT NULL DEFAULT 0 CHECK (triple_kills >= 0),
    quadra_kills INTEGER NOT NULL DEFAULT 0 CHECK (quadra_kills >= 0),
    penta_kills INTEGER NOT NULL DEFAULT 0 CHECK (penta_kills >= 0),
    total_minions_killed INTEGER NOT NULL DEFAULT 0 CHECK (total_minions_killed >= 0),
    neutral_minions_killed INTEGER NOT NULL DEFAULT 0 CHECK (neutral_minions_killed >= 0),
    gold_earned INTEGER NOT NULL DEFAULT 0 CHECK (gold_earned >= 0),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (patch = '' OR season = ''),
    PRIMARY KEY (puuid, queue_scope, patch, season),
    FOREIGN KEY (puuid) REFERENCES accounts(puuid) ON DELETE CASCADE
);

CREATE TABLE champion_aggregates (
    puuid TEXT NOT NULL,
    champion_id INTEGER NOT NULL,
    queue_scope INTEGER NOT NULL,
    patch TEXT NOT NULL DEFAULT '',
    season TEXT NOT NULL DEFAULT '',
    games INTEGER NOT NULL DEFAULT 0 CHECK (games >= 0),
    wins INTEGER NOT NULL DEFAULT 0 CHECK (wins >= 0),
    losses INTEGER NOT NULL DEFAULT 0 CHECK (losses >= 0),
    kills INTEGER NOT NULL DEFAULT 0 CHECK (kills >= 0),
    deaths INTEGER NOT NULL DEFAULT 0 CHECK (deaths >= 0),
    assists INTEGER NOT NULL DEFAULT 0 CHECK (assists >= 0),
    playtime_seconds INTEGER NOT NULL DEFAULT 0 CHECK (playtime_seconds >= 0),
    double_kills INTEGER NOT NULL DEFAULT 0 CHECK (double_kills >= 0),
    triple_kills INTEGER NOT NULL DEFAULT 0 CHECK (triple_kills >= 0),
    quadra_kills INTEGER NOT NULL DEFAULT 0 CHECK (quadra_kills >= 0),
    penta_kills INTEGER NOT NULL DEFAULT 0 CHECK (penta_kills >= 0),
    total_minions_killed INTEGER NOT NULL DEFAULT 0 CHECK (total_minions_killed >= 0),
    neutral_minions_killed INTEGER NOT NULL DEFAULT 0 CHECK (neutral_minions_killed >= 0),
    gold_earned INTEGER NOT NULL DEFAULT 0 CHECK (gold_earned >= 0),
    highest_kills INTEGER NOT NULL DEFAULT 0 CHECK (highest_kills >= 0),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (patch = '' OR season = ''),
    PRIMARY KEY (puuid, champion_id, queue_scope, patch, season),
    FOREIGN KEY (puuid) REFERENCES accounts(puuid) ON DELETE CASCADE
);

WITH facts AS (
    SELECT pm.puuid, m.queue_id, m.patch, COALESCE(m.season_key, '') AS season,
           m.game_duration, pm.win, pm.kills, pm.deaths, pm.assists,
           pm.double_kills, pm.triple_kills, pm.quadra_kills, pm.penta_kills,
           pm.total_minions_killed, pm.neutral_minions_killed, pm.gold_earned
    FROM player_matches pm
    JOIN matches m ON m.match_id = pm.match_id
), scoped AS (
    SELECT *, -1 AS queue_scope, '' AS scope_patch, '' AS scope_season FROM facts
    UNION ALL SELECT *, CASE WHEN queue_id IN (400, 430) THEN -2 ELSE queue_id END, '', '' FROM facts
    UNION ALL SELECT *, -1, patch, '' FROM facts
    UNION ALL SELECT *, CASE WHEN queue_id IN (400, 430) THEN -2 ELSE queue_id END, patch, '' FROM facts
    UNION ALL SELECT *, -1, '', season FROM facts WHERE season <> ''
    UNION ALL SELECT *, CASE WHEN queue_id IN (400, 430) THEN -2 ELSE queue_id END, '', season FROM facts WHERE season <> ''
)
INSERT INTO career_aggregates (
    puuid, queue_scope, patch, season, games, wins, losses, kills, deaths, assists,
    playtime_seconds, double_kills, triple_kills, quadra_kills, penta_kills,
    total_minions_killed, neutral_minions_killed, gold_earned
)
SELECT puuid, queue_scope, scope_patch, scope_season, COUNT(*), SUM(win), SUM(1 - win),
       SUM(kills), SUM(deaths), SUM(assists), SUM(game_duration), SUM(double_kills),
       SUM(triple_kills), SUM(quadra_kills), SUM(penta_kills), SUM(total_minions_killed),
       SUM(neutral_minions_killed), SUM(gold_earned)
FROM scoped
GROUP BY puuid, queue_scope, scope_patch, scope_season;

WITH facts AS (
    SELECT pm.puuid, pm.champion_id, m.queue_id, m.patch,
           COALESCE(m.season_key, '') AS season, m.game_duration, pm.win, pm.kills,
           pm.deaths, pm.assists, pm.double_kills, pm.triple_kills, pm.quadra_kills,
           pm.penta_kills, pm.total_minions_killed, pm.neutral_minions_killed, pm.gold_earned
    FROM player_matches pm
    JOIN matches m ON m.match_id = pm.match_id
), scoped AS (
    SELECT *, -1 AS queue_scope, '' AS scope_patch, '' AS scope_season FROM facts
    UNION ALL SELECT *, CASE WHEN queue_id IN (400, 430) THEN -2 ELSE queue_id END, '', '' FROM facts
    UNION ALL SELECT *, -1, patch, '' FROM facts
    UNION ALL SELECT *, CASE WHEN queue_id IN (400, 430) THEN -2 ELSE queue_id END, patch, '' FROM facts
    UNION ALL SELECT *, -1, '', season FROM facts WHERE season <> ''
    UNION ALL SELECT *, CASE WHEN queue_id IN (400, 430) THEN -2 ELSE queue_id END, '', season FROM facts WHERE season <> ''
)
INSERT INTO champion_aggregates (
    puuid, champion_id, queue_scope, patch, season, games, wins, losses, kills,
    deaths, assists, playtime_seconds, double_kills, triple_kills, quadra_kills,
    penta_kills, total_minions_killed, neutral_minions_killed, gold_earned, highest_kills
)
SELECT puuid, champion_id, queue_scope, scope_patch, scope_season, COUNT(*), SUM(win),
       SUM(1 - win), SUM(kills), SUM(deaths), SUM(assists), SUM(game_duration),
       SUM(double_kills), SUM(triple_kills), SUM(quadra_kills), SUM(penta_kills),
       SUM(total_minions_killed), SUM(neutral_minions_killed), SUM(gold_earned), MAX(kills)
FROM scoped
GROUP BY puuid, champion_id, queue_scope, scope_patch, scope_season;

CREATE INDEX idx_career_aggregates_scope
    ON career_aggregates(puuid, queue_scope, patch, season);
CREATE INDEX idx_champion_aggregates_scope
    ON champion_aggregates(puuid, champion_id, queue_scope, patch, season);
