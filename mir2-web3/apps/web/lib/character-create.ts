import type { CreateCharacterDraft, EntityClassKey } from "../app/components/original-client-types";

export type CharacterCreateStatus = "idle" | "pending" | "created" | "failed" | "slotsFull" | "nameTaken";

export type CharacterCreateControls = {
  classes: readonly EntityClassKey[];
  status: CharacterCreateStatus;
  submit: (draft: CreateCharacterDraft) => void;
  clearNotice: () => void;
};

const classicClasses = ["warrior", "wizard", "taoist"] as const;
const fullClasses = [...classicClasses, "assassin", "archer"] as const;
const noClasses: readonly EntityClassKey[] = [];

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

/** Use only the current authenticated Gateway's realm handshake. The server
 * still checks each request against its active content profile. */
export function characterCreateClasses(realm: unknown): readonly EntityClassKey[] {
  const info = record(realm);
  if (!info || info.schema !== "mir2-realm-handshake/1" || typeof info.realmId !== "string"
    || !info.realmId.trim() || typeof info.profileVersion !== "number"
    || !Number.isSafeInteger(info.profileVersion)) return noClasses;
  if (info.profileId === "platinum_176" && info.profileVersion > 0) return classicClasses;
  if ((info.profileId === "platinum_fiveclass" && info.profileVersion === 1)
    || (info.profileId === "crystal_full" && info.profileVersion === 0)) return fullClasses;
  return noClasses;
}

export function matchesCreatedCharacter(value: unknown, draft: CreateCharacterDraft): boolean {
  const row = record(value);
  if (!row) return false;
  const index = row.index ?? row.Index;
  const name = row.name ?? row.Name;
  const level = row.level ?? row.Level;
  const job = row.class ?? row.Class;
  const gender = row.gender ?? row.Gender;
  return typeof index === "number" && Number.isSafeInteger(index) && index >= 0 && index <= 2_147_483_647
    // R16 creation replies use level 0 as the selection preview sentinel.
    && name === draft.name && typeof level === "number" && Number.isSafeInteger(level) && level >= 0 && level <= 65_535
    && (typeof job === "string" ? job.toLowerCase() === draft.classKey : job === fullClasses.indexOf(draft.classKey))
    && (typeof gender === "string" ? gender.toLowerCase() === draft.gender : gender === (draft.gender === "female" ? 1 : 0));
}
