"use client";

import { useMemo, type CSSProperties } from "react";

import {
  activeGuideQuest,
  classAwareObjectiveLine,
  type GuideQuestLike,
} from "../../lib/onboarding-guidance";

// Net-new in-viewport "Current Objective" tracker (gap A + B).
//
// The guided first quest (sim GUIDE_QUEST_ID = 1001, "Field Wasp Trial") names a
// target but, before this, the only on-screen surface that mentioned it was the
// NPC dialog body — which closes the moment the player walks away, leaving a
// brand-new player with no idea WHERE the wasp / quest-giver is. This always-on
// HUD strip keeps the quest's directional `tracker` line (e.g. "Village Guide is
// waiting near the west banner.", "Field Wasp 0/1 defeated. Bring back proof.")
// visible the whole time the guided quest is active, plus a class-aware objective
// line so a ranged class isn't told to melee.
//
// Presentation-only and non-blocking: the root is pointer-events:none so it never
// intercepts world clicks (the player must be able to click the ground / the
// wasp / the NPC underneath it). It self-hides once the player is off the guided
// quest, so it never clutters the screen for veteran characters.

/** Structural subset of DisplayQuest this HUD needs. */
export type ObjectiveTrackerQuest = GuideQuestLike & {
  title?: string;
  objective?: string;
  progressLabel?: string;
  current?: number;
  required?: number;
};

export type ObjectiveTrackerProps = {
  /** The player's quest log (only the guide quest is surfaced). */
  questLog?: ObjectiveTrackerQuest[] | null;
  /** Lowercase player class key, for class-aware objective copy. */
  playerClass?: string | null;
};

const STAGE_LABEL: Record<string, string> = {
  available: "New Quest",
  inProgress: "Current Objective",
  readyToTurnIn: "Ready to Turn In",
};

export function ObjectiveTracker({ questLog, playerClass }: ObjectiveTrackerProps) {
  const quest = useMemo(() => activeGuideQuest(questLog ?? null), [questLog]);
  if (!quest) return null;

  const stageLabel = STAGE_LABEL[quest.stage] ?? "Current Objective";
  const objective = quest.objective
    ? classAwareObjectiveLine(quest.objective, playerClass)
    : "";
  const tracker = quest.tracker ?? "";
  const showProgress =
    typeof quest.required === "number" && quest.required > 0;

  return (
    <div style={ROOT_STYLE} aria-live="polite">
      <div style={PANEL_STYLE} role="status">
        <div style={HEADER_STYLE}>
          <span aria-hidden="true">🎯</span>
          <span style={HEADER_LABEL_STYLE}>{stageLabel}</span>
          {showProgress ? (
            <span style={PROGRESS_PILL_STYLE}>
              {`${quest.current ?? 0}/${quest.required}`}
            </span>
          ) : null}
        </div>
        {objective ? <div style={OBJECTIVE_STYLE}>{objective}</div> : null}
        {tracker ? <div style={TRACKER_STYLE}>📍 {tracker}</div> : null}
      </div>
    </div>
  );
}

// The tracker floats at the top-center of the play area. pointer-events:none on
// the root keeps world clicks reaching the canvas; the panel itself doesn't opt
// back in (it's read-only).
const ROOT_STYLE: CSSProperties = {
  position: "absolute",
  top: 10,
  left: "50%",
  transform: "translateX(-50%)",
  zIndex: 30,
  pointerEvents: "none",
  maxWidth: 360,
  width: "max-content",
};

const PANEL_STYLE: CSSProperties = {
  background: "linear-gradient(180deg, rgba(34, 23, 8, 0.92) 0%, rgba(14, 10, 6, 0.92) 100%)",
  border: "1px solid #9c8151",
  borderRadius: 3,
  boxShadow:
    "inset 0 0 0 1px rgba(212, 193, 141, 0.18), 0 4px 14px rgba(0, 0, 0, 0.55)",
  padding: "7px 12px",
  color: "#e9dcbf",
  fontSize: 12,
  lineHeight: 1.4,
  fontFamily: "inherit",
  textShadow: "1px 1px 0 #000",
};

const HEADER_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  marginBottom: 4,
};

const HEADER_LABEL_STYLE: CSSProperties = {
  flex: "1 1 auto",
  fontWeight: 700,
  color: "#ffdf9b",
  letterSpacing: 0.3,
  textTransform: "uppercase",
  fontSize: 11,
};

const PROGRESS_PILL_STYLE: CSSProperties = {
  flex: "0 0 auto",
  fontSize: 11,
  fontWeight: 700,
  color: "#241708",
  background: "linear-gradient(180deg, #e6c373, #b98a3c)",
  border: "1px solid #d4c18d",
  borderRadius: 2,
  padding: "0 6px",
  textShadow: "none",
};

const OBJECTIVE_STYLE: CSSProperties = {
  color: "#f4e7c6",
  marginBottom: 2,
};

const TRACKER_STYLE: CSSProperties = {
  color: "#bdf0b5",
  fontSize: 11,
};

export default ObjectiveTracker;
