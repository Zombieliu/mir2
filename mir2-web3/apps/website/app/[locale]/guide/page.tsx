import { DocumentationPage } from "@/app/components/documentation-page";
import { documentationMetadata } from "@/lib/documentation";
import { isLocale } from "@/lib/site-copy";

type Props = { params: Promise<{ locale: string }> };

export async function generateMetadata({ params }: Props) {
  const { locale } = await params;
  return isLocale(locale) ? documentationMetadata(locale, "guide") : {};
}

export default async function Page({ params }: Props) {
  return <DocumentationPage locale={(await params).locale} area="guide" />;
}
