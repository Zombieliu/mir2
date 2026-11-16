import { sameQuestWorldIdentity, type QuestWorldIdentity } from "./bevy-quest-world-context";

export type MapWorldBaselineSource = QuestWorldIdentity & Readonly<{ socket: object }>;
export const sameMapWorldBaselineSource = (a: MapWorldBaselineSource, b: MapWorldBaselineSource | null): boolean =>
  b !== null && a.socket === b.socket && sameQuestWorldIdentity(a, b);

type Timer = number | ReturnType<typeof setTimeout>;
type Options = {
  current: () => MapWorldBaselineSource | null;
  complete: (source: MapWorldBaselineSource) => boolean;
  request: (source: MapWorldBaselineSource) => boolean;
  setTimer?: (callback: () => void, delay: number) => Timer;
  clearTimer?: (timer: Timer) => void;
};

/** Read a fresh personal baseline after a map packet retires the old scene.
 * Never carries movement, purchases or a previous navigation objective.
 */
export class MapWorldBaselineRefresh {
  private pending: { source: MapWorldBaselineSource; attempts: number; timer: Timer | null } | null = null;
  private readonly delays = [100, 750, 2000] as const;
  constructor(private readonly options: Options) {}

  hasPending(): boolean { return this.pending !== null; }

  cancel(): void {
    if (this.pending?.timer !== null && this.pending?.timer !== undefined)
      (this.options.clearTimer ?? clearTimeout)(this.pending.timer);
    this.pending = null;
  }

  schedule(source: MapWorldBaselineSource): void {
    this.cancel();
    if (!sameQuestWorldIdentity(source, source) || !source.socket) return;
    // MapInformation is observed before the world projection switches maps.
    // Defer the read, and check the actual connection/session/scene at dispatch.
    const pending = { source: { ...source }, attempts: 0, timer: null as Timer | null };
    this.pending = pending;
    const run = () => {
      if (this.pending !== pending) return;
      pending.timer = null;
      if (!sameMapWorldBaselineSource(pending.source, this.options.current()) || this.options.complete(pending.source)) {
        this.cancel(); return;
      }
      pending.attempts++;
      // A read may be definitely unsent or have an unknown transport outcome.
      // Both are bounded; a response is still validated by the ordinary ingress.
      try { this.options.request(pending.source); } catch { /* bounded retry */ }
      if (this.pending !== pending) return;
      if (!sameMapWorldBaselineSource(pending.source, this.options.current()) || this.options.complete(pending.source)
        || pending.attempts >= this.delays.length) { this.cancel(); return; }
      pending.timer = (this.options.setTimer ?? setTimeout)(run, this.delays[pending.attempts]);
    };
    pending.timer = (this.options.setTimer ?? setTimeout)(run, this.delays[0]);
  }
}
