import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getPromotionCopy } from "@/lib/promotion-copy";
import { getCopy, getLocalizedAlternates, isLocale, locales } from "@/lib/site-copy";

type PromotionPageProps = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: PromotionPageProps): Promise<Metadata> {
  const { locale } = await params;
  if (!isLocale(locale)) return {};
  const copy = getPromotionCopy(locale);
  const alternates = getLocalizedAlternates(locale, "/promotions");
  return {
    title: copy.label,
    description: copy.summary,
    alternates,
    openGraph: {
      type: "website",
      title: `${copy.label} | NUMERON`,
      description: copy.summary,
      url: alternates.canonical,
      locale,
      images: [{ url: "/og.png", width: 1200, height: 630, alt: "NUMERON" }],
    },
    twitter: {
      card: "summary_large_image",
      title: `${copy.label} | NUMERON`,
      description: copy.summary,
      images: ["/og.png"],
    },
  };
}

export default async function PromotionPage({ params }: PromotionPageProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  const copy = getPromotionCopy(locale);
  const siteCopy = getCopy(locale);
  const playUrl = `/${locale}/play`;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;
  const chrome = { locale, copy: siteCopy, gameUrl: playUrl, explorerUrl, currentPage: "promotions" as const, currentPath: "/promotions" };

  return (
    <main className="legal-page promotion-page">
      <SiteHeader {...chrome} />
      <article className="legal-shell">
        <header>
          <div className="eyebrow"><span />{copy.status}</div>
          <h1>{copy.label}</h1>
          <p>{copy.summary}</p>
        </header>
        <aside>{copy.notice}</aside>
        <div className="legal-sections">
          {copy.sections.map((section, index) => (
            <section key={section.title}>
              <span aria-hidden="true">{String(index + 1).padStart(2, "0")}</span>
              <h2>{section.title}</h2>
              <p>{section.copy}</p>
            </section>
          ))}
        </div>
      </article>
      <SiteFooter {...chrome} />
    </main>
  );
}
