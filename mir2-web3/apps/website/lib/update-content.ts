import { getProductCopy } from "@/lib/product-copy";
import { locales, publicSiteUrl, type Locale } from "@/lib/site-copy";

export const translationNotices: Record<Locale, string> = {
  "zh-CN": "当前显示中文原文。", "zh-TW": "譯文尚待審校，以下顯示簡體中文原文。",
  en: "Translation is pending review. The source text is shown below.",
  ja: "翻訳は未審校のため、以下に原文を表示します。", ko: "번역 검토 전이므로 아래에 원문을 표시합니다.",
  es: "La traducción está pendiente de revisión. Se muestra el texto original.",
  "pt-BR": "A tradução aguarda revisão. O texto original é exibido abaixo.",
  de: "Die Übersetzung wird noch geprüft. Unten steht der Originaltext.",
  fr: "La traduction attend une révision. Le texte original figure ci-dessous.",
  ru: "Перевод ещё не проверен. Ниже показан исходный текст.",
  pl: "Tłumaczenie wymaga weryfikacji. Poniżej wyświetlono tekst źródłowy.",
  tr: "Çeviri inceleme bekliyor. Aşağıda kaynak metin gösterilmektedir.",
  th: "คำแปลยังรอการตรวจสอบ ด้านล่างแสดงข้อความต้นฉบับ",
  vi: "Bản dịch đang chờ kiểm tra. Nội dung gốc được hiển thị bên dưới.",
  id: "Terjemahan menunggu peninjauan. Teks sumber ditampilkan di bawah.",
  ar: "الترجمة بانتظار المراجعة. يُعرض النص الأصلي أدناه.",
};

export function getUpdateSlugs() {
  return [...new Set(locales.flatMap(locale => getProductCopy(locale).updates.map(entry => entry.slug)))];
}

export function getUpdateEntry(locale: Locale, slug: string) {
  const translated = getProductCopy(locale).updates.find(entry => entry.slug === slug);
  if (translated) return { entry: translated, sourceLocale: locale, fallback: false };
  for (const sourceLocale of ["zh-CN", ...locales.filter(item => item !== "zh-CN")] as Locale[]) {
    const entry = getProductCopy(sourceLocale).updates.find(item => item.slug === slug);
    if (entry) return { entry, sourceLocale, fallback: true };
  }
  return undefined;
}

export function getUpdateAlternates(locale: Locale, slug: string) {
  const resolved = getUpdateEntry(locale, slug);
  if (!resolved) return undefined;
  const path = `/updates/${slug}`;
  const available = locales.filter(item => getProductCopy(item).updates.some(entry => entry.slug === slug));
  return {
    canonical: `${publicSiteUrl}/${resolved.sourceLocale}${path}`,
    languages: { ...Object.fromEntries(available.map(item => [item, `${publicSiteUrl}/${item}${path}`])), "x-default": `${publicSiteUrl}/${available.includes("en") ? "en" : available[0]}${path}` },
  };
}
