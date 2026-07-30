"use client";

import { useEffect, useReducer, useRef, useState, type CSSProperties } from "react";

import {
  TUTORIAL_CONTROL_EVENT,
  createTutorialState,
  currentStep,
  pickText,
  reduceTutorial,
  tutorialCompletionStorageKey,
  tutorialStepsForInput,
  type TutorialEvent,
  type TutorialGamepadFamily,
  type TutorialInputProfile,
  type TutorialLang,
  type TutorialWindow,
} from "../../lib/tutorial-steps";
import { guideQuestAttackHint, type GuideQuestLike } from "../../lib/onboarding-guidance";

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
  input: TutorialInputProfile;
  gamepadFamily: TutorialGamepadFamily;
  // Live open/closed state of the tracked windows (drives window-trigger steps).
  windows: { inventory: boolean; character: boolean; questLog: boolean };
  // The player's active quest log (drives guide-quest coordination on the
  // "attack a monster" step so the generic card and the guided first quest name
  // the same target). Optional + defensive: absent → no coordination line.
  questLog?: GuideQuestLike[] | null;
  // Lowercase player class key, used to keep the coordination hint class-aware
  // (don't tell an Archer to melee the wasp).
  playerClass?: string | null;
  // Called once when the player finishes or skips the whole flow.
  onClose: () => void;
};

const ACTION_EVENT = "mir2:action";

// Detail shape dispatched by page.tsx send(): { type: <ClientPacket type> }.
type ActionEventDetail = { type?: unknown };

export function OriginalClientTutorialOverlay({
  language,
  input,
  gamepadFamily,
  windows,
  questLog,
  playerClass,
  onClose,
}: TutorialOverlayProps) {
  const [state, dispatch] = useReducer(
    reduceTutorial,
    { input, gamepadFamily },
    (seed) => createTutorialState(seed.input, seed.gamepadFamily),
  );
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

  // Touch and gamepad adapters emit semantic control events that do not always
  // correspond 1:1 with a network packet (for example toggling Run or opening a
  // panel). Feed those into the same pure reducer as outbound game actions.
  useEffect(() => {
    const onControl = (event: Event) => {
      const detail = (event as CustomEvent<{ action?: unknown }>).detail;
      const action = typeof detail?.action === "string" ? detail.action : null;
      if (action) dispatch({ kind: "action", type: action });
    };
    window.addEventListener(TUTORIAL_CONTROL_EVENT, onControl);
    return () => window.removeEventListener(TUTORIAL_CONTROL_EVENT, onControl);
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
        window.localStorage.setItem(
          tutorialCompletionStorageKey(state.input, state.gamepadFamily),
          "1",
        );
        if (state.input === "keyboardMouse") {
          window.localStorage.setItem("mir2:tutorialCompleted", "1");
        }
      } catch {
        /* ignore storage failures (private mode etc.) */
      }
      onClose();
    }
  }, [state.done, state.gamepadFamily, state.input, onClose]);

  const step = currentStep(state);

  // Keep the active tutorial step visible to the project's browser QA hook.
  // This is deliberately read-only and contains no player/account data.
  useEffect(() => {
    const gameWindow = window as typeof window & {
      __mir2Tutorial?: {
        input: TutorialInputProfile;
        gamepadFamily: TutorialGamepadFamily;
        stepId: string | null;
        stepIndex: number;
        done: boolean;
      };
    };
    gameWindow.__mir2Tutorial = {
      input: state.input,
      gamepadFamily: state.gamepadFamily,
      stepId: step?.id ?? null,
      stepIndex: state.stepIndex,
      done: state.done,
    };
    return () => {
      delete gameWindow.__mir2Tutorial;
    };
  }, [state.done, state.gamepadFamily, state.input, state.stepIndex, step?.id]);

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

  const steps = tutorialStepsForInput(state.input, state.gamepadFamily);
  const total = steps.length;
  const stepNumber = state.stepIndex + 1;
  const isManual = step.trigger.kind === "manual";
  const isLast = state.stepIndex === total - 1;

  // Gap D: on the generic "attack a monster" step, if the player is on the guided
  // first quest, name the specific target so the card and the quest read as one
  // coherent next step instead of two unrelated prompts. Class-aware so a ranged
  // class isn't told to melee.
  const guideHint =
    step.id === "attack" ? guideQuestAttackHint(questLog, playerClass) : null;

  const send = (event: TutorialEvent) => dispatch(event);

  const touchPresentation = state.input === "touch";

  return (
    <div
      className="mir-tutorial-overlay"
      data-tutorial-input={state.input}
      data-tutorial-gamepad-family={state.gamepadFamily}
      data-tutorial-step={step.id}
      style={ROOT_STYLE}
      aria-hidden={false}
    >
      {spotlightRect ? <div style={spotlightStyle(spotlightRect)} /> : null}

      <div
        className="mir-tutorial-card"
        style={touchPresentation ? { ...CARD_STYLE, ...TOUCH_CARD_STYLE } : CARD_STYLE}
        role="dialog"
        aria-label={language === "zh-CN" ? "操作教学" : "Controls tutorial"}
      >
        <span style={studStyle("tl")} aria-hidden="true" />
        <span style={studStyle("tr")} aria-hidden="true" />
        <span style={studStyle("bl")} aria-hidden="true" />
        <span style={studStyle("br")} aria-hidden="true" />

        <div style={TITLEBAR_STYLE}>
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

        <div style={CONTENT_STYLE}>
          <p style={BODY_STYLE}>{pickText(step.body, language)}</p>

          {step.hint ? (
            <div style={HINT_STYLE}>👉 {pickText(step.hint, language)}</div>
          ) : null}

          {guideHint ? <div style={QUEST_HINT_STYLE}>🎯 {guideHint}</div> : null}

          <ProgressBar value={stepNumber} max={total} />

          <div style={ACTIONS_STYLE}>
            <button
              type="button"
              style={tutorialButtonStyle(
                state.stepIndex === 0
                  ? { ...GHOST_BUTTON_STYLE, opacity: 0.4, cursor: "default" }
                  : GHOST_BUTTON_STYLE,
                touchPresentation,
              )}
              onClick={() => send({ kind: "back" })}
              disabled={state.stepIndex === 0}
            >
              {language === "zh-CN" ? "上一步" : "Back"}
            </button>

            {!isManual ? (
              <button
                type="button"
                style={tutorialButtonStyle(GHOST_BUTTON_STYLE, touchPresentation)}
                onClick={() => send({ kind: "skipStep" })}
              >
                {language === "zh-CN" ? "跳过这步" : "Skip step"}
              </button>
            ) : null}

            <button
              type="button"
              style={tutorialButtonStyle(PRIMARY_BUTTON_STYLE, touchPresentation)}
              onClick={() => send({ kind: "next" })}
            >
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

// The original Crystal client frames its windows with a heavy beveled gold rail over
// dark parchment, an embossed title band and riveted corners (cf. the inventory /
// character window sprites + `.npc-dialog-panel` in globals.css). This net-new tutorial
// card replicates that metalwork in pure CSS — so it reflows with variable step content
// and needs no bespoke fixed-size sprite: a metallic gradient border (border-image),
// layered inner bevels, a stamped title band, brass buttons and corner studs.
const CARD_STYLE: CSSProperties = {
  position: "absolute",
  left: "50%",
  bottom: 24,
  transform: "translateX(-50%)",
  width: 432,
  maxWidth: "calc(100vw - 32px)",
  pointerEvents: "auto",
  overflow: "hidden",
  background: "linear-gradient(180deg, #221708 0%, #0e0a06 100%)",
  border: "3px solid transparent",
  borderImage:
    "linear-gradient(140deg, #5a431d 0%, #f4dd95 16%, #b07f2e 34%, #f0d089 52%, #7a5e26 70%, #e6c879 86%, #5a431d 100%) 1",
  boxShadow:
    "inset 0 0 0 1px #2b1d0d, inset 0 0 0 4px rgba(212, 193, 141, 0.18), inset 0 0 22px rgba(0, 0, 0, 0.55), 0 12px 30px rgba(0, 0, 0, 0.62)",
  color: "#e9dcbf",
  fontSize: 13,
  lineHeight: 1.55,
  fontFamily: 'Georgia, "Times New Roman", serif',
  textShadow: "1px 1px 0 #000",
};

const TOUCH_CARD_STYLE: CSSProperties = {
  bottom: 8,
  width: 400,
  maxWidth: "calc(100vw - 280px)",
  maxHeight: "calc(100dvh - 16px)",
  overflowY: "auto",
  fontSize: 16,
  lineHeight: 1.35,
};

// Embossed gold title band, like the original window headers.
const TITLEBAR_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "7px 12px",
  background: "linear-gradient(180deg, #4a3a1c 0%, #2a2010 55%, #170f07 100%)",
  borderBottom: "1px solid #6b5320",
  boxShadow:
    "inset 0 1px 0 rgba(244, 221, 149, 0.35), inset 0 -1px 0 rgba(0, 0, 0, 0.6)",
};

const CONTENT_STYLE: CSSProperties = {
  padding: "12px 16px 14px",
};

// Riveted corner stud — four are pinned just inside the gold rail.
const STUD_BASE: CSSProperties = {
  position: "absolute",
  width: 6,
  height: 6,
  borderRadius: "50%",
  background:
    "radial-gradient(circle at 35% 30%, #ffe9a8, #b1832f 60%, #5a431d)",
  boxShadow: "0 0 0 1px #2b1d0d",
  pointerEvents: "none",
  zIndex: 2,
};

const BADGE_STYLE: CSSProperties = {
  flex: "0 0 auto",
  fontSize: 11,
  fontWeight: 700,
  color: "#241708",
  background: "linear-gradient(180deg, #e6c373, #b98a3c)",
  border: "1px solid #d4c18d",
  borderRadius: 2,
  padding: "1px 7px",
  boxShadow: "inset 0 1px 0 rgba(255, 255, 255, 0.4)",
  textShadow: "none",
};

const TITLE_STYLE: CSSProperties = {
  flex: "1 1 auto",
  fontSize: 15,
  fontWeight: 700,
  color: "#ffdf9b",
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
  background: "rgba(156, 129, 81, 0.16)",
  border: "1px solid rgba(156, 129, 81, 0.5)",
  borderRadius: 2,
  padding: "6px 10px",
  marginBottom: 10,
  color: "#ffdf9b",
  fontWeight: 600,
};

// Guide-quest coordination line — a distinct green-tinted callout so it reads as
// "your actual quest" rather than another generic tutorial hint.
const QUEST_HINT_STYLE: CSSProperties = {
  background: "rgba(106, 168, 95, 0.16)",
  border: "1px solid rgba(123, 224, 122, 0.5)",
  borderRadius: 2,
  padding: "6px 10px",
  marginBottom: 10,
  color: "#bdf0b5",
  fontWeight: 600,
};

const PROGRESS_TRACK_STYLE: CSSProperties = {
  height: 6,
  borderRadius: 0,
  background: "#0b0805",
  border: "1px solid #3a2c16",
  boxShadow: "inset 0 1px 2px rgba(0, 0, 0, 0.6)",
  marginBottom: 12,
  overflow: "hidden",
};

const PROGRESS_FILL_STYLE: CSSProperties = {
  height: "100%",
  background: "linear-gradient(180deg, #f0d089, #c79a45)",
  transition: "width 0.25s ease",
};

const ACTIONS_STYLE: CSSProperties = {
  display: "flex",
  justifyContent: "flex-end",
  gap: 8,
};

const BUTTON_BASE: CSSProperties = {
  borderRadius: 2,
  padding: "5px 12px",
  fontSize: 12,
  fontWeight: 600,
  cursor: "pointer",
  fontFamily: 'Georgia, "Times New Roman", serif',
  textShadow: "1px 1px 0 rgba(0, 0, 0, 0.5)",
};

const PRIMARY_BUTTON_STYLE: CSSProperties = {
  ...BUTTON_BASE,
  background: "linear-gradient(180deg, #e6c373 0%, #b1832f 100%)",
  color: "#241708",
  border: "1px solid #d4c18d",
  boxShadow: "inset 0 1px 0 rgba(255, 255, 255, 0.45), 0 1px 0 #2b1d0d",
  textShadow: "none",
};

const GHOST_BUTTON_STYLE: CSSProperties = {
  ...BUTTON_BASE,
  background: "linear-gradient(180deg, #4a3c22 0%, #2a2010 100%)",
  color: "#e9dcbf",
  border: "1px solid #9c8151",
  boxShadow: "inset 0 1px 0 rgba(212, 193, 141, 0.3)",
};

function studStyle(corner: "tl" | "tr" | "bl" | "br"): CSSProperties {
  const vertical = corner[0] === "t" ? { top: 5 } : { bottom: 5 };
  const horizontal = corner[1] === "l" ? { left: 5 } : { right: 5 };
  return { ...STUD_BASE, ...vertical, ...horizontal };
}

function tutorialButtonStyle(style: CSSProperties, touchPresentation: boolean): CSSProperties {
  return touchPresentation
    ? { ...style, minHeight: 40, padding: "8px 12px", fontSize: 14 }
    : style;
}

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
