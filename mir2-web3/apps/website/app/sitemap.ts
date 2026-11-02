import type { MetadataRoute } from "next";
import { getProductCopy, legalDocuments, productSections } from "@/lib/product-copy";
import { locales, publicSiteUrl } from "@/lib/site-copy";
import { documentation } from "@/lib/documentation";
import { documentationUpdated } from "@/lib/documentation-types";

export default function sitemap(): MetadataRoute.Sitemap {
  const siteUrl = publicSiteUrl;
  const localizedPaths = [
    "",
    "/play",
    "/download",
    "/watch",
    "/membership",
    "/account",
    ...productSections.map((section) => `/${section}`),
    ...legalDocuments.map((document) => `/legal/${document}`),
    ...getProductCopy("en").updates.map((entry) => `/updates/${entry.slug}`),
  ];
  const lastModified = new Date("2026-08-28T00:00:00.000Z");
  const localized = locales.flatMap((locale) => localizedPaths.map((path) => ({
    url: `${siteUrl}/${locale}${path}`,
    lastModified,
    changeFrequency: path === "/updates" || path === "/status" || path === "/watch" ? "daily" as const : "weekly" as const,
    priority: path === "" ? locale === "zh-CN" ? 1 : 0.9 : path === "/play" ? 0.9 : 0.72,
    alternates: {
      languages: {
        ...Object.fromEntries(locales.map((item) => [item, `${siteUrl}/${item}${path}`])),
        "x-default": `${siteUrl}/en${path}`,
      },
    },
  })));
  const archivePaths = [
    "/archive/world/world-01", "/archive/map/bichon-province", "/archive/map/mengzhong-province", "/archive/map/red-moon-gorge", "/archive/map/sabuk",
    "/archive/monster/red-moon-demon", "/archive/monster/zuma-leader", "/archive/event/sabuk-0826", "/archive/character/cangyue-lone-wolf",
    "/archive/character/qingfeng-taoist", "/archive/guild/dragon-soul-hall", "/archive/equipment/dragon-slayer", "/archive/equipment/judgement-mace",
    "/archive/item/paralysis-ring", "/archive/item/black-iron-ore", "/archive/item/blessing-oil", "/archive/item/zuma-portrait", "/archive/market/world-01",
  ];
  const explorerPaths = locales.flatMap((locale) => ["/explore", ...archivePaths].map((path) => ({
    url: `${siteUrl}/${locale}${path}`,
    lastModified,
    changeFrequency: "daily" as const,
    priority: path === "/explore" ? 0.9 : 0.65,
    alternates: { languages: { ...Object.fromEntries(locales.map((item) => [item, `${siteUrl}/${item}${path}`])), "x-default": `${siteUrl}/en${path}` } },
  })));

  const documentationPaths = (["guide", "developers"] as const).flatMap((area) => [
    `/${area}`,
    ...documentation[area].articles.map(({ slug }) => `/${area}/${slug}`),
  ]).map((path) => ({
    url: `${siteUrl}/zh-CN${path}`,
    lastModified: new Date(`${documentationUpdated}T00:00:00.000Z`),
    changeFrequency: "weekly" as const,
    priority: 0.75,
    alternates: { languages: { "zh-CN": `${siteUrl}/zh-CN${path}` } },
  }));
  return [...localized, ...explorerPaths, ...documentationPaths];
}
