import type { Locale } from "@/lib/site-copy";

const promotionLabels = {
  "zh-CN": "团长推广",
  "zh-TW": "團長推廣",
  en: "Referral program",
  ja: "紹介プログラム",
  ko: "초대 프로그램",
  es: "Programa de referidos",
  "pt-BR": "Programa de indicações",
  de: "Empfehlungsprogramm",
  fr: "Parrainage",
  ru: "Реферальная программа",
  pl: "Program poleceń",
  tr: "Davet programı",
  th: "โปรแกรมแนะนำเพื่อน",
  vi: "Chương trình giới thiệu",
  id: "Program referral",
  ar: "برنامج الإحالة",
} satisfies Record<Locale, string>;

export function getPromotionLink(locale: Locale) {
  return {
    label: promotionLabels[locale],
    href: `https://mir2.obelisk.build/promotions?lang=${encodeURIComponent(locale)}`,
  };
}
