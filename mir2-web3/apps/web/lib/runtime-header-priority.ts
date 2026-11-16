export type RuntimeHeaderRelease = "headers" | "finished" | "retired" | "timeout" | "cancelled" | "superseded";
type Outcome = { generation: number; reason: RuntimeHeaderRelease | "inactive" };
type Cycle = {
  generation: number;
  reason?: RuntimeHeaderRelease;
  timer?: ReturnType<typeof setTimeout>;
  waiters: Set<(outcome: Outcome) => void>;
};

/** Briefly reserve download admission for runtime headers, never for its full body. */
export class RuntimeHeaderPriority {
  private generation = 0;
  private cycle?: Cycle;

  constructor(private readonly maxWaitMs = 8_000) {
    if (!Number.isSafeInteger(maxWaitMs) || maxWaitMs < 1 || maxWaitMs > 8_000) {
      throw new RangeError("Invalid runtime header priority budget");
    }
  }

  beginCycle() {
    if (this.cycle) this.finish(this.cycle, "superseded");
    this.cycle = { generation: ++this.generation, waiters: new Set() };
  }

  cancelCycle() {
    if (this.cycle) {
      this.finish(this.cycle, "cancelled");
      // A queued background callback may enter after the screen changed.
      // Keep a tombstone even if headers had already released the old cycle.
      this.cycle = { generation: this.cycle.generation, reason: "cancelled", waiters: new Set() };
    }
  }

  captureRelease() {
    const cycle = this.cycle;
    return (reason: "headers" | "finished" | "retired") => cycle ? this.finish(cycle, reason) : false;
  }

  wait(signal?: AbortSignal): Promise<Outcome> {
    if (signal?.aborted) return Promise.reject(signal.reason);
    const cycle = this.cycle;
    if (!cycle || cycle.reason) return Promise.resolve({ generation: cycle?.generation ?? 0, reason: cycle?.reason ?? "inactive" });
    return new Promise((resolve, reject) => {
      const settle = (outcome: Outcome) => {
        signal?.removeEventListener("abort", abort);
        cycle.waiters.delete(settle);
        resolve(outcome);
      };
      const abort = () => {
        cycle.waiters.delete(settle);
        signal?.removeEventListener("abort", abort);
        reject(signal?.reason);
      };
      cycle.waiters.add(settle);
      signal?.addEventListener("abort", abort, { once: true });
      cycle.timer ??= setTimeout(() => this.finish(cycle, "timeout"), this.maxWaitMs);
    });
  }

  private finish(cycle: Cycle, reason: RuntimeHeaderRelease) {
    if (cycle.reason) return false;
    cycle.reason = reason;
    if (cycle.timer !== undefined) clearTimeout(cycle.timer);
    for (const settle of [...cycle.waiters]) settle({ generation: cycle.generation, reason });
    return true;
  }
}

let documentPriority: RuntimeHeaderPriority | undefined;
export function getRuntimeHeaderPriority() {
  if (typeof window === "undefined") return undefined;
  return documentPriority ??= new RuntimeHeaderPriority();
}
