import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { MotionController } from "@/app/components/motion-controller";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getProductCopy } from "@/lib/product-copy";
import { getCopy, getLocalizedAlternates, isLocale, locales } from "@/lib/site-copy";

type UpdatePageProps = { params: Promise<{ locale: string; slug: string }> };

export function generateStaticParams() {
  return locales.flatMap((locale) => getProductCopy(locale).updates.map((entry) => ({ locale, slug: entry.slug })));
}

export async function generateMetadata({ params }: UpdatePageProps): Promise<Metadata> {
  const { locale, slug } = await params;
  if (!isLocale(locale)) return {};
  const entry = getProductCopy(locale).updates.find((item) => item.slug === slug);
  return entry ? { title: entry.title, description: entry.summary, alternates: getLocalizedAlternates(locale, `/updates/${slug}`) } : {};
}

export default async function UpdatePage({ params }: UpdatePageProps) {
  const { locale, slug } = await params;
  if (!isLocale(locale)) notFound();
  const productCopy = getProductCopy(locale);
  const entry = productCopy.updates.find((item) => item.slug === slug);
  if (!entry) notFound();
  const siteCopy = getCopy(locale);
  const playUrl = `/${locale}/play`;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;
  return (
    <main className="article-page">
      <MotionController /><SiteHeader locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath={`/updates/${slug}`} />
      <article className="article-shell">
        <header data-reveal="hero"><Link href={`/${locale}/updates`}>← {productCopy.chrome.updates}</Link><div className="eyebrow"><span />{entry.date} · {entry.tag}</div><h1>{entry.title}</h1><p>{entry.summary}</p></header>
        <div className="article-sections">{entry.sections.map((section, index) => <section key={section.title} data-reveal><span>{String(index + 1).padStart(2, "0")}</span><h2>{section.title}</h2><p>{section.copy}</p></section>)}</div>
        <footer><span>{productCopy.common.updated}</span><Link href={playUrl}>{siteCopy.hero.primary} ↗</Link></footer>
      </article>
      <SiteFooter locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath={`/updates/${slug}`} />
    </main>
  );
}
