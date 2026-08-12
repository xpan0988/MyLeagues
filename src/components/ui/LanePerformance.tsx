import type { LanePerformanceSummaryDto } from "../../lib/types";
import { Metric } from "./DataDisplay";

export function formatSignedPercent(value: number | null): string {
  if (value === null) return "—";
  if (value === 0) return "0%";
  return `${value > 0 ? "+" : ""}${value}%`;
}

export function LanePerformance({
  title,
  summary,
}: {
  title: string;
  summary: LanePerformanceSummaryDto;
}) {
  const historyRange = summary.historyStartUtc && summary.historyEndUtc
    ? `${summary.historyStartUtc.slice(0, 10)} → ${summary.historyEndUtc.slice(0, 10)}`
    : "HISTORY UNAVAILABLE";
  return <section className="data-section">
    <div className="section-heading">
      <h2>{title}</h2>
      <span>EXPERIMENTAL · {summary.scoredMatches} SCORED / {summary.trackedMatches} TRACKED</span>
    </div>
    <div className="metric-grid performance-grid">
      <Metric label="Average Lane Score" value={formatSignedPercent(summary.averageLaneScorePercent)} detail="SIGNED DOMINANCE" />
      <Metric label="Lane Advantage Rate" value={summary.laneAdvantageRate === null ? "—" : `${summary.laneAdvantageRate.toFixed(1)}%`} detail={summary.laneAdvantageRate === null ? "CATEGORY THRESHOLDS UNAVAILABLE" : undefined} />
      <Metric label="Crush Rate" value={summary.crushRate === null ? "—" : `${summary.crushRate.toFixed(1)}%`} detail={summary.crushRate === null ? "CATEGORY THRESHOLDS UNAVAILABLE" : undefined} />
      <Metric label="Scored Matches" value={summary.scoredMatches} detail={`${summary.excludedMatches} EXCLUDED`} />
      <Metric label="Coverage" value={`${summary.coveragePercent.toFixed(1)}%`} detail={summary.derivationVersion} />
      <Metric label="History" value={historyRange} detail={`${summary.compatibleRulesetVersions.length} COMPATIBLE RULESETS`} />
    </div>
  </section>;
}
