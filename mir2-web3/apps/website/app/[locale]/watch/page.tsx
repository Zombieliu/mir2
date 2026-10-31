import type { Metadata } from "next";
import Image from "next/image";
import { notFound } from "next/navigation";
import { HighlightGallery } from "@/app/components/highlight-gallery";
import { MotionController } from "@/app/components/motion-controller";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getProductCopy } from "@/lib/product-copy";
import { getCopy, getLocalizedAlternates, isLocale } from "@/lib/site-copy";

type WatchPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({ params }: WatchPageProps): Promise<Metadata> {
  const { locale } = await params;
  if (!isLocale(locale)) return {};
  const copy = getCopy(locale);
  return {
    title: copy.watch.highlightsTitle,
    description: copy.watch.copy,
    alternates: getLocalizedAlternates(locale, "/watch"),
  };
}

export default async function WatchPage({ params }: WatchPageProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();

  const copy = getCopy(locale);
  const productCopy = getProductCopy(locale);
  const playUrl = `/${locale}/play`;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;
  const liveUrl = process.env.NEXT_PUBLIC_LIVE_STREAM_URL;
  const liveEmbedUrl = process.env.NEXT_PUBLIC_LIVE_EMBED_URL;

  return (
    <main className="watch-page">
      <MotionController />
      <SiteHeader locale={locale} copy={copy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPage="watch" liveNow={Boolean(liveUrl || liveEmbedUrl)} />

      <section className="watch-hero">
        <div className="watch-hero-grid" aria-hidden="true" />
        <div className="watch-hero-orbit" aria-hidden="true"><i /><i /><i /></div>
        <div className="watch-hero-copy" data-reveal="hero">
          <div className="eyebrow"><span />{copy.watch.eyebrow}</div>
          <h1>{copy.watch.title.split("\n").map((line) => <span key={line}>{line}</span>)}</h1>
          <p>{copy.watch.copy}</p>
        </div>
      </section>

      <section className="live-section" id="live">
        <div className="live-frame" data-reveal>
          {liveEmbedUrl ? <iframe src={liveEmbedUrl} title={copy.watch.liveTitle} allow="autoplay; encrypted-media; picture-in-picture" allowFullScreen /> : <Image src="/gameplay/shared-world.png" alt={`${copy.watch.liveTitle} · ${productCopy.watch.still}`} fill priority sizes="(max-width: 960px) 100vw, 68vw" />}
          {!liveEmbedUrl ? <div className="live-frame-shade" /> : null}
          <div className="live-frame-topline"><span><i />{copy.watch.live}</span><b>{liveUrl || liveEmbedUrl ? copy.watch.live : copy.watch.demo}</b></div>
          {!liveEmbedUrl ? <><div className="live-reticle" aria-hidden="true"><i /><i /><i /></div><div className="live-frame-caption"><small>{copy.nav.world} 01 / {copy.watch.live}</small><strong>{copy.watch.liveTitle}</strong></div></> : null}
        </div>

        <aside className="live-context" data-reveal>
          <span className="live-context-index">/ {copy.watch.live} 01</span>
          <h2>{copy.watch.liveTitle}</h2>
          <p>{copy.watch.liveCopy}</p>
          <dl>
            <div><dt>{copy.nav.pulse}</dt><dd>120 SEC</dd></div>
            <div><dt>{productCopy.chrome.privacy}</dt><dd>{productCopy.common.draft}</dd></div>
            <div><dt>{copy.nav.watch}</dt><dd>AI</dd></div>
          </dl>
          <div className="live-actions">
            {liveUrl ? <a className="button button-primary" href={liveUrl} target="_blank" rel="noreferrer"><span>{copy.watch.live}</span><i>↗</i></a> : null}
            <a className="button button-ghost" href={playUrl} data-track="watch_enter_game"><span>{copy.watch.enter}</span><i>↗</i></a>
            <a className="text-link" href={explorerUrl}>{copy.watch.openAtlas}<span>↗</span></a>
          </div>
        </aside>
      </section>

      <section className="highlights-section section" id="highlights">
        <div className="section-heading" data-reveal>
          <div className="eyebrow"><span />{copy.watch.highlightsEyebrow}</div>
          <h2>{copy.watch.highlightsTitle}</h2>
          <p>{copy.watch.highlightsCopy}</p>
        </div>
        <div className="clip-grid">
          {copy.watch.clips.map((clip, index) => (
            <article className={`clip-card clip-card-${index + 1}`} key={clip.title} data-reveal>
              <div className="clip-visual" aria-hidden="true">
                <span>{String(index + 1).padStart(2, "0")}</span>
                <i className="clip-play">▶</i>
                <b>{clip.duration}</b>
              </div>
              <div className="clip-meta"><span>{clip.tag}</span><small>{productCopy.ui.prototypeData}</small></div>
              <h3>{clip.title}</h3>
              <p>{clip.copy}</p>
            </article>
          ))}
        </div>
        <div className="highlight-archive-heading" data-reveal>
          <h2>{productCopy.watch.archiveTitle}</h2><p>{productCopy.watch.archiveCopy}</p>
        </div>
        <HighlightGallery copy={productCopy.watch} />
      </section>

      <section className="channels-section" id="channels">
        <div data-reveal>
          <div className="eyebrow"><span />{copy.watch.channelsEyebrow}</div>
          <h2>{copy.watch.channelsTitle}</h2>
          <p>{copy.watch.channelsCopy}</p>
        </div>
        <div className="channel-rail" aria-label={copy.watch.channelsTitle} data-reveal>
          <span>YOUTUBE / 16:9</span>
          <span>SHORTS / 9:16</span>
          <span>BILIBILI / 16:9</span>
          <span>{productCopy.ui.accountUsage[2]}</span>
        </div>
      </section>

      <SiteFooter locale={locale} copy={copy} gameUrl={playUrl} explorerUrl={explorerUrl} currentPage="watch" />
    </main>
  );
}
