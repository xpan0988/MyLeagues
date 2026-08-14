# MyLeague V1 architecture

## Dependency direction

```text
React UI -> typed Tauri wrapper -> thin command -> service
                                             |-> repository -> SQLite
                                             |-> Riot adapter -> official Web API
                                             |-> launcher -> Windows process/executable
                                   stable DTO <- domain model
```

React renders, routes, sorts, charts, and manages interaction state. It never calls Riot, SQLite, or platform processes and never receives the API key. Statistics services never call Riot: they operate from SQLite and remain usable offline.

## Source of truth and aggregate cache

Normalized `matches`, `player_matches`, `player_match_items`, and `player_match_runes` are permanent facts. `career_aggregates` and `champion_aggregates` are disposable performance caches. They store additive counters and `MAX(highest_kills)` across real queue, patch, and season dimensions; win rate, KDA, and averages remain derived values.

A genuinely new match is ingested in one immediate SQLite transaction: match fact, player fact, items, runes, relevant aggregate increments, and persistent sync-queue completion. A duplicate `match_id` skips fact and aggregate insertion, preventing double counts. Any failure rolls the whole transaction back. `rebuild_aggregates()` clears and reconstructs both caches from normalized facts in one transaction. Recent Form is a sliding normalized-fact query and is not materialized.

## Synchronization lifecycle

Startup opens/migrates SQLite and reads settings before it starts any network work; it only starts when both Riot ID fields and a backend API key are available. Home renders SQLite immediately. Each attempt records a non-secret trigger (`startup`, `settings_saved`, `periodic`, `resume`, `manual`, or `archive_reset`), enters `checking`, resolves the Riot ID to PUUID, then enters `syncing`. A single-flight coordinator coalesces overlapping requests.

Saving a configured Riot ID starts a background check. While the desktop UI is foregrounded, it asks for a freshness check every five minutes and on focus/resume; the backend skips it when the previous check is still fresh or a job is running. `Sync Now` uses this same worker but bypasses the freshness interval as a manual check/retry. A failed attempt leaves offline data usable and is eligible for a later periodic, resume, settings, or manual retry.

Riot ID resolves to PUUID; PUUID is the single account primary identity. Initial discovery pages Match IDs into `sync_match_queue`; each completed match stays completed, and interrupted `fetching` rows resume on restart. Incremental sync stops when it reaches a known match. Match ingestion is transactional per match, so one failed download does not erase successful prior work.

Match detail fetching claims at most five persistent queue rows atomically and performs those network requests concurrently. The shared Riot limiter enforces both the 20-request/second and 100-request/120-second windows; SQLite ingestion remains serialized and transactional per match. Progress events are batch-based and throttled to 500 ms, while final completion is always emitted. Structured batch diagnostics record network, limiter wait, backoff, JSON decode, normalized parsing, database, aggregate UPSERT, queue-update, and throughput timings. Static metadata is not loaded or parsed in the ingestion path.

`last_check_at`, `last_successful_sync_at`, trigger, progress, and the useful final error are represented in sync state. Successful summary sync invalidates only Home, Champions, Champion Profile, Matches, and Career query families; timeline completion emits one separate archive-refresh event for those same families rather than invalidating per match.

## Timeline facts and laning @10

Timeline enrichment uses Match-V5 `GET /lol/match/v5/matches/{matchId}/timeline` on the account's Match-V5 regional route. It reads `info.frameInterval`, `info.frames[].timestamp`, and `participantFrames[participantId]` fields `totalGold`, `xp`, `level`, `minionsKilled`, and `jungleMinionsKilled`. The normal Match-V5 participant's `participantId` is stored when available; pre-migration facts use the timeline metadata participant order as a guarded fallback.

`match_laning_snapshots` is a compact normalized source-of-truth fact table rather than a raw timeline JSON cache. Its V1 snapshot is minute 10: `lane_minions = minionsKilled`, `neutral_minions = jungleMinionsKilled`, `total_gold = totalGold`, XP, and level. **CS @10 means lane-minion CS only**; jungle monsters are persisted separately and never silently merged. The frame rule is exact `timestamp == 600000`; if a payload omits that boundary, the first later frame is used, while a timeline with no frame at or after 10:00 remains an explicit coverage gap. Events occurring at exactly 10:00 are not replayed: the selected participant frame is the authoritative snapshot boundary.

Eligible V1 enrichment is only Summoner's Rift Ranked Solo (`420`) and current Normal classifications (`400`, `430`) with a duration of at least 600 seconds. ARAM, Arena, and other queues never enter the queue. `timeline_sync_queue` is persistent and idempotent. Summary ingestion completes first, then a single low-priority timeline worker fetches one timeline at a time through the same global Riot limiter; it exits between jobs when a higher-priority summary sync begins and resumes later. Coverage is therefore honest (`covered / eligible`) while historical backfill is incomplete.

Champion Profile derives laning averages directly from those facts under the existing Champion × Queue × Time Range predicates. This avoids rebuilding aggregate caches after each timeline insertion at the current archive size. Lane-opponent matching and differential metrics are intentionally not implemented: future work should require compatible team/individual position evidence and treat ambiguous mappings as unavailable rather than assuming enemy TOP is the opponent.

## Future LaneScore architecture

Current top-lane analytics are factual LANING @10 data only. The future path is
normalized Match/Timeline facts → versioned lane derivations → a rebuildable,
explainable LaneScore cache → presentation. LaneScore, lane-win categories, and
lane-crushing results are not implemented. The frozen design and its fact,
coverage, model, ruleset-versioning, and calibration requirements are in
[LaneScore Architecture v0](lane-score-architecture.md).

## Reset Local Archive

Reset Local Archive deletes reconstructable Riot-derived facts: matches and child facts, masteries, rank snapshots, aggregate caches, both sync queues, sync history, and timeline snapshots. It intentionally preserves `accounts`, `app_settings`, Riot ID/routing, client path, user preferences, and API-key configuration (which is not stored in SQLite). When an API key is available it starts the normal `archive_reset` sync pipeline automatically.

## Data Dragon

The static-data service checks `versions.json`, reuses a locally cached version when present, and transactionally caches raw JSON plus normalized champion/item/rune/style/spell metadata. A version change creates the corresponding versioned rows and activates the new version. Match facts keep numeric IDs only; DTO enrichment adds names and official CDN icon URLs. If Data Dragon is unavailable, cached facts and metadata continue to work.

Item semantics live in one classification module. It uses Data Dragon tags, gold/purchasable/from/into metadata, and narrowly documented exceptions to classify boots, trinkets, consumables, components, completed items, and valid core items.

Data Dragon `runesReforged.json` does not contain the 500x stat-shard catalog used by Match-V5 perk pages. Stat shards therefore use a dedicated backend compatibility table in `domain/static_data.rs`, limited strictly to shard IDs and official static asset URLs. Known shards are enriched before IPC; unknown values use `Unknown Shard (<id>)`. They are never treated as ordinary rune-tree selections.

## Core Build Paths

Champion Core Build Paths are the most common ordered first-three completed
core-item paths across the selected matches; they are not a latest-match view.
They derive from normalized Timeline `ITEM_PURCHASED`, `ITEM_UNDO`,
`ITEM_SOLD`, and `ITEM_DESTROYED` facts, never Match-V5 final inventory slots.
Item metadata excludes low-cost starter items, boots, trinkets, consumables,
wards, and components.
An undo removes the matching latest purchase from the path; a later sale keeps
the historical completion. `[A, B, C]` and `[B, A, C]` are different paths.
Boots remain a separate final-inventory usage statistic.

## Runes and filters

A rune-page grouping key contains primary style, ordered primary selections, secondary style, ordered secondary selections, and ordered stat shards. Same keystone with different secondary runes is a different full page. Keystone usage is an additional, separate aggregate.

Queue filters are centralized in Rust: Ranked Solo `420`; Normal `400/430`; ARAM `450`; All. Current Patch resolves from the active Data Dragon major/minor version, falling back to the newest local match. Current Season uses the UTC calendar year as the explicit centralized V1 rule because Match-V5 does not expose a stable universal season integer. All Tracked has no time predicate.

## IPC contracts

`ChampionProfile` returns enriched champion/mastery, resolved filter context, tracked overview, performance, canonical core builds, boots, full rune pages, keystone usage, and spell pairs. Match list is database-paginated and Match Detail returns only the configured user's normalized participant data. Database rows are never exposed directly.

## Riot Client boundary

The launcher checks saved configuration and platform-appropriate common locations. Windows process detection remains Windows-only; macOS never tries to execute `tasklist.exe`. PLAY launches only the configured official client executable. If Riot is already open, the user may select League in the official client. There is no UI clicking, password storage, undocumented client control, LCU, or lockfile access.

## Project layout

```text
src/{app,components,features,lib,routes}
src-tauri/migrations
src-tauri/src/{commands,config,db,domain,dto,riot,services}
```

Commands stay transport-thin; services orchestrate; repositories own SQL; Riot/Data Dragon adapters own HTTP; domain types and stable DTOs keep those boundaries explicit.
