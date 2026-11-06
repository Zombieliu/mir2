import Link from "next/link";
import { getOperationsCopy, operationSectionIds, operationsUpdatedAt } from "@/lib/operations-copy";
import { getLegalPresentation } from "@/lib/legal-content";
import { getLocaleMeta, type Locale } from "@/lib/site-copy";

export function OperationsDisclosure({ locale }: { locale: Locale }) {
  const copy = getOperationsCopy(locale);
  const labels = getLegalPresentation(locale).labels;
  return (
    <section className="operations-disclosure" lang={locale} dir={getLocaleMeta(locale).dir} aria-labelledby="operations-title" data-operations-locale={locale}>
      <header><h2 id="operations-title">{copy.title}</h2><p data-service-operator>{copy.operator}</p></header>
      <div className="operations-grid">{operationSectionIds.map(id => <article key={id} data-operation-id={id}><h3>{copy.sections[id].title}</h3><p>{copy.sections[id].copy}</p></article>)}</div>
      <p className="operations-notice">{copy.notice} <time dateTime={operationsUpdatedAt}>{operationsUpdatedAt}</time></p>
      <nav aria-label={copy.title}>
        <Link href={`/${locale}/legal/refunds`}>{labels.refunds}</Link>
        <Link href={`/${locale}/legal/privacy`}>{labels.privacy}</Link>
        <Link href={`/${locale}/legal/terms`}>{labels.terms}</Link>
      </nav>
    </section>
  );
}
