/**
 * Framework-agnostic world-model store.
 *
 * `createWorldStore` returns a store holding a `WorldState` snapshot. All
 * mutations are named, mirroring what `page.tsx` does today through
 * `setWorld(...)` calls. There are no React imports — this module is pure TS
 * and can be consumed by Bevy/WASM, test harnesses, or a future non-React
 * client.
 *
 * Selector subscriptions fire only when the selected slice changes by
 * reference, so React (or any other consumer) can subscribe to slices without
 * per-frame churn.
 */

import type {
  WorldState,
  WorldEntity,
  GroundDrop,
  DamageFloater,
} from "./types";
import { DEFAULT_WORLD_STATE } from "./types";

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/** Replace or append an entity by objectId. */
function upsertEntityInList(list: WorldEntity[], next: WorldEntity): WorldEntity[] {
  const found = list.some((e) => e.objectId === next.objectId);
  if (found) {
    return list.map((e) => (e.objectId === next.objectId ? { ...e, ...next } : e));
  }
  return [...list, next];
}

/** Apply an updater to the entity with the given objectId (no-op if absent). */
function patchEntityInList(
  list: WorldEntity[],
  objectId: string,
  updater: (e: WorldEntity) => WorldEntity,
): WorldEntity[] {
  return list.map((e) => (e.objectId === objectId ? updater(e) : e));
}

/**
 * Remove (or decrement quantity of) a WorldItem with the given uniqueId.
 * Matches `removeItemByUniqueId` in `lib/extended-server-packets.ts`.
 */
function removeItemByUniqueId<T extends { uniqueId: number; quantity: number }>(
  items: T[],
  uniqueId: number,
  count: number,
): T[] {
  let changed = false;
  const next = items.flatMap((item) => {
    if (changed || item.uniqueId !== uniqueId) return [item];
    changed = true;
    if (count > 0 && item.quantity > count) {
      return [{ ...item, quantity: item.quantity - count }];
    }
    return [];
  });
  return changed ? next : items;
}

// Maximum live damage floaters; matches the cap in page.tsx (line 9026).
const DAMAGE_FLOATER_CAP = 48;

// ---------------------------------------------------------------------------
// Subscription internals
// ---------------------------------------------------------------------------

type Listener = () => void;

type SelectorSubscription<S> = {
  selector: (state: WorldState) => S;
  listener: Listener;
  lastValue: S;
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/** The full public surface returned by `createWorldStore`. */
export type WorldStore = {
  /** Return the current state (always the latest reference). */
  getState(): WorldState;

  /**
   * Return a snapshot suitable for serialization. Identical to `getState()` —
   * exposed as a separate entry point so callers can pass it to the emitter
   * without coupling to internal state.
   */
  getSnapshot(): WorldState;

  /**
   * Subscribe to a selected slice of state. The listener fires only when
   * `selector(state)` returns a different reference from the last call.
   * Returns an unsubscribe function.
   *
   * Usage:
   *   const unsub = store.subscribe(s => s.entities, () => { ... });
   */
  subscribe<S>(selector: (state: WorldState) => S, listener: Listener): () => void;

  /**
   * Subscribe to EVERY state change (no selector / no slice gate). The listener
   * fires whenever `set` produces a new state reference. Additive companion to
   * `subscribe` — intended as the `useSyncExternalStore` subscribe arg for the
   * React `useWorldSelector` hook, which then does selector-level memoization on
   * its own side (the `useSyncExternalStoreWithSelector` cache). Returns an
   * unsubscribe function.
   */
  subscribeFull(listener: Listener): () => void;

  /**
   * Generic updater — accepts an updater function (like React's setState) or
   * a partial patch object.
   */
  set(updater: ((current: WorldState) => WorldState) | Partial<WorldState>): void;

  // Named mutations ------------------------------------------------------------

  /** Replace or merge a new entity into the `entities` list. */
  upsertEntity(entity: WorldEntity): void;

  /**
   * Patch fields of the entity with the given objectId.
   * No-op if the entity is not in the list.
   */
  patchEntity(objectId: string, patch: Partial<WorldEntity>): void;

  /** Remove the entity with the given objectId from the list. */
  removeEntity(objectId: string): void;

  /**
   * Set map-level fields. When `mapChanged` is true the ephemeral per-map
   * collections (entities, groundDrops, mineNodes, projectiles,
   * damageFloaters, sceneView, terrainPatches, decorObjects,
   * originalMapRegion, selectedObjectId, activeNpcDialog) are cleared — exactly
   * as `page.tsx`'s MapChanged handler does.
   */
  setMap(patch: {
    mapTitle?: string | null;
    mapFileName?: string | null;
    miniMapIndex?: number | null;
    bigMapIndex?: number | null;
    mapChanged?: boolean;
  }): void;

  /**
   * Merge a UserLocation / movement packet's position onto the player entity.
   * Creates the entity if absent (unlikely in practice, but safe).
   */
  applyUserLocation(patch: {
    objectId: string;
    x: number;
    y: number;
    direction?: string;
  }): void;

  /** Replace or merge a GroundDrop by objectId. */
  upsertGroundDrop(drop: GroundDrop): void;

  /** Remove the GroundDrop with the given objectId. */
  removeGroundDrop(objectId: string): void;

  /**
   * Remove (or quantity-decrement) a WorldItem from bag/belt/storage by
   * uniqueId. Mirrors the `DeleteItem` handler in page.tsx.
   */
  removeWorldItem(options: {
    uniqueId: number;
    count: number;
    fromInventory?: boolean;
    fromBelt?: boolean;
    fromStorage?: boolean;
  }): void;

  /**
   * Spawn a damage floater. Mirrors `pushDamageFloater` in page.tsx:
   *   - prunes already-expired floaters,
   *   - caps the live set at 48 (drops oldest when over cap),
   *   - generates a unique key using the provided seqNo.
   */
  addDamageFloater(floater: Omit<DamageFloater, "key">, seqNo: number): void;

  /**
   * Mark an entity as "struck" (sets struckStartedAt / struckUntil).
   * duration is the struck-animation window in ms.
   */
  markEntityStruck(objectId: string, fields: {
    x?: number;
    y?: number;
    direction?: string;
    struckStartedAt: number;
    struckUntil: number;
  }): void;
};

/** Create a new world store, optionally seeded with an initial state. */
export function createWorldStore(initial?: Partial<WorldState>): WorldStore {
  let state: WorldState = initial
    ? { ...DEFAULT_WORLD_STATE, ...initial }
    : { ...DEFAULT_WORLD_STATE };

  // We keep subscriptions in an array; each entry holds the selector, last
  // value, and listener so we can do shallow (reference) comparison.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const subscriptions: SelectorSubscription<any>[] = [];
  // Full (unfiltered) listeners — fire on every state change. Used by the React
  // `useWorldSelector` hook, which memoizes per-selector on its own side.
  const fullListeners: Listener[] = [];

  function notify(prev: WorldState, next: WorldState): void {
    if (prev === next) return;
    for (const sub of subscriptions) {
      const nextValue = sub.selector(next);
      if (nextValue !== sub.lastValue) {
        sub.lastValue = nextValue;
        sub.listener();
      }
    }
    // Snapshot the list so an unsubscribe during dispatch can't skip a sibling.
    for (const listener of fullListeners.slice()) {
      listener();
    }
  }

  function setState(updater: (current: WorldState) => WorldState): void {
    const prev = state;
    const next = updater(prev);
    if (next === prev) return;
    state = next;
    notify(prev, next);
  }

  const store: WorldStore = {
    getState() {
      return state;
    },

    getSnapshot() {
      return state;
    },

    subscribe<S>(selector: (s: WorldState) => S, listener: Listener): () => void {
      const entry: SelectorSubscription<S> = {
        selector,
        listener,
        lastValue: selector(state),
      };
      subscriptions.push(entry);
      return () => {
        const idx = subscriptions.indexOf(entry);
        if (idx !== -1) subscriptions.splice(idx, 1);
      };
    },

    subscribeFull(listener: Listener): () => void {
      fullListeners.push(listener);
      return () => {
        const idx = fullListeners.indexOf(listener);
        if (idx !== -1) fullListeners.splice(idx, 1);
      };
    },

    set(updater) {
      if (typeof updater === "function") {
        setState(updater);
      } else {
        setState((current) => ({ ...current, ...updater }));
      }
    },

    upsertEntity(entity) {
      setState((current) => ({
        ...current,
        entities: upsertEntityInList(current.entities, entity),
      }));
    },

    patchEntity(objectId, patch) {
      setState((current) => ({
        ...current,
        entities: patchEntityInList(current.entities, objectId, (e) => ({ ...e, ...patch })),
      }));
    },

    removeEntity(objectId) {
      setState((current) => ({
        ...current,
        entities: current.entities.filter((e) => e.objectId !== objectId),
      }));
    },

    setMap({ mapTitle, mapFileName, miniMapIndex, bigMapIndex, mapChanged }) {
      setState((current) => {
        const nextMapFileName =
          mapFileName !== undefined ? mapFileName : current.mapFileName;
        const preservedSelfEntity =
          mapChanged && current.playerObjectId
            ? current.entities.find((e) => e.objectId === current.playerObjectId)
            : undefined;

        return {
          ...current,
          mapTitle: mapTitle !== undefined ? mapTitle : current.mapTitle,
          mapFileName: nextMapFileName,
          miniMapIndex: miniMapIndex !== undefined ? miniMapIndex : current.miniMapIndex,
          bigMapIndex: bigMapIndex !== undefined ? bigMapIndex : current.bigMapIndex,
          selectedObjectId: mapChanged ? null : current.selectedObjectId,
          activeNpcDialog: mapChanged ? null : current.activeNpcDialog,
          entities: mapChanged
            ? preservedSelfEntity
              ? [preservedSelfEntity]
              : []
            : current.entities,
          groundDrops: mapChanged ? [] : current.groundDrops,
          mineNodes: mapChanged ? [] : current.mineNodes,
          projectiles: mapChanged ? [] : current.projectiles,
          damageFloaters: mapChanged ? [] : current.damageFloaters,
          sceneView: mapChanged ? null : current.sceneView,
          terrainPatches: mapChanged ? [] : current.terrainPatches,
          decorObjects: mapChanged ? [] : current.decorObjects,
          originalMapRegion: mapChanged ? null : current.originalMapRegion,
        };
      });
    },

    applyUserLocation({ objectId, x, y, direction }) {
      setState((current) => {
        const exists = current.entities.some((e) => e.objectId === objectId);
        if (exists) {
          return {
            ...current,
            entities: patchEntityInList(current.entities, objectId, (e) => ({
              ...e,
              x,
              y,
              direction: direction ?? e.direction,
            })),
          };
        }
        // Fallback: insert a minimal entity (self-player placeholder).
        return {
          ...current,
          entities: [
            ...current.entities,
            {
              objectId,
              kind: "selfPlayer" as const,
              name: current.playerName ?? "",
              x,
              y,
              direction,
              disposition: "friendly" as const,
            },
          ],
        };
      });
    },

    upsertGroundDrop(drop) {
      setState((current) => {
        const exists = current.groundDrops.some((d) => d.objectId === drop.objectId);
        const groundDrops = exists
          ? current.groundDrops.map((d) => (d.objectId === drop.objectId ? drop : d))
          : [...current.groundDrops, drop];
        return { ...current, groundDrops };
      });
    },

    removeGroundDrop(objectId) {
      setState((current) => ({
        ...current,
        groundDrops: current.groundDrops.filter((d) => d.objectId !== objectId),
      }));
    },

    removeWorldItem({ uniqueId, count, fromInventory = true, fromBelt = true, fromStorage = true }) {
      setState((current) => ({
        ...current,
        inventoryItems: fromInventory
          ? removeItemByUniqueId(current.inventoryItems, uniqueId, count)
          : current.inventoryItems,
        beltItems: fromBelt
          ? removeItemByUniqueId(current.beltItems, uniqueId, count)
          : current.beltItems,
        storageItems: fromStorage
          ? removeItemByUniqueId(current.storageItems, uniqueId, count)
          : current.storageItems,
      }));
    },

    addDamageFloater(floater, seqNo) {
      setState((current) => {
        const now = Date.now();
        const full: DamageFloater = {
          ...floater,
          key: `dmg-${floater.objectId}-${now}-${seqNo}`,
        };
        // Drop already-expired floaters; cap live set so AoE bursts are bounded.
        const live = current.damageFloaters.filter((f) => f.expiresAt > now);
        const trimmed =
          live.length >= DAMAGE_FLOATER_CAP ? live.slice(live.length - (DAMAGE_FLOATER_CAP - 1)) : live;
        return { ...current, damageFloaters: [...trimmed, full] };
      });
    },

    markEntityStruck(objectId, fields) {
      setState((current) => ({
        ...current,
        entities: patchEntityInList(current.entities, objectId, (e) => ({
          ...e,
          x: fields.x !== undefined ? fields.x : e.x,
          y: fields.y !== undefined ? fields.y : e.y,
          direction: fields.direction !== undefined ? fields.direction : e.direction,
          struckStartedAt: fields.struckStartedAt,
          struckUntil: fields.struckUntil,
        })),
      }));
    },
  };

  return store;
}
