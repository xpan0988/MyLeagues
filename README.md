# MyLeague

MyLeague is a private local-first Windows/macOS League of Legends launcher and offline-first career dashboard built with React, TypeScript, Vite, Tauri, Rust, and SQLite. It complements the official Riot Client; Riot still owns login, matchmaking, champion select, and gameplay.

## V1 capabilities

- Home, Champions, Champion Profile, Matches, Career, and Settings desktop views.
- Incremental Match-V5 synchronization with a persistent, resumable work queue.
- Automatic startup, settings-save, foreground-periodic, and resume freshness checks; Sync Now remains a manual check/retry.
- Persistent low-priority Match-V5 Timeline enrichment for laning @10 facts and honest coverage reporting.
- Normalized SQLite match facts plus rebuildable persistent career/champion aggregate caches.
- Queue and time filters resolved by Rust: Ranked Solo, Normal, ARAM, All; Current Patch, Current Season, All Tracked.
- Version-aware Data Dragon metadata cache for champions, items, rune trees, and summoner spells.
- Full rune-page, keystone, spell-pair, boots, and canonical Core Build Combination analytics.
- Offline browsing when Riot API access or the network is unavailable.
- Official Riot/League process detection and safe Riot Client launch.

LANING @10 is factual analytics. An explainable LaneScore is architected but
not implemented; the current UI does not claim lane-win or crushed-lane
results. See [LaneScore Architecture v0](docs/lane-score-architecture.md).

## Setup

Requirements: macOS or Windows, Node.js/npm, the platform Tauri toolchain, and a Riot API key.

```powershell
npm.cmd install
$env:RIOT_API_KEY = "your-development-key"
npm.cmd run tauri dev
```

Open Settings and configure the single Riot ID, account routing region, platform region, and optionally the full path to `RiotClientServices.exe`. `RIOT_API_KEY` is read only by Rust; never expose it through a `VITE_` variable.

## Verification

```powershell
npm.cmd run build
cargo test --manifest-path src-tauri\Cargo.toml --locked
npm.cmd run tauri build -- --debug --no-bundle
```

The SQLite database and static metadata cache live under the Tauri application data directory. See [architecture](docs/architecture.md) for ownership, rebuild, filter, and synchronization semantics.

## Safety boundary

V1 does **not** implement LCU or lockfile authentication, champion-select/lobby control, rune or spell automation, opponent scouting, overlay, Lane Advantage Rate, AI coaching, memory reading, DLL/process injection, Vanguard interaction, password storage, cloud sync, or multi-account support.

Official references: [Riot Developer Portal](https://developer.riotgames.com/docs/lol), [Riot API reference](https://developer.riotgames.com/apis), [Data Dragon versions](https://ddragon.leagueoflegends.com/api/versions.json).
