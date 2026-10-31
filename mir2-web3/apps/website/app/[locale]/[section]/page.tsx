import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { GameStatusPanel } from "@/app/components/game-status-panel";
import { ProductPageShell } from "@/app/components/product-page-shell";
import { getProductCopy, isProductSection, productSections } from "@/lib/product-copy";
import { getCopy, getLocalizedAlternates, isLocale, locales } from "@/lib/site-copy";

type SectionPageProps = { params: Promise<{ locale: string; section: string }> };

export function generateStaticParams() {
  return locales.flatMap((locale) => productSections.map((section) => ({ locale, section })));
}

export async function generateMetadata({ params }: SectionPageProps): Promise<Metadata> {
  const { locale, section } = await params;
  if (!isLocale(locale) || !isProductSection(section)) return {};
  const copy = getProductCopy(locale).pages[section];
  return { title: copy.title, description: copy.copy, alternates: getLocalizedAlternates(locale, `/${section}`) };
}

export default async function SectionPage({ params }: SectionPageProps) {
  const { locale, section } = await params;
  if (!isLocale(locale) || !isProductSection(section)) notFound();
  const productCopy = getProductCopy(locale);
  const page = productCopy.pages[section];
  const cards = section === "updates"
    ? productCopy.updates.map((entry) => ({ eyebrow: `${entry.date} · ${entry.tag}`, title: entry.title, copy: entry.summary, href: `updates/${entry.slug}`, label: productCopy.common.learnMore }))
    : page.cards;
  const supportEmail = process.env.NEXT_PUBLIC_SUPPORT_EMAIL ?? "support@obelisk.build";
  const statusLabels = { checking: `${productCopy.chrome.status}…`, online: getCopy(locale).hero.status, degraded: productCopy.common.unavailable, checked: productCopy.common.updated };

  return (
    <ProductPageShell locale={locale} siteCopy={getCopy(locale)} productCopy={productCopy} currentPath={`/${section}`} eyebrow={page.eyebrow} title={page.title} copy={page.copy} cards={cards}>
      {section === "status" ? <section className="status-live-section"><GameStatusPanel labels={statusLabels} /></section> : null}
      {section === "support" ? <section className="support-contact"><span>{productCopy.ui.official} · {productCopy.chrome.support}</span><h2>{supportEmail}</h2><p>{productCopy.ui.supportWarning}</p><a className="button button-primary" href={`mailto:${supportEmail}`} data-track="support_email"><span>{productCopy.chrome.support}</span><i>↗</i></a></section> : null}
      {section === "community" ? <section className="community-channels section"><div><small>DISCORD</small><strong>{process.env.NEXT_PUBLIC_DISCORD_URL ? productCopy.ui.official : productCopy.common.unavailable}</strong>{process.env.NEXT_PUBLIC_DISCORD_URL ? <a href={process.env.NEXT_PUBLIC_DISCORD_URL} target="_blank" rel="noreferrer">{productCopy.ui.join} ↗</a> : null}</div><div><small>YOUTUBE</small><strong>{process.env.NEXT_PUBLIC_YOUTUBE_URL ? productCopy.ui.official : productCopy.common.unavailable}</strong>{process.env.NEXT_PUBLIC_YOUTUBE_URL ? <a href={process.env.NEXT_PUBLIC_YOUTUBE_URL} target="_blank" rel="noreferrer">{productCopy.ui.watch} ↗</a> : null}</div><div><small>BILIBILI</small><strong>{process.env.NEXT_PUBLIC_BILIBILI_URL ? productCopy.ui.official : productCopy.common.unavailable}</strong>{process.env.NEXT_PUBLIC_BILIBILI_URL ? <a href={process.env.NEXT_PUBLIC_BILIBILI_URL} target="_blank" rel="noreferrer">{productCopy.ui.watch} ↗</a> : null}</div></section> : null}
      {section === "press" ? <section className="press-downloads section"><Link href="/og.png" target="_blank">OG / 1200×630 ↗</Link><Link href="/gameplay/character-select.png" target="_blank">CHARACTER SELECT ↗</Link><Link href="/gameplay/shared-world.png" target="_blank">SHARED WORLD ↗</Link><Link href="/gameplay/skills-and-inventory.png" target="_blank">SYSTEMS ↗</Link></section> : null}
    </ProductPageShell>
  );
}
