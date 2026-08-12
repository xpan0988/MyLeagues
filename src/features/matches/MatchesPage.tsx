import { Fragment, useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { EntityIcon, formatMatchDuration, friendlyError, QueryState } from "../../components/ui/DataDisplay";
import { PageHeader } from "../../components/ui/PageHeader";
import { formatSignedPercent } from "../../components/ui/LanePerformance";
import { backend } from "../../lib/tauri";
import type { LaneMatchDetailDto, MatchDetailDto, QueueFilter } from "../../lib/types";

export function MatchesPage() {
  const [queue, setQueue] = useState<QueueFilter>("all");
  const [championId, setChampionId] = useState<number | undefined>();
  const [selected, setSelected] = useState<string | null>(null);
  const champions = useQuery({
    queryKey: ["champions", "match-filter"],
    queryFn: () => backend.listChampions({ queue: "all", timeRange: "allTracked" }),
  });
  const matches = useInfiniteQuery({
    queryKey: ["matches", queue, championId],
    queryFn: ({ pageParam }) => backend.listMatches({
      queue,
      timeRange: "allTracked",
      championId,
      cursor: pageParam,
      limit: 40,
    }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.nextCursor ?? undefined,
  });
  const rows = matches.data?.pages.flatMap((page) => page.items) ?? [];

  return <div className="page">
    <PageHeader eyebrow="LOCAL MATCH HISTORY" title="Matches" description="Paginated local match history with versioned experimental lane diagnostics where available." />
    <div className="toolbar">
      <label>Queue <select value={queue} onChange={(event) => { setQueue(event.target.value as QueueFilter); setSelected(null); }}><option value="all">All</option><option value="rankedSolo">Ranked Solo</option><option value="normal">Normal</option><option value="aram">ARAM</option></select></label>
      <label>Champion <select value={championId ?? ""} onChange={(event) => { setChampionId(event.target.value ? Number(event.target.value) : undefined); setSelected(null); }}><option value="">All champions</option>{champions.data?.map((champion) => <option key={champion.champion.id} value={champion.champion.id}>{champion.champion.name}</option>)}</select></label>
    </div>
    <QueryState loading={matches.isPending} error={matches.error} empty={!rows.length}>
      <div className="match-list">
        {rows.map((match) => {
          const expanded = selected === match.matchId;
          return <Fragment key={match.matchId}>
            <button
              className={`match-row ${match.win ? "victory" : "defeat"}${expanded ? " expanded" : ""}`}
              type="button"
              aria-expanded={expanded}
              aria-controls={`match-detail-${match.matchId}`}
              onClick={() => setSelected(expanded ? null : match.matchId)}
            >
              <EntityIcon entity={match.champion} size={46} />
              <div className="match-main"><strong>{match.champion.name}</strong><small>{match.queueDisplayName} · {new Date(match.gameCreation).toLocaleDateString()}</small>{match.matchupOpponent ? <span className="match-lane-summary">vs {match.matchupOpponent.name}{match.matchupRole === "TOP" ? <> · {match.lane?.laneScorePercent === null || !match.lane ? "Lane unavailable" : <>Lane Score {formatSignedPercent(match.lane.laneScorePercent)}</>}</> : null}</span> : null}</div>
              <b className={match.win ? "result-win" : "result-loss"}>{match.win ? "VICTORY" : "DEFEAT"}</b>
              <strong>{match.kills} / {match.deaths} / {match.assists}</strong>
              <span>{formatMatchDuration(match.durationSeconds)}</span>
              <div className="icon-stack">{match.keystone ? <EntityIcon entity={match.keystone} size={28} /> : null}{match.summonerSpells.map((spell) => <EntityIcon key={spell.id} entity={spell} size={28} />)}</div>
            </button>
            {expanded ? <MatchDetail matchId={match.matchId} /> : null}
          </Fragment>;
        })}
      </div>
      <div className="load-more">{matches.hasNextPage ? <button type="button" className="secondary-button" onClick={() => void matches.fetchNextPage()} disabled={matches.isFetchingNextPage}>{matches.isFetchingNextPage ? "Loading…" : "Load more"}</button> : <span>End of local history</span>}</div>
    </QueryState>
  </div>;
}

function MatchDetail({ matchId }: { matchId: string }) {
  const query = useQuery({
    queryKey: ["match-detail", matchId],
    queryFn: () => backend.getMatchDetail(matchId),
  });
  return <div id={`match-detail-${matchId}`} className="match-detail-inline">
    {query.isPending ? <div className="match-detail-state">Loading match detail…</div> : null}
    {query.error ? <div className="match-detail-state match-detail-error">{friendlyError(query.error)}</div> : null}
    {query.data ? <Detail data={query.data} /> : null}
  </div>;
}

function Detail({ data }: { data: MatchDetailDto }) {
  const multiKills = [
    ["Double", data.doubleKills],
    ["Triple", data.tripleKills],
    ["Quadra", data.quadraKills],
    ["Penta", data.pentaKills],
  ].filter(([, count]) => Number(count) > 0);
  return <aside className="match-detail">
    <header><div><span className={data.win ? "result-win" : "result-loss"}>{data.win ? "VICTORY" : "DEFEAT"}</span><h2>{data.champion.name}</h2>{data.matchupOpponent ? <small>vs {data.matchupOpponent.name}</small> : null}</div><span>{data.queueDisplayName} · Patch {data.patch} · {new Date(data.gameCreation).toLocaleString()}</span></header>
    <div className="detail-stats"><span><small>K / D / A</small><b>{data.kills} / {data.deaths} / {data.assists}</b></span><span><small>CS</small><b>{data.totalCs}</b></span><span><small>GOLD</small><b>{data.goldEarned.toLocaleString()}</b></span><span><small>DURATION</small><b>{formatMatchDuration(data.durationSeconds)}</b></span></div>
    {data.matchupRole === "TOP" && data.lane ? <LaneDetail champion={data.champion.name} opponent={data.matchupOpponent} lane={data.lane} /> : null}
    {data.matchupRole === "TOP" && !data.lane ? <section className="lane-matchup-detail"><div className="section-heading"><h2>Laning / Matchup</h2></div><p className="muted">Lane unavailable</p></section> : null}
    <div className="detail-groups">
      <div className="detail-group"><small>FINAL ITEMS</small><div className="icon-stack">{data.finalItems.map(({ item, slot }) => <EntityIcon entity={item} key={slot} />)}</div></div>
      <div className="detail-group"><small>SUMMONER SPELLS</small><div className="icon-stack">{data.summonerSpells.map((spell) => <EntityIcon entity={spell} key={spell.id} />)}</div></div>
      <div className="detail-group"><small>RUNES</small><div className="icon-stack">{[...data.runePage.primaryRunes, ...data.runePage.secondaryRunes, ...data.runePage.statShards].map((rune, index) => <EntityIcon entity={rune} key={`${rune.id}-${index}`} size={30} />)}</div></div>
      <div className="detail-group detail-multi-kills"><small>MULTI-KILLS</small>{multiKills.length ? <div className="detail-multi-kill-list">{multiKills.map(([name, count]) => <span key={name}><span>{name}</span><b>×{count}</b></span>)}</div> : <span className="detail-empty">None</span>}</div>
    </div>
  </aside>;
}

function LaneDetail({ champion, opponent, lane }: { champion: string; opponent: MatchDetailDto["matchupOpponent"]; lane: LaneMatchDetailDto }) {
  const dimensions = [["EXP", lane.exp], ["Combat", lane.combat], ["Farm", lane.farm], ["Pressure", lane.pressure], ["Objective Conversion", lane.conversion]] as const;
  return <section className="lane-matchup-detail">
    <div className="section-heading"><h2>Laning / Matchup</h2><span>EXPERIMENTAL · {lane.derivationVersion}</span></div>
    <div className="lane-matchup-heading"><div><strong>{champion} vs {opponent?.name ?? "Unavailable"}</strong><small>OPPONENT CONFIDENCE {lane.confidence}</small></div><b>{lane.laneScorePercent === null ? "Lane unavailable" : `Lane Score ${formatSignedPercent(lane.laneScorePercent)}`}</b></div>
    {lane.exclusionReason ? <p className="muted">Excluded: {readable(lane.exclusionReason)}</p> : null}
    <div className="detail-stats lane-dimensions"><span><small>CUTOFF</small><b>{lane.cutoffTimestampMs === null ? "—" : timestamp(lane.cutoffTimestampMs)}</b><em>{lane.cutoffReason ?? "Unavailable"}</em></span>{dimensions.map(([label, value]) => <span key={label}><small>{label}</small><b>{value === null ? "—" : value.toFixed(3)}</b></span>)}<span><small>GOLD</small><b>{readable(lane.goldConsistency ?? "unavailable")}</b></span></div>
    {lane.checkpoints.length ? <div className="lane-evidence-block"><small>CHECKPOINTS</small><div className="lane-checkpoint-grid">{lane.checkpoints.map((checkpoint) => <span key={`${checkpoint.label}-${checkpoint.timestampMs}`}><b>{checkpoint.label} · {timestamp(checkpoint.timestampMs)}</b><em>Lvl {signed(checkpoint.levelDifference)} · XP {signed(checkpoint.xpDifference)} · CS {signed(checkpoint.laneCsDifference)} · Gold {signed(checkpoint.goldDifference)}</em></span>)}</div></div> : null}
    {lane.combatClusters.length ? <div className="lane-evidence-block"><small>ATOMIC COMBAT CLUSTERS</small>{lane.combatClusters.map((cluster, index) => <p key={`${cluster.startTimestampMs}-${index}`}>{readable(cluster.classification)} · {timestamp(cluster.startTimestampMs)}–{timestamp(cluster.endTimestampMs)} · {cluster.signedStrength.toFixed(2)}{cluster.attributions.filter((attribution) => attribution.signedLanePairShare !== 0).map((attribution) => <em key={attribution.sourceEventId}> · {attribution.contributorCount} contributors · Lane-pair share {(Math.abs(attribution.signedLanePairShare) * 100).toFixed(0)}%</em>)}</p>)}</div> : null}
    <div className="lane-event-columns">
      <LaneEvents title="PRESSURE" events={lane.pressureEvents} />
      <LaneEvents title="GRUBS / HERALD CONVERSION" events={lane.objectiveEvents} />
    </div>
    {lane.coverage.conversion === "not_applicable_by_queue" ? <p className="muted">Objective Conversion: Not applicable for Swiftplay.</p> : null}
    <p className="lane-model-line">{lane.modelVersion} · {lane.rulesetVersion} · Coverage {Object.entries(lane.coverage).map(([key, value]) => `${key}: ${value}`).join(" · ") || "unavailable"}</p>
  </section>;
}

function LaneEvents({ title, events }: { title: string; events: LaneMatchDetailDto["pressureEvents"] }) {
  return <div className="lane-evidence-block"><small>{title}</small>{events.length ? events.map((event, index) => <p key={`${event.timestampMs}-${index}`}>{timestamp(event.timestampMs)} · {readable(event.detail ?? event.eventType)} · {readable(event.attributionConfidence)}</p>) : <p className="muted">No qualifying stored facts.</p>}</div>;
}

function timestamp(milliseconds: number) { const seconds = Math.floor(milliseconds / 1000); return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`; }
function signed(value: number) { return value === 0 ? "0" : `${value > 0 ? "+" : ""}${value}`; }
function readable(value: string) { return value.toLowerCase().replaceAll("_", " "); }
