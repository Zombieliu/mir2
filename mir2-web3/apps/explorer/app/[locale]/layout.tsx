import type { Metadata, Viewport } from "next";
import { notFound } from "next/navigation";
import type { ReactNode } from "react";
import { copy, facts, isLocale, locales } from "@/lib/atlas-data.mjs";
import "../globals.css";

const origin = "https://numeron.obelisk.build";

export async function generateMetadata({
  params,
}: {
  params: Promise<{ locale: string }>;
}): Promise<Metadata> {
  const { locale } = await params;
  if (!isLocale(locale)) return {};
  const title = `NUMERON ATLAS · ${facts[locale][0]}`;
  const description = facts[locale][1];
  return {
    metadataBase: new URL(origin),
    title,
    description,
    alternates: {
      canonical: `${origin}/${locale}/explore`,
      languages: Object.fromEntries(
        locales.map((value) => [value, `${origin}/${value}/explore`]),
      ),
    },
    openGraph: {
      type: "website",
      title,
      description,
      siteName: "NUMERON ATLAS",
      images: [
        {
          url: `${origin}/explore/og.png`,
          width: 1200,
          height: 630,
          alt: "NUMERON ATLAS · Interface prototype",
        },
      ],
    },
    twitter: {
      card: "summary_large_image",
      title,
      description,
      images: [`${origin}/explore/og.png`],
    },
    icons: { icon: "/explore/explorer-icon" },
  };
}

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  themeColor: "#080b0c",
  colorScheme: "dark",
};

export default async function LocaleLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  return (
    <html lang={locale} dir={copy[locale].dir}>
      <body>{children}</body>
    </html>
  );
}
