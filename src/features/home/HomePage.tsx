import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Play, RefreshCw } from "lucide-react";
import { Link } from "react-router-dom";
import { PageHeader } from "../../components/ui/PageHeader";
import { EntityIcon, formatDuration, Metric, QueryState } from "../../components/ui/DataDisplay";
import { backend } from "../../lib/tauri";
import type { SyncStateDto } from "../../lib/types";

export function HomePage() {
  const client = useQueryClient();
  const home = useQuery({ queryKey: ["home"], queryFn: backend.getHome });
  const clientState = useQuery({ queryKey: ["client-state"], queryFn: backend.getClientState, refetchInterval: 5_000 });
  const launch = useMutation({ mutationFn: backend.launchClient, onSuccess: () => void client.invalidateQueries({ queryKey: ["home"] }) });
  const sync = useMutation({ mutationFn: backend.startSync, onSuccess: () => void client.invalidateQueries({ queryKey: ["home"] }) });
  const data = home.data;
  const overview = data?.trackedCareer;
  const account = data?.account;
  const process = clientState.data ?? data?.clientState;
  return <div className="page">
    <PageHeader eyebrow="PERSONAL ARCHIVE" title={account ? `${account.gameName}#${account.tagLine}` : "Home"} description="Cached career data loads first; synchronization continues in the background." actions={<button className="primary-button" type="button" onClick={() => launch.mutate()} disabled={launch.isPending}><Play size={16} fill="currentColor" />PLAY</button>} />
    <div className="status-strip">
      <span><span className={`status-dot ${process?.leagueClientRunning || process?.riotClientRunning ? "status-online" : "status-offline"}`} />{process?.gameRunning ? "Game running" : process?.leagueClientRunning ? "League running" : process?.riotClientRunning ? "Riot Client running" : "Client offline"}</span>
      <span>{syncLabel(data?.syncState)}</span>
      <button className="text-button" type="button" onClick={() => sync.mutate()} disabled={sync.isPending || data?.syncState.currentlyRunning}><RefreshCw size={13} /> Sync now</button>
    </div>
    {data?.account ? <section className="history-diagnostics" aria-label="Historical synchronization diagnostics"><span className="history-diagnostics-title">Historical sync</span><span><small>Matches tracked</small><b>{data.historicalSync.matchesTracked.toLocaleString()}</b></span><span><small>Oldest tracked match</small><b>{data.historicalSync.oldestTrackedAt ? new Date(data.historicalSync.oldestTrackedAt).toLocaleDateString() : "—"}</b></span><span><small>Tracked playtime</small><b>{formatDuration(data.historicalSync.trackedPlaytimeSeconds)}</b></span><span><small>History status</small><b>{data.historicalSync.historyStatus}</b></span><span><small>Match-V5 next cursor</small><b>{data.historicalSync.nextMatchStart}</b></span></section> : null}
    <QueryState loading={home.isPending} error={home.error} empty={!account}>
      {overview ? <>
        <section className="hero-metrics">
          <Metric label="Rank" value={data?.rank ? `${data.rank.tier} ${data.rank.division}` : "Unranked"} detail={data?.rank ? `${data.rank.leaguePoints} LP · ${data.rank.wins}W ${data.rank.losses}L` : undefined} />
          <Metric label="Tracked Games" value={overview.games.toLocaleString()} detail={`${overview.wins}W · ${overview.losses}L`} />
          <Metric label="Win Rate" value={`${overview.winRate.toFixed(1)}%`} />
          <Metric label="Tracked Playtime" value={formatDuration(overview.playtimeSeconds)} />
          <Metric label="Ranked Games" value={data?.rankedGames.toLocaleString() ?? "0"} />
          <Metric label="Tracked KDA" value={overview.kda.toFixed(2)} detail={`${overview.kills} / ${overview.deaths} / ${overview.assists}`} />
        </section>
        <div className="two-column">
          <section className="data-section"><div className="section-heading"><h2>Recent form</h2><span>LAST 20 GAMES</span></div><div className="form-track">{data?.recentForm.length ? data.recentForm.map((win, index) => <span key={index} className={win ? "form-win" : "form-loss"} title={win ? "Victory" : "Defeat"} />) : <span className="muted">No recent matches</span>}</div></section>
          <section className="data-section"><div className="section-heading"><h2>Top champions</h2><Link to="/champions">View all</Link></div><div className="compact-list">{data?.topChampions.map((champion) => <Link className="compact-row" to={`/champions/${champion.champion.id}`} key={champion.champion.id}><EntityIcon entity={champion.champion} /><span><strong>{champion.champion.name}</strong><small>{champion.trackedGames} games · {champion.winRate.toFixed(1)}% WR</small></span><b>{formatDuration(champion.playtimeSeconds)}</b></Link>)}</div></section>
        </div>
      </> : null}
    </QueryState>
  </div>;
}

function syncLabel(sync: SyncStateDto | undefined) {
  if (!sync) return "Local data ready";
  if (sync.status === "checking") return "Checking…";
  if (sync.status === "syncing") return sync.total ? `Syncing ${sync.completed} matches…` : "Syncing…";
  if (sync.status === "error") return "Sync failed — Retry";
  if (sync.lastSuccessfulSyncAt) {
    const minutes = Math.max(0, Math.floor((Date.now() - new Date(sync.lastSuccessfulSyncAt).getTime()) / 60_000));
    return minutes < 1 ? "Synced just now" : `Last synced ${minutes} min ago`;
  }
  return "Local data ready";
}
