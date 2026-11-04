import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { AtlasFooter } from "@/app/components/atlas-footer";
import {
  archiveHref,
  archiveRecords,
  copy,
  facts,
  getArchiveRecord,
  isLocale,
  locales,
} from "@/lib/atlas-data.mjs";

type Props = {
  params: Promise<{ locale: string; kind: string; slug: string }>;
};
const origin = "https://numeron.obelisk.build";

export function generateStaticParams() {
  return locales.flatMap((locale) =>
    archiveRecords.map(({ kind, slug }) => ({ locale, kind, slug })),
  );
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale, kind, slug } = await params;
  if (!isLocale(locale)) return {};
  const record = getArchiveRecord(locale, kind, slug);
  if (!record) return {};
  const title = `${record.name} | NUMERON ATLAS`;
  return {
    title,
    description: facts[locale][1],
    alternates: {
      canonical: `${origin}${archiveHref(record, locale)}`,
      languages: Object.fromEntries(
        locales.map((value) => [
          value,
          `${origin}${archiveHref(record, value)}`,
        ]),
      ),
    },
    openGraph: {
      title,
      description: facts[locale][1],
      url: `${origin}${archiveHref(record, locale)}`,
      images: [`${origin}/explore/og.png`],
    },
    twitter: {
      title,
      description: facts[locale][1],
      images: [`${origin}/explore/og.png`],
    },
  };
}

export default async function ArchivePage({ params }: Props) {
  const { locale, kind, slug } = await params;
  if (!isLocale(locale)) notFound();
  const record = getArchiveRecord(locale, kind, slug);
  if (!record) notFound();
  const text = copy[locale];
  const related = record.related
    .map((link) => getArchiveRecord(locale, link.kind, link.slug))
    .filter((value) => value !== undefined);
  return (
    <main
      className={`archive-shell tone-${record.tone}`}
      lang={locale}
      dir={text.dir}
    >
      <header className="detail-topbar">
        <Link
          className="atlas-brand"
          href={`/${locale}`}
          aria-label={text.record[8]}
        >
          <span className="brand-mark">
            <i />
            <i />
          </span>
          <span>
            <strong>NUMERON</strong>
            <small>ATLAS</small>
          </span>
        </Link>
        <nav aria-label={text.record[0]}>
          <Link href={`/${locale}`}>{text.nav[0]}</Link>
          <Link href={`/${locale}#world-activity`}>{text.nav[1]}</Link>
          <Link href={`/${locale}#market-pulse`}>{text.nav[5]}</Link>
        </nav>
        <details className="atlas-locale">
          <summary>{text.short}</summary>
          <div>
            {locales.map((value) => (
              <Link
                key={value}
                href={archiveHref(record, value)}
                lang={value}
                aria-current={value === locale ? "page" : undefined}
              >
                {copy[value].name}
              </Link>
            ))}
          </div>
        </details>
        <div className="realm-switch">
          NUMERON<small>{facts[locale][0]}</small>
          <i>⌄</i>
        </div>
      </header>
      <section className="archive-hero">
        <div className="archive-hero-copy">
          <div className="archive-breadcrumb">
            <Link href={`/${locale}`}>ATLAS</Link>
            <span>/</span>
            <span>{record.kindLabel}</span>
            <span>/</span>
            <strong>{record.name}</strong>
          </div>
          <div className="archive-overline">
            <span />
            {record.eyebrow}
          </div>
          <h1>{record.name}</h1>
          <p className="archive-english">{record.englishName}</p>
          <p className="archive-summary">{facts[locale][1]}</p>
          <div className="archive-state">
            <i />
            {record.status}
            <small>{text.ui[20]}</small>
          </div>
        </div>
        <div className="archive-emblem" aria-hidden="true">
          <i />
          <i />
          <span>{record.emblem}</span>
          <small>{record.kindLabel}</small>
        </div>
      </section>
      <section
        className="archive-metrics"
        aria-label={`${record.name} · ${text.record[5]}`}
      >
        {record.metrics.map((metric) => (
          <article key={metric.id} data-metric-id={metric.id} data-data-state="prototype">
            <small>{metric.label}</small>
            <strong>{metric.value}</strong>
            <span>{metric.note}</span>
          </article>
        ))}
      </section>
      <section className="archive-content">
        <article className="archive-panel archive-overview">
          <div className="archive-panel-heading">
            <span>01</span>
            <div>
              <small>ARCHIVE</small>
              <h2>{text.ui[21]}</h2>
            </div>
          </div>
          <div className="archive-prose">
            {record.overview.map((paragraph) => (
              <p key={paragraph}>{paragraph}</p>
            ))}
          </div>
          <div className="privacy-note">
            <i>◈</i>
            <span>
              <strong>{text.ui[19]}</strong>
              <small>{text.record[4]}</small>
            </span>
          </div>
        </article>
        <aside className="archive-panel archive-facts">
          <div className="archive-panel-heading">
            <span>02</span>
            <div>
              <small>FIELDS</small>
              <h2>{text.record[6]}</h2>
            </div>
          </div>
          <dl>
            {record.facts.map((field) => (
              <div key={field.label}>
                <dt>{field.label}</dt>
                <dd>{field.value}</dd>
              </div>
            ))}
          </dl>
        </aside>
        <article className="archive-panel archive-timeline">
          <div className="archive-panel-heading">
            <span>03</span>
            <div>
              <small>TIMELINE</small>
              <h2>{record.timelineTitle}</h2>
            </div>
          </div>
          <ol>
            {record.timeline.map((entry, index) => (
              <li key={index}>
                <i>—</i>
                <time>{entry.time}</time>
                <div>
                  <strong>{entry.title}</strong>
                  <p>{entry.detail}</p>
                </div>
              </li>
            ))}
          </ol>
        </article>
      </section>
      <section className="related-archives">
        <div className="related-heading">
          <div>
            <small>CONNECTED RECORDS</small>
            <h2>{text.ui[22]}</h2>
          </div>
          <Link href={`/${locale}`}>{text.record[8]} ↗</Link>
        </div>
        <div className="related-grid">
          {related.map((item) => (
            <Link
              key={`${item.kind}/${item.slug}`}
              className={`related-card tone-${item.tone}`}
              href={archiveHref(item, locale)}
            >
              <span>{item.emblem}</span>
              <small>{item.kindLabel}</small>
              <strong>{item.name}</strong>
              <p>{item.summary}</p>
              <i>{text.ui[14]} ↗</i>
            </Link>
          ))}
        </div>
      </section>
      <AtlasFooter locale={locale} />
    </main>
  );
}
