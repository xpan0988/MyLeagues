CREATE TABLE static_data_versions (
    version TEXT PRIMARY KEY NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)),
    fetched_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_static_data_active_version
    ON static_data_versions(is_active) WHERE is_active = 1;

CREATE TABLE static_champions (
    version TEXT NOT NULL,
    champion_id INTEGER NOT NULL,
    champion_key TEXT NOT NULL,
    name TEXT NOT NULL,
    icon TEXT NOT NULL,
    PRIMARY KEY (version, champion_id),
    FOREIGN KEY (version) REFERENCES static_data_versions(version) ON DELETE CASCADE
);

CREATE TABLE static_items (
    version TEXT NOT NULL,
    item_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    icon TEXT NOT NULL,
    gold INTEGER NOT NULL,
    purchasable INTEGER NOT NULL CHECK (purchasable IN (0, 1)),
    tags_json TEXT NOT NULL,
    from_json TEXT NOT NULL,
    into_json TEXT NOT NULL,
    maps_json TEXT NOT NULL,
    classification TEXT NOT NULL CHECK (classification IN ('boot', 'trinket', 'consumable', 'component', 'core', 'other')),
    PRIMARY KEY (version, item_id),
    FOREIGN KEY (version) REFERENCES static_data_versions(version) ON DELETE CASCADE
);

CREATE TABLE static_rune_styles (
    version TEXT NOT NULL,
    style_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    icon TEXT NOT NULL,
    PRIMARY KEY (version, style_id),
    FOREIGN KEY (version) REFERENCES static_data_versions(version) ON DELETE CASCADE
);

CREATE TABLE static_runes (
    version TEXT NOT NULL,
    rune_id INTEGER NOT NULL,
    style_id INTEGER NOT NULL,
    slot_order INTEGER NOT NULL,
    name TEXT NOT NULL,
    icon TEXT NOT NULL,
    PRIMARY KEY (version, rune_id),
    FOREIGN KEY (version, style_id) REFERENCES static_rune_styles(version, style_id) ON DELETE CASCADE
);

CREATE TABLE static_summoner_spells (
    version TEXT NOT NULL,
    spell_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    icon TEXT NOT NULL,
    PRIMARY KEY (version, spell_id),
    FOREIGN KEY (version) REFERENCES static_data_versions(version) ON DELETE CASCADE
);

CREATE TABLE static_payload_cache (
    version TEXT NOT NULL,
    payload_kind TEXT NOT NULL,
    json TEXT NOT NULL,
    PRIMARY KEY (version, payload_kind),
    FOREIGN KEY (version) REFERENCES static_data_versions(version) ON DELETE CASCADE
);

CREATE INDEX idx_static_items_active_lookup ON static_items(item_id, version);
CREATE INDEX idx_static_runes_active_lookup ON static_runes(rune_id, version);

