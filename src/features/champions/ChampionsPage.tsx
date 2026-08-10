import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { EntityIcon, Filters, formatDuration, QueryState } from "../../components/ui/DataDisplay";
import { PageHeader } from "../../components/ui/PageHeader";
import { backend } from "../../lib/tauri";
import type { AnalyticsFilter, ChampionSummaryDto } from "../../lib/types";

type SortKey = "games" | "mastery" | "winRate" | "kills" | "playtime";
const initialFilter: AnalyticsFilter = { queue: "rankedSolo", timeRange: "currentSeason" };

export function ChampionsPage() {
  const [filter, setFilter] = useState(initialFilter);
  const [sort, setSort] = useState<SortKey>("games");
  const query = useQuery({ queryKey: ["champions", filter], queryFn: () => backend.listChampions(filter) });
  const rows = useMemo(() => [...(query.data ?? [])].sort((a, b) => sortValue(b, sort) - sortValue(a, sort)), [query.data, sort]);
  return <div className="page"><PageHeader eyebrow="TRACKED CAREER" title="Champions" description="Mastery and normalized match history are shown as separate measures." />
    <div className="toolbar"><Filters value={filter} onChange={setFilter} /><label>Sort <select value={sort} onChange={(event) => setSort(event.target.value as SortKey)}><option value="games">Most Played</option><option value="mastery">Mastery</option><option value="winRate">Win Rate</option><option value="kills">Kills</option><option value="playtime">Playtime</option></select></label></div>
    <QueryState loading={query.isPending} error={query.error} empty={!rows.length}><div className="champion-list">{rows.map((row) => <ChampionRow key={row.champion.id} row={row} />)}</div></QueryState>
  </div>;
}

function ChampionRow({ row }: { row: ChampionSummaryDto }) {
  return <Link className="champion-row" to={`/champions/${row.champion.id}`}><div className="champion-identity"><EntityIcon entity={row.champion} size={52} /><span><strong>{row.champion.name}</strong><small>MASTERY {row.masteryPoints?.toLocaleString() ?? "—"} · LEVEL {row.masteryLevel ?? "—"}</small></span></div><div><small>TRACKED GAMES</small><b>{row.trackedGames}</b></div><div><small>WIN RATE</small><b>{row.winRate.toFixed(1)}%</b></div><div><small>KILLS</small><b>{row.kills.toLocaleString()}</b></div><div><small>PLAYTIME</small><b>{formatDuration(row.playtimeSeconds)}</b></div><div className="build-preview">{row.mostUsedCoreBuild?.items.slice(0, 3).map((item) => <EntityIcon key={item.id} entity={item} size={32} />) ?? <small>NO CORE BUILD</small>}</div></Link>;
}

function sortValue(row: ChampionSummaryDto, key: SortKey) {
  if (key === "mastery") return row.masteryPoints ?? 0;
  if (key === "winRate") return row.winRate;
  if (key === "kills") return row.kills;
  if (key === "playtime") return row.playtimeSeconds;
  return row.trackedGames;
}
