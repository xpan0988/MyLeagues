export type QueueFilter = "all" | "rankedSolo" | "normal" | "aram";

export type TimeRangeFilter =
  | "currentPatch"
  | "currentSeason"
  | "allTracked";

export interface AnalyticsFilter {
  queue: QueueFilter;
  timeRange: TimeRangeFilter;
}

export type SyncStatus = "idle" | "checking" | "syncing" | "success" | "error";

export interface AccountDto {
  puuid: string;
  gameName: string;
  tagLine: string;
  accountRegion: string;
  platformRegion: string;
}

export interface ClientStateDto {
  riotClientRunning: boolean;
  leagueClientRunning: boolean;
  gameRunning: boolean;
  configuredExecutableFound: boolean;
}

export interface SyncStateDto {
  status: SyncStatus;
  currentlyRunning: boolean;
  trigger: string | null;
  completed: number;
  total: number | null;
  message: string | null;
  lastCheckAt: string | null;
  lastSuccessfulSyncAt: string | null;
}

export interface CoreBuildDto {
  items: GameEntityDto[];
  games: number;
  usageRate: number;
  winRate: number;
}

export interface GameEntityDto {
  id: number;
  name: string;
  icon: string;
}

export interface PreferenceDto {
  ids: number[];
  games: number;
  usageRate: number;
  winRate: number;
}

export interface ChampionSummaryDto {
  champion: GameEntityDto;
  masteryPoints: number | null;
  masteryLevel: number | null;
  trackedGames: number;
  wins: number;
  losses: number;
  winRate: number;
  playtimeSeconds: number;
  kills: number;
  deaths: number;
  assists: number;
  kda: number;
  mostUsedCoreBuild: CoreBuildDto | null;
  mostUsedKeystone: PreferenceDto | null;
}

export interface TrackedOverviewDto {
  games: number;
  wins: number;
  losses: number;
  winRate: number;
  playtimeSeconds: number;
  kills: number;
  deaths: number;
  assists: number;
  kda: number;
}

export interface ChampionProfileDto {
  champion: GameEntityDto;
  mastery: {
    points: number | null;
    level: number | null;
  };
  filterContext: {
    queue: QueueFilter;
    timeRange: TimeRangeFilter;
    currentPatch: string;
    currentSeason: string;
  };
  overview: TrackedOverviewDto;
  performance: {
    averageKills: number;
    averageDeaths: number;
    averageAssists: number;
    averageCsPerMinute: number;
    averageMatchDurationSeconds: number;
    highestKills: number;
    doubleKills: number;
    tripleKills: number;
    quadraKills: number;
    pentaKills: number;
  };
  laningAtTen: {
    eligibleGames: number;
    coveredGames: number;
    averageCsAtTen: number | null;
    averageCsPerMinuteAtTen: number | null;
    averageGoldAtTen: number | null;
    averageXpAtTen: number | null;
    averageLevelAtTen: number | null;
  };
  coreBuilds: Array<{
    items: GameEntityDto[];
    games: number;
    wins: number;
    usageRate: number;
    winRate: number;
  }>;
  boots: EntityUsageDto[];
  runePages: Array<{
    primaryStyle: GameEntityDto;
    primaryRunes: GameEntityDto[];
    secondaryStyle: GameEntityDto;
    secondaryRunes: GameEntityDto[];
    statShards: GameEntityDto[];
    games: number;
    wins: number;
    usageRate: number;
    winRate: number;
  }>;
  keystoneUsage: EntityUsageDto[];
  summonerSpellPairs: Array<{
    spells: GameEntityDto[];
    games: number;
    wins: number;
    usageRate: number;
    winRate: number;
  }>;
}

export interface EntityUsageDto {
  entity: GameEntityDto;
  games: number;
  wins: number;
  usageRate: number;
  winRate: number;
}

export interface MatchSummaryDto {
  matchId: string;
  champion: GameEntityDto;
  win: boolean;
  queueId: number;
  kills: number;
  deaths: number;
  assists: number;
  durationSeconds: number;
  keystone: GameEntityDto | null;
  summonerSpells: GameEntityDto[];
  gameCreation: string;
  patch: string;
}

export interface RunePageDto {
  primaryStyle: GameEntityDto | null;
  primaryRunes: GameEntityDto[];
  secondaryStyle: GameEntityDto | null;
  secondaryRunes: GameEntityDto[];
  statShards: GameEntityDto[];
}

export interface MatchDetailDto {
  matchId: string;
  champion: GameEntityDto;
  win: boolean;
  queueId: number;
  gameCreation: string;
  durationSeconds: number;
  patch: string;
  kills: number;
  deaths: number;
  assists: number;
  totalCs: number;
  goldEarned: number;
  summonerSpells: GameEntityDto[];
  runePage: RunePageDto;
  finalItems: Array<{ item: GameEntityDto; slot: number }>;
  doubleKills: number;
  tripleKills: number;
  quadraKills: number;
  pentaKills: number;
}

export interface HomeDto {
  account: AccountDto | null;
  rank: { tier: string; division: string; leaguePoints: number; wins: number; losses: number; winRate: number } | null;
  clientState: ClientStateDto;
  syncState: SyncStateDto;
  historicalSync: {
    matchesTracked: number;
    oldestTrackedAt: string | null;
    trackedPlaytimeSeconds: number;
    historyStatus: "Complete" | "Still backfilling" | "Interrupted" | "Not configured";
    nextMatchStart: number;
  };
  trackedCareer: TrackedOverviewDto;
  rankedGames: number;
  recentForm: boolean[];
  topChampions: ChampionSummaryDto[];
}

export interface CareerDto {
  overall: TrackedOverviewDto;
  byQueue: Record<Exclude<QueueFilter, "all">, TrackedOverviewDto>;
  averageMatchDurationSeconds: number;
  mostPlayedChampionId: number | null;
  championPool: number;
}

export interface SettingsDto {
  gameName: string;
  tagLine: string;
  accountRegion: string;
  platformRegion: string;
  riotClientPath: string | null;
  apiKeyConfigured: boolean;
  dataDragonVersion: string | null;
}

export interface UpdateSettingsInput {
  gameName: string;
  tagLine: string;
  accountRegion: string;
  platformRegion: string;
  riotClientPath: string | null;
}

export interface MatchQuery {
  queue: QueueFilter;
  timeRange: TimeRangeFilter;
  championId?: number;
  cursor?: string;
  limit?: number;
}
