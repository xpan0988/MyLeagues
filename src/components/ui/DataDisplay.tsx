import type { ReactNode } from "react";
import type { AnalyticsFilter, GameEntityDto, QueueFilter, TimeRangeFilter } from "../../lib/types";

export function EntityIcon({ entity, size = 36 }: { entity: GameEntityDto; size?: number }) {
  return entity.icon ? (
    <img className="entity-icon" src={entity.icon} alt="" title={entity.name} width={size} height={size} loading="lazy" />
  ) : (
    <span className="entity-icon entity-icon-fallback" title={entity.name} aria-label={entity.name} style={{ width: size, height: size }}>?</span>
  );
}

export function Metric({ label, value, detail }: { label: string; value: ReactNode; detail?: ReactNode }) {
  return <div className="metric"><span className="metric-label">{label}</span><strong>{value}</strong>{detail ? <span className="metric-detail">{detail}</span> : null}</div>;
}

export function Filters({ value, onChange }: { value: AnalyticsFilter; onChange: (next: AnalyticsFilter) => void }) {
  const queues: Array<[QueueFilter, string]> = [["rankedSolo", "Ranked Solo"], ["normal", "Normal"], ["aram", "ARAM"], ["all", "All"]];
  const times: Array<[TimeRangeFilter, string]> = [["currentPatch", "Current Patch"], ["currentSeason", "Current Season"], ["allTracked", "All Tracked"]];
  return <div className="filter-row" aria-label="Analytics filters">
    <div className="segmented">{queues.map(([id, label]) => <button key={id} type="button" className={value.queue === id ? "selected" : ""} onClick={() => onChange({ ...value, queue: id })}>{label}</button>)}</div>
    <div className="segmented">{times.map(([id, label]) => <button key={id} type="button" className={value.timeRange === id ? "selected" : ""} onClick={() => onChange({ ...value, timeRange: id })}>{label}</button>)}</div>
  </div>;
}

export function QueryState({ loading, error, empty, children }: { loading: boolean; error: unknown; empty?: boolean; children: ReactNode }) {
  if (loading) return <div className="notice">Loading local data…</div>;
  if (error) return <div className="notice notice-error">{friendlyError(error)}</div>;
  if (empty) return <div className="notice">No locally tracked data for this view.</div>;
  return <>{children}</>;
}

export function formatDuration(seconds: number) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return hours ? `${hours}h ${minutes}m` : `${minutes}m`;
}

export function formatMatchDuration(seconds: number) {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export function queueName(id: number) {
  if (id === 420) return "Ranked Solo";
  if (id === 450) return "ARAM";
  if (id === 400 || id === 430) return "Normal";
  return `Queue ${id}`;
}

export function friendlyError(error: unknown) {
  const value = error instanceof Error ? error.message : String(error);
  if (/401|403|expired|invalid.*key/i.test(value)) return "Sync unavailable: the Riot API key is missing or expired. Cached data remains available.";
  if (/429|rate/i.test(value)) return "Riot API rate limit reached. Synchronization will resume later.";
  if (/network|connect|dns|offline/i.test(value)) return "Network unavailable. Showing locally cached data.";
  return value.replace(/^.*?error:\s*/i, "") || "The local service could not complete this request.";
}
