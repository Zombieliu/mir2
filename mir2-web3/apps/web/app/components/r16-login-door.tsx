import { useEffect, useState } from "react";
import { ORIGINAL_UI } from "../../lib/original-ui";
import { preloadSceneAssetUrls } from "../../lib/scene-asset-preloader";
import {
  LOGIN_STATIC_BACKGROUND_FRAME,
  LOGIN_TRANSITION_FRAME_MS,
  loginDoorFrameAt,
  type LoginDoorOutcome,
} from "./original-client-shell-flow";

const LOGIN_DOOR_PRELOAD_TIMEOUT_MS = 30_000;
const doorFrames = ORIGINAL_UI.login.backgroundFrames;

export function R16LoginDoor({ token, onStart, onOpened }: {
  token: number;
  onStart: () => void;
  onOpened: (token: number, outcome: LoginDoorOutcome) => void;
}) {
  const [frame, setFrame] = useState(LOGIN_STATIC_BACKGROUND_FRAME);

  useEffect(() => {
    let cancelled = false;
    let timer: number | null = null;
    const preload = new AbortController();
    // These large frames are prepared after auth, outside the critical login pack.
    // Keep only one speculative frame in flight; leaving retires its queue.
    void preloadSceneAssetUrls(doorFrames, LOGIN_DOOR_PRELOAD_TIMEOUT_MS, {
      concurrency: 1,
      signal: preload.signal,
    }).then(({ visualReady }) => {
      if (cancelled) return;
      if (!visualReady) {
        onOpened(token, "unavailable");
        return;
      }

      const startedAt = performance.now();
      onStart();
      function advance() {
        if (cancelled) return;
        const elapsed = performance.now() - startedAt;
        const next = loginDoorFrameAt(elapsed, doorFrames.length);
        if (next === null) {
          onOpened(token, "played");
          return;
        }
        setFrame(next);
        timer = window.setTimeout(advance, Math.max(1, Math.ceil(next * LOGIN_TRANSITION_FRAME_MS - elapsed)));
      }
      advance();
    });

    return () => {
      cancelled = true;
      preload.abort();
      if (timer !== null) window.clearTimeout(timer);
    };
  }, [token, onStart, onOpened]);

  return (
    <div className="client-scene-overlay login-transition-overlay" aria-hidden="true" data-login-door-frame={frame}>
      <img className="client-scene-background" src={doorFrames[frame]} alt="" draggable={false} />
    </div>
  );
}
