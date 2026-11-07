"use client";
import { useCallback, useEffect, useRef, useState } from "react";
import { readHudStatus, supportsHud, type HudAction, type HudNavigation, type HudRuntime, type HudSnapshot, type HudStatus } from "./bevy-hud-ui";
type Options = { requested: boolean; runtimeGeneration: number; runtimeRef: { current: HudRuntime | null };
  snapshot: () => Omit<HudSnapshot, "revision"> | null; onNavigation: (navigation: HudNavigation) => void;
  /** Host identity/realm/world/scene fence; never sent through the Native ABI. */
  navigationSource: () => string | null };
type NavigationOwner = { runtime: HudRuntime; runtimeGeneration: number };
type NavigationBaseline = { owner: NavigationOwner; signature: string; generation: number; revision: number; frame: number };
function navigationSignature(options: Options, snapshot: Omit<HudSnapshot, "revision"> | null) {
  const source = options.navigationSource();
  if (!options.requested || !snapshot?.inGame || !snapshot.hostVisible || !snapshot.player
    || typeof source !== "string" || source.length === 0) return null;
  // A stat projection may wait for its next native frame without invalidating
  // panel navigation. Identity, scene, presentation and explicit navigation
  // edits still require their own fully ingested ready snapshot.
  return JSON.stringify({ source, generation: snapshot.generation, inGame: snapshot.inGame, hostVisible: snapshot.hostVisible,
    logicalWidth: snapshot.logicalWidth, logicalHeight: snapshot.logicalHeight, stageCssScale: snapshot.stageCssScale,
    touch: snapshot.touch, hpView: snapshot.hpView, navigationRevision: snapshot.navigationRevision,
    player: { name: snapshot.player.name, className: snapshot.player.className, gender: snapshot.player.gender } });
}
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
  const navigationOwner = useRef<NavigationOwner | null>(null);
  const navigationBaseline = useRef<NavigationBaseline | null>(null);
  const navigationDiagnostics = useRef<Array<{ action: string; accepted: boolean; fresh: boolean;
    generation: number | null; revision: number | null; snapshotMatches: boolean; appliedPending: boolean;
    ready: boolean; errorPresent: boolean; navigationSourceMatches: boolean; nativeBaseline: boolean; navigationFresh: boolean }>>([]);
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
  const readNavigationCurrent = useCallback(() => {
    try {
      const o = latest.current, owner = navigationOwner.current, baseline = navigationBaseline.current;
      const runtime = o.runtimeRef.current;
      if (!owner || !baseline || baseline.owner !== owner || runtime !== owner.runtime
        || o.runtimeGeneration !== owner.runtimeGeneration || !supportsHud(runtime)) {
        navigationBaseline.current = null; return null;
      }
      const snapshot = o.snapshot(), signature = navigationSignature(o, snapshot);
      if (!snapshot || signature === null || signature !== baseline.signature || snapshot.generation !== baseline.generation) {
        navigationBaseline.current = null; return null;
      }
      const current = readHudStatus(runtime, snapshot.generation), now = performance.now();
      if (!current?.ready || current.error !== null || current.revision < baseline.revision || current.frame < baseline.frame
        || current.frame < health.current.frame) {
        navigationBaseline.current = null; return null;
      }
      if (current.frame > health.current.frame) health.current = { frame: current.frame, at: now };
      if (now - health.current.at > 2000) { navigationBaseline.current = null; return null; }
      baseline.revision = current.revision; baseline.frame = current.frame;
      return { owner, current, signature };
    } catch { navigationBaseline.current = null; return null; }
  }, []);
  const dispatch = useCallback((action: HudAction) => {
    const navigation = readNavigationCurrent();
    let accepted = false;
    try {
      if (navigation) {
        const o = latest.current, { owner, current, signature } = navigation;
        const sourceMatches = navigationSignature(o, o.snapshot()) === signature;
        const capable = supportsHud(owner.runtime);
        // Recheck the host source after reading Native. Native itself accepts
        // only this exact current generation/revision; false/throw is final.
        if (sourceMatches && capable && navigationOwner.current === owner && navigationBaseline.current?.owner === owner
          && o.runtimeRef.current === owner.runtime && o.runtimeGeneration === owner.runtimeGeneration
          && o.requested) {
          accepted = owner.runtime.dispatchMir2HudNavigation?.(JSON.stringify({ generation: current.generation, revision: current.revision, action })) === true;
        } else navigationBaseline.current = null;
      }
    } catch { navigationBaseline.current = null; /* A navigation edge is never queued or retried. */ }
    try {
      const o = latest.current, snapshot = o.snapshot(), stamp = submittedStamp.current;
      const observed = snapshot ? readHudStatus(latest.current.runtimeRef.current, snapshot.generation) : null;
      const baseline = navigationBaseline.current;
      navigationDiagnostics.current.push({ action: action.type, accepted, fresh: !!readCurrent(),
        generation: observed?.generation ?? null, revision: observed?.revision ?? null,
        snapshotMatches: !!snapshot && JSON.stringify({ ...snapshot, navigation: undefined }) === stamp?.signature,
        appliedPending: !!observed && !!stamp && observed.revision < stamp.revision,
        ready: observed?.ready === true, errorPresent: !!observed?.error,
        navigationSourceMatches: !!baseline && navigationSignature(o, snapshot) === baseline.signature,
        nativeBaseline: !!baseline, navigationFresh: !!navigation });
      if (navigationDiagnostics.current.length > 8) navigationDiagnostics.current.shift();
    } catch { /* Diagnostics never authorize a navigation action. */ }
    return accepted;
  }, [readCurrent, readNavigationCurrent]);
  const diagnostic = useCallback(() => [...navigationDiagnostics.current], []);
  useEffect(() => {
    const runtime = latest.current.runtimeRef.current;
    let stopped = false, signature = "", appliedRevision = 0, previousNav = "", withdrawn = false;
    let submittedNavigationSignature: string | null = null;
    let lastSubmitted: HudSnapshot | null = null;
    const withdraw = () => {
      signature = ""; submittedStamp.current = null; navigationBaseline.current = null; submittedNavigationSignature = null;
      if (lastSubmitted && !withdrawn) {
        const hidden = { ...lastSubmitted, revision: nextRevision(), hostVisible: false };
        runtime?.setMir2HudUiSnapshot?.(JSON.stringify(hidden));
        withdrawn = true;
      }
    };
    health.current = { frame: -1, at: 0 };
    navigationOwner.current = null; navigationBaseline.current = null;
    setStatus(null); liveStatus.current = null;
    if (!options.requested || !runtime || !supportsHud(runtime)) return;
    const owner: NavigationOwner = { runtime, runtimeGeneration: options.runtimeGeneration };
    navigationOwner.current = owner;
    const tick = () => {
      if (stopped || latest.current.runtimeRef.current !== runtime || latest.current.runtimeGeneration !== owner.runtimeGeneration) return;
      try {
        const snapshot = latest.current.snapshot();
        if (!snapshot) { withdraw(); setStatus(null); liveStatus.current = null; return; }
        const navSignature = navigationSignature(latest.current, snapshot);
        if (navSignature !== navigationBaseline.current?.signature) navigationBaseline.current = null;
        // Navigation is only an initial-generation bootstrap. Ordinary React
        // projections cannot become independent state authority after handoff.
        const serialized = JSON.stringify({ ...snapshot, navigation: undefined });
        if (serialized !== signature || navSignature !== submittedNavigationSignature) {
          appliedRevision = nextRevision();
          const submitted = { ...snapshot, revision: appliedRevision };
          if (!runtime?.setMir2HudUiSnapshot?.(JSON.stringify(submitted))) {
            withdraw();
            setStatus(null); liveStatus.current = null; return;
          }
          signature = serialized;
          submittedNavigationSignature = navSignature;
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
        if (navSignature !== null && current.error === null && navigationOwner.current === owner
          && latest.current.runtimeRef.current === runtime && latest.current.runtimeGeneration === owner.runtimeGeneration
          && navigationSignature(latest.current, latest.current.snapshot()) === navSignature) {
          navigationBaseline.current = { owner, signature: navSignature, generation: current.generation,
            revision: current.revision, frame: current.frame };
        }
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
      if (navigationOwner.current === owner) navigationOwner.current = null;
      withdraw();
    };
  }, [options.requested, options.runtimeGeneration, readCurrent]);
  return { ready: status?.ready === true, characterStatsReady: status?.characterStatsReady === true, status, refresh, dispatch, readCurrent, diagnostic };
}
