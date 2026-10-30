import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { MotionController } from "@/app/components/motion-controller";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getProductCopy, isLegalDocument, legalDocuments } from "@/lib/product-copy";
import { getCopy, getLocalizedAlternates, isLocale, locales } from "@/lib/site-copy";

type LegalPageProps = { params: Promise<{ locale: string; document: string }> };

export function generateStaticParams() {
  return locales.flatMap((locale) => legalDocuments.map((document) => ({ locale, document })));
}

export async function generateMetadata({ params }: LegalPageProps): Promise<Metadata> {
  const { locale, document } = await params;
  if (!isLocale(locale) || !isLegalDocument(document)) return {};
  const copy = getProductCopy(locale).legal[document];
  return { title: copy.title, description: copy.summary, alternates: getLocalizedAlternates(locale, `/legal/${document}`) };
}

export default async function LegalPage({ params }: LegalPageProps) {
  const { locale, document } = await params;
  if (!isLocale(locale) || !isLegalDocument(document)) notFound();
  const productCopy = getProductCopy(locale);
  const copy = productCopy.legal[document];
  const siteCopy = getCopy(locale);
  const playUrl = `/${locale}/play`;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;
  return (
    <main className="legal-page">
      <MotionController /><SiteHeader locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath={`/legal/${document}`} />
      <article className="legal-shell">
        <header data-reveal="hero"><div className="eyebrow"><span />{productCopy.ui.publicDraft}</div><h1>{copy.title}</h1><p>{copy.summary}</p><small>{productCopy.common.updated} · {productCopy.common.draft}</small></header>
        <nav aria-label={productCopy.chrome.legal}>{legalDocuments.map((item) => <Link key={item} href={`/${locale}/legal/${item}`} aria-current={item === document ? "page" : undefined}>{productCopy.chrome[item === "ai-data" ? "aiData" : item]}</Link>)}</nav>
        <div className="legal-sections">{copy.sections.map((section, index) => <section key={section.title} data-reveal><span>{String(index + 1).padStart(2, "0")}</span><h2>{section.title}</h2><p>{section.copy}</p></section>)}</div>
        <aside>{productCopy.ui.legalDisclaimer}</aside>
      </article>
      <SiteFooter locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath={`/legal/${document}`} />
    </main>
  );
}
