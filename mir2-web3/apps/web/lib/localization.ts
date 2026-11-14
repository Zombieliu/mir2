import localizationBundle from "./generated/localization_bundle.json";
import { R16_NATIVE_LANGUAGES, hasR16GameplayLocalization, nativeLanguageInfo, nativeSystemText, nativeText, type R16NativeLanguage } from "./r16-native-localization";
import { browserText } from "./r16-browser-localization";

// Spanish remains a legacy internal type for older shared hosts and saved QA
// fixtures. New player choices follow the canonical Windows locale catalog.
export type Mir2Language = R16NativeLanguage | "es";

type LocalizationTexts = Record<string, string>;

type LocalizationLanguageEntry = {
  nativeName: string;
  locale: string;
  texts: LocalizationTexts;
};

type LocalizationBundle = {
  defaultLanguage: Mir2Language;
  languages: Record<string, LocalizationLanguageEntry>;
};

const bundle = localizationBundle as LocalizationBundle;
const defaultLanguage = bundle.defaultLanguage;
// Only finite authored legacy keys are cached, never arbitrary player input.
const projectedLegacyTexts = new Map<string, string>();
const runtimeMessageKeyByValue: Record<string, string> = {
  "Runtime not booted": "runtime.notBooted",
  "Bevy runtime entry reached": "runtime.message.runtime-entered",
  "Handing off to Bevy app loop": "runtime.message.running",
  "Camera ready": "runtime.message.scene-ready",
};

export const SUPPORTED_LANGUAGES = R16_NATIVE_LANGUAGES;

function supportedLanguage(value: string | null | undefined): R16NativeLanguage | null {
  const normalized = value?.trim().replaceAll("_", "-").toLowerCase();
  if (!normalized) return null;
  if (/^zh-(?:tw|hk|mo|hant)(?:-|$)/.test(normalized)) return "zh-TW";
  if (normalized === "zh" || /^zh-(?:cn|sg|hans)(?:-|$)/.test(normalized)) return "zh-CN";
  const base = normalized.split("-")[0];
  if (base === "pt") return "pt-BR";
  return SUPPORTED_LANGUAGES.find(code => code.toLowerCase() === normalized || code === base) ?? null;
}

export function normalizeLanguage(value: string | null | undefined): Mir2Language {
  return supportedLanguage(value) ?? "en";
}

/** Saved preference wins; first use follows OS/browser languages, like Windows. */
export function preferredLanguage(saved: string | null, browserLanguages: readonly string[] = []): R16NativeLanguage {
  return supportedLanguage(saved) ?? browserLanguages.map(supportedLanguage).find(code => code !== null) ?? "en";
}

export function languageLocale(language: Mir2Language) {
  return nativeLanguageInfo(language)?.locale ?? bundle.languages[language]?.locale ?? "en-US";
}

export function languageNativeName(language: Mir2Language) {
  return nativeLanguageInfo(language)?.nativeName ?? bundle.languages[language]?.nativeName ?? language;
}

export function languageFontFamily(language: Mir2Language) {
  return `"${nativeLanguageInfo(language)?.fontFamily ?? "Noto Sans"}", sans-serif`;
}

export function text(
  language: Mir2Language,
  key: string,
  args: Array<string | number> = [],
  fallback?: string,
): string {
  const browser = browserText(language, key);
  if (browser !== undefined) return formatTemplate(browser, args);
  const native = nativeText(language, key);
  if (native !== key) return formatTemplate(native, args);
  const languageTexts = bundle.languages[language]?.texts;
  const defaultTexts = bundle.languages[defaultLanguage].texts;
  // Only known authored system templates enter alias translation. An unknown
  // key's fallback may be a player's name or other opaque server text.
  const authored = defaultTexts[key];
  const cacheKey = `${hasR16GameplayLocalization()}\u0000${language}\u0000${key}`;
  let projected = authored === undefined ? undefined : projectedLegacyTexts.get(cacheKey);
  if (authored !== undefined && projected === undefined) {
    projected = nativeSystemText(language, authored);
    projectedLegacyTexts.set(cacheKey, projected);
  }
  const template = projected !== undefined && projected !== authored ? projected
    : languageTexts?.[key] ?? authored ?? fallback ?? key;
  return formatTemplate(String(template), args);
}

export function buildTranslator(language: Mir2Language) {
  const translate = (key: string, args: Array<string | number> = [], fallback?: string) => text(language, key, args, fallback);
  // Callers opt in only at a known system-content display boundary. This is
  // deliberately separate from arbitrary fallback, chat and character names.
  translate.systemText = (value: string, domain: "system" | "npc" = "system") => nativeSystemText(language, value, domain);
  return translate;
}

export function formatRuntimePhase(language: Mir2Language, runtimePhase: string) {
  return text(language, `runtime.phase.${runtimePhase}`, [], runtimePhase);
}

export function formatRuntimeMessage(language: Mir2Language, message: string) {
  const runtimeKey = runtimeMessageKeyByValue[message];
  if (!runtimeKey) {
    return message;
  }
  return text(language, runtimeKey, [], message);
}

function formatTemplate(template: string, args: Array<string | number>) {
  return template.replace(/\{(\d+)\}/g, (placeholder, index: string) => Number(index) < args.length ? String(args[Number(index)]) : placeholder);
}
