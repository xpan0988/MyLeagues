-- Minimal normalized Timeline item facts. These support only reproducible
-- first-three completed-core-item paths; raw Timeline JSON remains unstored.
CREATE TABLE timeline_item_events (
    match_id TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL CHECK (timestamp_ms >= 0),
    participant_id INTEGER NOT NULL CHECK (participant_id BETWEEN 1 AND 10),
    event_type TEXT NOT NULL CHECK (event_type IN ('ITEM_PURCHASED', 'ITEM_SOLD', 'ITEM_UNDO', 'ITEM_DESTROYED')),
    item_id INTEGER,
    before_item_id INTEGER,
    after_item_id INTEGER,
    PRIMARY KEY (match_id, source_event_id),
    FOREIGN KEY (match_id) REFERENCES matches(match_id) ON DELETE CASCADE
);

CREATE INDEX idx_timeline_item_events_match_participant_time
    ON timeline_item_events(match_id, participant_id, timestamp_ms, source_event_id);
