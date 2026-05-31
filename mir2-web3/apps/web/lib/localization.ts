import localizationBundle from "./generated/localization_bundle.json";

export type Mir2Language = "en" | "zh-CN" | "zh-TW" | "es";

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
  if (
    normalized === "zh-tw" ||
    normalized === "zh-hant" ||
    normalized === "zh-hk" ||
    normalized === "zh-mo"
  ) {
    return "zh-TW";
  }
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
  return formatTemplate(String(template), args, languageLocale(language));
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

// Resolves a .NET-style composite format string (the format used by the
// Crystal client/server localization, e.g. `Bid {0:#,##0} Gold for {1}?`).
// Supports positional `{n}` holes, `:format` specifiers (`#,##0`, `###,###`,
// `#.00`, `P0`...), and escaped `{{` / `}}` braces. Numeric specifiers are
// formatted for the active locale, so Spanish renders `1.234.567` / `50 %`
// while English/Chinese render `1,234,567` / `50%`.
function formatTemplate(
  template: string,
  args: Array<string | number>,
  locale: string,
) {
  let result = "";
  for (let index = 0; index < template.length; index += 1) {
    const char = template[index];
    if (char === "{") {
      if (template[index + 1] === "{") {
        result += "{";
        index += 1;
        continue;
      }
      const close = template.indexOf("}", index + 1);
      if (close !== -1) {
        const token = template.slice(index + 1, close);
        const colon = token.indexOf(":");
        const holeText = colon === -1 ? token : token.slice(0, colon);
        if (/^\d+$/.test(holeText) && Number(holeText) < args.length) {
          const spec = colon === -1 ? "" : token.slice(colon + 1);
          result += formatArgument(args[Number(holeText)], spec, locale);
          index = close;
          continue;
        }
      }
      result += char;
      continue;
    }
    if (char === "}") {
      if (template[index + 1] === "}") {
        result += "}";
        index += 1;
      } else {
        result += char;
      }
      continue;
    }
    result += char;
  }
  return result;
}

const numberFormatCache = new Map<string, Intl.NumberFormat>();

function numberFormat(locale: string, options: Intl.NumberFormatOptions) {
  const cacheKey = `${locale}|${JSON.stringify(options)}`;
  let formatter = numberFormatCache.get(cacheKey);
  if (!formatter) {
    formatter = new Intl.NumberFormat(locale, options);
    numberFormatCache.set(cacheKey, formatter);
  }
  return formatter;
}

function formatArgument(
  value: string | number,
  spec: string,
  locale: string,
): string {
  if (!spec) {
    return String(value);
  }
  const numeric = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(numeric)) {
    return String(value);
  }
  const percent = /^P(\d*)$/i.exec(spec);
  if (percent) {
    const digits = percent[1] === "" ? 2 : Number(percent[1]);
    return numberFormat(locale, {
      style: "percent",
      minimumFractionDigits: digits,
      maximumFractionDigits: digits,
    }).format(numeric);
  }
  // Custom numeric pattern: `,` enables grouping, the run after `.` sets the
  // fraction digits (`0` is required, `#` is optional).
  if (/^[#0,.]+$/.test(spec)) {
    const fraction = spec.includes(".") ? spec.slice(spec.indexOf(".") + 1) : "";
    const minimumFractionDigits = (fraction.match(/0/g) || []).length;
    return numberFormat(locale, {
      useGrouping: spec.includes(","),
      minimumFractionDigits,
      maximumFractionDigits: Math.max(fraction.length, minimumFractionDigits),
    }).format(numeric);
  }
  return String(numeric);
}
