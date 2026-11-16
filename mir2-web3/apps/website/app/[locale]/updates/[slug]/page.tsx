import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { MotionController } from "@/app/components/motion-controller";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getProductCopy } from "@/lib/product-copy";
import { getCopy, isLocale, locales } from "@/lib/site-copy";
import { getUpdateAlternates, getUpdateEntry, getUpdateSlugs, translationNotices } from "@/lib/update-content";

type UpdatePageProps = { params: Promise<{ locale: string; slug: string }> };

export function generateStaticParams() {
  return locales.flatMap(locale => getUpdateSlugs().map(slug => ({ locale, slug })));
}

export async function generateMetadata({ params }: UpdatePageProps): Promise<Metadata> {
  const { locale, slug } = await params;
  if (!isLocale(locale)) return {};
  const resolved = getUpdateEntry(locale, slug);
  if (!resolved) return {};
  const { entry, fallback } = resolved;
  const description = fallback ? `${translationNotices[locale]} ${entry.summary}` : entry.summary;
  return { title: entry.title, description, alternates: getUpdateAlternates(locale, slug), robots: fallback ? { index: false, follow: true } : undefined, openGraph: { title: `${entry.title} | NUMERON`, description }, twitter: { title: `${entry.title} | NUMERON`, description } };
}

export default async function UpdatePage({ params }: UpdatePageProps) {
  const { locale, slug } = await params;
  if (!isLocale(locale)) notFound();
  const productCopy = getProductCopy(locale);
  const resolved = getUpdateEntry(locale, slug);
  if (!resolved) notFound();
  const { entry, sourceLocale, fallback } = resolved;
  const siteCopy = getCopy(locale);
  const playUrl = `/${locale}/play`;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;
  return (
    <main className="article-page">
      <MotionController /><SiteHeader locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath={`/updates/${slug}`} />
      {fallback ? <aside className="update-language-note"><p>{translationNotices[locale]}</p><Link href={`/${sourceLocale}/updates/${slug}`} lang={sourceLocale}>{getCopy(sourceLocale).languageName} →</Link></aside> : null}
      <article className="article-shell" lang={sourceLocale} dir={sourceLocale === "ar" ? "rtl" : "ltr"}>
        <header data-reveal="hero"><Link href={`/${locale}/updates`}>← {productCopy.chrome.updates}</Link><div className="eyebrow"><span />{entry.date} · {entry.tag}</div><h1>{entry.title}</h1><p>{entry.summary}</p></header>
        <div className="article-sections">{entry.sections.map((section, index) => <section key={section.title} data-reveal><span>{String(index + 1).padStart(2, "0")}</span><h2>{section.title}</h2><p>{section.copy}</p></section>)}</div>
        <footer><span>{productCopy.common.updated}</span><Link href={playUrl}>{siteCopy.hero.primary} ↗</Link></footer>
      </article>
      <SiteFooter locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath={`/updates/${slug}`} />
    </main>
  );
}
