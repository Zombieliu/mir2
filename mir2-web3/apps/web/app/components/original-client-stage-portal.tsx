"use client";

import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

type OriginalClientStagePortalProps = {
  children: ReactNode;
};

/**
 * Mounts web-shell overlays inside the scaled Crystal client stage.
 *
 * The page itself fills the browser viewport, while `.client-stage-frame` is a
 * contained and transformed 4:3 coordinate space. Any game-facing overlay that
 * stays under the page root will otherwise drift into the letterbox gutters on
 * large or ultra-wide displays.
 */
export function OriginalClientStagePortal({ children }: OriginalClientStagePortalProps) {
  const [stageRoot, setStageRoot] = useState<HTMLElement | null>(null);

  useEffect(() => {
    // Fast Refresh and screen remounts may replace the stage without remounting
    // this portal. Resolve after every render so children never remain attached
    // to a detached stage node.
    const nextStageRoot = document.querySelector<HTMLElement>(".client-stage-frame");
    setStageRoot((current) => (current === nextStageRoot ? current : nextStageRoot));
  });

  if (!stageRoot?.isConnected) return null;
  return createPortal(children, stageRoot);
}
