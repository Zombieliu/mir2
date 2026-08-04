import type { BackgroundPrewarmMode } from "./asset-prewarm-policy";
import type { AssetPrewarmStage } from "./asset-cache-packs";

export const MIR2_ASSET_FIRST_PLAYABLE_EVENT = "mir2:asset-first-playable";

export type AssetPrewarmLane = "critical" | "background";

export type AssetOrchestratorRun = (
  stage: AssetPrewarmStage,
  lane: AssetPrewarmLane,
  signal: AbortSignal,
) => Promise<void>;

export type AssetOrchestratorSnapshot = {
  backgroundMode: BackgroundPrewarmMode;
  firstPlayable: boolean;
  requestedStage: AssetPrewarmStage | null;
  pendingBackgroundStage: AssetPrewarmStage | null;
  activeBackgroundStage: AssetPrewarmStage | null;
  completed: string[];
  disposed: boolean;
};

type AssetOrchestratorOptions = {
  backgroundMode: BackgroundPrewarmMode;
  run: AssetOrchestratorRun;
  hasWork?: (stage: AssetPrewarmStage, lane: AssetPrewarmLane) => boolean;
};

type PendingBackground = {
  stage: AssetPrewarmStage;
  promise: Promise<void>;
  resolve: () => void;
  reject: (error: unknown) => void;
};

type ActiveBackground = {
  stage: AssetPrewarmStage;
  controller: AbortController;
  promise: Promise<void>;
};

declare global {
  interface Window {
    __mir2AssetFirstPlayable?: boolean;
    __mir2AssetFirstPlayableDetail?: Record<string, unknown>;
  }
}

/**
 * Owns prewarm ordering and lifecycle gates. Critical work is serialized and is
 * never cancelled. Background work is either disabled, immediate, or held until
 * the first playable frame. A newer screen replaces stale background work so a
 * fast login -> select -> game transition cannot make obsolete media compete
 * with the active scene.
 */
export class AssetPrewarmOrchestrator {
  private readonly backgroundMode: BackgroundPrewarmMode;
  private readonly run: AssetOrchestratorRun;
  private readonly hasWork: (stage: AssetPrewarmStage, lane: AssetPrewarmLane) => boolean;
  private readonly completed = new Set<string>();
  private readonly criticalPromises = new Map<string, Promise<void>>();
  private criticalQueue: Promise<void> = Promise.resolve();
  private pendingBackground: PendingBackground | null = null;
  private activeBackground: ActiveBackground | null = null;
  private firstPlayable = false;
  private requestedStage: AssetPrewarmStage | null = null;
  private disposed = false;

  constructor(options: AssetOrchestratorOptions) {
    this.backgroundMode = options.backgroundMode;
    this.run = options.run;
    this.hasWork = options.hasWork ?? (() => true);
  }

  requestStage(stage: AssetPrewarmStage): Promise<void> {
    if (this.disposed) return Promise.resolve();
    this.requestedStage = stage;
    const critical = this.scheduleCritical(stage);
    const background = this.scheduleBackground(stage);
    return Promise.all([critical, background]).then(() => undefined);
  }

  markFirstPlayable(): Promise<void> {
    if (this.disposed) return Promise.resolve();
    this.firstPlayable = true;
    const pending = this.pendingBackground;
    if (!pending) return Promise.resolve();
    this.pendingBackground = null;
    const running = this.startBackground(pending.stage);
    running.then(pending.resolve, pending.reject);
    return running;
  }

  snapshot(): AssetOrchestratorSnapshot {
    return {
      backgroundMode: this.backgroundMode,
      firstPlayable: this.firstPlayable,
      requestedStage: this.requestedStage,
      pendingBackgroundStage: this.pendingBackground?.stage ?? null,
      activeBackgroundStage: this.activeBackground?.stage ?? null,
      completed: [...this.completed].sort(),
      disposed: this.disposed,
    };
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.activeBackground?.controller.abort("asset-orchestrator-disposed");
    this.pendingBackground?.resolve();
    this.pendingBackground = null;
  }

  private scheduleCritical(stage: AssetPrewarmStage): Promise<void> {
    const key = phaseKey(stage, "critical");
    if (!this.hasWork(stage, "critical")) {
      return Promise.resolve();
    }
    if (this.completed.has(key)) return Promise.resolve();
    const existing = this.criticalPromises.get(key);
    if (existing) return existing;

    const controller = new AbortController();
    const promise = this.criticalQueue
      .catch(() => undefined)
      .then(async () => {
        if (this.disposed) return;
        await this.run(stage, "critical", controller.signal);
        if (!this.disposed) this.completed.add(key);
      })
      .finally(() => {
        if (this.criticalPromises.get(key) === promise) {
          this.criticalPromises.delete(key);
        }
      });
    this.criticalQueue = promise.catch(() => undefined);
    this.criticalPromises.set(key, promise);
    return promise;
  }

  private scheduleBackground(stage: AssetPrewarmStage): Promise<void> {
    const key = phaseKey(stage, "background");
    if (!this.hasWork(stage, "background") || this.backgroundMode === "off") {
      return Promise.resolve();
    }
    if (this.completed.has(key)) return Promise.resolve();
    if (this.activeBackground?.stage === stage) return this.activeBackground.promise;
    if (this.pendingBackground?.stage === stage) return this.pendingBackground.promise;

    if (this.backgroundMode === "afterPlayable" && !this.firstPlayable) {
      // Only the newest not-yet-visible screen remains useful. Resolve the
      // superseded caller without running its obsolete background pack.
      this.pendingBackground?.resolve();
      const deferred = createDeferredBackground(stage);
      this.pendingBackground = deferred;
      return deferred.promise;
    }

    return this.startBackground(stage);
  }

  private startBackground(stage: AssetPrewarmStage): Promise<void> {
    const key = phaseKey(stage, "background");
    if (this.completed.has(key) || this.disposed) return Promise.resolve();
    if (this.activeBackground?.stage === stage) return this.activeBackground.promise;

    const previous = this.activeBackground;
    previous?.controller.abort("asset-stage-superseded");
    const controller = new AbortController();
    const promise = (previous?.promise.catch(() => undefined) ?? Promise.resolve())
      .then(async () => {
        if (this.disposed || controller.signal.aborted) return;
        await this.run(stage, "background", controller.signal);
        if (!this.disposed && !controller.signal.aborted) this.completed.add(key);
      })
      .catch((error) => {
        if (!controller.signal.aborted && !isAbortError(error)) throw error;
      })
      .finally(() => {
        if (this.activeBackground?.promise === promise) {
          this.activeBackground = null;
        }
      });
    this.activeBackground = { stage, controller, promise };
    return promise;
  }
}

export function signalAssetFirstPlayable(detail?: Record<string, unknown>) {
  if (typeof window === "undefined") return;
  window.__mir2AssetFirstPlayable = true;
  window.__mir2AssetFirstPlayableDetail = detail;
  window.dispatchEvent(
    new CustomEvent(MIR2_ASSET_FIRST_PLAYABLE_EVENT, {
      detail,
    }),
  );
}

export function isAbortError(error: unknown) {
  return (
    (typeof DOMException !== "undefined" &&
      error instanceof DOMException &&
      error.name === "AbortError") ||
    (error instanceof Error && error.name === "AbortError")
  );
}

function phaseKey(stage: AssetPrewarmStage, lane: AssetPrewarmLane) {
  return `${stage}:${lane}`;
}

function createDeferredBackground(stage: AssetPrewarmStage): PendingBackground {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { stage, promise, resolve, reject };
}
