-- Preserve event-side provenance needed for payload validation and conservative
-- structural derivations. Older fact rows remain valid with NULLs.
ALTER TABLE lane_timeline_events ADD COLUMN team_id INTEGER;
