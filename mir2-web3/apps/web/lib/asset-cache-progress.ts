type ProgressSummary = { prewarmRequested: number; prewarmOk: number; prewarmFailed: number };
type ProgressRun = { status: string; recovery?: { status: string } };

/** Presentation only: original failure counters/history remain diagnostic facts. */
export function assetCacheProgressView(summary: ProgressSummary, runs: readonly ProgressRun[]) {
  const active = runs.filter(run => run.status !== "cancelled");
  const recovering = active.some(run => run.status === "running" && run.recovery?.status === "running");
  const busy = active.some(run => run.status === "pending" || run.status === "running");
  const total = Math.max(0, summary.prewarmRequested), ok = Math.max(0, summary.prewarmOk);
  const complete = total > 0 && ok >= total && summary.prewarmFailed === 0 && !recovering;
  const state = complete ? "complete" : recovering ? "retrying" : busy ? "loading"
    : summary.prewarmFailed > 0 ? "failed" : "loading";
  return {
    state, visible: total > 0 && !complete, total, ok,
    // A completed failed attempt is not a downloaded asset. Retrying must not
    // display 100% or count its eventual success twice.
    percent: total > 0 ? Math.min(100, Math.floor(ok / total * 100)) : 0,
    unresolved: Math.max(0, summary.prewarmFailed),
  };
}
