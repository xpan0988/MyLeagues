import { Fragment, useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { EntityIcon, formatMatchDuration, friendlyError, QueryState, queueName } from "../../components/ui/DataDisplay";
import { PageHeader } from "../../components/ui/PageHeader";
import { backend } from "../../lib/tauri";
import type { MatchDetailDto, QueueFilter } from "../../lib/types";

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
    <PageHeader eyebrow="LOCAL MATCH HISTORY" title="Matches" description="Paginated participant history from SQLite; no full 10-player analysis." />
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
              <div className="match-main"><strong>{match.champion.name}</strong><small>{queueName(match.queueId)} · {new Date(match.gameCreation).toLocaleDateString()}</small></div>
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
    <header><div><span className={data.win ? "result-win" : "result-loss"}>{data.win ? "VICTORY" : "DEFEAT"}</span><h2>{data.champion.name}</h2></div><span>{queueName(data.queueId)} · Patch {data.patch} · {new Date(data.gameCreation).toLocaleString()}</span></header>
    <div className="detail-stats"><span><small>K / D / A</small><b>{data.kills} / {data.deaths} / {data.assists}</b></span><span><small>CS</small><b>{data.totalCs}</b></span><span><small>GOLD</small><b>{data.goldEarned.toLocaleString()}</b></span><span><small>DURATION</small><b>{formatMatchDuration(data.durationSeconds)}</b></span></div>
    <div className="detail-groups">
      <div><small>FINAL ITEMS</small><div className="icon-stack">{data.finalItems.map(({ item, slot }) => <EntityIcon entity={item} key={slot} />)}</div></div>
      <div><small>SUMMONER SPELLS</small><div className="icon-stack">{data.summonerSpells.map((spell) => <EntityIcon entity={spell} key={spell.id} />)}</div></div>
      <div><small>RUNES</small><div className="icon-stack">{[...data.runePage.primaryRunes, ...data.runePage.secondaryRunes, ...data.runePage.statShards].map((rune, index) => <EntityIcon entity={rune} key={`${rune.id}-${index}`} size={30} />)}</div></div>
      <div><small>MULTI-KILLS</small><b>{multiKills.length ? multiKills.map(([name, count]) => `${name} ×${count}`).join(" · ") : "None"}</b></div>
    </div>
  </aside>;
}
