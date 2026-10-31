import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { FounderInterestForm } from "@/app/components/founder-interest-form";
import { MotionController } from "@/app/components/motion-controller";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { SubscriptionPlans } from "@/app/components/subscription-plans";
import { getProductCopy } from "@/lib/product-copy";
import { getCopy, getLocalizedAlternates, isLocale } from "@/lib/site-copy";

type MembershipPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({ params }: MembershipPageProps): Promise<Metadata> {
  const { locale } = await params;
  if (!isLocale(locale)) return {};
  const copy = getCopy(locale);
  return {
    title: "NUMERON Token",
    description: copy.membership.copy,
    alternates: getLocalizedAlternates(locale, "/membership"),
  };
}

export default async function MembershipPage({ params }: MembershipPageProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();

  const copy = getCopy(locale);
  const productCopy = getProductCopy(locale);
  const playUrl = `/${locale}/play`;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;
  const checkoutUrl = process.env.NEXT_PUBLIC_TOKEN_CHECKOUT_URL;

  return (
    <main className="membership-page">
      <MotionController />
      <SiteHeader locale={locale} copy={copy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPage="membership" />

      <section className="membership-hero" aria-labelledby="membership-title">
        <div className="membership-hero-grid" aria-hidden="true" />
        <div className="token-orbit" aria-hidden="true">
          <i /><i /><i />
          <b><small>NUMERON</small>TOKEN<span>{productCopy.ui.aiService}</span></b>
        </div>
        <div className="membership-hero-copy" data-reveal="hero">
          <div className="eyebrow"><span />{copy.membership.eyebrow}</div>
          <h1 id="membership-title">
            {copy.membership.title.split("\n").map((line) => <span key={line}>{line}</span>)}
          </h1>
          <p>{copy.membership.copy}</p>
          <div className="membership-hero-actions">
            <a className="button button-primary" href="#plans"><span>{copy.nav.token}</span><i aria-hidden="true">↓</i></a>
            <strong><i />{copy.membership.serviceBadge}</strong>
          </div>
        </div>
      </section>

      <section className="plans-section section" id="plans">
        <div className="section-heading membership-heading" data-reveal>
          <div className="eyebrow"><span />{productCopy.ui.founderService} // 2026</div>
          <h2>{copy.membership.plans[1].name}</h2>
          <p>{copy.membership.copy}</p>
        </div>
        <div data-reveal>
          <SubscriptionPlans copy={copy.membership} checkoutUrl={checkoutUrl} playUrl={playUrl} waitlistLabel={productCopy.membership.waitlist} />
        </div>
      </section>

      <section className="membership-details section">
        <div className="section-heading" data-reveal>
          <div className="eyebrow"><span />{productCopy.ui.usageContract}</div>
          <h2>{productCopy.membership.examplesTitle}</h2>
        </div>
        <div className="membership-example-grid">
          {productCopy.membership.examples.map((item) => <article key={item.title} data-reveal><small>{item.eyebrow}</small><h3>{item.title}</h3><p>{item.copy}</p></article>)}
        </div>
        <ul className="membership-rule-list" data-reveal>
          {[productCopy.membership.rollover, productCopy.membership.cancellation, productCopy.membership.refund, productCopy.membership.rights].map((rule) => <li key={rule}><i>✓</i>{rule}</li>)}
        </ul>
      </section>

      <section className="founder-section section" id="founder-access">
        <div className="founder-copy" data-reveal>
          <div className="eyebrow"><span />{productCopy.membership.founderEyebrow}</div>
          <h2>{productCopy.membership.founderTitle}</h2>
          <p>{productCopy.membership.founderCopy}</p>
        </div>
        <div data-reveal><FounderInterestForm copy={productCopy.membership} locale={locale} /></div>
      </section>

      <section className="usage-section section">
        <div className="usage-intro" data-reveal>
          <div className="eyebrow"><span />{copy.membership.usageEyebrow}</div>
          <h2>{copy.membership.usageTitle}</h2>
          <p>{copy.membership.usageCopy}</p>
        </div>
        <div className="usage-table-wrap" data-reveal>
          <table className="usage-table">
            <thead>
              <tr>
                <th scope="col">{productCopy.ui.aiService}</th>
                {copy.membership.plans.map((plan) => <th scope="col" key={plan.id}>{plan.name}</th>)}
              </tr>
            </thead>
            <tbody>
              {copy.membership.usage.map((item) => (
                <tr key={item.label}>
                  <th scope="row">{item.label}</th>
                  <td>{item.free}</td>
                  <td className="usage-featured">{item.token}</td>
                  <td>{item.architect}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className="fairness-section">
        <div className="fairness-sigil" aria-hidden="true"><i /><i /><b>NO<br />P2W</b></div>
        <div className="fairness-copy" data-reveal>
          <div className="eyebrow"><span />{copy.membership.fairnessEyebrow}</div>
          <h2>{copy.membership.fairnessTitle}</h2>
          <p>{copy.membership.fairnessCopy}</p>
          <ul>
            {copy.membership.fairnessPoints.map((point) => <li key={point}><i>✓</i>{point}</li>)}
          </ul>
        </div>
      </section>

      <section className="membership-faq section">
        <div className="section-heading" data-reveal>
          <div className="eyebrow"><span />{copy.membership.faqEyebrow}</div>
          <h2>{copy.membership.faqTitle}</h2>
        </div>
        <div className="faq-list" data-reveal>
          {copy.membership.faq.map((item, index) => (
            <details key={item.question} open={index === 0}>
              <summary><span>{String(index + 1).padStart(2, "0")}</span>{item.question}<i aria-hidden="true">+</i></summary>
              <p>{item.answer}</p>
            </details>
          ))}
        </div>
      </section>

      <SiteFooter locale={locale} copy={copy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPage="membership" />
    </main>
  );
}
