import type { Metadata } from "next";
import { guideArticles } from "@/lib/guide-content";
import { developerArticles } from "@/lib/developer-content";
import type { DocumentationArea } from "@/lib/documentation-types";
import { getPromotionLink } from "@/lib/promotion-links";
import { documentationGroups } from "@/lib/documentation-navigation";
import type { Locale } from "@/lib/site-copy";

export const documentation = {
  guide: { title: "玩家指南", eyebrow: "THE PLAYER’S FIELD GUIDE", description: "从第一次登录，到一起冒险。", intro: "第一次来，先按四步完成下载、登录、创角与入场。开始游玩后，再按需要查操作、玩法和规则。", articles: guideArticles },
  developers: { title: "开发者文档", eyebrow: "BUILD WITH A CLEAR BOUNDARY", description: "了解边界，从一次真实读取开始。", intro: "认识 Numeron 与 Dubhe 的分工，运行一个经过验证的只读示例，再确认更深入的接入权限。", articles: developerArticles },
} as const;

export function orderedDocumentationArticles(area: DocumentationArea) {
  return documentationGroups[area].flatMap((group) => group.slugs.map((slug) => {
    const article = documentation[area].articles.find((entry) => entry.slug === slug);
    if (!article) throw new Error(`Missing documentation article: ${area}/${slug}`);
    return article;
  }));
}

export function documentationHref(locale: Locale, href: string) {
  if (href === "@promotions") return getPromotionLink(locale).href;
  if (href.startsWith("/") || href.startsWith("https://") || href.startsWith("http://")) return href;
  return `/${locale}/${href}`;
}

export function documentationMetadata(locale: Locale, area: DocumentationArea, slug?: string): Metadata {
  const collection = documentation[area];
  const article = slug ? collection.articles.find((item) => item.slug === slug) : undefined;
  const path = `/${area}${slug ? `/${slug}` : ""}`;
  return {
    title: article ? `${article.title} · ${collection.title}` : collection.title,
    description: article?.description ?? collection.description,
    // Only the Chinese edition has reviewed text. Do not advertise translations.
    alternates: { canonical: `/zh-CN${path}`, languages: { "zh-CN": `/zh-CN${path}` } },
    robots: { index: locale === "zh-CN", follow: true },
  };
}
