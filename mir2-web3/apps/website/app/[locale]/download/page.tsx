import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { MotionController } from "@/app/components/motion-controller";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { getDownloadCopy } from "@/lib/download-copy";
import { getLocalizedAlternates, getCopy, isLocale, locales } from "@/lib/site-copy";

type DownloadPageProps = { params: Promise<{ locale: string }> };

function publicHttpsUrl(value?: string) {
  if (!value?.trim()) return null;
  try {
    const url = new URL(value.trim());
    return url.protocol === "https:" ? url.href : null;
  } catch {
    return null;
  }
}

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: DownloadPageProps): Promise<Metadata> {
  const { locale } = await params;
  if (!isLocale(locale)) return {};
  const copy = getDownloadCopy(locale);
  return { title: copy.nav, description: copy.intro, alternates: getLocalizedAlternates(locale, "/download") };
}

export default async function DownloadPage({ params }: DownloadPageProps) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();

  const copy = getDownloadCopy(locale);
  const siteCopy = getCopy(locale);
  const gameUrl = process.env.NEXT_PUBLIC_GAME_URL ?? "https://mir2.obelisk.build";
  const windowsUrl = publicHttpsUrl(process.env.NEXT_PUBLIC_WINDOWS_DOWNLOAD_URL);
  const androidUrl = publicHttpsUrl(process.env.NEXT_PUBLIC_ANDROID_DOWNLOAD_URL);
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;
  const [os, browser, memory, graphics, storage, network, input, display] = copy.labels;

  const platforms = [
    { id: "web", index: "01", name: "WEB", format: "BROWSER · PWA", status: copy.live, body: copy.webCopy },
    { id: "windows", index: "02", name: "WINDOWS", format: "X64 · TAURI / BEVY", status: windowsUrl ? copy.candidate : copy.unavailable, body: copy.windowsCopy },
    { id: "android", index: "03", name: "ANDROID", format: "PWA · CAPACITOR", status: androidUrl ? copy.candidate : copy.pwa, body: copy.androidCopy },
  ] as const;

  const requirements = [
    {
      id: "web", title: "WEB", state: copy.live,
      baseline: [[os, "64-BIT DESKTOP"], [browser, "CHROME / EDGE / SAFARI · LATEST 2 · HTTPS"], [memory, "8 GB RAM"], [graphics, "WEBGL2 / WEBGPU · HW ACCELERATION"], [storage, "≥ 1 GB CACHE"], [network, "STABLE BROADBAND · COLD START 1–2 MIN"], [input, "KEYBOARD / MOUSE"]],
      recommended: [[memory, "16 GB RAM"], [graphics, "WEBGPU · STABLE DRIVER"], [network, "≥ 20 MBPS"], [display, "≥ 1280 × 720"]],
      unsupported: "WEBGL2 / WEBGPU: N/A · HW ACCELERATION: OFF · HTTPS: REQUIRED",
    },
    {
      id: "windows", title: "WINDOWS", state: windowsUrl ? copy.candidate : copy.unavailable,
      baseline: [[os, "WINDOWS 10 / 11 · X86_64"], [browser, "MICROSOFT EDGE WEBVIEW2 · CURRENT"], [memory, "8 GB RAM"], [graphics, "WEBGL2 / WEBGPU · HW ACCELERATION"], [storage, "≥ 1 GB CACHE · INSTALLER SIZE TBD"], [input, "KEYBOARD / MOUSE"], [display, "≥ 960 × 640"]],
      recommended: [[os, "WINDOWS 11 · X86_64"], [memory, "16 GB RAM"], [graphics, "WEBGPU · STABLE DRIVER"], [storage, "SSD"], [display, "≥ 1920 × 1080"]],
      unsupported: "SIGNED INSTALLER: N/A · TAURI: CANDIDATE · NATIVE BEVY: ENGINEERING GATE",
    },
    {
      id: "android", title: "ANDROID", state: androidUrl ? copy.candidate : copy.pwa,
      baseline: [[os, "ANDROID 12+ · 64-BIT"], [browser, "CHROME / SAMSUNG INTERNET · CURRENT"], [memory, "6 GB PHYSICAL RAM"], [graphics, "WEBGL2 / WEBGPU · MAX_TEXTURE_SIZE ≥ 4096"], [storage, "128 GB · ≥ 1 GB FREE"], [input, "TOUCH · LANDSCAPE"]],
      recommended: [[os, "ANDROID 13+ · 64-BIT"], [memory, "8 GB PHYSICAL RAM"], [graphics, "WEBGPU · VULKAN DRIVER"], [storage, "128 / 256 GB"], [network, "≥ 20 MBPS · FIRST ENTRY"]],
      unsupported: "ANDROID ≤ 11 · 32-BIT · RAM ≤ 2 GB · WEBGL2: N/A · MAX_TEXTURE_SIZE < 4096 · 3 GB: EXPERIMENTAL · 4 GB: ENGINEERING",
    },
  ] as const;

  return (
    <main className="download-page">
      <MotionController />
      <SiteHeader locale={locale} copy={siteCopy} gameUrl={`/${locale}/play`} explorerUrl={explorerUrl} currentPage="download" />

      <section className="download-hero">
        <div className="download-hero-media" aria-hidden="true">
          <Image src="/gameplay/shared-world.png" alt="" fill priority sizes="100vw" />
          <i /><i /><i />
        </div>
        <div className="download-hero-copy" data-reveal="hero">
          <div className="eyebrow"><span />{copy.eyebrow}</div>
          <h1>{copy.title}</h1>
          <p>{copy.intro}</p>
          <div className="download-hero-actions">
            <a className="button button-primary" href={gameUrl} target="_blank" rel="noreferrer" data-track="download_web_hero"><span>{copy.openWeb}</span><i>↗</i></a>
            <Link className="button button-ghost" href="#requirements"><span>{copy.viewRequirements}</span><i>↓</i></Link>
          </div>
        </div>
        <p className="download-platform-lead">{copy.platformLead}</p>
      </section>

      <section className="download-platforms section" aria-label={copy.platformLead}>
        {platforms.map((platform) => (
          <article className={`download-platform platform-${platform.id}`} key={platform.id} data-reveal>
            <div className="platform-topline"><span>{platform.index}</span><b>{platform.status}</b></div>
            <div className="platform-mark" aria-hidden="true"><i />{platform.name.slice(0, 2)}</div>
            <small>{platform.format}</small>
            <h2>{platform.name}</h2>
            <p>{platform.body}</p>
            {platform.id === "web" ? (
              <a className="download-action" href={gameUrl} target="_blank" rel="noreferrer" data-track="download_web"><span>{copy.openWeb}</span><i>↗</i></a>
            ) : platform.id === "windows" && windowsUrl ? (
              <a className="download-action" href={windowsUrl} rel="noreferrer" data-track="download_windows"><span>{copy.downloadWindows}</span><i>↓</i></a>
            ) : platform.id === "android" ? (
              <a className="download-action" href={androidUrl ?? gameUrl} target="_blank" rel="noreferrer" data-track={androidUrl ? "download_android" : "install_android_pwa"}><span>{copy.installAndroid}</span><i>{androidUrl ? "↓" : "↗"}</i></a>
            ) : (
              <span className="download-action is-disabled" aria-disabled="true"><span>{copy.unavailable}</span><i>—</i></span>
            )}
          </article>
        ))}
      </section>

      <section className="download-requirements section" id="requirements">
        <header className="download-requirements-heading" data-reveal>
          <div className="eyebrow"><span />SYSTEM PROFILE // 2026.08</div>
          <h2>{copy.requirementsTitle}</h2>
          <p>{copy.requirementsCopy}</p>
        </header>
        <div className="requirement-groups">
          {requirements.map((group, index) => (
            <article className="requirement-group" key={group.id} data-reveal>
              <header><span>{String(index + 1).padStart(2, "0")}</span><div><small>{group.state}</small><h3>{group.title}</h3></div></header>
              <div className="requirement-columns">
                <section>
                  <h4>{copy.baseline}</h4>
                  <dl>{group.baseline.map(([label, value]) => <div key={`${label}-${value}`}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
                </section>
                <section>
                  <h4>{copy.recommended}</h4>
                  <dl>{group.recommended.map(([label, value]) => <div key={`${label}-${value}`}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
                </section>
              </div>
              <aside><strong>{copy.unsupported}</strong><p>{group.unsupported}</p></aside>
            </article>
          ))}
        </div>
      </section>

      <section className="download-truth section">
        <div className="download-truth-orbit" aria-hidden="true"><i /><i /><b>01</b></div>
        <div data-reveal>
          <div className="eyebrow"><span />RELEASE INTEGRITY</div>
          <h2>{copy.truthTitle}</h2>
          <p>{copy.truthCopy}</p>
          <div className="truth-statuses">
            <span><i className="is-live" />WEB · {copy.live}</span>
            <span><i />WINDOWS · {windowsUrl ? copy.candidate : copy.unavailable}</span>
            <span><i className={androidUrl ? "" : "is-live"} />ANDROID · {androidUrl ? copy.candidate : copy.pwa}</span>
          </div>
          <Link className="button button-ghost" href={`/${locale}/support`}><span>{copy.support}</span><i>↗</i></Link>
        </div>
      </section>

      <SiteFooter locale={locale} copy={siteCopy} gameUrl={`/${locale}/play`} explorerUrl={explorerUrl} currentPage="download" />
    </main>
  );
}
