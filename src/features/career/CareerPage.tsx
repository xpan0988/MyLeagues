import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Filters, formatDuration, formatMatchDuration, Metric, QueryState } from "../../components/ui/DataDisplay";
import { PageHeader } from "../../components/ui/PageHeader";
import { LanePerformance } from "../../components/ui/LanePerformance";
import { backend } from "../../lib/tauri";
import type { AnalyticsFilter, TrackedOverviewDto } from "../../lib/types";

const initial: AnalyticsFilter = { queue: "all", timeRange: "allTracked" };
export function CareerPage() {
  const [filter, setFilter] = useState(initial);
  const query = useQuery({ queryKey: ["career", filter], queryFn: () => backend.getCareer(filter) });
  const data = query.data;
  return <div className="page"><PageHeader eyebrow="ALL CHAMPIONS" title="Career" description="Long-term tracked totals, derived from rebuildable local aggregate caches." /><Filters value={filter} onChange={setFilter} />
    <QueryState loading={query.isPending} error={query.error} empty={!data?.overall.games}>{data ? <><section className="hero-metrics"><Metric label="Tracked Games" value={data.overall.games.toLocaleString()} /><Metric label="Wins / Losses" value={`${data.overall.wins} / ${data.overall.losses}`} /><Metric label="Win Rate" value={`${data.overall.winRate.toFixed(1)}%`} /><Metric label="Tracked Playtime" value={formatDuration(data.overall.playtimeSeconds)} /><Metric label="Average Duration" value={formatMatchDuration(data.averageMatchDurationSeconds)} /><Metric label="Champion Pool" value={data.championPool} /></section><LanePerformance title="Laning" summary={data.lanePerformance} /><section className="data-section"><div className="section-heading"><h2>Queue breakdown</h2></div><div className="queue-breakdown"><QueueStats name="Ranked Solo" data={data.byQueue.rankedSolo} /><QueueStats name="Normal" data={data.byQueue.normal} /><QueueStats name="ARAM" data={data.byQueue.aram} /></div></section><section className="data-section"><div className="section-heading"><h2>Tracked Combat Totals</h2></div><div className="metric-grid"><Metric label="Kills" value={data.overall.kills.toLocaleString()} /><Metric label="Deaths" value={data.overall.deaths.toLocaleString()} /><Metric label="Assists" value={data.overall.assists.toLocaleString()} /><Metric label="KDA" value={data.overall.kda.toFixed(2)} /></div></section></> : null}</QueryState>
  </div>;
}
function QueueStats({ name, data }: { name: string; data: TrackedOverviewDto }) { return <article><span>{name}</span><strong>{data.games}</strong><small>GAMES</small><b>{data.winRate.toFixed(1)}% WR</b><div className="ratio-bar"><i style={{ width: `${data.winRate}%` }} /></div></article>; }
