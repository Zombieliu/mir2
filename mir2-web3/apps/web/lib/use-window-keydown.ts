"use client";

import { useLayoutEffect, useRef } from "react";

/** Keep capture ownership stable while forwarding only committed UI handlers. */
export function useWindowKeydown(handler: ((event: KeyboardEvent) => void) | null): void {
  const committedHandler = useRef<((event: KeyboardEvent) => void) | null>(null);

  useLayoutEffect(() => {
    committedHandler.current = handler;
    return () => { committedHandler.current = null; };
  }, [handler]);

  useLayoutEffect(() => {
    const listener = (event: KeyboardEvent) => committedHandler.current?.(event);
    window.addEventListener("keydown", listener, true);
    return () => { window.removeEventListener("keydown", listener, true); };
  }, []);
}
