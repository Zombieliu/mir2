import type { MetadataRoute } from "next";
import { locales } from "@/lib/site-copy";

export default function sitemap(): MetadataRoute.Sitemap {
  const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? "https://mir2.obelisk.build";

  return locales.flatMap((locale) => [
    {
      url: `${siteUrl}/${locale}`,
      lastModified: new Date("2026-08-26T00:00:00.000Z"),
      changeFrequency: "weekly" as const,
      priority: locale === "zh-CN" ? 1 : 0.9,
      alternates: {
        languages: Object.fromEntries(locales.map((item) => [item, `${siteUrl}/${item}`])),
      },
    },
    {
      url: `${siteUrl}/${locale}/watch`,
      lastModified: new Date("2026-08-26T00:00:00.000Z"),
      changeFrequency: "daily" as const,
      priority: locale === "zh-CN" ? 0.9 : 0.8,
      alternates: {
        languages: Object.fromEntries(locales.map((item) => [item, `${siteUrl}/${item}/watch`])),
      },
    },
    {
      url: `${siteUrl}/${locale}/membership`,
      lastModified: new Date("2026-08-26T00:00:00.000Z"),
      changeFrequency: "weekly" as const,
      priority: locale === "zh-CN" ? 0.85 : 0.75,
      alternates: {
        languages: Object.fromEntries(locales.map((item) => [item, `${siteUrl}/${item}/membership`])),
      },
    },
  ]);
}
