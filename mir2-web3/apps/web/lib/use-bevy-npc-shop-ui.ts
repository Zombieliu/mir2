"use client";
import { useCallback, useEffect, useRef, useState } from "react";
import { NpcShopHost, type NpcShopHostOptions, type NpcShopHostState, type NpcShopIntent,
  type NpcShopPointerEdge, type NpcShopRuntime } from "./bevy-npc-shop-ui";

type Options = Omit<NpcShopHostOptions, "runtime" | "now" | "onState"> & {
  requested: boolean; runtimeGeneration: number; runtimeRef: { current: NpcShopRuntime | null };
};
export function useBevyNpcShopUi(options: Options) {
  const latest = useRef(options); latest.current = options;
  const host = useRef<NpcShopHost | null>(null);
  const [state, setState] = useState<NpcShopHostState>({ active: false, transitioning: false, ownerRevision: 0, error: null });
  useEffect(() => {
    const runtime = latest.current.runtimeRef.current;
    if (!options.requested || !runtime) {
      setState(s => s.active || s.transitioning ? { ...s, active: false, transitioning: false } : s);
      return;
    }
    let live = true;
    let current: NpcShopHost | null = null;
    current = new NpcShopHost({ runtime, now: () => performance.now(), read: () => latest.current.read(),
      onOwner: (...args) => { if (live && current && host.current === current) latest.current.onOwner(...args); },
      onIntent: intent => Boolean(live && current && host.current === current && latest.current.onIntent(intent)),
      onState: next => { if (live && current && host.current === current) setState(s =>
        s.active === next.active && s.transitioning === next.transitioning && s.ownerRevision === next.ownerRevision
          && s.error === next.error ? s : next); },
    });
    const installed = current;
    host.current = installed; installed.tick();
    const timer = window.setInterval(() => installed.tick(), 50);
    const pause = () => { if (host.current === installed) installed.withdraw(); };
    window.addEventListener("blur", pause); document.addEventListener("visibilitychange", pause);
    return () => {
      live = false;
      if (host.current === installed) host.current = null;
      window.clearInterval(timer); window.removeEventListener("blur", pause); document.removeEventListener("visibilitychange", pause);
      installed.stop();
    };
  }, [options.requested, options.runtimeGeneration]);
  const blocksInput = useCallback(() => host.current?.blocksInput() ?? false, []);
  const readStatus = useCallback(() => host.current?.readStatus() ?? null, []);
  const pointerContext = useCallback(() => host.current?.pointerContext() ?? null, []);
  const pointer = useCallback((edge: NpcShopPointerEdge) => host.current?.pointer(edge) ?? false, []);
  const allows = useCallback((intent: NpcShopIntent) => host.current?.allows(intent) ?? false, []);
  const claim = useCallback((intent: NpcShopIntent) => host.current?.claim(intent) ?? false, []);
  const claimCurrent = useCallback((intent: NpcShopIntent) => host.current?.claimCurrent(intent) ?? false, []);
  const deferRetirement = useCallback((intent: NpcShopIntent) => host.current?.deferRetirement(intent) ?? false, []);
  const withdraw = useCallback(() => host.current?.withdraw(), []);
  return { ...state, blocksInput, readStatus, pointerContext, pointer, allows, claim, claimCurrent, deferRetirement, withdraw };
}
