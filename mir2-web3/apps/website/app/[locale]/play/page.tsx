import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { GameStatusPanel } from "@/app/components/game-status-panel";
import { MotionController } from "@/app/components/motion-controller";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getProductCopy } from "@/lib/product-copy";
import { getCopy, getLocalizedAlternates, isLocale } from "@/lib/site-copy";

const gameUrl = process.env.NEXT_PUBLIC_GAME_URL ?? "https://mir2.obelisk.build";

type PlayPageProps = { params: Promise<{ locale: string }> };

export async function generateMetadata({ params }: PlayPageProps): Promise<Metadata> {
  const { locale } = await params;
  if (!isLocale(locale)) return {};
  const copy = getProductCopy(locale).play;
  return { title: copy.launch, description: copy.copy, alternates: getLocalizedAlternates(locale, "/play") };
}

export default async function PlayPage({ params }: PlayPageProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  const siteCopy = getCopy(locale);
  const copy = getProductCopy(locale).play;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;
  const labels = { checking: `${copy.candidate}…`, online: siteCopy.hero.status, degraded: getProductCopy(locale).common.unavailable, checked: getProductCopy(locale).common.updated };

  return (
    <main className="play-page">
      <MotionController />
      <SiteHeader locale={locale} copy={siteCopy} gameUrl={`/${locale}/play`} explorerUrl={explorerUrl} currentPath="/play" />
      <section className="launch-hero">
        <div className="launch-visual" aria-hidden="true"><Image src="/gameplay/character-select.png" alt="" fill priority sizes="(max-width: 920px) 100vw, 55vw" /></div>
        <div className="launch-copy" data-reveal="hero">
          <div className="eyebrow"><span />{copy.eyebrow}</div>
          <h1>{copy.title.split("\n").map((line) => <span key={line}>{line}</span>)}</h1>
          <p>{copy.copy}</p>
          <GameStatusPanel labels={labels} />
          <div className="launch-actions">
            <a className="button button-primary" href={gameUrl} target="_blank" rel="noreferrer" data-track="launch_browser_client"><span>{copy.launch}</span><i>↗</i></a>
            <Link className="button button-ghost" href="#requirements"><span>{copy.requirements}</span><i>↓</i></Link>
          </div>
        </div>
      </section>
      <section className="launch-options section">
        <article data-reveal><span>01</span><small>{copy.candidate}</small><h2>{copy.browser}</h2><p>{copy.browserCopy}</p><a href={gameUrl} target="_blank" rel="noreferrer" data-track="launch_browser_option">{copy.launch} ↗</a></article>
        <article data-reveal><span>02</span><small>{copy.desktop} · {getProductCopy(locale).common.unavailable}</small><h2>{copy.desktop}</h2><p>{copy.desktopCopy}</p><b>{copy.candidate}</b></article>
      </section>
      <section className="launch-requirements section" id="requirements">
        <div data-reveal><div className="eyebrow"><span />{getProductCopy(locale).ui.systemSafety}</div><h2>{copy.requirements}</h2><ul>{copy.requirementItems.map((item) => <li key={item}><i>✓</i>{item}</li>)}</ul></div>
        <aside data-reveal><span>{copy.candidate}</span><h2>{copy.safetyTitle}</h2><p>{copy.candidateCopy}</p><ol>{copy.safetyPoints.map((point, index) => <li key={point}><b>{String(index + 1).padStart(2, "0")}</b>{point}</li>)}</ol><Link href={`/${locale}/support`}>{getProductCopy(locale).chrome.support} ↗</Link></aside>
      </section>
      <SiteFooter locale={locale} copy={siteCopy} gameUrl={`/${locale}/play`} explorerUrl={explorerUrl} currentPath="/play" />
    </main>
  );
}
