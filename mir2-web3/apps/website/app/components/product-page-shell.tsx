import Link from "next/link";
import type { ReactNode } from "react";
import { MotionController } from "@/app/components/motion-controller";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import type { Locale, SiteCopy } from "@/lib/site-copy";
import type { ProductCard, ProductCopy } from "@/lib/product-copy";

type ProductPageShellProps = {
  locale: Locale;
  siteCopy: SiteCopy;
  productCopy: ProductCopy;
  currentPath: string;
  eyebrow: string;
  title: string;
  copy: string;
  cards: ProductCard[];
  children?: ReactNode;
};

export function ProductPageShell({ locale, siteCopy, productCopy, currentPath, eyebrow, title, copy, cards, children }: ProductPageShellProps) {
  const playUrl = `/${locale}/play`;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;

  return (
    <main className="product-page">
      <MotionController />
      <SiteHeader locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath={currentPath} />
      <section className="product-hero">
        <div className="product-hero-grid" aria-hidden="true" />
        <div className="product-orbit" aria-hidden="true"><i /><i /><b>01</b></div>
        <div className="product-hero-copy" data-reveal="hero">
          <div className="eyebrow"><span />{eyebrow}</div>
          <h1>{title}</h1>
          <p>{copy}</p>
          <small>{productCopy.common.updated} · {productCopy.common.draft}</small>
        </div>
      </section>
      {children}
      {cards.length > 0 ? (
        <section className="product-card-section section">
          <div className="product-card-grid">
            {cards.map((card, index) => {
              const href = card.href ? card.href.startsWith("http") || card.href.startsWith("/") ? card.href : `/${locale}/${card.href}` : null;
              return (
                <article className="product-card" key={`${card.title}-${index}`} data-reveal>
                  <span>/{String(index + 1).padStart(2, "0")}</span><small>{card.eyebrow}</small><h2>{card.title}</h2><p>{card.copy}</p>
                  {href ? <Link href={href}>{card.label ?? productCopy.common.learnMore}<i>↗</i></Link> : null}
                </article>
              );
            })}
          </div>
        </section>
      ) : null}
      <SiteFooter locale={locale} copy={siteCopy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPath={currentPath} />
    </main>
  );
}
