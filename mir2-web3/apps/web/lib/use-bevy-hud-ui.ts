"use client";
import { useCallback, useEffect, useRef, useState } from "react";
import { readHudStatus, supportsHud, type HudAction, type HudNavigation, type HudRuntime, type HudSnapshot, type HudStatus } from "./bevy-hud-ui";
type Options = { requested: boolean; runtimeGeneration: number; runtimeRef: { current: HudRuntime | null };
  snapshot: () => Omit<HudSnapshot, "revision"> | null; onNavigation: (navigation: HudNavigation) => void };
function nextRevision() {
  const scope = globalThis as typeof globalThis & { __mir2HudRevision?: number };
  const next = (scope.__mir2HudRevision ?? 0) + 1;
  if (!Number.isSafeInteger(next)) throw Error("HUD revision exhausted");
  return scope.__mir2HudRevision = next;
}
export function useBevyHudUi(options: Options) {
  const latest = useRef(options); latest.current = options;
  const [status, setStatus] = useState<HudStatus | null>(null);
  const liveStatus = useRef<HudStatus | null>(null);
  const refreshRef = useRef<(() => void) | null>(null);
  const health = useRef({ frame: -1, at: 0 });
  const submittedStamp = useRef<{ signature: string; revision: number; generation: number } | null>(null);
  const refresh = useCallback(() => refreshRef.current?.(), []);
  const readCurrent = useCallback(() => {
    try {
    const o = latest.current, snapshot = o.snapshot(), runtime = o.runtimeRef.current;
    if (!o.requested || !snapshot?.inGame || !snapshot.hostVisible || !supportsHud(runtime)) return null;
    const current = readHudStatus(runtime, snapshot.generation);
    const stamp = submittedStamp.current;
    if (!stamp || stamp.generation !== snapshot.generation || current && current.revision < stamp.revision
      || JSON.stringify({ ...snapshot, navigation: undefined }) !== stamp.signature) return null;
    const now = performance.now();
    if (current && current.frame > health.current.frame) health.current = { frame: current.frame, at: now };
    return current?.ready && now - health.current.at <= 2000 ? current : null;
    } catch { return null; }
  }, []);
  const dispatch = useCallback((action: HudAction) => {
    const current = readCurrent();
    if (!current) return false;
    return latest.current.runtimeRef.current?.dispatchMir2HudNavigation?.(JSON.stringify({ generation: current.generation, revision: current.revision, action })) === true;
  }, [readCurrent]);
  useEffect(() => {
    const runtime = latest.current.runtimeRef.current;
    let stopped = false, signature = "", appliedRevision = 0, previousNav = "", withdrawn = false;
    let lastSubmitted: HudSnapshot | null = null;
    const withdraw = () => {
      if (lastSubmitted && !withdrawn) {
        const hidden = { ...lastSubmitted, revision: nextRevision(), hostVisible: false };
        runtime?.setMir2HudUiSnapshot?.(JSON.stringify(hidden));
        withdrawn = true;
      }
      signature = "";
      submittedStamp.current = null;
    };
    health.current = { frame: -1, at: 0 };
    setStatus(null); liveStatus.current = null;
    if (!options.requested || !supportsHud(runtime)) return;
    const tick = () => {
      if (stopped || latest.current.runtimeRef.current !== runtime) return;
      try {
        const snapshot = latest.current.snapshot();
        if (!snapshot) { withdraw(); setStatus(null); liveStatus.current = null; return; }
        // Navigation is only an initial-generation bootstrap. Ordinary React
        // projections cannot become independent state authority after handoff.
        const serialized = JSON.stringify({ ...snapshot, navigation: undefined });
        if (serialized !== signature) {
          appliedRevision = nextRevision();
          const submitted = { ...snapshot, revision: appliedRevision };
          if (!runtime?.setMir2HudUiSnapshot?.(JSON.stringify(submitted))) {
            withdraw();
            setStatus(null); liveStatus.current = null; return;
          }
          signature = serialized;
          lastSubmitted = submitted; withdrawn = false;
          submittedStamp.current = { signature: serialized, revision: appliedRevision, generation: snapshot.generation };
        }
        const observed = readHudStatus(runtime, snapshot.generation)
          ?? (liveStatus.current ? readHudStatus(runtime, liveStatus.current.generation) : null);
        const now = performance.now();
        if (observed && observed.frame > health.current.frame) health.current = { frame: observed.frame, at: now };
        const waitingForIngest = observed && (observed.generation < snapshot.generation || observed.revision < appliedRevision)
          && now - health.current.at <= 2000;
        // A setter queues work for the next Bevy frame. Keep the current
        // painter through that bounded wait; a hidden snapshot would replace
        // the just-submitted full snapshot in the same mailbox.
        if (waitingForIngest) {
          if (!observed.ready) { setStatus(null); liveStatus.current = null; }
          return;
        }
        const current = readCurrent();
        if (!current && liveStatus.current?.ready) { withdraw(); }
        if (!current || current.revision < appliedRevision) { setStatus(null); liveStatus.current = null; return; }
        const nav = JSON.stringify(current.navigation);
        if (nav !== previousNav) { previousNav = nav; latest.current.onNavigation(current.navigation); }
        liveStatus.current = current;
        setStatus(old => JSON.stringify(old && { ...old, frame: 0 }) === JSON.stringify({ ...current, frame: 0 }) ? old : current);
      } catch { withdraw(); setStatus(null); liveStatus.current = null; }
    };
    refreshRef.current = tick; tick();
    const timer = window.setInterval(tick, 50);
    return () => {
      stopped = true; window.clearInterval(timer); refreshRef.current = null; liveStatus.current = null;
      withdraw();
    };
  }, [options.requested, options.runtimeGeneration, readCurrent]);
  return { ready: status?.ready === true, characterStatsReady: status?.characterStatsReady === true, status, refresh, dispatch, readCurrent };
}
