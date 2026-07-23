"use client";

// Imperative scene-motion driver (perf: kills the 30 Hz `motionNow` React render).
//
// When the GPU/Bevy entity renderer is active it interpolates the floor, the
// self-camera and every monster at DISPLAY refresh rate from step-stable motion
// windows (self-camera + entity-interp active in the shell). That makes
// the ~33 Hz `motionNow` React clock unnecessary for smooth motion — yet it still
// re-rendered the whole scene tree (jsxDEV-dominated, profiled at ~30 % of CPU).
//
// This driver lets the React `motionNow` clock drop to the slow expiry cadence
// (~10 Hz) WITHOUT the residual DOM layers (ground-drop markers + the overlay
// health-bars / chat-bubbles / damage-floaters / hit-flashes that draw on top of
// the Bevy canvas) stuttering behind the smoothly-panned Bevy sprites. It runs one
// requestAnimationFrame loop that, every display frame, writes a compositor
// transform onto the elements callers register:
//   • a CAMERA SURFACE (an existing overlay root, e.g. the ground-drop layer) gets
//     just the camera pan — one write pans all of its children; and
//   • a per-ENTITY overlay element (a monster health-bar / bubble / floater) gets
//     camera + that entity's sub-tile glide, so it tracks its Bevy sprite exactly.
// Element BASE positions (origin + tile-delta) stay React-rendered; this driver only
// adds the camera + per-entity motion as transforms. It writes to EXISTING DOM nodes
// (no new wrapper) so stacking / pointer-events are unchanged. Bevy already receives
// its own motion windows, so the driver does NOT touch the runtime.
//
// Timebase: motion snapshots stamp startedAt/expiresAt with Date.now() (the old
// `motionNow` value), so the interpolation helpers must be sampled with Date.now()
// here too — NOT performance.now() — or the offsets would be wildly off.

import { useEffect, useRef } from "react";

import {
  cameraMotionOffsetForEntity,
  entityMotionOffsetForEntity,
} from "./original-client-scene-motion";
import {
  BEVY_PRESENTATION_POSE_MAX_AGE_MS,
  compareBevyPresentationPoseProvenance,
  parseBevyPresentationPoseFrame,
  readBevyPresentationPoseFrame,
  type BevyPresentationPoseExpectedProvenance,
  type BevyPresentationEntityMotion,
  type BevyPresentationPoseFrame,
  type BevyPresentationPoseProvenanceComparison,
  type BevyPresentationPoseRuntime,
} from "./original-client-presentation-pose";
import type { ViewportOffset } from "./original-client-scene-layout";
import type { DisplayEntity, EntityMotionSnapshot } from "./original-client-types";

function translateFor(x: number, y: number): string {
  return `${x}px ${y}px`;
}

export type ScenePresentationContext = BevyPresentationPoseExpectedProvenance;

export type SceneCameraMotionDriver = {
  /**
   * Callback-ref (stable per key) for an overlay ROOT that should pan with the camera.
   * The camera transform is applied here; all descendants inherit it, so a per-entity
   * element nested under a surface only needs its own glide (see registerEntityEl).
   */
  registerCameraSurface: (key: string) => (el: HTMLElement | null) => void;
  /**
   * Callback-ref (stable per key) for a per-entity element that should add its sub-tile
   * GLIDE on top of the inherited camera. `key` is unique per element (e.g.
   * `stack:<id>`, `name:<id>`); `objectId` selects the entity's motion snapshot.
   */
  registerEntityEl: (key: string, objectId: string) => (el: HTMLElement | null) => void;
  getDiagnostics: () => SceneCameraMotionDriverDiagnostics;
};

export type SceneCameraMotionDriverDiagnostics = {
  enabled: boolean;
  bevyPoseRequested: boolean;
  poseCommitRequested: boolean;
  poseCommitActive: boolean;
  poseCommitSinkAvailable: boolean;
  poseCommitRegistrationError: string | null;
  runtimeGeneration: number;
  poseCommitFrames: number;
  polledPoseFrames: number;
  poseCommitReady: boolean;
  bevyPoseSamples: number;
  fallbackSamples: number;
  stalePoseFrames: number;
  duplicatePoseFrames: number;
  provenanceUnavailableCount: number;
  provenanceMismatchCount: number;
  postWarmupStalePoseFrames: number;
  postWarmupProvenanceUnavailableCount: number;
  postWarmupProvenanceMismatchCount: number;
  lastProvenanceComparison: BevyPresentationPoseProvenanceComparison | null;
  entityPoseHits: number;
  entityPoseMisses: number;
  remotePacketPoseHits: number;
  lastFrameId: number | null;
  lastPoseAgeMs: number | null;
  lastSinkLagMs: number | null;
  registeredSurfaceCount: number;
  registeredEntityElementCount: number;
};

type LocalCommandPoseLatencyProbe = {
  version: 1;
  armedAtMs: number;
  sinkCallbackCount: number;
  droppedSinkEventCount: number;
  sinkEvents: Array<{
    frameId: number;
    generatedAtMs: number;
    sinkAtMs: number;
    cameraSource: "localCommand";
    cameraX: number;
    cameraY: number;
    selfObjectId: string | null;
    selfSource: string | null;
    selfX: number | null;
    selfY: number | null;
    poseToSinkMs: number;
  }>;
};

const MAX_LOCAL_COMMAND_POSE_LATENCY_EVENTS = 512;

function localCommandPoseLatencyProbe(): LocalCommandPoseLatencyProbe | null {
  const probe = (
    window as typeof window & { __mir2PresentationPoseLatencyProbe?: unknown }
  ).__mir2PresentationPoseLatencyProbe;
  if (
    !probe ||
    typeof probe !== "object" ||
    (probe as Partial<LocalCommandPoseLatencyProbe>).version !== 1 ||
    !Array.isArray((probe as Partial<LocalCommandPoseLatencyProbe>).sinkEvents)
  ) {
    return null;
  }
  return probe as LocalCommandPoseLatencyProbe;
}

function recordPoseSinkCallback() {
  const probe = localCommandPoseLatencyProbe();
  if (!probe) return;
  probe.sinkCallbackCount = Math.min(
    Number.MAX_SAFE_INTEGER,
    Math.max(0, Number(probe.sinkCallbackCount) || 0) + 1,
  );
}

function recordAcceptedLocalCommandPose(
  pose: BevyPresentationPoseFrame,
  sinkAtMs: number,
  selfObjectId: string | null,
) {
  if (
    pose.camera.source !== "localCommand" ||
    (Math.abs(pose.camera.x) <= 0.001 && Math.abs(pose.camera.y) <= 0.001)
  ) {
    return;
  }
  const probe = localCommandPoseLatencyProbe();
  if (!probe) return;
  if (probe.sinkEvents.length >= MAX_LOCAL_COMMAND_POSE_LATENCY_EVENTS) {
    probe.droppedSinkEventCount = Math.min(
      Number.MAX_SAFE_INTEGER,
      Math.max(0, Number(probe.droppedSinkEventCount) || 0) + 1,
    );
    return;
  }
  const selfPose = selfObjectId ? pose.entities.get(selfObjectId) : null;
  probe.sinkEvents.push({
    frameId: pose.frameId,
    generatedAtMs: pose.generatedAtMs,
    sinkAtMs,
    cameraSource: "localCommand",
    cameraX: pose.camera.x,
    cameraY: pose.camera.y,
    selfObjectId,
    selfSource: selfPose?.source ?? null,
    selfX: selfPose?.x ?? null,
    selfY: selfPose?.y ?? null,
    poseToSinkMs: sinkAtMs - pose.generatedAtMs,
  });
}

/**
 * One rAF loop that applies the camera pan (registered surfaces) + per-entity sub-tile
 * glide (registered entity overlays) imperatively at display Hz. Inert when `enabled`
 * is false — callers keep the React `motionNow` fold for the DOM-entity fallback path,
 * where there is no Bevy interpolation to track.
 */
export function useSceneCameraMotionDriver(
  enabled: boolean,
  getRenderPlayer: () => DisplayEntity | null,
  snapshotsRef: { current: Record<string, EntityMotionSnapshot> },
  preferBevyPresentationPose = false,
  atomicPoseCommit = false,
  runtimeGeneration = 0,
  getPresentationContext?: () => ScenePresentationContext,
  onLocalSelfMotionChange?: (motion: BevyPresentationEntityMotion | null) => void,
): SceneCameraMotionDriver {
  const surfacesRef = useRef<Map<string, HTMLElement>>(new Map());
  const entityElsRef = useRef<Map<string, { el: HTMLElement; objectId: string }>>(new Map());
  const surfaceCbCacheRef = useRef<Map<string, (el: HTMLElement | null) => void>>(new Map());
  const entityCbCacheRef = useRef<Map<string, (el: HTMLElement | null) => void>>(new Map());

  // Keep the latest inputs in refs so the rAF reads fresh values every frame without
  // tearing down and re-subscribing the loop on each render.
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;
  const getRenderPlayerRef = useRef(getRenderPlayer);
  getRenderPlayerRef.current = getRenderPlayer;
  const preferBevyPresentationPoseRef = useRef(preferBevyPresentationPose);
  preferBevyPresentationPoseRef.current = preferBevyPresentationPose;
  const atomicPoseCommitRef = useRef(atomicPoseCommit);
  atomicPoseCommitRef.current = atomicPoseCommit;
  const getPresentationContextRef = useRef(getPresentationContext);
  getPresentationContextRef.current = getPresentationContext;
  const onLocalSelfMotionChangeRef = useRef(onLocalSelfMotionChange);
  onLocalSelfMotionChangeRef.current = onLocalSelfMotionChange;
  const diagnosticsRef = useRef<SceneCameraMotionDriverDiagnostics>({
    enabled,
    bevyPoseRequested: preferBevyPresentationPose,
    poseCommitRequested: atomicPoseCommit,
    poseCommitActive: false,
    poseCommitSinkAvailable: false,
    poseCommitRegistrationError: null,
    runtimeGeneration,
    poseCommitFrames: 0,
    polledPoseFrames: 0,
    poseCommitReady: false,
    bevyPoseSamples: 0,
    fallbackSamples: 0,
    stalePoseFrames: 0,
    duplicatePoseFrames: 0,
    provenanceUnavailableCount: 0,
    provenanceMismatchCount: 0,
    postWarmupStalePoseFrames: 0,
    postWarmupProvenanceUnavailableCount: 0,
    postWarmupProvenanceMismatchCount: 0,
    lastProvenanceComparison: null,
    entityPoseHits: 0,
    entityPoseMisses: 0,
    remotePacketPoseHits: 0,
    lastFrameId: null,
    lastPoseAgeMs: null,
    lastSinkLagMs: null,
    registeredSurfaceCount: 0,
    registeredEntityElementCount: 0,
  });
  diagnosticsRef.current.enabled = enabled;
  diagnosticsRef.current.bevyPoseRequested = preferBevyPresentationPose;
  diagnosticsRef.current.poseCommitRequested = atomicPoseCommit;
  diagnosticsRef.current.runtimeGeneration = runtimeGeneration;
  diagnosticsRef.current.registeredSurfaceCount = surfacesRef.current.size;
  diagnosticsRef.current.registeredEntityElementCount = entityElsRef.current.size;

  useEffect(() => {
    let frame = 0;
    let dirty = false; // did we set any transform last frame? (so we can clear on disable)
    let sinkRegistered = false;
    let lastAcceptedSinkAt = 0;
    let lastSinkFrameId = -1;
    let lastLocalSelfMotionKey: string | null = null;
    const appliedTranslations = new WeakMap<HTMLElement, string>();
    const poseDebugStampsEnabled =
      atomicPoseCommitRef.current ||
      new URLSearchParams(window.location.search).get("mir2Debug") === "1";

    const publishLocalSelfMotion = (pose: BevyPresentationPoseFrame | null) => {
      const playerId = getRenderPlayerRef.current()?.objectId;
      const selfPose = playerId ? pose?.entities.get(playerId) : null;
      const motion = selfPose?.source === "localCommand" ? selfPose.motion : null;
      const key = motion
        ? `${motion.mode}:${motion.direction}:${motion.frameIndex}`
        : "";
      if (key === lastLocalSelfMotionKey) return;
      lastLocalSelfMotionKey = key;
      onLocalSelfMotionChangeRef.current?.(motion);
    };

    const stampPoseFrame = (
      el: HTMLElement,
      pose: BevyPresentationPoseFrame | null,
      source: string | null,
    ) => {
      if (!poseDebugStampsEnabled) return;
      if (pose) {
        el.dataset.bevyPoseFrame = String(pose.frameId);
        if (source) el.dataset.bevyPoseSource = source;
        else delete el.dataset.bevyPoseSource;
      } else {
        delete el.dataset.bevyPoseFrame;
        delete el.dataset.bevyPoseSource;
      }
    };
    const applyTranslation = (
      el: HTMLElement,
      offset: ViewportOffset,
      pose: BevyPresentationPoseFrame | null,
      source: string | null,
    ) => {
      // Individual CSS translate composes with the element's intrinsic transform
      // (nameplate centering, selection-ring transforms and damage animations).
      const translation = translateFor(offset.x, offset.y);
      if (appliedTranslations.get(el) !== translation) {
        el.style.translate = translation;
        appliedTranslations.set(el, translation);
      }
      stampPoseFrame(el, pose, source);
    };
    const clearAll = () => {
      for (const el of surfacesRef.current.values()) {
        if (appliedTranslations.get(el) !== "") {
          el.style.translate = "";
          appliedTranslations.set(el, "");
        }
        stampPoseFrame(el, null, null);
      }
      for (const entry of entityElsRef.current.values()) {
        if (appliedTranslations.get(entry.el) !== "") {
          entry.el.style.translate = "";
          appliedTranslations.set(entry.el, "");
        }
        stampPoseFrame(entry.el, null, null);
      }
    };

    const applyFrame = (bevyPose: BevyPresentationPoseFrame | null, now: number) => {
      publishLocalSelfMotion(bevyPose);
      const snapshots = snapshotsRef.current;
      const player = getRenderPlayerRef.current();
      const playerId = player?.objectId;
      const camera = bevyPose?.camera ??
        (player ? cameraMotionOffsetForEntity(player, snapshots, now) : { x: 0, y: 0 });
      if (bevyPose) {
        diagnosticsRef.current.bevyPoseSamples += 1;
        diagnosticsRef.current.lastFrameId = bevyPose.frameId;
        diagnosticsRef.current.lastPoseAgeMs = bevyPose.ageMs;
      } else {
        diagnosticsRef.current.fallbackSamples += 1;
        diagnosticsRef.current.lastFrameId = null;
        diagnosticsRef.current.lastPoseAgeMs = null;
      }

      for (const el of surfacesRef.current.values()) {
        applyTranslation(el, camera, bevyPose, bevyPose?.camera.source ?? null);
      }

      if (entityElsRef.current.size > 0) {
        for (const entry of entityElsRef.current.values()) {
          // Per-entity elements inherit the camera surface. Non-self overlays add
          // their own sub-tile glide; the self overlay cancels the parent camera so
          // it remains pinned to the Crystal stage center.
          const bevyEntityPose = bevyPose?.entities.get(entry.objectId);
          if (bevyEntityPose) {
            diagnosticsRef.current.entityPoseHits += 1;
            if (bevyEntityPose.source === "remotePacket") {
              diagnosticsRef.current.remotePacketPoseHits += 1;
            }
          } else if (bevyPose) {
            diagnosticsRef.current.entityPoseMisses += 1;
          }
          const glide: ViewportOffset = bevyEntityPose ??
            (entry.objectId === playerId
              ? { x: -camera.x, y: -camera.y }
              : entityMotionOffsetForEntity({ objectId: entry.objectId } as DisplayEntity, snapshots, now));
          applyTranslation(entry.el, glide, bevyPose, bevyEntityPose?.source ?? null);
        }
      }
      dirty = true;
    };

    const applyAtomicPose = (pose: BevyPresentationPoseFrame, now: number) => {
      if (pose.frameId <= lastSinkFrameId) {
        diagnosticsRef.current.duplicatePoseFrames += 1;
        return;
      }
      const comparison = compareBevyPresentationPoseProvenance(
        pose,
        getPresentationContextRef.current?.(),
      );
      diagnosticsRef.current.lastProvenanceComparison = comparison;
      if (comparison !== "match") {
        if (comparison === "unavailable") {
          diagnosticsRef.current.provenanceUnavailableCount += 1;
          if (diagnosticsRef.current.poseCommitReady) {
            diagnosticsRef.current.postWarmupProvenanceUnavailableCount += 1;
          }
        } else {
          diagnosticsRef.current.provenanceMismatchCount += 1;
          if (diagnosticsRef.current.poseCommitReady) {
            diagnosticsRef.current.postWarmupProvenanceMismatchCount += 1;
          }
        }
        // Keep the last coherent pose for this short producer handoff. Switching
        // immediately to the independently-timed TS fallback creates a visible
        // cross-layer phase jump; the rAF watchdog below still falls back if no
        // complete pose arrives within the normal freshness budget.
        return;
      }
      diagnosticsRef.current.poseCommitReady = true;
      lastSinkFrameId = pose.frameId;
      diagnosticsRef.current.poseCommitFrames += 1;
      diagnosticsRef.current.lastSinkLagMs = pose.ageMs;
      lastAcceptedSinkAt = now;
      recordAcceptedLocalCommandPose(
        pose,
        now,
        getRenderPlayerRef.current()?.objectId ?? null,
      );
      applyFrame(pose, now);
    };

    const runtime = (
      window as typeof window & { __mir2BevyRuntime?: BevyPresentationPoseRuntime }
    ).__mir2BevyRuntime;
    diagnosticsRef.current.poseCommitSinkAvailable =
      typeof runtime?.setMir2PresentationPoseSink === "function";
    diagnosticsRef.current.poseCommitRegistrationError = null;
    if (
      atomicPoseCommitRef.current &&
      preferBevyPresentationPoseRef.current &&
      typeof runtime?.setMir2PresentationPoseSink === "function"
    ) {
      diagnosticsRef.current.poseCommitReady = false;
      diagnosticsRef.current.postWarmupStalePoseFrames = 0;
      diagnosticsRef.current.postWarmupProvenanceUnavailableCount = 0;
      diagnosticsRef.current.postWarmupProvenanceMismatchCount = 0;
      const sink = (json: string) => {
        if (!enabledRef.current) return;
        const now = Date.now();
        recordPoseSinkCallback();
        try {
          const pose = parseBevyPresentationPoseFrame(json, now);
          if (!pose) {
            diagnosticsRef.current.stalePoseFrames += 1;
            if (diagnosticsRef.current.poseCommitReady) {
              diagnosticsRef.current.postWarmupStalePoseFrames += 1;
            }
            return;
          }
          applyAtomicPose(pose, now);
        } catch {
          diagnosticsRef.current.stalePoseFrames += 1;
          if (diagnosticsRef.current.poseCommitReady) {
            diagnosticsRef.current.postWarmupStalePoseFrames += 1;
          }
        }
      };
      try {
        runtime.setMir2PresentationPoseSink.call(runtime, sink);
        sinkRegistered = true;
      } catch (error) {
        sinkRegistered = false;
        diagnosticsRef.current.poseCommitRegistrationError =
          error instanceof Error ? error.message : String(error);
      }
    }
    diagnosticsRef.current.poseCommitActive = sinkRegistered;

    const tick = () => {
      frame = window.requestAnimationFrame(tick);
      if (!enabledRef.current) {
        if (dirty) {
          clearAll();
          dirty = false;
        }
        return;
      }
      const now = Date.now();
      if (sinkRegistered) {
        // A missing/stalled sink must never freeze the residual DOM layers. The
        // display-rate TypeScript path remains the deterministic safety fallback.
        if (
          lastAcceptedSinkAt === 0 ||
          now - lastAcceptedSinkAt > BEVY_PRESENTATION_POSE_MAX_AGE_MS
        ) {
          applyFrame(null, now);
        }
        return;
      }
      const bevyPose = preferBevyPresentationPoseRef.current
        ? readBevyPresentationPoseFrame(runtime, now)
        : null;
      if (bevyPose) diagnosticsRef.current.polledPoseFrames += 1;
      applyFrame(bevyPose, now);
    };
    frame = window.requestAnimationFrame(tick);
    return () => {
      window.cancelAnimationFrame(frame);
      if (sinkRegistered) {
        try {
          runtime?.clearMir2PresentationPoseSink?.call(runtime);
        } catch {
          // Runtime teardown is best-effort; DOM cleanup below is deterministic.
        }
      }
      diagnosticsRef.current.poseCommitActive = false;
      publishLocalSelfMotion(null);
      if (dirty) clearAll();
    };
  }, [atomicPoseCommit, enabled, preferBevyPresentationPose, runtimeGeneration, snapshotsRef]);

  // Stable callback ref per surface key so unmounts clean up correctly.
  const registerCameraSurface = (key: string) => {
    const cache = surfaceCbCacheRef.current;
    let cb = cache.get(key);
    if (!cb) {
      cb = (el: HTMLElement | null) => {
        const previous = surfacesRef.current.get(key);
        if (previous && previous !== el) {
          delete previous.dataset.bevyPoseRole;
          delete previous.dataset.bevyPoseKey;
        }
        if (el) {
          el.dataset.bevyPoseRole = "camera";
          el.dataset.bevyPoseKey = key;
          surfacesRef.current.set(key, el);
        } else {
          surfacesRef.current.delete(key);
        }
      };
      cache.set(key, cb);
    }
    return cb;
  };

  // Stable callback ref per element key so React does not detach/reattach every render
  // (and so multiple elements of the same entity — stack, nameplate, health-bar — do
  // not collide). The objectId selects the motion snapshot for the glide.
  const registerEntityEl = (key: string, objectId: string) => {
    const cache = entityCbCacheRef.current;
    let cb = cache.get(key);
    if (!cb) {
      cb = (el: HTMLElement | null) => {
        const previous = entityElsRef.current.get(key)?.el;
        if (previous && previous !== el) {
          delete previous.dataset.bevyPoseRole;
          delete previous.dataset.bevyPoseKey;
          delete previous.dataset.bevyPoseObjectId;
        }
        if (el) {
          el.dataset.bevyPoseRole = "entity";
          el.dataset.bevyPoseKey = key;
          el.dataset.bevyPoseObjectId = objectId;
          entityElsRef.current.set(key, { el, objectId });
        } else {
          entityElsRef.current.delete(key);
        }
      };
      cache.set(key, cb);
    }
    return cb;
  };

  return {
    registerCameraSurface,
    registerEntityEl,
    getDiagnostics: () => ({
      ...diagnosticsRef.current,
      registeredSurfaceCount: surfacesRef.current.size,
      registeredEntityElementCount: entityElsRef.current.size,
    }),
  };
}
