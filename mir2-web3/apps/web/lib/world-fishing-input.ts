import type { EntityAnimationRuntime } from "./client-presentation-runtime";
import type { WorldFishingSourceRecord } from "./world-fishing-source";

export type WorldFishingGesture = Readonly<{ token: object }>;
export type WorldFishingPointer = Readonly<{ source: object; stage: HTMLElement; pointerId: number;
  pointerType: "mouse" | "touch"; startedAt: number; current: () => boolean }>;
export type WorldFishingAnimationContext = Readonly<{ runtime: EntityAnimationRuntime; record: WorldFishingSourceRecord }>;
export type WorldFishingAnimationCommit = Readonly<{ context: WorldFishingAnimationContext;
  worldKey: string; worldSeed: number; atMs: number; token: object }>;
export type WorldFishingCallbacks = {
  worldFishingAnimation?: WorldFishingAnimationContext | null;
  onWorldFishingAnimationCommit?: (commit: WorldFishingAnimationCommit | null, previous?: object) => void;
  onBeginWorldFishingGesture?: (pointer: WorldFishingPointer) => WorldFishingGesture | null;
  onCancelWorldFishingGesture?: (gesture: WorldFishingGesture) => void;
};
