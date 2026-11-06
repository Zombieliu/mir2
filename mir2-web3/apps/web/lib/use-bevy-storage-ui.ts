"use client";
import { useEffect, useRef, useState } from "react";
import { BevyStorageHost, type StorageHostOptions, type StorageHostState, type StorageRejectReason, type StorageIntent, type StoragePointerEdge, type StorageRuntime } from "./bevy-storage-ui";

type Options = Omit<StorageHostOptions, "runtime" | "now" | "onState"> & {
  requested: boolean; runtimeGeneration: number; runtimeRef: { current: StorageRuntime | null };
  inactiveReason?: StorageRejectReason | null;
};
export function useBevyStorageUi(options: Options) {
  const latest = useRef(options); latest.current = options;
  const host = useRef<BevyStorageHost | null>(null);
  const diagnosticRun = useRef<{ host: BevyStorageHost; runtime: StorageRuntime; generation: number } | null>(null);
  const [state, setState] = useState<StorageHostState>({ active: false, transitioning: false, ownerRevision: 0, error: null,
    diagnostic: { owner: "react", phase: "inactive", reason: null }, preparePresentation: null });
  useEffect(() => {
    const runtime = latest.current.runtimeRef.current;
    if (!options.requested || !runtime) { setState(s => s.active || s.transitioning || s.preparePresentation ? { ...s, active: false, transitioning: false, preparePresentation: null } : s); return; }
    let live = true;
    const current: BevyStorageHost = new BevyStorageHost({ runtime, now: () => performance.now(), read: () => latest.current.read(),
      onOwner: (...args) => { if (live && host.current === current) latest.current.onOwner(...args); },
      onIntent: intent => live && host.current === current && latest.current.onIntent(intent),
      onState: next => { if (live && host.current === current) {
        const sameRun = diagnosticRun.current?.host === current;
        diagnosticRun.current = { host: current, runtime, generation: options.runtimeGeneration };
        setState(s => sameRun && s.active === next.active && s.transitioning === next.transitioning
          && s.ownerRevision === next.ownerRevision && s.error === next.error
          && s.diagnostic.owner === next.diagnostic.owner && s.diagnostic.phase === next.diagnostic.phase
          && s.diagnostic.reason === next.diagnostic.reason
          && JSON.stringify(s.preparePresentation) === JSON.stringify(next.preparePresentation) ? s : next);
      } },
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
  const diagnostic = !options.requested
    ? { owner: "react" as const, phase: "inactive" as const, reason: options.inactiveReason ?? "sharedNotRequested" as const }
    : !options.runtimeRef.current ? { owner: "react" as const, phase: "inactive" as const, reason: "runtimeUnavailable" as const }
    : diagnosticRun.current?.host === host.current && diagnosticRun.current?.runtime === options.runtimeRef.current
      && diagnosticRun.current?.generation === options.runtimeGeneration ? state.diagnostic
    : { owner: "react" as const, phase: "inactive" as const, reason: null };
  const preparePresentation = options.requested && options.runtimeRef.current
    && diagnosticRun.current?.host === host.current && diagnosticRun.current?.runtime === options.runtimeRef.current
    && diagnosticRun.current?.generation === options.runtimeGeneration ? state.preparePresentation : null;
  return { ...state, diagnostic, preparePresentation, pointerContext: () => host.current?.pointerContext() ?? null,
    pointer: (edge: StoragePointerEdge) => host.current?.pointer(edge) ?? false,
    allows: (intent: StorageIntent) => host.current?.allows(intent) ?? false,
    claim: (intent: StorageIntent) => host.current?.claim(intent) ?? false,
    withdraw: () => host.current?.withdraw() };
}
