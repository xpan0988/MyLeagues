# MyLeague V1 architecture

## Dependency direction

```text
React UI -> typed Tauri wrapper -> thin command -> service
                                             |-> repository -> SQLite
                                             |-> Riot adapter -> official Web API
                                             |-> launcher -> Windows process/executable
                                   stable DTO <- domain model
```

React renders, routes, sorts, charts, and manages interaction state. It never calls Riot, SQLite, or Windows processes and never receives the API key. Statistics services never call Riot: they operate from SQLite and remain usable offline.

## Source of truth and aggregate cache

Normalized `matches`, `player_matches`, `player_match_items`, and `player_match_runes` are permanent facts. `career_aggregates` and `champion_aggregates` are disposable performance caches. They store additive counters and `MAX(highest_kills)` across real queue, patch, and season dimensions; win rate, KDA, and averages remain derived values.

A genuinely new match is ingested in one immediate SQLite transaction: match fact, player fact, items, runes, relevant aggregate increments, and persistent sync-queue completion. A duplicate `match_id` skips fact and aggregate insertion, preventing double counts. Any failure rolls the whole transaction back. `rebuild_aggregates()` clears and reconstructs both caches from normalized facts in one transaction. Recent Form is a sliding normalized-fact query and is not materialized.

## Synchronization lifecycle

Startup opens/migrates SQLite first, renders cached data, then starts network work. Riot ID resolves to PUUID; PUUID is the single account primary identity. Initial discovery pages Match IDs into `sync_match_queue`; each completed match stays completed, and interrupted `fetching` rows resume on restart. Incremental sync stops when it reaches a known match. Match ingestion is transactional per match, so one failed download does not erase successful prior work.

Match detail fetching claims at most five persistent queue rows atomically and performs those network requests concurrently. The shared Riot limiter enforces both the 20-request/second and 100-request/120-second windows; SQLite ingestion remains serialized and transactional per match. Progress events are batch-based and throttled to 500 ms, while final completion is always emitted. Structured batch diagnostics record network, limiter wait, backoff, JSON decode, normalized parsing, database, aggregate UPSERT, queue-update, and throughput timings. Static metadata is not loaded or parsed in the ingestion path.

## Data Dragon

The static-data service checks `versions.json`, reuses a locally cached version when present, and transactionally caches raw JSON plus normalized champion/item/rune/style/spell metadata. A version change creates the corresponding versioned rows and activates the new version. Match facts keep numeric IDs only; DTO enrichment adds names and official CDN icon URLs. If Data Dragon is unavailable, cached facts and metadata continue to work.

Item semantics live in one classification module. It uses Data Dragon tags, gold/purchasable/from/into metadata, and narrowly documented exceptions to classify boots, trinkets, consumables, components, completed items, and valid core items.

Data Dragon `runesReforged.json` does not contain the 500x stat-shard catalog used by Match-V5 perk pages. Stat shards therefore use a dedicated backend compatibility table in `domain/static_data.rs`, limited strictly to shard IDs and official static asset URLs. Known shards are enriched before IPC; unknown values use `Unknown Shard (<id>)`. They are never treated as ordinary rune-tree selections.

## Core Build Combination

V1 reads final inventory only; it does not fetch Match Timeline and never claims purchase order. It removes empty slots, trinkets, consumables, boots, and obvious components, retains at most three valid completed/core items, sorts IDs into deterministic canonical order, and groups that combination. `[A, B, C]` and `[B, C, A]` are identical. Boots are excluded and analyzed separately. UI displays item tiles without arrows.

## Runes and filters

A rune-page grouping key contains primary style, ordered primary selections, secondary style, ordered secondary selections, and ordered stat shards. Same keystone with different secondary runes is a different full page. Keystone usage is an additional, separate aggregate.

Queue filters are centralized in Rust: Ranked Solo `420`; Normal `400/430`; ARAM `450`; All. Current Patch resolves from the active Data Dragon major/minor version, falling back to the newest local match. Current Season uses the UTC calendar year as the explicit centralized V1 rule because Match-V5 does not expose a stable universal season integer. All Tracked has no time predicate.

## IPC contracts

`ChampionProfile` returns enriched champion/mastery, resolved filter context, tracked overview, performance, canonical core builds, boots, full rune pages, keystone usage, and spell pairs. Match list is database-paginated and Match Detail returns only the configured user's normalized participant data. Database rows are never exposed directly.

## Riot Client boundary

The launcher checks saved configuration, common Windows install locations, and then manual configuration. It enumerates Riot, League, and game process names at a low frequency. PLAY returns without duplicating an existing Riot/League process; otherwise it launches only the official `RiotClientServices.exe`. If Riot is already open, the user may select League in the official client. There is no UI clicking, password storage, undocumented client control, LCU, or lockfile access.

## Project layout

```text
src/{app,components,features,lib,routes}
src-tauri/migrations
src-tauri/src/{commands,config,db,domain,dto,riot,services}
```

Commands stay transport-thin; services orchestrate; repositories own SQL; Riot/Data Dragon adapters own HTTP; domain types and stable DTOs keep those boundaries explicit.
