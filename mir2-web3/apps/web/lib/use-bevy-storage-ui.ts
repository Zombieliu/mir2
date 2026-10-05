"use client";
import { useEffect, useRef, useState } from "react";
import { BevyStorageHost, type StorageHostOptions, type StorageHostState, type StorageIntent, type StoragePointerEdge, type StorageRuntime } from "./bevy-storage-ui";

type Options = Omit<StorageHostOptions, "runtime" | "now" | "onState"> & {
  requested: boolean; runtimeGeneration: number; runtimeRef: { current: StorageRuntime | null };
};
export function useBevyStorageUi(options: Options) {
  const latest = useRef(options); latest.current = options;
  const host = useRef<BevyStorageHost | null>(null);
  const [state, setState] = useState<StorageHostState>({ active: false, transitioning: false, ownerRevision: 0, error: null });
  useEffect(() => {
    const runtime = latest.current.runtimeRef.current;
    if (!options.requested || !runtime) { setState(s => s.active || s.transitioning ? { ...s, active: false, transitioning: false } : s); return; }
    let live = true;
    const current: BevyStorageHost = new BevyStorageHost({ runtime, now: () => performance.now(), read: () => latest.current.read(),
      onOwner: (...args) => { if (live && host.current === current) latest.current.onOwner(...args); },
      onIntent: intent => live && host.current === current && latest.current.onIntent(intent),
      onState: next => { if (live && host.current === current) setState(s => s.active === next.active && s.transitioning === next.transitioning
        && s.ownerRevision === next.ownerRevision && s.error === next.error ? s : next); },
    });
    host.current = current; current.tick();
    const timer = window.setInterval(() => current.tick(), 50);
    const pause = () => current.withdraw();
    window.addEventListener("blur", pause); document.addEventListener("visibilitychange", pause);
    return () => {
      live = false;
      if (host.current === current) host.current = null;
      window.clearInterval(timer); window.removeEventListener("blur", pause); document.removeEventListener("visibilitychange", pause);
      current.stop();
    };
  }, [options.requested, options.runtimeGeneration]);
  return { ...state, pointerContext: () => host.current?.pointerContext() ?? null,
    pointer: (edge: StoragePointerEdge) => host.current?.pointer(edge) ?? false,
    allows: (intent: StorageIntent) => host.current?.allows(intent) ?? false,
    claim: (intent: StorageIntent) => host.current?.claim(intent) ?? false,
    withdraw: () => host.current?.withdraw() };
}
