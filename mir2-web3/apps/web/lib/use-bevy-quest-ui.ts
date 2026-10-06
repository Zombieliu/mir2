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
  readBevyQuestUiStatus, readBevyQuestNameTargets,
  type BevyQuestUiIntent,
  type BevyQuestUiIntentResult,
  type BevyQuestUiRuntime,
  type BevyQuestUiSnapshot,
} from "./bevy-quest-ui";
import { sameQuestWorldDraft, sameQuestWorldStamp, stampBevyQuestWorldContext,
  supportsBevyQuestWorldContext, type QuestWorldDraft } from "./bevy-quest-world-context";
import { isQuestWorldAction, parseQuestWorldAction } from "./bevy-quest-world-actions";
import { readQuestWorldControls, type QuestWorldControls } from "./bevy-quest-world-controls";

type SnapshotInput = Omit<BevyQuestUiSnapshot, "revision" | "openRevision" | "presentation" | "hudBarPlan" | "worldContext"> & {
  presentation: BevyQuestUiSnapshot["presentation"] | null;
  worldDraft?: QuestWorldDraft | null;
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
  const liveWorldControlsReaderRef = useRef<(() => QuestWorldControls | null) | null>(null);
  const worldControlBlockersReaderRef = useRef<(() => QuestWorldControls | null) | null>(null);
  const questNameTargetsReaderRef = useRef<(() => readonly number[] | null) | null>(null);
  const readLiveQuestNameTargets = useCallback(() => {
    try { return questNameTargetsReaderRef.current?.() ?? null; } catch { return null; }
  }, []);
  const paintedWorldControlsRef = useRef<{ runtime: BevyQuestUiRuntime; runtimeGeneration: number;
    controls: QuestWorldControls } | null>(null);
  const readLiveWorldControls = useCallback(() => {
    try { return liveWorldControlsReaderRef.current?.() ?? null; } catch { return null; }
  }, []);
  const readLiveWorldControlBlockers = useCallback(() => {
    try { return worldControlBlockersReaderRef.current?.() ?? null; } catch { return null; }
  }, []);
  const liveHudBarReaderRef = useRef<(() => CurrentHudBarPlans | null) | null>(null);
  const readLiveHudBarPlans = useCallback(() => {
    try { return liveHudBarReaderRef.current?.() ?? null; }
    catch { return null; }
  }, []);
  const [state, setState] = useState({ ready: false, worldControlsReady: false, capturesPointer: false, error: null as string | null,
    hpOrb: null as BevyHpOrbStatus | null, mpOrb: null as BevyMpOrbStatus | null,
    experienceBar: null as BevyExperienceBarStatus | null, weightBar: null as BevyWeightBarStatus | null,
    hudBarPlans: null as CurrentHudBarPlans | null, questNameTargetObjectIds: [] as readonly number[] });
  useEffect(() => {
    const runtime = latest.current.runtimeRef.current;
    const publishState = (ready: boolean, capturesPointer: boolean, error: string | null,
      hpOrb: BevyHpOrbStatus | null = null, mpOrb: BevyMpOrbStatus | null = null,
      experienceBar: BevyExperienceBarStatus | null = null,
      weightBar: BevyWeightBarStatus | null = null,
      hudBarPlans: CurrentHudBarPlans | null = null, worldControlsReady = false, questNameTargetObjectIds: readonly number[] = []) => {
      setState((current) => current.ready === ready && current.worldControlsReady === worldControlsReady && current.capturesPointer === capturesPointer && current.error === error
        && JSON.stringify(current.hpOrb) === JSON.stringify(hpOrb)
        && JSON.stringify(current.mpOrb) === JSON.stringify(mpOrb)
        && JSON.stringify(current.experienceBar) === JSON.stringify(experienceBar)
        && JSON.stringify(current.weightBar) === JSON.stringify(weightBar)
        && JSON.stringify(current.hudBarPlans) === JSON.stringify(hudBarPlans)
        && JSON.stringify(current.questNameTargetObjectIds) === JSON.stringify(questNameTargetObjectIds)
        ? current : { ready, worldControlsReady, capturesPointer, error, hpOrb, mpOrb, experienceBar, weightBar, hudBarPlans, questNameTargetObjectIds });
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
    let currentSnapshot: Omit<SnapshotInput, "worldDraft"> | null = null;
    let lastValidSnapshot: BevyQuestUiSnapshot | null = null;
    let acknowledgedSnapshot: BevyQuestUiSnapshot | null = null;
    let requiredPresentationRevision = 0;
    let lastPresentation = "";
    let lastFrame = -1;
    let lastFrameAt = 0;
    let rendererHealthy = false;
    const documentVisible = () => typeof document === "undefined" || document.visibilityState !== "hidden";
    const liveHudBarReader = (): CurrentHudBarPlans | null => {
      if (stopped || !latest.current.requested || latest.current.runtimeRef.current !== runtime
        || latest.current.runtimeGeneration !== options.runtimeGeneration
        || !supportsHudBarDrawPlan(runtime) || !acknowledgedSnapshot?.hudBarPlan) return null;
      const live = latest.current.snapshot();
      if (!documentVisible() || !live.inGame || !live.hostVisible || !live.presentation || live.dialog.hasInput
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
    const liveWorldControlsReader = (): QuestWorldControls | null => {
      const context = acknowledgedSnapshot?.worldContext;
      if (stopped || !context || !latest.current.requested || latest.current.runtimeRef.current !== runtime
        || latest.current.runtimeGeneration !== options.runtimeGeneration || !supportsBevyQuestWorldContext(runtime)) return null;
      const live = latest.current.snapshot();
      if (!documentVisible() || !live.inGame || !live.hostVisible || !live.presentation || live.dialog.hasInput
        || JSON.stringify(live.presentation) !== JSON.stringify(acknowledgedSnapshot?.presentation)
        || !sameQuestWorldDraft(live.worldDraft ?? null, context)
        || JSON.stringify({quests:live.quests,completedKnown:live.completedKnown,completedQuestIds:live.completedQuestIds,profile:live.profile})
          !== JSON.stringify({quests:acknowledgedSnapshot?.quests,completedKnown:acknowledgedSnapshot?.completedKnown,completedQuestIds:acknowledgedSnapshot?.completedQuestIds,profile:acknowledgedSnapshot?.profile})) return null;
      const status = readBevyQuestUiStatus(runtime, live.generation), now = performance.now();
      if (!status?.ready || !Number.isFinite(now) || status.frame < lastFrame
        || status.revision !== context.revision || !currentBevyQuestLocale(status, live.language, requiredPresentationRevision)) return null;
      if (status.frame > lastFrame) { lastFrame = status.frame; lastFrameAt = now; }
      if (now < lastFrameAt || now - lastFrameAt > 2000) return null;
      return readQuestWorldControls(runtime, context, live.presentation.logicalWidth, live.presentation.logicalHeight);
    };
    liveWorldControlsReaderRef.current = liveWorldControlsReader;
    const questNameTargetsReader = (): readonly number[] | null => {
      const context = acknowledgedSnapshot?.worldContext;
      if (stopped || !context || !latest.current.requested || latest.current.runtimeRef.current !== runtime
        || latest.current.runtimeGeneration !== options.runtimeGeneration || !documentVisible()) return null;
      const live = latest.current.snapshot();
      if (!live.inGame || !live.hostVisible || !sameQuestWorldDraft(live.worldDraft ?? null, context)
        || JSON.stringify({ quests: live.quests, completedKnown: live.completedKnown, completedQuestIds: live.completedQuestIds, profile: live.profile })
          !== JSON.stringify({ quests: acknowledgedSnapshot?.quests, completedKnown: acknowledgedSnapshot?.completedKnown,
            completedQuestIds: acknowledgedSnapshot?.completedQuestIds, profile: acknowledgedSnapshot?.profile })) return null;
      const status = readBevyQuestUiStatus(runtime, live.generation), now = performance.now();
      if (!status?.ready || status.revision !== context.revision || status.frame < lastFrame
        || !Number.isFinite(now) || now < lastFrameAt || now - lastFrameAt > 2000) return null;
      return readBevyQuestNameTargets(runtime, context);
    };
    questNameTargetsReaderRef.current = questNameTargetsReader;
    const worldControlBlockersReader = (): QuestWorldControls | null => {
      if (stopped || !latest.current.requested || !documentVisible() || latest.current.runtimeRef.current !== runtime
        || latest.current.runtimeGeneration !== options.runtimeGeneration) return null;
      const live = latest.current.snapshot();
      if (!live.inGame || !live.hostVisible || !live.presentation || live.dialog.hasInput) return null;
      const context = acknowledgedSnapshot?.worldContext;
      const status = readBevyQuestUiStatus(runtime, live.generation), now = performance.now();
      if (context && status?.ready && status.revision === context.revision
        && status.frame >= lastFrame && Number.isFinite(now) && now >= lastFrameAt && now - lastFrameAt <= 2000) {
        // These rectangles describe painted pixels only. Source freshness is
        // still required separately before any Quest action is accepted.
        const measured = readQuestWorldControls(runtime, context, live.presentation.logicalWidth, live.presentation.logicalHeight, true);
        if (measured) paintedWorldControlsRef.current = { runtime, runtimeGeneration: options.runtimeGeneration, controls: measured };
      }
      const painted = paintedWorldControlsRef.current;
      return painted?.runtime === runtime && painted.runtimeGeneration === options.runtimeGeneration ? painted.controls : null;
    };
    worldControlBlockersReaderRef.current = worldControlBlockersReader;
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
        if (stopped || !documentVisible() || !rendererHealthy || !currentStatus?.ready || now - lastFrameAt > 2000
          || latest.current.runtimeRef.current !== runtime
          || latest.current.runtimeGeneration !== options.runtimeGeneration
          || !latest.current.requested || !live.inGame || !live.hostVisible
          || !live.presentation || !currentSnapshot?.presentation
          || JSON.stringify(live.presentation) !== JSON.stringify(currentSnapshot.presentation)
          || (supportsBevyQuestLocale(runtime) && live.language !== currentSnapshot.language)
          || !currentBevyQuestLocale(currentStatus, live.language, requiredPresentationRevision)
          || currentStatus.revision < requiredPresentationRevision || intent.generation !== live.generation
          || intent.generation !== currentSnapshot.generation || typeof intent.type !== "string") {
          return JSON.stringify({ accepted: false, error: "The game session changed." });
        }
        if (isQuestWorldAction(intent.type)) {
          const context = acknowledgedSnapshot?.worldContext ?? null;
          if (!parseQuestWorldAction(intent) || !context || !sameQuestWorldStamp(intent, context)
            || currentStatus.revision < context.revision
            || !sameQuestWorldDraft(live.worldDraft ?? null, context)
            || JSON.stringify({ quests: live.quests, completedKnown: live.completedKnown,
              completedQuestIds: live.completedQuestIds, profile: live.profile })
              !== JSON.stringify({ quests: acknowledgedSnapshot?.quests,
                completedKnown: acknowledgedSnapshot?.completedKnown,
                completedQuestIds: acknowledgedSnapshot?.completedQuestIds,
                profile: acknowledgedSnapshot?.profile })) {
            return JSON.stringify({ accepted: false, error: "The quest world changed." });
          }
        }
        return JSON.stringify(latest.current.onIntent(intent));
      } catch {
        return JSON.stringify({ accepted: false, error: "The quest action could not be sent." });
      }
    });
    const tick = () => {
      if (stopped) return;
      try {
        const inputSnapshot = latest.current.snapshot();
        const liveSnapshot = { ...inputSnapshot, hostVisible: inputSnapshot.hostVisible && documentVisible() };
        // Older UI runtimes reject unknown snapshot fields, including nested
        // player keys. Probe each image capability before serializing the DTO.
        const hpSupported = supportsBevyHpOrb(runtime);
        const mpSupported = supportsBevyMpOrb(runtime);
        const experienceSupported = supportsBevyExperienceBar(runtime);
        const weightSupported = supportsBevyWeightBar(runtime);
        const drawPlanSupported = supportsHudBarDrawPlan(runtime);
        const localeSupported = supportsBevyQuestLocale(runtime);
        const worldContextSupported = supportsBevyQuestWorldContext(runtime);
        const { hudBarPlayerObjectId, hudBarRuntimeLifetime, worldDraft, ...hostSnapshot } = liveSnapshot;
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
        const serialized = JSON.stringify({ ...currentSnapshot, openRevision,
          ...(worldContextSupported ? { worldDraft } : {}) });
        if (serialized !== lastSnapshot) {
          const sentRevision = nextRevision();
          const worldContext = worldContextSupported && currentSnapshot.inGame && currentSnapshot.hostVisible
            ? stampBevyQuestWorldContext(worldDraft ?? null, sentRevision) : null;
          const sentSnapshot: BevyQuestUiSnapshot = { ...currentSnapshot, presentation: currentSnapshot.presentation,
            openRevision, revision: sentRevision, ...(worldContext ? { worldContext } : {}) };
          if (!runtime.setMir2QuestUiSnapshot!(JSON.stringify(sentSnapshot))) {
            throw new Error("Shared quest UI rejected the current snapshot");
          }
          lastValidSnapshot = sentSnapshot;
          acknowledgedSnapshot = sentSnapshot;
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
          hpOrb, mpOrb, experienceBar, weightBar, hudBarPlans, ready && liveWorldControlsReader() !== null,
          questNameTargetsReader() ?? []);
        worldControlBlockersReader();
        if (ready && status && status.openRevision === openRevision && status.questLogOpen !== lastRequestedOpen) {
          lastRequestedOpen = status.questLogOpen;
          latest.current.onOpenChange(status.questLogOpen);
        }
      } catch (error) {
        rendererHealthy = false;
        publishState(false, false, error instanceof Error ? error.message : "Shared quest UI unavailable");
        if (lastValidSnapshot) {
          try {
            runtime.setMir2QuestUiSnapshot!(JSON.stringify({ ...lastValidSnapshot, worldContext: undefined, inGame: false,
              hostVisible: false, revision: nextRevision(), openRevision }));
            lastSnapshot = "";
          } catch { /* The compatibility UI remains the owner. */ }
        }
      }
    };
    refreshRef.current = tick;
    tick();
    if (typeof document !== "undefined") document.addEventListener("visibilitychange", tick);
    const timer = window.setInterval(tick, 100);
    return () => {
      stopped = true;
      if (liveHudBarReaderRef.current === liveHudBarReader) liveHudBarReaderRef.current = null;
      if (liveWorldControlsReaderRef.current === liveWorldControlsReader) liveWorldControlsReaderRef.current = null;
      if (worldControlBlockersReaderRef.current === worldControlBlockersReader) worldControlBlockersReaderRef.current = null;
      if (questNameTargetsReaderRef.current === questNameTargetsReader) questNameTargetsReaderRef.current = null;
      if (refreshRef.current === tick) refreshRef.current = null;
      window.clearInterval(timer);
      if (typeof document !== "undefined") document.removeEventListener("visibilitychange", tick);
      // HMR/remount must not leave a callable transport from the old page.
      runtime.clearMir2QuestUiIntentSink?.();
      if (lastValidSnapshot) {
        runtime.setMir2QuestUiSnapshot?.(JSON.stringify({ ...lastValidSnapshot, worldContext: undefined, inGame: false,
          hostVisible: false, questLogOpen: false, revision: nextRevision(), openRevision: nextOpenRevision() }));
      }
    };
  }, [options.requested, options.runtimeGeneration]);
  return { ...state, refresh, readLiveHudBarPlans, readLiveWorldControls, readLiveWorldControlBlockers, readLiveQuestNameTargets };
}
