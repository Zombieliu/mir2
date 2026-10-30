import type { Metadata, Viewport } from "next";
import { notFound } from "next/navigation";
import type { ReactNode } from "react";
import { AttributionCapture } from "@/app/components/attribution-capture";
import { getCopy, getLocaleMeta, getLocalizedAlternates, isLocale, locales } from "@/lib/site-copy";
import "../globals.css";

type LocaleLayoutProps = {
  children: ReactNode;
  params: Promise<{ locale: string }>;
};

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: LocaleLayoutProps): Promise<Metadata> {
  const { locale } = await params;
  if (!isLocale(locale)) return {};
  const copy = getCopy(locale);
  const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? "http://localhost:3080";
  const pageTitle = locale === "zh-CN"
    ? `${copy.hero.titleTop}${copy.hero.titleBottom}`
    : `${copy.hero.titleTop} ${copy.hero.titleBottom}`;

  return {
    metadataBase: new URL(siteUrl),
    title: {
      default: `${pageTitle} | NUMERON`,
      template: "%s | NUMERON",
    },
    description: copy.hero.description,
    alternates: getLocalizedAlternates(locale),
    openGraph: {
      type: "website",
      title: pageTitle,
      description: copy.hero.description,
      locale,
      siteName: "NUMERON · Legend of Rebirth",
      images: [
        {
          url: "/og.png",
          width: 1200,
          height: 630,
          alt: "NUMERON — Legend of Rebirth · 传奇重生",
        },
      ],
    },
    twitter: {
      card: "summary_large_image",
      title: pageTitle,
      description: copy.hero.description,
      images: ["/og.png"],
    },
  };
}

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  themeColor: "#080807",
  colorScheme: "dark",
};

export default async function LocaleLayout({ children, params }: LocaleLayoutProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();

  const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? "http://localhost:3080";
  const structuredData = {
    "@context": "https://schema.org",
    "@graph": [
      { "@type": "Organization", "@id": `${siteUrl}/#organization`, name: "NUMERON", url: siteUrl },
      { "@type": "WebSite", "@id": `${siteUrl}/#website`, url: siteUrl, name: "NUMERON · Legend of Rebirth", publisher: { "@id": `${siteUrl}/#organization` }, inLanguage: locales },
      { "@type": "VideoGame", name: "Legend of Rebirth", alternateName: "传奇重生", url: `${siteUrl}/${locale}`, applicationCategory: "Game", gamePlatform: ["Web Browser", "Windows", "macOS"], genre: ["MMORPG", "Action RPG"], publisher: { "@id": `${siteUrl}/#organization` } },
    ],
  };

  return (
    <html lang={locale} dir={getLocaleMeta(locale).dir}>
      <body><AttributionCapture />{children}<script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(structuredData).replace(/</g, "\\u003c") }} /></body>
    </html>
  );
}
