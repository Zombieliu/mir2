import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getLegalPresentation, legalSourceLocale, legalSourceUpdatedAt } from "@/lib/legal-content";
import { getProductCopy, isLegalDocument, legalDocuments } from "@/lib/product-copy";
import { getCopy, getLocaleMeta, isLocale, locales, publicSiteUrl } from "@/lib/site-copy";
import "@/app/legal-content.css";

type LegalPageProps = { params: Promise<{ locale: string; document: string }> };

export function generateStaticParams() {
  return locales.flatMap((locale) => legalDocuments.map((document) => ({ locale, document })));
}

export async function generateMetadata({ params }: LegalPageProps): Promise<Metadata> {
  const { locale, document } = await params;
  if (!isLocale(locale) || !isLegalDocument(document)) return {};
  const copy = getProductCopy(locale).legal[document];
  const presentation = getLegalPresentation(locale);
  const title = `${copy.title} · 中文草案`;
  const description = `${presentation.notice} ${copy.summary}`;
  const sourceUrl = `${publicSiteUrl}/${legalSourceLocale}/legal/${document}`;
  return {
    title,
    description,
    alternates: { canonical: sourceUrl, languages: { [legalSourceLocale]: sourceUrl, "x-default": sourceUrl } },
    robots: locale === legalSourceLocale ? undefined : { index: false, follow: true },
    openGraph: { title: `${title} | NUMERON`, description, url: sourceUrl, locale: legalSourceLocale, images: ["/og.png"] },
    twitter: { card: "summary_large_image", title: `${title} | NUMERON`, description, images: ["/og.png"] },
  };
}

export default async function LegalPage({ params }: LegalPageProps) {
  const { locale, document } = await params;
  if (!isLocale(locale) || !isLegalDocument(document)) notFound();
  const productCopy = getProductCopy(locale);
  const copy = productCopy.legal[document];
  const presentation = getLegalPresentation(locale);
  const siteCopy = getCopy(locale);
  const playUrl = `/${locale}/play`;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;
  return (
    <main className="legal-page">
      <SiteHeader locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath={`/legal/${document}`} />
      <article className="legal-shell" lang={legalSourceLocale} dir="ltr" data-legal-document={document} data-legal-source-language={legalSourceLocale}>
        <aside className="legal-source-notice" lang={locale} dir={getLocaleMeta(locale).dir} data-legal-fallback={locale !== legalSourceLocale}>
          <p>{presentation.notice}</p>
          <div className="legal-source-actions">
            {locale !== legalSourceLocale ? <Link href={`/${legalSourceLocale}/legal/${document}`} hrefLang={legalSourceLocale} data-track="legal_source_draft">{presentation.sourceLink}</Link> : null}
            <Link href={`/${locale}/recharge`}>{presentation.rechargeLink}</Link>
          </div>
        </aside>
        <header><div className="eyebrow"><span />中文原文 · 公开草案 · 尚未完成法律审校</div><h1>{copy.title}</h1><p>{copy.summary}</p><small><time dateTime={legalSourceUpdatedAt}>2026.11.05</time> · 草案</small></header>
        <nav lang={locale} dir={getLocaleMeta(locale).dir} aria-label={productCopy.chrome.legal}>{legalDocuments.map((item) => <Link key={item} href={`/${locale}/legal/${item}`} aria-current={item === document ? "page" : undefined}>{productCopy.chrome[item === "ai-data" ? "aiData" : item]}</Link>)}</nav>
        <div className="legal-sections">{copy.sections.map((section, index) => <section key={section.title}><span>{String(index + 1).padStart(2, "0")}</span><h2>{section.title}</h2><p>{section.copy}</p></section>)}</div>
        <aside>本页为待核对的中文原文草案，不表示相关商业、退款或数据处理规则已经批准，也不表示素材和代码授权已经核实。</aside>
      </article>
      <SiteFooter locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath={`/legal/${document}`} />
    </main>
  );
}
