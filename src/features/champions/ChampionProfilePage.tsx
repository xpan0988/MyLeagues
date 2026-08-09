import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useParams } from "react-router-dom";
import { EntityIcon, Filters, formatDuration, formatMatchDuration, Metric, QueryState } from "../../components/ui/DataDisplay";
import { backend } from "../../lib/tauri";
import type { AnalyticsFilter, GameEntityDto } from "../../lib/types";

const initialFilter: AnalyticsFilter = { queue: "rankedSolo", timeRange: "currentSeason" };

export function ChampionProfilePage() {
  const championId = Number(useParams().championId);
  const [filter, setFilter] = useState(initialFilter);
  const query = useQuery({ queryKey: ["champion-profile", championId, filter], queryFn: () => backend.getChampionProfile(championId, filter), enabled: Number.isFinite(championId) });
  const data = query.data;
  return <div className="page champion-profile">
    <QueryState loading={query.isPending} error={query.error}>
      {data ? <>
        <header className="champion-header"><EntityIcon entity={data.champion} size={76} /><div><p className="eyebrow">CHAMPION PROFILE</p><h1>{data.champion.name}</h1><p className="page-description">Mastery reflects Riot mastery data. Tracked Career covers only locally synchronized matches.</p></div><div className="mastery-block"><small>MASTERY</small><strong>{data.mastery.points?.toLocaleString() ?? "—"}</strong><span>Level {data.mastery.level ?? "—"}</span></div></header>
        <Filters value={filter} onChange={setFilter} />
        <section className="data-section"><div className="section-heading"><h2>Tracked Career</h2><span>{data.filterContext.queue} · {data.filterContext.timeRange}</span></div><div className="metric-grid"><Metric label="Games" value={data.overview.games} detail={`${data.overview.wins}W · ${data.overview.losses}L`} /><Metric label="Win Rate" value={`${data.overview.winRate.toFixed(1)}%`} /><Metric label="Playtime" value={formatDuration(data.overview.playtimeSeconds)} /><Metric label="Kills / Deaths / Assists" value={`${data.overview.kills} / ${data.overview.deaths} / ${data.overview.assists}`} /><Metric label="KDA" value={data.overview.kda.toFixed(2)} /></div></section>
        <section className="data-section"><div className="section-heading"><h2>Performance</h2></div><div className="metric-grid performance-grid"><Metric label="Average K / D / A" value={`${data.performance.averageKills.toFixed(1)} / ${data.performance.averageDeaths.toFixed(1)} / ${data.performance.averageAssists.toFixed(1)}`} /><Metric label="CS / min" value={data.performance.averageCsPerMinute.toFixed(1)} /><Metric label="Avg duration" value={formatMatchDuration(Math.round(data.performance.averageMatchDurationSeconds))} /><Metric label="Highest kills" value={data.performance.highestKills} /><Metric label="Multi-kills" value={`${data.performance.doubleKills} · ${data.performance.tripleKills} · ${data.performance.quadraKills} · ${data.performance.pentaKills}`} detail="DOUBLE · TRIPLE · QUADRA · PENTA" /></div></section>
        <div className="profile-columns"><section className="data-section"><div className="section-heading"><h2>Core Build Combinations</h2><span>FINAL INVENTORY · CANONICAL</span></div><div className="analytics-list">{data.coreBuilds.length ? data.coreBuilds.slice(0, 5).map((build, index) => <div className="analytics-row" key={index}><div className="icon-stack">{build.items.map((item) => <EntityIcon key={item.id} entity={item} />)}</div><b>{build.games} games</b><span>{build.usageRate.toFixed(1)}% usage</span><span>{build.winRate.toFixed(1)}% WR</span></div>) : <p className="muted">No completed core items in this scope.</p>}</div></section>
          <section className="data-section"><div className="section-heading"><h2>Most Used Boots</h2></div><div className="analytics-list">{data.boots.slice(0, 5).map((boot) => <EntityUsage key={boot.entity.id} entity={boot.entity} games={boot.games} usage={boot.usageRate} winRate={boot.winRate} />)}</div></section></div>
        <section className="data-section"><div className="section-heading"><h2>Your Rune Preferences</h2><span>FULL PAGE GROUPING</span></div><div className="rune-page-grid">{data.runePages.slice(0, 4).map((page, index) => <article className="rune-page" key={index}><header><EntityIcon entity={page.primaryStyle} /><div><strong>{page.primaryStyle.name}</strong><small>{page.games} games · {page.usageRate.toFixed(1)}% · {page.winRate.toFixed(1)}% WR</small></div></header><RuneLine label="PRIMARY" entities={page.primaryRunes} /><RuneLine label={page.secondaryStyle.name.toUpperCase()} entities={page.secondaryRunes} /><RuneLine label="SHARDS" entities={page.statShards} /></article>)}</div></section>
        <div className="profile-columns"><section className="data-section"><div className="section-heading"><h2>Keystone Usage</h2></div>{data.keystoneUsage.slice(0, 5).map((entry) => <EntityUsage key={entry.entity.id} entity={entry.entity} games={entry.games} usage={entry.usageRate} winRate={entry.winRate} />)}</section><section className="data-section"><div className="section-heading"><h2>Summoner Spells</h2></div>{data.summonerSpellPairs.slice(0, 5).map((entry, index) => <div className="analytics-row" key={index}><div className="icon-stack">{entry.spells.map((spell) => <EntityIcon entity={spell} key={spell.id} />)}</div><b>{entry.spells.map((spell) => spell.name).join(" + ")}</b><span>{entry.games} games</span><span>{entry.usageRate.toFixed(1)}% · {entry.winRate.toFixed(1)}% WR</span></div>)}</section></div>
      </> : null}
    </QueryState>
  </div>;
}

function RuneLine({ label, entities }: { label: string; entities: GameEntityDto[] }) { return <div className="rune-line"><small>{label}</small><div className="icon-stack">{entities.map((entity) => <EntityIcon entity={entity} key={entity.id} size={30} />)}</div><span>{entities.map((entity) => entity.name).join(" · ")}</span></div>; }
function EntityUsage({ entity, games, usage, winRate }: { entity: GameEntityDto; games: number; usage: number; winRate: number }) { return <div className="analytics-row"><EntityIcon entity={entity} /><b>{entity.name}</b><span>{games} games</span><span>{usage.toFixed(1)}% · {winRate.toFixed(1)}% WR</span></div>; }
