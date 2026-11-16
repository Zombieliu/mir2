import baseline from "./generated/r16-gameplay-baseline.json";
import type { QuestGuidanceProfile } from "./quest-action-policy";

/** The Windows R16 rules are the common client baseline, not a per-Web flag. */
export function r16QuestGuidanceProfile(): QuestGuidanceProfile {
  return baseline.questGuidance as QuestGuidanceProfile;
}

/** R16 uses the shared, authoritative Quest UI. The old browser action-count
 * tour is not the game's onboarding, including on a fresh browser or character. */
export function shouldAutoStartR16Tutorial(): boolean {
  return baseline.onboarding !== "native";
}
