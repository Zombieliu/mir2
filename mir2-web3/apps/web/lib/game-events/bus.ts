// Typed synchronous pub/sub event bus for client-side game events.
//
// Usage:
//   const bus = createGameEventBus();
//   const unsub = bus.on("entityStruck", (e) => { /* e is EntityStruckEvent */ });
//   bus.emit({ type: "entityStruck", objectId: "42" });
//   unsub();
//
// Design:
//   • Synchronous dispatch — subscribers are called in insertion order.
//   • Zero deps — no React, no DOM, no third-party libs.
//   • Fully typed: `on(type, handler)` narrows the handler argument to the
//     correct variant so call sites get full inference.
//   • `onAny(handler)` receives the full union for cross-cutting concerns
//     (logging, analytics, test assertions).
//   • Unsubscribe is idempotent.

import type { GameEvent, GameEventOf, GameEventType } from "./events";

export type GameEventHandler<T extends GameEventType> = (event: GameEventOf<T>) => void;
export type AnyGameEventHandler = (event: GameEvent) => void;

export type Unsubscribe = () => void;

export type GameEventBus = {
  /** Emit an event; all registered handlers for its type + onAny handlers are called synchronously. */
  emit(event: GameEvent): void;
  /** Subscribe to a specific event type. Returns an idempotent unsubscribe function. */
  on<T extends GameEventType>(type: T, handler: GameEventHandler<T>): Unsubscribe;
  /** Subscribe to ALL events regardless of type. Returns an idempotent unsubscribe function. */
  onAny(handler: AnyGameEventHandler): Unsubscribe;
};

export function createGameEventBus(): GameEventBus {
  // Per-type handler sets (keyed by the string discriminant).
  const typed = new Map<GameEventType, Set<AnyGameEventHandler>>();
  // Handlers that receive every event.
  const anyHandlers = new Set<AnyGameEventHandler>();

  function emit(event: GameEvent): void {
    const handlers = typed.get(event.type);
    if (handlers) {
      for (const handler of handlers) {
        handler(event);
      }
    }
    for (const handler of anyHandlers) {
      handler(event);
    }
  }

  function on<T extends GameEventType>(type: T, handler: GameEventHandler<T>): Unsubscribe {
    let handlers = typed.get(type);
    if (!handlers) {
      handlers = new Set();
      typed.set(type, handlers);
    }
    // Cast is safe: the handler only sees GameEventOf<T>, which is a subtype of GameEvent.
    const anyHandler = handler as AnyGameEventHandler;
    handlers.add(anyHandler);
    let removed = false;
    return () => {
      if (removed) return;
      removed = true;
      handlers!.delete(anyHandler);
    };
  }

  function onAny(handler: AnyGameEventHandler): Unsubscribe {
    anyHandlers.add(handler);
    let removed = false;
    return () => {
      if (removed) return;
      removed = true;
      anyHandlers.delete(handler);
    };
  }

  return { emit, on, onAny };
}
