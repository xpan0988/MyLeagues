import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AnalyticsFilter,
  CareerDto,
  ChampionProfileDto,
  ChampionSummaryDto,
  ClientStateDto,
  HomeDto,
  MatchQuery,
  MatchDetailDto,
  MatchSummaryDto,
  SettingsDto,
  SyncStateDto,
  UpdateSettingsInput,
} from "./types";

export interface PageDto<T> {
  items: T[];
  nextCursor: string | null;
}

export const backend = {
  getHome: () => invoke<HomeDto>("get_home"),
  listChampions: (filter: AnalyticsFilter) =>
    invoke<ChampionSummaryDto[]>("list_champions", { filter }),
  getChampionProfile: (championId: number, filter: AnalyticsFilter) =>
    invoke<ChampionProfileDto>("get_champion_profile", {
      championId,
      filter,
    }),
  listMatches: (query: MatchQuery) =>
    invoke<PageDto<MatchSummaryDto>>("list_matches", { query }),
  getMatchDetail: (matchId: string) =>
    invoke<MatchDetailDto>("get_match_detail", { matchId }),
  getCareer: (filter: AnalyticsFilter) =>
    invoke<CareerDto>("get_career", { filter }),
  getSettings: () => invoke<SettingsDto>("get_settings"),
  updateSettings: (settings: UpdateSettingsInput) =>
    invoke<SettingsDto>("update_settings", { settings }),
  getClientState: () => invoke<ClientStateDto>("get_client_state"),
  launchClient: () => invoke<ClientStateDto>("launch_client"),
  startSync: () => invoke<SyncStateDto>("start_sync"),
  requestFreshnessCheck: (trigger: "periodic" | "resume") => invoke<SyncStateDto>("request_freshness_check", { trigger }),
  getSyncState: () => invoke<SyncStateDto>("get_sync_state"),
  rebuildAggregates: () => invoke<void>("rebuild_aggregates"),
  clearStaticCache: () => invoke<void>("clear_static_cache"),
  resetLocalArchive: () => invoke<void>("reset_local_archive"),
  onSyncStateChanged: (handler: (state: SyncStateDto) => void): Promise<UnlistenFn> =>
    listen<SyncStateDto>("sync-state-changed", (event) => handler(event.payload)),
  onTimelineFactsChanged: (handler: () => void): Promise<UnlistenFn> =>
    listen("timeline-facts-changed", () => handler()),
} as const;
