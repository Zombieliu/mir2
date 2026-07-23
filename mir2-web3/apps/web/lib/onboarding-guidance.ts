// Net-new onboarding guidance helpers — pure, DOM-free, unit-testable.
//
// The web client layers two *non-Crystal* onboarding systems on top of the sim:
// the card tutorial (lib/tutorial-steps.ts) and the sim's guided first quest
// (GUIDE_QUEST_ID = 1001, "Field Wasp Trial"). Both are additive UX scaffolding,
// so the constraint here is CLARITY, not 1:1 parity.
//
// This module owns two presentation-only fixes that must stay testable and out of
// the big page.tsx / component bodies:
//
//   1. Class-aware objective text (gap C). The sim assembles the guide-quest
//      objective / NPC-dialog body server-side from a SHARED localization bundle
//      and ships the *resolved* string to the client. One of those strings tells
//      every class to "Stay in melee range before attacking." — wrong for ranged
//      classes (Archer, and the offensive Wizard/Taoist casters). The sim copy is
//      class-blind and a sim change is out of scope for this web-only loop, so we
//      rewrite the *displayed* string per class as a defensive post-process.
//
//   2. Guide-quest coordination (gap D). The card tutorial says "click any
//      monster" while the guide quest wants the *specific* Field Wasp. When the
//      guide quest is the player's active quest we surface the concrete target
//      name + where-to-go tracker so the two systems read as one coherent next
//      step instead of two unrelated prompts.
//
// Everything here is a pure function of its inputs; the React layers
// (NpcDialogPanel, the tutorial overlay, the objective beacon) call in.

// The guided first quest id, mirrored from the sim
// (apps/simulation/src/runtime/crystal_compat.rs `GUIDE_QUEST_ID`). Kept as a
// literal so this module has no cross-package import; the value is a fixed
// onboarding contract, not gameplay data.
export const GUIDE_QUEST_ID = 1001;

// Lowercase class keys as used by the web client (DisplayEntity.classKey /
// SelectCharacterEntry.classKey). Mir 2 has five classes; only the Warrior and
// the dual-wield Assassin are primarily melee. The Archer is ranged; the Wizard
// and Taoist fight at range with spells. The tutorial/quest copy must not tell
// any of the latter to close to melee.
export type OnboardingClassKey =
  | "warrior"
  | "wizard"
  | "taoist"
  | "assassin"
  | "archer"
  | string;

/** True when the class fights primarily in melee range (Warrior / Assassin). */
export function isMeleeClass(classKey: OnboardingClassKey | null | undefined): boolean {
  const key = (classKey ?? "").toLowerCase();
  return key === "warrior" || key === "assassin";
}

/** A short, class-appropriate range instruction for attacking a monster. */
export function rangeHintForClass(classKey: OnboardingClassKey | null | undefined): string {
  if (isMeleeClass(classKey)) {
    return "Close to melee range, then attack.";
  }
  const key = (classKey ?? "").toLowerCase();
  if (key === "archer") {
    return "Keep your distance and shoot from range.";
  }
  // Wizard / Taoist (and any unknown caster fallback) — cast from range.
  return "Attack from range with your skills.";
}

// Phrases the sim ships that hard-code a melee assumption. We only rewrite the
// melee-range instruction; the rest of the line (target, progress, return hint)
// is class-agnostic and left untouched. Matching is conservative (anchored on the
// known Crystal-side copy + light variants) so we never mangle unrelated text.
const MELEE_INSTRUCTION_RE =
  /\s*(?:Stay in|Get into|Close to|Move into|Keep to)\s+melee\s+range(?:\s+before\s+attacking)?\.?/gi;

/**
 * Rewrite a sim-resolved objective / dialog line so its attack-range instruction
 * matches the player's class. For melee classes the original copy is already
 * correct and returned unchanged; for ranged classes the melee instruction is
 * swapped for a class-appropriate one. Idempotent and safe on lines that contain
 * no melee instruction (returns them unchanged).
 */
export function classAwareObjectiveLine(
  line: string,
  classKey: OnboardingClassKey | null | undefined,
): string {
  if (typeof line !== "string" || line.length === 0) return line;
  if (!MELEE_INSTRUCTION_RE.test(line)) return line;
  // Melee classes: the original melee instruction is correct — leave it.
  if (isMeleeClass(classKey)) return line;
  const replacement = ` ${rangeHintForClass(classKey)}`;
  return line.replace(MELEE_INSTRUCTION_RE, replacement).replace(/\s{2,}/g, " ").trim();
}

/** Apply {@link classAwareObjectiveLine} across a list of lines. */
export function classAwareObjectiveLines(
  lines: string[],
  classKey: OnboardingClassKey | null | undefined,
): string[] {
  return lines.map((line) => classAwareObjectiveLine(line, classKey));
}

// ---- Guide-quest coordination (gap D) -----------------------------------------

/** Minimal quest shape this module needs (a structural subset of QuestEntry). */
export interface GuideQuestLike {
  questId: number;
  stage: string; // "available" | "inProgress" | "readyToTurnIn" | "completed"
  tracker?: string;
}

/**
 * Find the active guide quest (id 1001, not yet completed) in the quest log, if
 * present. Returns null when the player isn't on the guided quest so callers can
 * cheaply no-op for everyone past onboarding.
 */
export function activeGuideQuest<Q extends GuideQuestLike>(questLog: Q[] | null | undefined): Q | null {
  if (!Array.isArray(questLog)) return null;
  return (
    questLog.find((q) => q && q.questId === GUIDE_QUEST_ID && q.stage !== "completed") ?? null
  );
}

/** The concrete target name the guide quest points the player at. */
export const GUIDE_QUEST_TARGET_NAME = "Field Wasp";

/**
 * A one-line, class-aware coordination hint to append to the tutorial's generic
 * "attack a monster" step while the guide quest is active, so the card and the
 * quest name the same target. Returns null when the player isn't on the in-
 * progress guide quest (the generic card copy stands on its own).
 */
export function guideQuestAttackHint(
  questLog: GuideQuestLike[] | null | undefined,
  classKey: OnboardingClassKey | null | undefined,
): string | null {
  const quest = activeGuideQuest(questLog);
  if (!quest) return null;
  if (quest.stage === "available") {
    return "Your first quest: talk to the Village Guide to accept the Field Wasp Trial.";
  }
  if (quest.stage === "readyToTurnIn") {
    return "Wasp Stinger secured — head back to the Village Guide to turn it in.";
  }
  // inProgress: name the specific target + the class-correct approach.
  const range = isMeleeClass(classKey)
    ? "close to melee range"
    : classKey && classKey.toLowerCase() === "archer"
      ? "from range"
      : "from range with your skills";
  return `Your quest target is the ${GUIDE_QUEST_TARGET_NAME} — attack it ${range}.`;
}
