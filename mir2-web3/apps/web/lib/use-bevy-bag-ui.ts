"use client";

import { useEffect, useRef, useState } from "react";
import { BevyBagHost, type BagCommandToken, type BagHostOptions, type BagHostState, type BagPointerEdge, type BevyBagUiRuntime } from "./bevy-bag-ui";

type Options = Omit<BagHostOptions, "runtime" | "now" | "onState"> & {
  requested: boolean;
  runtimeGeneration: number;
  runtimeRef: { current: BevyBagUiRuntime | null };
};

export function useBevyBagUi(options: Options) {
  const latest = useRef(options);
  latest.current = options;
  const host = useRef<BevyBagHost | null>(null);
  const [state, setState] = useState<BagHostState>({ active: false, ownerRevision: 0, error: null });
  useEffect(() => {
    const runtime = latest.current.runtimeRef.current;
    if (!options.requested || !runtime) {
      setState(old => old.active ? { ...old, active: false } : old);
      return;
    }
    let live = true;
    let initializing = true;
    const current: BevyBagHost = new BevyBagHost({ runtime, now: () => performance.now(),
      read: () => latest.current.read(), onOwner: (...args) => { if (live && (initializing || host.current === current)) latest.current.onOwner(...args); },
      onIntent: (intent) => live && host.current === current ? latest.current.onIntent(intent) : { accepted: false },
      onState: (next) => { if (live && (initializing || host.current === current)) setState((old) => old.active === next.active && old.ownerRevision === next.ownerRevision
        && old.error === next.error ? old : next);
      },
    });
    host.current = current;
    initializing = false;
    const tick = () => current.tick();
    tick();
    const timer = window.setInterval(tick, 50);
    const pause = () => current.yieldToReact();
    window.addEventListener("blur", pause);
    document.addEventListener("visibilitychange", tick);
    return () => {
      live = false;
      if (host.current === current) host.current = null;
      window.clearInterval(timer);
      window.removeEventListener("blur", pause);
      document.removeEventListener("visibilitychange", tick);
      current.stop();
    };
  }, [options.requested, options.runtimeGeneration]);
  return { ...state,
    pointerContext: () => host.current?.pointerContext() ?? null,
    pointer: (edge: BagPointerEdge) => host.current?.pointer(edge) ?? false,
    allows: (token: BagCommandToken) => host.current?.allows(token) ?? false,
    yieldToReact: () => host.current?.yieldToReact(),
  };
}
