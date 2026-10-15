import { CRYSTAL_COMBAT_MODE_FUNCTIONS } from "./shared-combat-mode-keys";

/** Native action identities; the host supplies current authority and focus guards. */
export const CRYSTAL_WINDOW_SHORTCUTS = {
  Inventory: "inventory", Inventory2: "inventory", Equipment: "equipment", Equipment2: "equipment",
  Skills: "skills", Skills2: "skills", HeroInventory: "heroInventory", HeroEquipment: "heroEquipment",
  HeroSkills: "heroSkills", Creature: "creatures", Mentor: "bonds", Relationship: "bonds", Friends: "friends",
  Guilds: "guild", GameShop: "gameShop", Quests: "quests", Options: "options", Options2: "options",
  Group: "group", Belt: "belt", BeltFlip: "beltFlip", Minimap: "minimap", Bigmap: "bigmap",
  Ranking: "ranking", Help: "help", Keybind: "keybind", Closeall: "closeAll", Skillbar: "skillBar",
} as const;
export type CrystalWindowShortcut = typeof CRYSTAL_WINDOW_SHORTCUTS[keyof typeof CRYSTAL_WINDOW_SHORTCUTS];
export function crystalWindowShortcut(functionId: string): CrystalWindowShortcut | null {
  return Object.hasOwn(CRYSTAL_WINDOW_SHORTCUTS, functionId)
    ? CRYSTAL_WINDOW_SHORTCUTS[functionId as keyof typeof CRYSTAL_WINDOW_SHORTCUTS] : null;
}
export function crystalBeltShortcutSlot(functionId: string): number | null {
  const match = /^Belt([1-8])(?:Alt)?$/.exec(functionId);
  return match?.[0] === functionId ? Number(match[1]) - 1 : null;
}
export function crystalSkillShortcutSlot(functionId: string): number | null {
  const match = /^Bar([12])Skill([1-8])$/.exec(functionId);
  return match?.[0] === functionId ? (Number(match[1]) - 1) * 8 + Number(match[2]) : null;
}
export const SUPPORTED_CRYSTAL_BROWSER_FUNCTIONS: readonly string[] = Object.freeze([
  ...Object.keys(CRYSTAL_WINDOW_SHORTCUTS),
  ...Array.from({ length: 16 }, (_, index) => `Bar${index < 8 ? 1 : 2}Skill${index % 8 + 1}`),
  ...Array.from({ length: 8 }, (_, index) => [`Belt${index + 1}`, `Belt${index + 1}Alt`]).flat(),
  "Logout", "Pickup", "Trade", "AddGroupMember", "DropView",
  ...CRYSTAL_COMBAT_MODE_FUNCTIONS,
]);
