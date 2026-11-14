/** Presentation locales may be richer than the current, versioned WASM/wire
 * vocabulary. Keep strict host/status guards and adapt before publishing. */
export function questRuntimeLanguage(language: string | undefined): "en" | "zh-CN" | "es" | "pt-BR" {
  return language === "zh-CN" || language === "es" || language === "pt-BR" ? language : "en";
}

export function commerceRuntimeLanguage(language: string): "en" | "zh-CN" {
  return language === "zh-CN" ? "zh-CN" : "en";
}
