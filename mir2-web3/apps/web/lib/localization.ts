import localizationBundle from "./generated/localization_bundle.json";

export type Mir2Language = "en" | "zh-CN" | "es";

type LocalizationTexts = Record<string, string>;

type LocalizationLanguageEntry = {
  nativeName: string;
  locale: string;
  texts: LocalizationTexts;
};

type LocalizationBundle = {
  defaultLanguage: Mir2Language;
  languages: Record<Mir2Language, LocalizationLanguageEntry>;
};

const bundle = localizationBundle as LocalizationBundle;
const defaultLanguage = bundle.defaultLanguage;
const runtimeMessageKeyByValue: Record<string, string> = {
  "Runtime not booted": "runtime.notBooted",
  "Bevy runtime entry reached": "runtime.message.runtime-entered",
  "Handing off to Bevy app loop": "runtime.message.running",
  "Camera ready": "runtime.message.scene-ready",
};

export const SUPPORTED_LANGUAGES = Object.keys(bundle.languages) as Mir2Language[];

export function normalizeLanguage(value: string | null | undefined): Mir2Language {
  if (!value) return defaultLanguage;
  const normalized = value.toLowerCase();
  if (normalized === "zh" || normalized === "zh-cn" || normalized === "zh-hans") {
    return "zh-CN";
  }
  if (normalized === "es" || normalized === "es-es") {
    return "es";
  }
  return SUPPORTED_LANGUAGES.includes(value as Mir2Language) ? (value as Mir2Language) : "en";
}

export function languageLocale(language: Mir2Language) {
  return bundle.languages[language]?.locale ?? bundle.languages[defaultLanguage].locale;
}

export function languageNativeName(language: Mir2Language) {
  return bundle.languages[language]?.nativeName ?? language;
}

export function text(
  language: Mir2Language,
  key: string,
  args: Array<string | number> = [],
  fallback?: string,
): string {
  const languageTexts = bundle.languages[language]?.texts ?? bundle.languages[defaultLanguage].texts;
  const defaultTexts = bundle.languages[defaultLanguage].texts;
  const template = languageTexts[key] ?? defaultTexts[key] ?? fallback ?? key;
  return formatTemplate(String(template), args);
}

export function buildTranslator(language: Mir2Language) {
  return (key: string, args: Array<string | number> = [], fallback?: string) =>
    text(language, key, args, fallback);
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
  return args.reduce<string>(
    (value, entry, index) => value.split(`{${index}}`).join(String(entry)),
    template,
  );
}
