import type { Metadata } from "next";
import Image from "next/image";
import { notFound } from "next/navigation";
import { MotionController } from "@/app/components/motion-controller";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getCopy, isLocale } from "@/lib/site-copy";

const gameUrl = process.env.NEXT_PUBLIC_GAME_URL ?? "https://mir2.obelisk.build";

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
  };
}

export default async function WatchPage({ params }: WatchPageProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();

  const copy = getCopy(locale);
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? "/zh-CN/explore";
  const liveUrl = process.env.NEXT_PUBLIC_LIVE_STREAM_URL;

  return (
    <main className="watch-page">
      <MotionController />
      <SiteHeader locale={locale} copy={copy} gameUrl={gameUrl} explorerUrl={explorerUrl} currentPage="watch" liveNow={Boolean(liveUrl)} />

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
          <Image
            src="https://mir2.obelisk.build/bootstrap/login/chrsel-0-1024.webp"
            alt=""
            fill
            priority
            sizes="(max-width: 960px) 100vw, 68vw"
          />
          <div className="live-frame-shade" />
          <div className="live-frame-topline"><span><i />{copy.watch.live}</span><b>{liveUrl ? "LIVE" : copy.watch.demo}</b></div>
          <div className="live-reticle" aria-hidden="true"><i /><i /><i /></div>
          <div className="live-frame-caption"><small>WORLD 01 / PUBLIC SPECTATOR</small><strong>{copy.watch.liveTitle}</strong></div>
        </div>

        <aside className="live-context" data-reveal>
          <span className="live-context-index">/ LIVE 01</span>
          <h2>{copy.watch.liveTitle}</h2>
          <p>{copy.watch.liveCopy}</p>
          <dl>
            <div><dt>DELAY</dt><dd>120 SEC</dd></div>
            <div><dt>PRIVACY</dt><dd>REDACTED</dd></div>
            <div><dt>DIRECTOR</dt><dd>AI ASSISTED</dd></div>
          </dl>
          <div className="live-actions">
            {liveUrl ? <a className="button button-primary" href={liveUrl} target="_blank" rel="noreferrer"><span>{copy.watch.live}</span><i>↗</i></a> : null}
            <a className="button button-ghost" href={gameUrl} target="_blank" rel="noreferrer"><span>{copy.watch.enter}</span><i>↗</i></a>
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
              <div className="clip-meta"><span>{clip.tag}</span><small>PROTOTYPE CLIP</small></div>
              <h3>{clip.title}</h3>
              <p>{clip.copy}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="channels-section" id="channels">
        <div data-reveal>
          <div className="eyebrow"><span />{copy.watch.channelsEyebrow}</div>
          <h2>{copy.watch.channelsTitle}</h2>
          <p>{copy.watch.channelsCopy}</p>
        </div>
        <div className="channel-rail" aria-label="Distribution formats" data-reveal>
          <span>YOUTUBE / 16:9</span>
          <span>SHORTS / 9:16</span>
          <span>BILIBILI / 16:9</span>
          <span>LOCAL VOICE</span>
        </div>
      </section>

      <SiteFooter locale={locale} copy={copy} gameUrl={gameUrl} explorerUrl={explorerUrl} currentPage="watch" />
    </main>
  );
}
