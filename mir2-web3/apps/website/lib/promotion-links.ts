import { getPromotionCopy } from "@/lib/promotion-copy";
import type { Locale } from "@/lib/site-copy";

export function getPromotionLink(locale: Locale) {
  return {
    label: getPromotionCopy(locale).label,
    href: `/${locale}/promotions`,
  };
}
