import { notFound } from "next/navigation";
import { ExplorerDashboard } from "@/app/components/explorer-dashboard";
import { isLocale, locales } from "@/lib/atlas-data.mjs";

type ExplorerPageProps = {
  params: Promise<{ locale: string }>;
};

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function ExplorerPage({ params }: ExplorerPageProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  return <ExplorerDashboard locale={locale} />;
}
