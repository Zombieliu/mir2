import nativeKeyboardDefaults from "../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json";

/** Native local_keyboard_ui: repeated down never extends the current five seconds. */
export function crystalDropViewDeadline(until: number | null, nowMs: number): number | null {
  if (!Number.isFinite(nowMs) || nowMs < 0 || nowMs > Number.MAX_SAFE_INTEGER - 5000
    || until !== null && (!Number.isFinite(until) || until < 0 || until > Number.MAX_SAFE_INTEGER)) return null;
  return until === null || nowMs > until ? nowMs + 5000 : until;
}

/** Exact seven local Crystal OptionDialog fields (ui-core/state.rs:122). */
export const CRYSTAL_OPTION_KEYS = ["skillMode", "skillBar", "effect", "dropView", "nameView", "hpView", "newMove"] as const;
export type CrystalOptionKey = typeof CRYSTAL_OPTION_KEYS[number];
export type PlayerUiPreferences = Record<CrystalOptionKey, boolean> & {
  musicEnabled: boolean; musicVolume: number; soundEnabled: boolean; soundVolume: number;
};
export const PLAYER_UI_PREFERENCES_STORAGE_KEY = "mir2.playerUiPreferences.v1";
export const CRYSTAL_KEY_BINDINGS_STORAGE_KEY = "mir2.crystalKeyBindings.v1";
export const CRYSTAL_HELP_STATE_STORAGE_KEY = "mir2.crystalHelpState.v1";
export const DEFAULT_PLAYER_UI_PREFERENCES: Readonly<PlayerUiPreferences> = Object.freeze({
  skillMode: false, skillBar: true, effect: true, dropView: true, nameView: true, hpView: true, newMove: false,
  musicEnabled: true, musicVolume: 80, soundEnabled: true, soundVolume: 80,
});
const preferenceKeys = [...CRYSTAL_OPTION_KEYS, "musicEnabled", "musicVolume", "soundEnabled", "soundVolume"];
function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function validPreferences(value: unknown): value is PlayerUiPreferences {
  return record(value) && Object.keys(value).length === preferenceKeys.length
    && preferenceKeys.every(key => key === "musicVolume" || key === "soundVolume"
      ? Number.isInteger(value[key]) && (value[key] as number) >= 0 && (value[key] as number) <= 100
      : typeof value[key] === "boolean");
}
export function parsePlayerUiPreferences(text: string): PlayerUiPreferences | null {
  if (typeof text !== "string" || text.length > 2048) return null;
  try { const value: unknown = JSON.parse(text); return record(value) && Object.keys(value).length === 2
    && value.version === 1 && validPreferences(value.preferences) ? { ...value.preferences } : null; } catch { return null; }
}
export function serializePlayerUiPreferences(preferences: PlayerUiPreferences): string | null {
  return validPreferences(preferences) ? JSON.stringify({ version: 1, preferences }) : null;
}
/** Host applies this to setOriginalAudioSettings; no storage or audio side effects here. */
export function playerUiPreferencesAudioSettings(preferences: PlayerUiPreferences) {
  return { musicEnabled: preferences.musicEnabled, effectsEnabled: preferences.soundEnabled,
    musicVolume: preferences.musicVolume / 100, effectsVolume: preferences.soundVolume / 100 };
}

export type CrystalModifierRule = 0 | 1 | 2;
export type CrystalKeyModifiers = { alt: boolean; ctrl: boolean; shift: boolean; tilde: boolean };
export type CrystalKeyBinding = {
  function: string; group: string; description: string; key: string;
  alt: CrystalModifierRule; ctrl: CrystalModifierRule; shift: CrystalModifierRule; tilde: CrystalModifierRule;
};
const modifierKeys = ["alt", "ctrl", "shift", "tilde"] as const;
/** Native keyboard_dialog.rs uses this same JSON and preserves declaration order. */
export function defaultCrystalKeyBindings(): CrystalKeyBinding[] {
  return nativeKeyboardDefaults.map(row => ({ ...row,
    alt: row.alt as CrystalModifierRule, ctrl: row.ctrl as CrystalModifierRule,
    shift: row.shift as CrystalModifierRule, tilde: row.tilde as CrystalModifierRule }));
}
function validSavedBinding(value: unknown): value is CrystalKeyBinding {
  return record(value) && typeof value.function === "string" && typeof value.group === "string"
    && typeof value.description === "string" && typeof value.key === "string"
    && /^[A-Za-z0-9]{1,32}$/.test(value.key)
    && modifierKeys.every(key => value[key] === 0 || value[key] === 1 || value[key] === 2);
}
/** Atomic Native load: metadata/order are canonical; saved unknown/duplicate functions fail. */
export function parseCrystalKeyBindings(text: string): CrystalKeyBinding[] | null {
  if (typeof text !== "string" || text.length > 65536) return null;
  try {
    const saved: unknown = JSON.parse(text);
    const next = defaultCrystalKeyBindings();
    if (!Array.isArray(saved) || saved.length > next.length) return null;
    const seen = new Set<string>();
    for (const item of saved) {
      if (!validSavedBinding(item) || seen.has(item.function)) return null;
      seen.add(item.function);
      const target = next.find(row => row.function === item.function);
      if (!target) return null;
      target.key = item.key;
      for (const key of modifierKeys) target[key] = item[key];
    }
    return next;
  } catch { return null; }
}
export function serializeCrystalKeyBindings(bindings: readonly CrystalKeyBinding[]): string | null {
  const text = JSON.stringify(bindings);
  const canonical = parseCrystalKeyBindings(text);
  return canonical ? JSON.stringify(canonical) : null;
}
export function displayCrystalKeyBinding(binding: CrystalKeyBinding): string {
  if (binding.key === "None") return "";
  return [...modifierKeys.filter(key => binding[key] === 1).map(key =>
    ({ alt: "Alt", ctrl: "Ctrl", shift: "Shift", tilde: "~" })[key]), binding.key].join(" + ");
}
export function crystalKeyBindingMatches(binding: CrystalKeyBinding, key: string, modifiers: CrystalKeyModifiers): boolean {
  return binding.key !== "None" && binding.key === key
    && !(key === "Insert" && modifiers.ctrl && !modifiers.alt && !modifiers.shift)
    && modifierKeys.every(name => binding[name] === 2 || binding[name] === Number(modifiers[name]));
}
/** Multiple actions on one key are intentional (default Tab: Pickup and DropView). */
export function matchingCrystalKeyFunctions(bindings: readonly CrystalKeyBinding[], key: string, modifiers: CrystalKeyModifiers): string[] {
  return bindings.filter(binding => crystalKeyBindingMatches(binding, key, modifiers)).map(binding => binding.function);
}
/** Apply only when SkillMode changes; keep user key, Alt/Shift, disabled and unconstrained rows. */
export function applyCrystalSkillMode(bindings: readonly CrystalKeyBinding[], skillMode: boolean): CrystalKeyBinding[] {
  return bindings.map<CrystalKeyBinding>(binding => /^Bar[12]Skill[1-8]$/.test(binding.function) && binding.key !== "None"
    && (binding.ctrl === 1 || binding.tilde === 1)
    ? { ...binding, ctrl: skillMode ? 0 : 1, tilde: skillMode ? 1 : 0 } : { ...binding });
}
export function captureCrystalKeyBinding(bindings: readonly CrystalKeyBinding[], functionId: string,
  key: string, modifiers: CrystalKeyModifiers, enforce = true): CrystalKeyBinding[] | null {
  if (!bindings.some(binding => binding.function === functionId)
    || !/^[A-Za-z0-9]{1,32}$/.test(key)
    || ["None", "ControlKey", "Menu", "ShiftKey", "Oem8"].includes(key)) return null;
  const cleared = key === "Delete";
  const rule = (pressed: boolean): CrystalModifierRule => cleared ? 2 : pressed ? 1 : enforce ? 0 : 2;
  return bindings.map<CrystalKeyBinding>(binding => binding.function === functionId ? { ...binding, key: cleared ? "None" : key,
    alt: rule(modifiers.alt), ctrl: rule(modifiers.ctrl), shift: rule(modifiers.shift), tilde: rule(modifiers.tilde) } : { ...binding });
}
export function clearCrystalKeyBinding(bindings: readonly CrystalKeyBinding[], functionId: string): CrystalKeyBinding[] | null {
  return captureCrystalKeyBinding(bindings, functionId, "Delete", { alt: false, ctrl: false, shift: false, tilde: false });
}
export type CrystalBrowserKeyEvent = { key: string; code: string; altKey: boolean; ctrlKey: boolean; shiftKey: boolean };
/** Native observed_name first uses logical ASCII letters, then physical WinForms key names. */
export function crystalKeyNameFromBrowserEvent(event: Pick<CrystalBrowserKeyEvent, "key" | "code">): string | null {
  if (/^[A-Za-z]$/.test(event.key)) return event.key.toUpperCase();
  if (/^Key[A-Z]$/.test(event.code)) return event.code.slice(-1);
  if (/^Digit[0-9]$/.test(event.code)) return "D" + event.code.slice(-1);
  if (/^Numpad[0-9]$/.test(event.code)) return "NumPad" + event.code.slice(-1);
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(event.code)) return event.code;
  const names: Record<string, string> = { Enter: "Return", NumpadEnter: "Return", Escape: "Escape", Tab: "Tab", Space: "Space",
    Backspace: "Back", Delete: "Delete", Insert: "Insert", Home: "Home", End: "End", PageUp: "Prior", PageDown: "Next",
    ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right", PrintScreen: "PrintScreen", Pause: "Pause",
    CapsLock: "Capital", NumLock: "NumLock", ScrollLock: "Scroll", ControlLeft: "ControlKey", ControlRight: "ControlKey",
    ShiftLeft: "ShiftKey", ShiftRight: "ShiftKey", AltLeft: "Menu", AltRight: "Menu", Backquote: "Oem8", Minus: "OemMinus",
    Equal: "Oemplus", BracketLeft: "OemOpenBrackets", BracketRight: "OemCloseBrackets", Backslash: "OemPipe",
    Semicolon: "OemSemicolon", Quote: "OemQuotes", Comma: "Oemcomma", Period: "OemPeriod", Slash: "OemQuestion",
    NumpadAdd: "Add", NumpadSubtract: "Subtract", NumpadMultiply: "Multiply", NumpadDivide: "Divide", NumpadDecimal: "Decimal" };
  return names[event.code] ?? null;
}

export type CrystalHelpWindowPosition = { left: number; top: number };
export type CrystalHelpWindowState = CrystalHelpWindowPosition & { page: number };
export const CRYSTAL_HELP_PAGE_COUNT = 45;
export const DEFAULT_CRYSTAL_HELP_STATE: Readonly<CrystalHelpWindowState> = Object.freeze({ page: 0, left: 244, top: 129 });
export function crystalHelpPage(page: number): number {
  return Number.isFinite(page) ? Math.max(0, Math.min(44, Math.trunc(page))) : 0;
}
export function stepCrystalHelpPage(page: number, direction: -1 | 1): number {
  return (crystalHelpPage(page) + direction + CRYSTAL_HELP_PAGE_COUNT) % CRYSTAL_HELP_PAGE_COUNT;
}
export function clampCrystalHelpPosition(position: CrystalHelpWindowPosition): CrystalHelpWindowPosition {
  return { left: Number.isFinite(position.left) ? Math.max(0, Math.min(487, position.left)) : 244,
    top: Number.isFinite(position.top) ? Math.max(0, Math.min(258, position.top)) : 129 };
}
export function parseCrystalHelpState(text: string): CrystalHelpWindowState | null {
  if (typeof text !== "string" || text.length > 1024) return null;
  try { const value: unknown = JSON.parse(text); if (!record(value) || Object.keys(value).length !== 4 || value.version !== 1
    || typeof value.page !== "number" || !Number.isInteger(value.page) || value.page < 0 || value.page > 44
    || typeof value.left !== "number" || !Number.isFinite(value.left) || value.left < 0 || value.left > 487
    || typeof value.top !== "number" || !Number.isFinite(value.top) || value.top < 0 || value.top > 258) return null;
    return { page: value.page, left: value.left, top: value.top }; } catch { return null; }
}
export function serializeCrystalHelpState(state: CrystalHelpWindowState): string {
  return JSON.stringify({ version: 1, page: crystalHelpPage(state.page), ...clampCrystalHelpPosition(state) });
}
