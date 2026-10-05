"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { currentBevyHpOrb, currentBevyMpOrb, supportsBevyHpOrb, supportsBevyMpOrb,
  type BevyHpOrbStatus, type BevyMpOrbStatus } from "./bevy-hp-orb";
import { currentBevyExperienceBar, stripUnsupportedExperienceBar, supportsBevyExperienceBar,
  type BevyExperienceBarStatus } from "./bevy-experience-bar";
import { currentBevyWeightBar, stripUnsupportedWeightBar, supportsBevyWeightBar,
  type BevyWeightBarStatus } from "./bevy-weight-bar";
import { createHudBarVisualClock, nextHudBarPlanCommit, readHudBarDrawPlan,
  currentHudBarPlans, supportsHudBarDrawPlan, type CurrentHudBarPlans } from "./bevy-hud-bar-draw-plan";
import {
  currentBevyQuestLocale, supportsBevyQuestLocale, stripUnsupportedQuestLocale,
  readBevyQuestUiStatus,
  type BevyQuestUiIntent,
  type BevyQuestUiIntentResult,
  type BevyQuestUiRuntime,
  type BevyQuestUiSnapshot,
} from "./bevy-quest-ui";

type SnapshotInput = Omit<BevyQuestUiSnapshot, "revision" | "openRevision" | "presentation" | "hudBarPlan"> & {
  presentation: BevyQuestUiSnapshot["presentation"] | null;
  hudBarPlayerObjectId?: string | null;
  hudBarRuntimeLifetime?: number;
};
type Options = {
  requested: boolean;
  runtimeGeneration: number;
  runtimeRef: { current: BevyQuestUiRuntime | null };
  snapshot: () => SnapshotInput;
  onIntent: (intent: BevyQuestUiIntent) => BevyQuestUiIntentResult;
  onOpenChange: (open: boolean) => void;
};

/** One UI owner at a time. A failed, stale, hidden, or unavailable surface yields to React. */
export function useBevyQuestUi(options: Options) {
  const latest = useRef(options);
  latest.current = options;
  // Effects restart on renderer changes and in React StrictMode. Their cleanup
  // snapshots and successor snapshots must share the same monotonic clock.
  const clock = useRef({ revision: 0, openRevision: 0 });
  const hudBarClock = useRef(createHudBarVisualClock());
  const refreshRef = useRef<(() => void) | null>(null);
  const refresh = useCallback(() => refreshRef.current?.(), []);
  const liveHudBarReaderRef = useRef<(() => CurrentHudBarPlans | null) | null>(null);
  const readLiveHudBarPlans = useCallback(() => {
    try { return liveHudBarReaderRef.current?.() ?? null; }
    catch { return null; }
  }, []);
  const [state, setState] = useState({ ready: false, capturesPointer: false, error: null as string | null,
    hpOrb: null as BevyHpOrbStatus | null, mpOrb: null as BevyMpOrbStatus | null,
    experienceBar: null as BevyExperienceBarStatus | null, weightBar: null as BevyWeightBarStatus | null,
    hudBarPlans: null as CurrentHudBarPlans | null });
  useEffect(() => {
    const runtime = latest.current.runtimeRef.current;
    const publishState = (ready: boolean, capturesPointer: boolean, error: string | null,
      hpOrb: BevyHpOrbStatus | null = null, mpOrb: BevyMpOrbStatus | null = null,
      experienceBar: BevyExperienceBarStatus | null = null,
      weightBar: BevyWeightBarStatus | null = null,
      hudBarPlans: CurrentHudBarPlans | null = null) => {
      setState((current) => current.ready === ready && current.capturesPointer === capturesPointer && current.error === error
        && JSON.stringify(current.hpOrb) === JSON.stringify(hpOrb)
        && JSON.stringify(current.mpOrb) === JSON.stringify(mpOrb)
        && JSON.stringify(current.experienceBar) === JSON.stringify(experienceBar)
        && JSON.stringify(current.weightBar) === JSON.stringify(weightBar)
        && JSON.stringify(current.hudBarPlans) === JSON.stringify(hudBarPlans)
        ? current : { ready, capturesPointer, error, hpOrb, mpOrb, experienceBar, weightBar, hudBarPlans });
    };
    publishState(false, false, null);
    if (!options.requested || !runtime?.setMir2QuestUiSnapshot || !runtime.setMir2QuestUiIntentSink) return;
    let stopped = false;
    let generation = -1;
    let revision = clock.current.revision;
    let openRevision = clock.current.openRevision;
    const nextRevision = () => revision = ++clock.current.revision;
    const nextOpenRevision = () => openRevision = ++clock.current.openRevision;
    let lastRequestedOpen = false;
    let lastSnapshot = "";
    let currentSnapshot: SnapshotInput | null = null;
    let lastValidSnapshot: SnapshotInput | null = null;
    let acknowledgedSnapshot: BevyQuestUiSnapshot | null = null;
    let requiredPresentationRevision = 0;
    let lastPresentation = "";
    let lastFrame = -1;
    let lastFrameAt = 0;
    let rendererHealthy = false;
    const liveHudBarReader = (): CurrentHudBarPlans | null => {
      if (stopped || !latest.current.requested || latest.current.runtimeRef.current !== runtime
        || latest.current.runtimeGeneration !== options.runtimeGeneration
        || !supportsHudBarDrawPlan(runtime) || !acknowledgedSnapshot?.hudBarPlan) return null;
      const live = latest.current.snapshot();
      if (!live.inGame || !live.hostVisible || !live.presentation || live.dialog.hasInput
        || live.hudBarRuntimeLifetime !== options.runtimeGeneration) return null;
      const commit = nextHudBarPlanCommit({ ...hudBarClock.current }, live.hudBarRuntimeLifetime,
        live.hudBarPlayerObjectId ?? null,
        { ...live, presentation: live.presentation });
      if (!commit || commit.lifetime !== acknowledgedSnapshot.hudBarPlan.lifetime) return null;
      const status = readBevyQuestUiStatus(runtime, live.generation);
      const now = performance.now();
      if (!status || !Number.isFinite(now) || status.frame < lastFrame) return null;
      if (status.frame > lastFrame) { lastFrame = status.frame; lastFrameAt = now; }
      if (now < lastFrameAt || now - lastFrameAt > 2000) return null;
      const liveSnapshot: BevyQuestUiSnapshot = { ...acknowledgedSnapshot, ...live,
        presentation: live.presentation, hudBarPlan: commit };
      return currentHudBarPlans(readHudBarDrawPlan(runtime), liveSnapshot, status.revision, true);
    };
    liveHudBarReaderRef.current = liveHudBarReader;
    runtime.setMir2QuestUiIntentSink((json) => {
      try {
        const intent = JSON.parse(json) as BevyQuestUiIntent;
        // Read the live snapshot as well: logout/character reset can precede
        // the next polling tick and must invalidate a queued old UI click.
        const live = latest.current.snapshot();
        // Browser suspension also pauses our interval. Never trust its cached
        // health flag when an input callback arrives before the next tick.
        const currentStatus = readBevyQuestUiStatus(runtime, live.generation);
        const now = performance.now();
        if (currentStatus && currentStatus.frame > lastFrame) {
          lastFrame = currentStatus.frame;
          lastFrameAt = now;
          rendererHealthy = currentStatus.ready;
        }
        if (stopped || !rendererHealthy || !currentStatus?.ready || now - lastFrameAt > 2000
          || !latest.current.requested || !live.inGame || !live.hostVisible
          || !live.presentation || !currentSnapshot?.presentation
          || JSON.stringify(live.presentation) !== JSON.stringify(currentSnapshot.presentation)
          || (supportsBevyQuestLocale(runtime) && live.language !== currentSnapshot.language)
          || !currentBevyQuestLocale(currentStatus, live.language, requiredPresentationRevision)
          || currentStatus.revision < requiredPresentationRevision || intent.generation !== live.generation
          || intent.generation !== currentSnapshot.generation || typeof intent.type !== "string") {
          return JSON.stringify({ accepted: false, error: "The game session changed." });
        }
        return JSON.stringify(latest.current.onIntent(intent));
      } catch {
        return JSON.stringify({ accepted: false, error: "The quest action could not be sent." });
      }
    });
    const tick = () => {
      if (stopped) return;
      try {
        const liveSnapshot = latest.current.snapshot();
        // Older UI runtimes reject unknown snapshot fields, including nested
        // player keys. Probe each image capability before serializing the DTO.
        const hpSupported = supportsBevyHpOrb(runtime);
        const mpSupported = supportsBevyMpOrb(runtime);
        const experienceSupported = supportsBevyExperienceBar(runtime);
        const weightSupported = supportsBevyWeightBar(runtime);
        const drawPlanSupported = supportsHudBarDrawPlan(runtime);
        const localeSupported = supportsBevyQuestLocale(runtime);
        const { hudBarPlayerObjectId, hudBarRuntimeLifetime, ...hostSnapshot } = liveSnapshot;
        const player = { ...liveSnapshot.player };
        if (!mpSupported) { delete player.mp; delete player.maxMp; }
        const hudBarPlan = drawPlanSupported && hudBarRuntimeLifetime !== undefined
          ? nextHudBarPlanCommit(hudBarClock.current, hudBarRuntimeLifetime, hudBarPlayerObjectId ?? null,
            { ...hostSnapshot, player, presentation: hostSnapshot.presentation as BevyQuestUiSnapshot["presentation"] })
          : null;
        currentSnapshot = stripUnsupportedQuestLocale(stripUnsupportedWeightBar(stripUnsupportedExperienceBar({ ...hostSnapshot, player,
          hpOrbSlot: hpSupported || mpSupported ? liveSnapshot.hpOrbSlot : undefined,
          experienceBarSlot: liveSnapshot.experienceBarSlot,
          weightBarSlot: liveSnapshot.weightBarSlot,
          ...(hudBarPlan ? { hudBarPlan } : {}) }, experienceSupported || drawPlanSupported), weightSupported || drawPlanSupported), localeSupported);
        if (generation !== currentSnapshot.generation) {
          generation = currentSnapshot.generation;
          lastSnapshot = "";
          lastPresentation = "";
          lastValidSnapshot = null;
          acknowledgedSnapshot = null;
          lastRequestedOpen = !currentSnapshot.questLogOpen;
        }
        if (currentSnapshot.questLogOpen !== lastRequestedOpen) {
          nextOpenRevision();
          lastRequestedOpen = currentSnapshot.questLogOpen;
        }
        if (!currentSnapshot.presentation) {
          rendererHealthy = false;
          lastPresentation = "";
          // Geometry loss is a surface handoff, not a session exit. Tell Rust
          // to hide and stop local input as well as hiding the HTML canvas.
          // The host DTO explicitly accepts null metrics, so do not invent a
          // size or retain an old session/model merely to publish this edge.
          const hidden = { ...currentSnapshot, hostVisible: false, openRevision };
          const serialized = JSON.stringify(hidden);
          if (serialized !== lastSnapshot) {
            const hiddenRevision = nextRevision();
            if (!runtime.setMir2QuestUiSnapshot!(JSON.stringify({ ...hidden, revision: hiddenRevision }))) {
              throw new Error("Shared quest UI rejected the layout handoff");
            }
            acknowledgedSnapshot = null;
            lastSnapshot = serialized;
          }
          publishState(false, false, "Shared quest UI is waiting for a valid stage layout");
          return;
        }
        lastValidSnapshot = currentSnapshot;
        const serialized = JSON.stringify({ ...currentSnapshot, openRevision });
        if (serialized !== lastSnapshot) {
          const sentRevision = nextRevision();
          if (!runtime.setMir2QuestUiSnapshot!(JSON.stringify({ ...currentSnapshot, openRevision, revision: sentRevision }))) {
            throw new Error("Shared quest UI rejected the current snapshot");
          }
          acknowledgedSnapshot = { ...currentSnapshot, presentation: currentSnapshot.presentation,
            openRevision, revision: sentRevision };
          // A new layout needs an applied geometry acknowledgement. Ordinary
          // HP/quest/NPC updates keep the same renderer owner while Bevy consumes
          // the model on its next frame; switching to React on each update
          // would visibly replace panels for an entire polling interval.
          const presentation = JSON.stringify({ presentation: currentSnapshot.presentation, language: currentSnapshot.language });
          if (presentation !== lastPresentation) {
            requiredPresentationRevision = sentRevision;
            lastPresentation = presentation;
          }
          lastSnapshot = serialized;
        }
        const status = readBevyQuestUiStatus(runtime, generation);
        const now = performance.now();
        if (status && status.frame > lastFrame) {
          lastFrame = status.frame;
          lastFrameAt = now;
        }
        // WASM may panic after publishing ready=true. An unchanged status is
        // not proof that its event loop still draws or consumes input.
        const stalled = Boolean(status?.ready && (status.frame < lastFrame || now - lastFrameAt > 2000));
        rendererHealthy = Boolean(status?.ready && !stalled
          && currentBevyQuestLocale(status, currentSnapshot.language, requiredPresentationRevision));
        const ready = Boolean(rendererHealthy && currentSnapshot.inGame && currentSnapshot.hostVisible && !currentSnapshot.dialog.hasInput);
        // Lock world input as soon as the host asks to display a window. The
        // prior Bevy frame can still report no dialog until its next update.
        const hpOrb = currentBevyHpOrb(status, acknowledgedSnapshot,
          Boolean(status && status.frame >= lastFrame && now - lastFrameAt <= 2000));
        const mpOrb = currentBevyMpOrb(status, acknowledgedSnapshot,
          Boolean(status && status.frame >= lastFrame && now - lastFrameAt <= 2000));
        const experienceBar = currentBevyExperienceBar(status, acknowledgedSnapshot,
          Boolean(status && status.frame >= lastFrame && now - lastFrameAt <= 2000));
        const weightBar = currentBevyWeightBar(status, acknowledgedSnapshot,
          Boolean(status && status.frame >= lastFrame && now - lastFrameAt <= 2000));
        const hudBarPlans = drawPlanSupported ? currentHudBarPlans(readHudBarDrawPlan(runtime), acknowledgedSnapshot,
          status?.revision ?? -1, Boolean(status && status.frame >= lastFrame && now - lastFrameAt <= 2000 && !stalled)) : null;
        publishState(ready, ready && Boolean(status?.capturesPointer || currentSnapshot.questLogOpen || currentSnapshot.dialog.isOpen),
          stalled ? "Shared quest renderer stopped responding" : status?.error ?? null,
          hpOrb, mpOrb, experienceBar, weightBar, hudBarPlans);
        if (ready && status && status.openRevision === openRevision && status.questLogOpen !== lastRequestedOpen) {
          lastRequestedOpen = status.questLogOpen;
          latest.current.onOpenChange(status.questLogOpen);
        }
      } catch (error) {
        rendererHealthy = false;
        publishState(false, false, error instanceof Error ? error.message : "Shared quest UI unavailable");
        if (lastValidSnapshot) {
          try {
            runtime.setMir2QuestUiSnapshot!(JSON.stringify({ ...lastValidSnapshot, inGame: false,
              hostVisible: false, revision: nextRevision(), openRevision }));
            lastSnapshot = "";
          } catch { /* The compatibility UI remains the owner. */ }
        }
      }
    };
    refreshRef.current = tick;
    tick();
    const timer = window.setInterval(tick, 100);
    return () => {
      stopped = true;
      if (liveHudBarReaderRef.current === liveHudBarReader) liveHudBarReaderRef.current = null;
      if (refreshRef.current === tick) refreshRef.current = null;
      window.clearInterval(timer);
      // HMR/remount must not leave a callable transport from the old page.
      runtime.clearMir2QuestUiIntentSink?.();
      if (lastValidSnapshot) {
        runtime.setMir2QuestUiSnapshot?.(JSON.stringify({ ...lastValidSnapshot, inGame: false,
          hostVisible: false, questLogOpen: false, revision: nextRevision(), openRevision: nextOpenRevision() }));
      }
    };
  }, [options.requested, options.runtimeGeneration]);
  return { ...state, refresh, readLiveHudBarPlans };
}
