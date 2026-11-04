import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getProductCopy } from "@/lib/product-copy";
import { getDocumentationLabels } from "@/lib/documentation-labels";
import { getRechargeCopy, publicRechargeOffer } from "@/lib/recharge-copy";
import { getCopy, getLocalizedAlternates, isLocale } from "@/lib/site-copy";

type RechargePageProps = { params: Promise<{ locale: string }> };

export async function generateMetadata({ params }: RechargePageProps): Promise<Metadata> {
  const { locale } = await params;
  if (!isLocale(locale)) return {};
  const copy = getRechargeCopy(locale);
  return {
    title: copy.label, description: copy.intro,
    alternates: getLocalizedAlternates(locale, "/recharge"),
    openGraph: { title: `${copy.label} | NUMERON`, description: copy.intro },
    twitter: { title: `${copy.label} | NUMERON`, description: copy.intro },
  };
}

export default async function RechargePage({ params }: RechargePageProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  const copy = getRechargeCopy(locale);
  const siteCopy = getCopy(locale);
  const productCopy = getProductCopy(locale);
  const playUrl = `/${locale}/play`;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;
  const price = new Intl.NumberFormat(locale, { style: "currency", currency: publicRechargeOffer.currency, currencyDisplay: "code" }).format(publicRechargeOffer.amountMinor / 100);

  return (
    <main className="membership-page recharge-page" data-purchase-state="unavailable">
      <SiteHeader locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath="/recharge" />
      <section className="membership-hero" aria-labelledby="recharge-title">
        <div className="membership-hero-grid" aria-hidden="true" />
        <div className="token-orbit" aria-hidden="true"><i /><i /><i /><b><small>NUMERON</small>CREDITS</b></div>
        <div className="membership-hero-copy">
          <div className="eyebrow"><span />{copy.status}</div>
          <h1 id="recharge-title">{copy.label}</h1><p>{copy.intro}</p>
          <div className="membership-hero-actions"><a className="button button-primary" href="#prices"><span>{copy.pricingTitle}</span><i aria-hidden="true">↓</i></a></div>
        </div>
      </section>
      <section className="plans-section section" id="prices" aria-labelledby="recharge-prices-title">
        <div className="section-heading membership-heading"><h2 id="recharge-prices-title">{copy.pricingTitle}</h2><p>{copy.billing}</p><p>{copy.pricingNote}</p></div>
        <div className="usage-table-wrap">
          <table className="usage-table recharge-prices" aria-label={copy.pricingTitle}>
            <thead><tr>{copy.columns.map(label => <th scope="col" key={label}>{label}</th>)}</tr></thead>
            <tbody><tr><th scope="row">{copy.label}</th><td data-price-minor={publicRechargeOffer.amountMinor}>{price}</td><td data-credits={publicRechargeOffer.credits}>{publicRechargeOffer.credits}</td><td>{copy.status}</td></tr></tbody>
          </table>
        </div>
      </section>
      <section className="membership-details section" aria-label={copy.label}>
        <div className="membership-example-grid">{copy.sections.map((section, index) => <article key={section.title}><small>{String(index + 1).padStart(2, "0")}</small><h2>{section.title}</h2><p>{section.copy}</p></article>)}</div>
        <nav className="recharge-policy-links" aria-label={productCopy.chrome.legal}>
          <Link href={`/${locale}/guide/payments`}>{getDocumentationLabels(locale).guide}</Link>
          <Link href={`/${locale}/membership`}>{copy.aiLabel}</Link>
          <Link href={`/${locale}/legal/terms`}>{productCopy.chrome.terms}</Link>
          <Link href={`/${locale}/legal/privacy`}>{productCopy.chrome.privacy}</Link>
          <Link href={`/${locale}/legal/refunds`}>{productCopy.chrome.refunds}</Link>
          <Link href={`/${locale}/support`}>{productCopy.chrome.support}</Link>
        </nav>
      </section>
      <SiteFooter locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath="/recharge" />
    </main>
  );
}
