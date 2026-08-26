import Image from "next/image";
import { notFound } from "next/navigation";
import { MotionController } from "@/app/components/motion-controller";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getCopy, isLocale } from "@/lib/site-copy";

const gameUrl = process.env.NEXT_PUBLIC_GAME_URL ?? "https://mir2.obelisk.build";

type HomePageProps = {
  params: Promise<{ locale: string }>;
};

export default async function HomePage({ params }: HomePageProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  const copy = getCopy(locale);
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? "/zh-CN/explore";
  const liveNow = Boolean(process.env.NEXT_PUBLIC_LIVE_STREAM_URL);

  return (
    <main>
      <MotionController />
      <SiteHeader locale={locale} copy={copy} gameUrl={gameUrl} explorerUrl={explorerUrl} liveNow={liveNow} />

      <section className="hero" aria-labelledby="hero-title">
        <div className="awakening-curtain" aria-hidden="true"><i /><i /></div>
        <div className="hero-noise" aria-hidden="true" />
        <div className="hero-orbit hero-orbit-one" aria-hidden="true" />
        <div className="hero-orbit hero-orbit-two" aria-hidden="true" />
        <div className="hero-art" aria-hidden="true">
          <div className="portal-flare" />
          <div className="portal-particles">
            {Array.from({ length: 9 }, (_, index) => <i key={index} />)}
          </div>
          <div className="hero-art-ring">
            <Image
              src="https://mir2.obelisk.build/bootstrap/login/chrsel-0-1024.webp"
              alt=""
              fill
              priority
              sizes="(max-width: 760px) 82vw, 54vw"
            />
          </div>
          <span className="hero-art-label hero-art-label-top">NUMERON SERIES // 0001</span>
          <span className="hero-art-label hero-art-label-bottom">LEGEND OF REBIRTH // WORLD 01</span>
        </div>

        <div className="hero-content" data-reveal="hero">
          <div className="eyebrow"><span />{copy.hero.eyebrow}</div>
          <h1 id="hero-title">
            <span>{copy.hero.titleTop}</span>
            <strong>{copy.hero.titleBottom}</strong>
          </h1>
          <p>{copy.hero.description}</p>
          <div className="hero-actions">
            <a className="button button-primary" href={gameUrl} target="_blank" rel="noreferrer">
              <span>{copy.hero.primary}</span><i aria-hidden="true">↗</i>
            </a>
            <a className="button button-ghost" href="#world">
              <span>{copy.hero.secondary}</span><i aria-hidden="true">↓</i>
            </a>
          </div>
          <div className="world-status"><span className="status-pulse" />{copy.hero.status}<b>01</b></div>
        </div>

        <div className="hero-metrics" aria-label="World highlights">
          {copy.metrics.map((metric) => (
            <div key={metric.label} className="metric">
              <strong>{metric.value}</strong>
              <span>{metric.label}</span>
            </div>
          ))}
        </div>
        <a className="scroll-cue" href="#world" aria-label={copy.hero.secondary}><span />SCROLL</a>
      </section>

      <section className="atlas-preview section" id="atlas">
        <div className="atlas-preview-map" aria-hidden="true" data-reveal>
          <div className="atlas-preview-grid" />
          <i className="atlas-ring ring-one" /><i className="atlas-ring ring-two" /><i className="atlas-ring ring-three" />
          <span className="atlas-node node-one" /><span className="atlas-node node-two" /><span className="atlas-node node-three" /><span className="atlas-node node-four" />
          <b>NUMERON<br />ATLAS</b>
        </div>
        <div className="atlas-preview-copy" data-reveal>
          <div className="eyebrow"><span />{copy.atlas.eyebrow}</div>
          <h2>{copy.atlas.title.split("\n").map((line) => <span key={line}>{line}</span>)}</h2>
          <p>{copy.atlas.copy}</p>
          <div className="atlas-live"><span className="status-pulse" />{copy.atlas.live}<small>PROTOTYPE DATA</small></div>
          <div className="atlas-preview-metrics">
            {copy.atlas.metrics.map((metric) => <span key={metric.label}><strong>{metric.value}</strong><small>{metric.label}</small></span>)}
          </div>
          <a className="button button-primary atlas-button" href={explorerUrl}><span>{copy.atlas.button}</span><i aria-hidden="true">↗</i></a>
        </div>
      </section>

      <section className="world-section section" id="world">
        <div className="section-heading" data-reveal>
          <div className="eyebrow"><span />{copy.world.eyebrow}</div>
          <h2>{copy.world.title.split("\n").map((line) => <span key={line}>{line}</span>)}</h2>
          <p>{copy.world.copy}</p>
        </div>
        <div className="world-grid">
          {copy.world.cards.map((card) => (
            <article key={card.number} className="world-card" data-reveal>
              <span className="card-number">/{card.number}</span>
              <div className="card-sigil" aria-hidden="true"><i /><i /><i /></div>
              <h3>{card.title}</h3>
              <p>{card.copy}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="classes-section section" id="classes">
        <div className="section-heading classes-heading" data-reveal>
          <div className="eyebrow"><span />{copy.classes.eyebrow}</div>
          <h2>{copy.classes.title}</h2>
          <p>{copy.classes.copy}</p>
        </div>
        <div className="class-list">
          {copy.classes.items.map((item, index) => (
            <article key={item.key} className={`class-row class-row-${index + 1}`} data-reveal>
              <span className="class-index">{item.key}</span>
              <div className={`class-emblem emblem-${index + 1}`} aria-hidden="true"><i /><b /></div>
              <div className="class-name"><h3>{item.title}</h3><span>{item.role}</span></div>
              <p>{item.copy}</p>
              <span className="class-arrow" aria-hidden="true">↗</span>
            </article>
          ))}
        </div>
      </section>

      <section className="global-section section" id="global">
        <div className="global-map" aria-hidden="true" data-reveal>
          <div className="map-grid" />
          <div className="map-ring map-ring-a" />
          <div className="map-ring map-ring-b" />
          <span className="map-dot dot-one" /><span className="map-dot dot-two" />
          <span className="map-dot dot-three" /><span className="map-dot dot-four" />
          <span className="map-route" />
          <strong>NUMERON // GLOBAL</strong>
        </div>
        <div className="global-copy" data-reveal>
          <div className="eyebrow"><span />{copy.global.eyebrow}</div>
          <h2>{copy.global.title.split("\n").map((line) => <span key={line}>{line}</span>)}</h2>
          <p>{copy.global.copy}</p>
          <div className="market-list">
            {copy.global.markets.map((market, index) => <span key={market}><i>{String(index + 1).padStart(2, "0")}</i>{market}</span>)}
          </div>
        </div>
      </section>

      <section className="chronicles-section section" id="chronicles">
        <div className="section-heading chronicles-heading" data-reveal>
          <div className="eyebrow"><span />{copy.chronicles.eyebrow}</div>
          <h2>{copy.chronicles.title}</h2>
        </div>
        <div className="chronicle-list">
          {copy.chronicles.items.map((item) => (
            <article key={item.title} data-reveal>
              <div className="chronicle-meta"><time>{item.date}</time><span>{item.tag}</span></div>
              <h3>{item.title}</h3>
              <p>{item.copy}</p>
              <span className="chronicle-arrow" aria-hidden="true">↗</span>
            </article>
          ))}
        </div>
      </section>

      <section className="final-cta" data-reveal>
        <div className="final-rings" aria-hidden="true"><i /><i /><i /></div>
        <div className="eyebrow"><span />{copy.finalCta.eyebrow}</div>
        <h2>{copy.finalCta.title}</h2>
        <p>{copy.finalCta.copy}</p>
        <a className="button button-primary" href={gameUrl} target="_blank" rel="noreferrer">
          <span>{copy.finalCta.button}</span><i aria-hidden="true">↗</i>
        </a>
      </section>

      <SiteFooter locale={locale} copy={copy} gameUrl={gameUrl} explorerUrl={explorerUrl} />
    </main>
  );
}
