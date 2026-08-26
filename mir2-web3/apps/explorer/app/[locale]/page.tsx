import { notFound } from "next/navigation";
import { ExplorerDashboard } from "@/app/components/explorer-dashboard";

type ExplorerPageProps = {
  params: Promise<{ locale: string }>;
};

export function generateStaticParams() {
  return [{ locale: "zh-CN" }];
}

export default async function ExplorerPage({ params }: ExplorerPageProps) {
  const { locale } = await params;
  if (locale !== "zh-CN") notFound();
  return <ExplorerDashboard />;
}
