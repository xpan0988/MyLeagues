-- Persist per-event equal-share Combat attribution for explainable LaneScore
-- diagnostics. Existing normalized Riot event facts remain authoritative.
ALTER TABLE lane_combat_clusters
    ADD COLUMN attribution_json TEXT NOT NULL DEFAULT '[]';
