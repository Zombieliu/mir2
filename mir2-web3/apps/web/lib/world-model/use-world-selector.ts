/**
 * React read primitive for the world-model store (render-perf Stage 5a).
 *
 * `useWorldSelector(store, selector, isEqual?)` subscribes a React component to a
 * *slice* of the world store via `useSyncExternalStore`, so only the components
 * that read a given slice re-render when that slice changes — instead of the
 * whole 12.7k-line monolith re-rendering on every coalesced `setWorld` flush.
 *
 * The load-bearing detail is the `useSyncExternalStoreWithSelector` cache. A naive
 * `useSyncExternalStore(subscribe, () => selector(store.getState()))` would loop
 * forever for any *derived* selector that builds a fresh object/array
 * (`s => ({ hp: s.playerHp, mp: s.playerMp })`), because each `getSnapshot()` call
 * returns a new reference and React keeps re-rendering to converge. The cache
 * below recomputes the selection ONLY when the underlying store snapshot changes
 * by reference, and applies `isEqual` to preserve the previous selection's
 * identity when the value is equal — so derived selectors are stable and do not
 * loop.
 *
 * `createSelectorMemo` is exported separately and intentionally framework-free so
 * its cache semantics can be unit-tested with plain `node`
 * (scripts/test-world-store-hook.mjs) — React itself is imported only by the hook.
 */

import { useCallback, useMemo, useRef, useSyncExternalStore } from "react";

import type { WorldState } from "./types";
import type { WorldStore } from "./store";

/**
 * A memoized selection function. Given the latest store snapshot, returns the
 * selected slice — recomputing only when the snapshot reference changes, and
 * (when `isEqual` is provided) returning the PREVIOUS selection's identity when
 * the freshly-computed value is `isEqual` to it. This is exactly the cache
 * `use-sync-external-store/with-selector` implements; replicated here so the
 * store stays a pure module and the cache is independently testable.
 */
export type SelectorMemo<S> = (snapshot: WorldState) => S;

/**
 * Build the per-selector memo. Pure — no React, no globals. Each call returns a
 * fresh memo closure with its own cache, so distinct subscribers never share
 * cache state.
 *
 * Behavior (mirrors useSyncExternalStoreWithSelector's `memoizedSelector`):
 *   - First call: compute `selector(snapshot)`, remember it + the snapshot.
 *   - Same snapshot reference as last time: return the cached selection
 *     (no recompute) — this is what makes `getSnapshot` stable for React.
 *   - New snapshot: recompute. If `isEqual(prev, next)` is true, KEEP the
 *     previous selection's reference (so derived object selectors stay stable
 *     and don't trigger a render / loop); otherwise adopt the new value.
 */
export function createSelectorMemo<S>(
  selector: (state: WorldState) => S,
  isEqual?: (a: S, b: S) => boolean,
): SelectorMemo<S> {
  let hasMemo = false;
  let lastSnapshot: WorldState | undefined;
  let lastSelection: S | undefined;

  return (snapshot: WorldState): S => {
    if (!hasMemo) {
      hasMemo = true;
      lastSnapshot = snapshot;
      const next = selector(snapshot);
      lastSelection = next;
      return next;
    }

    // The store keeps the same reference until a mutation produces a new one, so
    // an unchanged snapshot short-circuits to the cached selection. This is the
    // stable-`getSnapshot` guarantee `useSyncExternalStore` requires.
    const prevSnapshot = lastSnapshot as WorldState;
    const prevSelection = lastSelection as S;
    if (snapshot === prevSnapshot) {
      return prevSelection;
    }

    const next = selector(snapshot);
    lastSnapshot = snapshot;

    // If the freshly-computed selection equals the previous one, preserve the
    // previous reference. For derived selectors this is what prevents a render
    // loop (a new {} every snapshot would otherwise never settle).
    if (isEqual !== undefined && isEqual(prevSelection, next)) {
      return prevSelection;
    }

    lastSelection = next;
    return next;
  };
}

/**
 * Subscribe the calling component to `selector(store.state)`. Re-renders only
 * when the selection changes (per `isEqual`, defaulting to `Object.is`).
 *
 * Prefer DIRECT-SLICE selectors (`s => s.entities`) — the store keeps those
 * identity-stable across unrelated mutations (store.ts `{ ...current }`), so they
 * need no `isEqual`. For DERIVED selectors (`s => ({ hp, mp })`) pass a shallow
 * `isEqual` so the cache can hold a stable reference.
 *
 * @example
 *   const entities = useWorldSelector(store, (s) => s.entities);
 *   const stats = useWorldSelector(store, (s) => ({ hp: s.playerHp, mp: s.playerMp }), shallowEqual);
 */
export function useWorldSelector<S>(
  store: WorldStore,
  selector: (state: WorldState) => S,
  isEqual?: (a: S, b: S) => boolean,
): S {
  // Keep the latest selector/isEqual in refs so the memo we build does not have
  // to be torn down and rebuilt every render when callers pass inline closures
  // (the common case). The memo is rebuilt only when the store identity changes
  // — which is once, since `worldStoreRef` is created once per session.
  const selectorRef = useRef(selector);
  const isEqualRef = useRef(isEqual);
  selectorRef.current = selector;
  isEqualRef.current = isEqual;

  const getSelection = useMemo(() => {
    const memo = createSelectorMemo<S>(
      (state) => selectorRef.current(state),
      (a, b) => (isEqualRef.current ? isEqualRef.current(a, b) : Object.is(a, b)),
    );
    return () => memo(store.getSnapshot());
    // Rebuild the cache only if the store instance itself changes.
  }, [store]);

  const subscribe = useCallback(
    (onStoreChange: () => void) => store.subscribeFull(onStoreChange),
    [store],
  );

  // Server snapshot path: SSR / hydration just runs the selector once against the
  // current snapshot; there is no subscription server-side. Safe because the
  // memo's first call has no prior reference to compare against.
  return useSyncExternalStore(subscribe, getSelection, getSelection);
}

/**
 * Shallow object/array equality helper for DERIVED selectors. Two values are
 * equal when they are the same reference, or are objects/arrays with the same
 * keys and `Object.is`-equal values one level deep.
 */
export function shallowEqual<T>(a: T, b: T): boolean {
  if (Object.is(a, b)) return true;
  if (typeof a !== "object" || a === null || typeof b !== "object" || b === null) {
    return false;
  }
  const aKeys = Object.keys(a as Record<string, unknown>);
  const bKeys = Object.keys(b as Record<string, unknown>);
  if (aKeys.length !== bKeys.length) return false;
  for (const key of aKeys) {
    if (
      !Object.prototype.hasOwnProperty.call(b, key) ||
      !Object.is((a as Record<string, unknown>)[key], (b as Record<string, unknown>)[key])
    ) {
      return false;
    }
  }
  return true;
}
