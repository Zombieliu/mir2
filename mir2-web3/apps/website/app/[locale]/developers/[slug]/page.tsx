import { DocumentationPage } from "@/app/components/documentation-page";
import { documentation, documentationMetadata } from "@/lib/documentation";
import { isLocale, locales } from "@/lib/site-copy";

type Props = { params: Promise<{ locale: string; slug: string }> };
export const dynamicParams = false;

export function generateStaticParams() {
  return locales.flatMap((locale) => documentation.developers.articles.map(({ slug }) => ({ locale, slug })));
}

export async function generateMetadata({ params }: Props) {
  const { locale, slug } = await params;
  return isLocale(locale) ? documentationMetadata(locale, "developers", slug) : {};
}

export default async function Page({ params }: Props) {
  const { locale, slug } = await params;
  return <DocumentationPage locale={locale} area="developers" slug={slug} />;
}
