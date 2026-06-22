"use client";

// Imperative scene-motion driver (perf: kills the 30 Hz `motionNow` React render).
//
// When the GPU/Bevy entity renderer is active it interpolates the floor, the
// self-camera and every monster at DISPLAY refresh rate from step-stable motion
// windows (see BEVY_SELF_CAMERA_ENABLED / BEVY_ENTITY_INTERP_ENABLED). That makes
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
import type { ViewportOffset } from "./original-client-scene-layout";
import type { DisplayEntity, EntityMotionSnapshot } from "./original-client-types";

function transformFor(x: number, y: number): string {
  if (x === 0 && y === 0) return "translate3d(0px, 0px, 0px)";
  return `translate3d(${x}px, ${y}px, 0px)`;
}

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
};

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

  useEffect(() => {
    let frame = 0;
    let dirty = false; // did we set any transform last frame? (so we can clear on disable)
    const clearAll = () => {
      for (const el of surfacesRef.current.values()) el.style.transform = "";
      for (const entry of entityElsRef.current.values()) entry.el.style.transform = "";
    };
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
      const snapshots = snapshotsRef.current;
      const player = getRenderPlayerRef.current();
      const playerId = player?.objectId;
      const camera = player ? cameraMotionOffsetForEntity(player, snapshots, now) : { x: 0, y: 0 };

      const cameraTransform = transformFor(camera.x, camera.y);
      for (const el of surfacesRef.current.values()) el.style.transform = cameraTransform;

      if (entityElsRef.current.size > 0) {
        for (const entry of entityElsRef.current.values()) {
          // Per-entity elements are nested under a camera surface, so they already
          // inherit the camera pan — they only add their own sub-tile GLIDE. The self
          // player is the camera anchor (pinned), so its glide is zero.
          const glide: ViewportOffset =
            entry.objectId === playerId
              ? { x: 0, y: 0 }
              : entityMotionOffsetForEntity({ objectId: entry.objectId } as DisplayEntity, snapshots, now);
          entry.el.style.transform = transformFor(glide.x, glide.y);
        }
      }
      dirty = true;
    };
    frame = window.requestAnimationFrame(tick);
    return () => window.cancelAnimationFrame(frame);
  }, [snapshotsRef]);

  // Stable callback ref per surface key so unmounts clean up correctly.
  const registerCameraSurface = (key: string) => {
    const cache = surfaceCbCacheRef.current;
    let cb = cache.get(key);
    if (!cb) {
      cb = (el: HTMLElement | null) => {
        if (el) surfacesRef.current.set(key, el);
        else surfacesRef.current.delete(key);
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
        if (el) entityElsRef.current.set(key, { el, objectId });
        else entityElsRef.current.delete(key);
      };
      cache.set(key, cb);
    }
    return cb;
  };

  return { registerCameraSurface, registerEntityEl };
}
