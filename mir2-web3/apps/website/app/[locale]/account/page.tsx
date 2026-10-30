import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ProductPageShell } from "@/app/components/product-page-shell";
import { getProductCopy } from "@/lib/product-copy";
import { getCopy, getLocalizedAlternates, isLocale } from "@/lib/site-copy";

type AccountPageProps = { params: Promise<{ locale: string }> };

export async function generateMetadata({ params }: AccountPageProps): Promise<Metadata> {
  const { locale } = await params;
  if (!isLocale(locale)) return {};
  const copy = getProductCopy(locale).account;
  return { title: copy.title, description: copy.copy, alternates: getLocalizedAlternates(locale, "/account") };
}

export default async function AccountPage({ params }: AccountPageProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  const productCopy = getProductCopy(locale);
  const copy = productCopy.account;
  return (
    <ProductPageShell locale={locale} siteCopy={getCopy(locale)} productCopy={productCopy} currentPath="/account" eyebrow={copy.eyebrow} title={copy.title} copy={copy.copy} cards={[]}>
      <section className="account-preview section">
        <div className="account-identity" data-reveal><span>{copy.preview}</span><h2>NUMERON ID</h2><p>{copy.copy}</p><a className="button button-primary" href={process.env.NEXT_PUBLIC_GAME_URL ?? "https://mir2.obelisk.build"} target="_blank" rel="noreferrer" data-track="account_sign_in"><span>{copy.signIn}</span><i>↗</i></a></div>
        <div className="account-usage" data-reveal><small>{copy.usageTitle}</small><h2>0 / 200</h2><i><b /></i><dl>{productCopy.ui.accountUsage.map((label) => <div key={label}><dt>{label}</dt><dd>0</dd></div>)}</dl></div>
        <div className="account-content" data-reveal><small>{copy.contentTitle}</small><p>{productCopy.watch.archiveCopy}</p><Link href={`/${locale}/membership`}>TOKEN ↗</Link></div>
      </section>
    </ProductPageShell>
  );
}
