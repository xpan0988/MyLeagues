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
  const checkpointCounts = new Map<string, number>();
  const checkpoints = lane.checkpoints.map((checkpoint) => {
    const base = checkpoint.label.split(":", 1)[0] ?? checkpoint.label;
    const count = (checkpointCounts.get(base) ?? 0) + 1;
    checkpointCounts.set(base, count);
    return { ...checkpoint, display: checkpoint.eventTimestampMs === null ? base : `${base}${base === "PRE_GRUBS" ? ` #${count}` : ""}` };
  });
  return <section className="lane-matchup-detail">
    <div className="section-heading"><h2>Laning / Matchup</h2><span>EXPERIMENTAL · {lane.derivationVersion}</span></div>
    <div className="lane-matchup-heading"><div><strong>{champion} vs {opponent?.name ?? "Unavailable"}</strong><small>OPPONENT CONFIDENCE {lane.confidence}</small></div><b>{lane.laneScorePercent === null ? "Lane unavailable" : `Lane Score ${formatSignedPercent(lane.laneScorePercent)}`}</b></div>
    {lane.exclusionReason ? <p className="muted">Excluded: {readable(lane.exclusionReason)}</p> : null}
    <div className="detail-stats lane-dimensions"><span><small>CUTOFF</small><b>{lane.cutoffTimestampMs === null ? "—" : timestamp(lane.cutoffTimestampMs)}</b><em>{lane.cutoffReason ?? "Unavailable"}</em></span>{dimensions.map(([label, value]) => <span key={label}><small>{label}</small><b>{value === null ? "—" : value.toFixed(3)}</b></span>)}<span><small>GOLD</small><b>{readable(lane.goldConsistency ?? "unavailable")}</b></span></div>
    <LaneTrajectory lane={lane} />
    {checkpoints.length ? <section className="lane-evidence-block"><small>DETAILED EVIDENCE · CHECKPOINTS</small><div className="lane-table-wrap"><table className="lane-evidence-table"><thead><tr><th>Checkpoint</th><th>Event time</th><th>Source frame</th><th>Δ Level</th><th>Δ XP</th><th>Δ CS</th><th>Δ Gold</th></tr></thead><tbody>{checkpoints.map((checkpoint) => <tr key={`${checkpoint.label}-${checkpoint.timestampMs}`}><th>{checkpoint.display}</th><td>{checkpoint.eventTimestampMs === null ? "—" : timestamp(checkpoint.eventTimestampMs)}</td><td>{timestamp(checkpoint.timestampMs)}</td><td>{signed(checkpoint.levelDifference)}</td><td>{signed(checkpoint.xpDifference)}</td><td>{signed(checkpoint.laneCsDifference)}</td><td>{signed(checkpoint.goldDifference)}</td></tr>)}</tbody></table></div></section> : null}
    {lane.combatClusters.length ? <section className="lane-evidence-block"><small>DETAILED EVIDENCE · ATOMIC COMBAT CLUSTERS</small><div className="lane-table-wrap"><table className="lane-evidence-table"><thead><tr><th>Time</th><th>Classification</th><th>Contribution</th><th>Contributors</th><th>Lane-pair share</th></tr></thead><tbody>{lane.combatClusters.map((cluster, index) => { const attributions = cluster.attributions.filter((value) => value.signedLanePairShare !== 0); return <tr key={`${cluster.startTimestampMs}-${index}`}><td>{timestamp(cluster.startTimestampMs)}–{timestamp(cluster.endTimestampMs)}</td><th>{readable(cluster.classification)}</th><td>{cluster.signedStrength.toFixed(2)}</td><td>{attributions.length ? attributions.map((value) => value.contributorCount).join(", ") : "—"}</td><td>{attributions.length ? attributions.map((value) => `${(Math.abs(value.signedLanePairShare) * 100).toFixed(1)}%`).join(", ") : "0%"}</td></tr>; })}</tbody></table></div></section> : null}
    <div className="lane-event-columns"><LaneEvents title="PRESSURE" events={lane.pressureEvents} /><LaneEvents title="GRUBS / HERALD CONVERSION" events={lane.objectiveEvents} /></div>
    {lane.coverage.conversion === "not_applicable_by_queue" ? <p className="muted">Objective Conversion: Not applicable for Swiftplay.</p> : null}
    <dl className="lane-metadata"><div><dt>Model</dt><dd>{lane.modelVersion}</dd></div><div><dt>Ruleset</dt><dd>{lane.rulesetVersion}</dd></div><div><dt>Derivation</dt><dd>{lane.derivationVersion}</dd></div>{Object.entries(lane.coverage).map(([key, value]) => <div key={key}><dt>{key} coverage</dt><dd>{value}</dd></div>)}</dl>
  </section>;
}

function LaneTrajectory({ lane }: { lane: LaneMatchDetailDto }) {
  const [hoveredTimestamp, setHoveredTimestamp] = useState<number | null>(null);
  const points = lane.trajectory;
  if (!points.length) return null;
  const endTimestamp = Math.max(lane.cutoffTimestampMs ?? 0, points.at(-1)?.timestampMs ?? 0, 840000);
  const firstPoint = points[0]!;
  const hovered = hoveredTimestamp === null ? null : points.slice(1).reduce((nearest, point) => Math.abs(point.timestampMs - hoveredTimestamp) < Math.abs(nearest.timestampMs - hoveredTimestamp) ? point : nearest, firstPoint);
  const markers = [
    ...lane.combatClusters.filter((cluster) => ["LANE_SOLO_KILL", "ASSISTED_LANE_KILL", "REINFORCEMENT_REVERSAL", "REINFORCEMENT_TRIPLE"].includes(cluster.classification)).map((cluster) => ({ timestampMs: cluster.startTimestampMs, label: readable(cluster.classification), tone: "combat" })),
    ...lane.pressureEvents.map((event) => ({ timestampMs: event.timestampMs, label: "top pressure", tone: "pressure" })),
    ...lane.objectiveEvents.map((event) => ({ timestampMs: event.timestampMs, label: readable(event.detail ?? event.eventType), tone: "objective" })),
    ...(lane.cutoffTimestampMs === null ? [] : [{ timestampMs: lane.cutoffTimestampMs, label: "lane cutoff", tone: "cutoff" }]),
  ].filter((marker) => marker.timestampMs <= endTimestamp);
  const charts = [["XP DIFFERENCE", "xpDifference", "xp"], ["GOLD DIFFERENCE", "goldDifference", "gold"]] as const;
  return <div className="lane-evidence-block lane-trajectory"><div className="lane-trajectory-heading"><small>LANE TRAJECTORY · ACTUAL TIMELINE FRAMES</small><span>REFERENCE @6 · @8 · @10 · @12 · @14</span></div>{charts.map(([label, field, unit], index) => <TrajectoryChart key={field} label={label} unit={unit} points={points} field={field} endTimestamp={endTimestamp} markers={markers} hovered={hovered} setHoveredTimestamp={setHoveredTimestamp} showMarkerLabels={false} showAxis={index === charts.length - 1} />)}<p className="lane-trajectory-legend">Markers: combat · top pressure · Grubs / Herald · lane cutoff</p>{hovered ? <p className="lane-trajectory-tooltip"><b>{timestamp(hovered.timestampMs)}</b> · Level {signed(hovered.levelDifference)} · XP {signed(hovered.xpDifference)} · CS {signed(hovered.laneCsDifference)} · Gold {signed(hovered.goldDifference)}</p> : <p className="lane-trajectory-tooltip muted">Hover any chart to inspect the nearest real Timeline frame.</p>}</div>;
}

type TrajectoryPoint = LaneMatchDetailDto["trajectory"][number];
type TrajectoryMarker = { timestampMs: number; label: string; tone: string };
function TrajectoryChart({ label, unit, points, field, endTimestamp, markers, hovered, setHoveredTimestamp, showMarkerLabels, showAxis }: { label: string; unit: string; points: TrajectoryPoint[]; field: "levelDifference" | "xpDifference" | "laneCsDifference" | "goldDifference"; endTimestamp: number; markers: TrajectoryMarker[]; hovered: TrajectoryPoint | null; setHoveredTimestamp: (timestamp: number | null) => void; showMarkerLabels: boolean; showAxis: boolean }) {
  const width = 720; const height = showAxis ? 132 : 108; const left = 42; const right = 12; const top = showMarkerLabels ? 22 : 10; const bottom = showAxis ? 28 : 12;
  const values = points.map((point) => point[field]); const extent = Math.max(1, ...values.map((value) => Math.abs(value))); const minY = -extent; const maxY = extent;
  const x = (timestampMs: number) => left + (timestampMs / endTimestamp) * (width - left - right);
  const y = (value: number) => top + ((maxY - value) / (maxY - minY)) * (height - top - bottom);
  const path = points.map((point) => `${x(point.timestampMs)},${y(point[field])}`).join(" ");
  const references = [360000, 480000, 600000, 720000, 840000].filter((value) => value <= endTimestamp);
  return <div className="lane-trajectory-chart"><div><b>{label}</b><small>{unit}</small></div><svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`${label} by actual Timeline timestamp`} onMouseMove={(event) => { const box = event.currentTarget.getBoundingClientRect(); const fraction = Math.max(0, Math.min(1, (event.clientX - box.left - (left / width) * box.width) / (((width - left - right) / width) * box.width))); setHoveredTimestamp(fraction * endTimestamp); }} onMouseLeave={() => setHoveredTimestamp(null)}><line className="trajectory-zero" x1={left} x2={width - right} y1={y(0)} y2={y(0)} /><text className="trajectory-y-label" x="1" y={y(maxY) + 4}>{signed(maxY)}</text><text className="trajectory-y-label" x="1" y={y(minY) + 4}>{signed(minY)}</text>{references.map((reference) => <g key={reference}><line className="trajectory-reference" x1={x(reference)} x2={x(reference)} y1={top} y2={height - bottom} />{showAxis ? <text className="trajectory-x-label" x={x(reference)} y={height - 7} textAnchor="middle">@{reference / 60000}</text> : null}</g>)}{markers.map((marker, index) => <g key={`${marker.timestampMs}-${marker.label}-${index}`} className={`trajectory-marker ${marker.tone}`}><line x1={x(marker.timestampMs)} x2={x(marker.timestampMs)} y1={top} y2={height - bottom} />{showMarkerLabels ? <text x={x(marker.timestampMs)} y={12 + (index % 2) * 9} textAnchor="middle">{marker.label}</text> : null}</g>)}<polyline className="trajectory-line" points={path} />{hovered ? <g className="trajectory-hover"><line x1={x(hovered.timestampMs)} x2={x(hovered.timestampMs)} y1={top} y2={height - bottom} /><circle cx={x(hovered.timestampMs)} cy={y(hovered[field])} r="4" /></g> : null}</svg></div>;
}

function LaneEvents({ title, events }: { title: string; events: LaneMatchDetailDto["pressureEvents"] }) {
  return <section className="lane-evidence-block"><small>DETAILED EVIDENCE · {title}</small>{events.length ? <div className="lane-table-wrap"><table className="lane-evidence-table"><thead><tr><th>Time</th><th>{title === "PRESSURE" ? "Event / location" : "Objective"}</th><th>Attribution</th></tr></thead><tbody>{events.map((event, index) => <tr key={`${event.timestampMs}-${index}`}><td>{timestamp(event.timestampMs)}</td><th>{readable(event.detail ?? event.eventType)}</th><td>{readable(event.attributionConfidence)}</td></tr>)}</tbody></table></div> : <p className="muted">No qualifying stored facts.</p>}</section>;
}

function timestamp(milliseconds: number) { const seconds = Math.floor(milliseconds / 1000); return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`; }
function signed(value: number) { return value === 0 ? "0" : `${value > 0 ? "+" : ""}${value}`; }
function readable(value: string) { return value.toLowerCase().replaceAll("_", " "); }
