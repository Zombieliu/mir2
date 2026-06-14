// VFX subscriber: maps GameEvents to combat-feedback visual calls.
//
// `registerVfxSubscriber(bus, sink)` wires damage-related events to the VFX
// helpers currently inlined in page.tsx (`pushDamageFloater`, the entity-struck
// hit-flash marker).  The `sink` parameter is injected so the subscriber is
// testable without mounting the full React world.
//
// Design:
//   • Injected VfxSink — no direct coupling to page.tsx state.
//   • `addDamageFloater` / `markEntityStruck` match the signatures page.tsx
//     already uses, so the eventual integration is a drop-in.
//   • The subscriber reacts only to `damageDealt` (floater) and `entityStruck`
//     (hit-flash); other events are sound-only and handled by sound-subscriber.

import type { GameEventBus, Unsubscribe } from "./bus";
import type { CrystalDamageType } from "./events";

// ---------------------------------------------------------------------------
// Injected VFX sink interface
// ---------------------------------------------------------------------------

/**
 * Payload for a floating damage number over a struck entity.
 * Mirrors the minimal fields pushDamageFloater computes in page.tsx.
 */
export type DamageFloaterPayload = {
  objectId: string;
  damage: number;
  damageType: CrystalDamageType;
};

/**
 * Injected interface the VFX subscriber calls.  The real implementations live
 * in page.tsx today (pushDamageFloater / markWorldEntityStruck) and are passed
 * in at integration time.
 */
export type VfxSink = {
  /** Spawn a floating combat number over the struck object. */
  addDamageFloater(payload: DamageFloaterPayload): void;
  /** Mark an entity with a brief hit-flash (struckStartedAt / struckUntil). */
  markEntityStruck(objectId: string): void;
};

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/**
 * Subscribe the VFX handlers to the bus.
 * Returns a single unsubscribe that tears down all handlers at once.
 */
export function registerVfxSubscriber(bus: GameEventBus, sink: VfxSink): Unsubscribe {
  const unsubs: Unsubscribe[] = [];

  // damageDealt -> floating damage number (Crystal DamageIndicator)
  unsubs.push(
    bus.on("damageDealt", (event) => {
      sink.addDamageFloater({
        objectId: event.objectId,
        damage: event.damage,
        damageType: event.damageType,
      });
    }),
  );

  // entityStruck -> hit-flash on the entity sprite
  unsubs.push(
    bus.on("entityStruck", (event) => {
      sink.markEntityStruck(event.objectId);
    }),
  );

  return () => {
    for (const unsub of unsubs) {
      unsub();
    }
  };
}
