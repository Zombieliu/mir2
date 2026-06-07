"use client";

import { useEffect, useReducer, useRef, useState, type CSSProperties } from "react";

import {
  TUTORIAL_STEPS,
  createTutorialState,
  currentStep,
  pickText,
  reduceTutorial,
  type TutorialEvent,
  type TutorialLang,
  type TutorialState,
  type TutorialWindow,
} from "../../lib/tutorial-steps";

// Net-new interactive beginner tutorial (the original Crystal client has none —
// see lib/tutorial-steps.ts header). Self-contained presentational overlay: it
// drives the pure reducer in lib/tutorial-steps.ts off two signal sources —
// `mir2:action` CustomEvents dispatched by page.tsx `send()` (every outbound
// ClientPacket) and the window-open booleans passed in as props.
//
// IMPORTANT: the root layer is pointer-events:none so it never intercepts world
// clicks — the player must be able to click monsters / the ground / NPCs to
// satisfy action steps. Only the instruction card itself is interactive.

export type TutorialOverlayProps = {
  language: TutorialLang;
  // Live open/closed state of the tracked windows (drives window-trigger steps).
  windows: { inventory: boolean; character: boolean; questLog: boolean };
  // Called once when the player finishes or skips the whole flow.
  onClose: () => void;
};

const ACTION_EVENT = "mir2:action";

// Detail shape dispatched by page.tsx send(): { type: <ClientPacket type> }.
type ActionEventDetail = { type?: unknown };

export function OriginalClientTutorialOverlay({ language, windows, onClose }: TutorialOverlayProps) {
  const [state, dispatch] = useReducer(reduceTutorial, undefined, createTutorialState);
  const [spotlightRect, setSpotlightRect] = useState<DOMRect | null>(null);
  const closedRef = useRef(false);

  // Outbound-action stream → reducer. One global listener; cheap.
  useEffect(() => {
    const onAction = (event: Event) => {
      const detail = (event as CustomEvent<ActionEventDetail>).detail;
      const type = typeof detail?.type === "string" ? detail.type : null;
      if (type) dispatch({ kind: "action", type });
    };
    window.addEventListener(ACTION_EVENT, onAction);
    return () => window.removeEventListener(ACTION_EVENT, onAction);
  }, []);

  // Window-open edges → reducer. Fire only on the closed→open transition.
  const prevWindowsRef = useRef(windows);
  useEffect(() => {
    const prev = prevWindowsRef.current;
    (Object.keys(windows) as TutorialWindow[]).forEach((key) => {
      if (windows[key] && !prev[key]) dispatch({ kind: "window", window: key, open: true });
    });
    prevWindowsRef.current = windows;
  }, [windows]);

  // Finish/skip → notify parent exactly once.
  useEffect(() => {
    if (state.done && !closedRef.current) {
      closedRef.current = true;
      try {
        window.localStorage.setItem("mir2:tutorialCompleted", "1");
      } catch {
        /* ignore storage failures (private mode etc.) */
      }
      onClose();
    }
  }, [state.done, onClose]);

  const step = currentStep(state);

  // Resolve the optional spotlight target each step / on resize. Degrades to no
  // ring if the selector is absent or doesn't match anything in the DOM.
  useEffect(() => {
    if (!step?.spotlight) {
      setSpotlightRect(null);
      return;
    }
    const update = () => {
      const el = document.querySelector(step.spotlight as string);
      setSpotlightRect(el ? el.getBoundingClientRect() : null);
    };
    update();
    window.addEventListener("resize", update);
    const interval = window.setInterval(update, 500);
    return () => {
      window.removeEventListener("resize", update);
      window.clearInterval(interval);
    };
  }, [step?.id, step?.spotlight]);

  if (state.done || !step) return null;

  const total = TUTORIAL_STEPS.length;
  const stepNumber = state.stepIndex + 1;
  const isManual = step.trigger.kind === "manual";
  const isLast = state.stepIndex === total - 1;

  const send = (event: TutorialEvent) => dispatch(event);

  return (
    <div style={ROOT_STYLE} aria-hidden={false}>
      {spotlightRect ? <div style={spotlightStyle(spotlightRect)} /> : null}

      <div style={CARD_STYLE} role="dialog" aria-label="Beginner tutorial">
        <div style={HEADER_STYLE}>
          <span style={BADGE_STYLE}>
            {stepNumber} / {total}
          </span>
          <span style={TITLE_STYLE}>{pickText(step.title, language)}</span>
          <button
            type="button"
            style={CLOSE_STYLE}
            onClick={() => send({ kind: "skipAll" })}
            aria-label={language === "zh-CN" ? "关闭教程" : "Close tutorial"}
          >
            ✕
          </button>
        </div>

        <p style={BODY_STYLE}>{pickText(step.body, language)}</p>

        {step.hint ? <div style={HINT_STYLE}>👉 {pickText(step.hint, language)}</div> : null}

        <ProgressBar value={stepNumber} max={total} />

        <div style={ACTIONS_STYLE}>
          <button
            type="button"
            style={GHOST_BUTTON_STYLE}
            onClick={() => send({ kind: "back" })}
            disabled={state.stepIndex === 0}
          >
            {language === "zh-CN" ? "上一步" : "Back"}
          </button>

          {!isManual ? (
            <button type="button" style={GHOST_BUTTON_STYLE} onClick={() => send({ kind: "skipStep" })}>
              {language === "zh-CN" ? "跳过这步" : "Skip step"}
            </button>
          ) : null}

          <button type="button" style={PRIMARY_BUTTON_STYLE} onClick={() => send({ kind: "next" })}>
            {isLast
              ? language === "zh-CN"
                ? "完成"
                : "Finish"
              : isManual
                ? language === "zh-CN"
                  ? "下一步"
                  : "Next"
                : language === "zh-CN"
                  ? "知道了,下一步"
                  : "Got it, next"}
          </button>
        </div>
      </div>
    </div>
  );
}

function ProgressBar({ value, max }: { value: number; max: number }) {
  const pct = Math.max(0, Math.min(100, (value / max) * 100));
  return (
    <div style={PROGRESS_TRACK_STYLE}>
      <div style={{ ...PROGRESS_FILL_STYLE, width: `${pct}%` }} />
    </div>
  );
}

// ---- styles (inline, matching the project's other window components) ----------

const ROOT_STYLE: CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: 4000,
  pointerEvents: "none", // never block world clicks; only the card opts back in
};

const CARD_STYLE: CSSProperties = {
  position: "absolute",
  left: "50%",
  bottom: 24,
  transform: "translateX(-50%)",
  width: 420,
  maxWidth: "calc(100vw - 32px)",
  pointerEvents: "auto",
  background: "rgba(18, 14, 10, 0.96)",
  border: "1px solid #b8893a",
  borderRadius: 8,
  boxShadow: "0 8px 28px rgba(0, 0, 0, 0.6)",
  padding: "14px 16px",
  color: "#f1e6d2",
  fontSize: 13,
  lineHeight: 1.5,
  fontFamily: "inherit",
};

const HEADER_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  marginBottom: 8,
};

const BADGE_STYLE: CSSProperties = {
  flex: "0 0 auto",
  fontSize: 11,
  fontWeight: 700,
  color: "#1a140c",
  background: "#d8a44b",
  borderRadius: 10,
  padding: "1px 8px",
};

const TITLE_STYLE: CSSProperties = {
  flex: "1 1 auto",
  fontSize: 15,
  fontWeight: 700,
  color: "#ffd98a",
};

const CLOSE_STYLE: CSSProperties = {
  flex: "0 0 auto",
  background: "transparent",
  border: "none",
  color: "#c9b896",
  fontSize: 14,
  cursor: "pointer",
  lineHeight: 1,
  padding: 2,
};

const BODY_STYLE: CSSProperties = { margin: "0 0 10px" };

const HINT_STYLE: CSSProperties = {
  background: "rgba(216, 164, 75, 0.14)",
  border: "1px solid rgba(216, 164, 75, 0.4)",
  borderRadius: 6,
  padding: "6px 10px",
  marginBottom: 10,
  color: "#ffd98a",
  fontWeight: 600,
};

const PROGRESS_TRACK_STYLE: CSSProperties = {
  height: 4,
  borderRadius: 2,
  background: "rgba(255, 255, 255, 0.12)",
  marginBottom: 12,
  overflow: "hidden",
};

const PROGRESS_FILL_STYLE: CSSProperties = {
  height: "100%",
  background: "#d8a44b",
  transition: "width 0.25s ease",
};

const ACTIONS_STYLE: CSSProperties = {
  display: "flex",
  justifyContent: "flex-end",
  gap: 8,
};

const BUTTON_BASE: CSSProperties = {
  borderRadius: 6,
  padding: "6px 12px",
  fontSize: 12,
  fontWeight: 600,
  cursor: "pointer",
  fontFamily: "inherit",
};

const PRIMARY_BUTTON_STYLE: CSSProperties = {
  ...BUTTON_BASE,
  background: "#d8a44b",
  color: "#1a140c",
  border: "1px solid #e7be6f",
};

const GHOST_BUTTON_STYLE: CSSProperties = {
  ...BUTTON_BASE,
  background: "transparent",
  color: "#e3d6bd",
  border: "1px solid rgba(216, 164, 75, 0.5)",
};

function spotlightStyle(rect: DOMRect): CSSProperties {
  const pad = 6;
  return {
    position: "absolute",
    left: rect.left - pad,
    top: rect.top - pad,
    width: rect.width + pad * 2,
    height: rect.height + pad * 2,
    border: "2px solid #ffd98a",
    borderRadius: 6,
    boxShadow: "0 0 0 9999px rgba(0, 0, 0, 0.45)",
    pointerEvents: "none",
    transition: "all 0.2s ease",
  };
}

export default OriginalClientTutorialOverlay;
