import Link from "next/link";
import { getCopy, locales, type Locale, type SiteCopy } from "@/lib/site-copy";

type SiteChromeProps = {
  locale: Locale;
  copy: SiteCopy;
  gameUrl: string;
  explorerUrl: string;
  currentPage?: "home" | "watch" | "membership";
};

type HeaderProps = SiteChromeProps & {
  liveNow?: boolean;
};

function currentPageSuffix(currentPage: SiteChromeProps["currentPage"]) {
  if (currentPage === "watch") return "/watch";
  if (currentPage === "membership") return "/membership";
  return "";
}

function LocaleMenu({ locale, copy, currentPage = "home" }: Pick<SiteChromeProps, "locale" | "copy" | "currentPage">) {
  return (
    <details className="language-menu">
      <summary aria-label={copy.nav.locale}>{locale.replace("zh-CN", "ZH")}</summary>
      <div className="language-options">
        {locales.map((item) => (
          <Link key={item} href={`/${item}${currentPageSuffix(currentPage)}`} lang={item} aria-current={item === locale ? "page" : undefined}>
            <span>{item.replace("zh-CN", "ZH").toUpperCase()}</span>
            <small>{getCopy(item).languageName}</small>
          </Link>
        ))}
      </div>
    </details>
  );
}

function NavigationLinks({ locale, copy, explorerUrl, currentPage }: Pick<SiteChromeProps, "locale" | "copy" | "explorerUrl" | "currentPage">) {
  const home = `/${locale}`;
  const watch = `/${locale}/watch`;
  const membership = `/${locale}/membership`;

  return (
    <>
      <details className="nav-group">
        <summary>{copy.nav.explore}</summary>
        <div className="nav-dropdown">
          <Link href={`${home}#world`}><span>01</span>{copy.nav.world}</Link>
          <Link href={`${home}#classes`}><span>02</span>{copy.nav.classes}</Link>
          <Link href={`${home}#global`}><span>03</span>{copy.nav.global}</Link>
        </div>
      </details>

      <details className="nav-group">
        <summary>{copy.nav.pulse}</summary>
        <div className="nav-dropdown">
          <Link href={`${watch}#live`} aria-current={currentPage === "watch" ? "page" : undefined}><span>01</span>{copy.nav.watch}</Link>
          <Link href={`${watch}#highlights`}><span>02</span>{copy.nav.highlights}</Link>
          <Link href={`${home}#chronicles`}><span>03</span>{copy.nav.chronicles}</Link>
        </div>
      </details>

      <Link className="nav-direct nav-token" href={membership} aria-current={currentPage === "membership" ? "page" : undefined}>
        {copy.nav.token}<span aria-hidden="true">◆</span>
      </Link>

      <a className="nav-direct" href={explorerUrl}>
        {copy.nav.atlas}<span aria-hidden="true">↗</span>
      </a>
    </>
  );
}

export function SiteHeader({ locale, copy, gameUrl, explorerUrl, currentPage = "home", liveNow = false }: HeaderProps) {
  return (
    <header className="site-header">
      <Link className="wordmark" href={`/${locale}`} aria-label="Legend of Rebirth home">
        <span className="wordmark-main">NUMERON</span>
        <span className="wordmark-sub">LEGEND OF REBIRTH</span>
      </Link>

      <nav className="primary-nav" aria-label="Primary navigation">
        <NavigationLinks locale={locale} copy={copy} explorerUrl={explorerUrl} currentPage={currentPage} />
      </nav>

      <div className="header-actions">
        {liveNow ? <Link className="header-live" href={`/${locale}/watch#live`}><i />LIVE</Link> : null}
        <LocaleMenu locale={locale} copy={copy} currentPage={currentPage} />
        <details className="mobile-menu">
          <summary aria-label={copy.nav.menu}><span>{copy.nav.menu}</span><i /><i /></summary>
          <nav className="mobile-menu-panel" aria-label="Mobile navigation">
            <NavigationLinks locale={locale} copy={copy} explorerUrl={explorerUrl} currentPage={currentPage} />
            <a className="mobile-play" href={gameUrl} target="_blank" rel="noreferrer">{copy.hero.primary}<span>↗</span></a>
          </nav>
        </details>
        <a className="header-play" href={gameUrl} target="_blank" rel="noreferrer">
          {copy.hero.primary}<span aria-hidden="true">↗</span>
        </a>
      </div>
    </header>
  );
}

export function SiteFooter({ locale, copy, gameUrl, explorerUrl, currentPage = "home" }: SiteChromeProps) {
  const home = `/${locale}`;
  const watch = `/${locale}/watch`;
  const membership = `/${locale}/membership`;
  const groups = [
    {
      title: copy.footer.game,
      links: [
        { label: copy.nav.home, href: home },
        { label: copy.nav.token, href: membership },
        { label: copy.nav.world, href: `${home}#world` },
        { label: copy.nav.classes, href: `${home}#classes` },
        { label: copy.nav.chronicles, href: `${home}#chronicles` },
      ],
    },
    {
      title: copy.footer.data,
      links: [
        { label: copy.nav.atlas, href: explorerUrl },
        { label: copy.footer.worldPulse, href: `${home}#atlas` },
        { label: copy.nav.global, href: `${home}#global` },
      ],
    },
    {
      title: copy.footer.watch,
      links: [
        { label: copy.nav.watch, href: `${watch}#live` },
        { label: copy.nav.highlights, href: `${watch}#highlights` },
        { label: copy.nav.chronicles, href: `${home}#chronicles` },
      ],
    },
    {
      title: copy.footer.regions,
      links: locales.map((item) => ({ label: getCopy(item).languageName, href: `/${item}${currentPageSuffix(currentPage)}` })),
    },
  ];

  return (
    <footer className="site-footer">
      <div className="footer-lead">
        <Link className="footer-wordmark" href={home}>NUMERON</Link>
        <p>{copy.footer.note}</p>
        <a className="footer-enter" href={gameUrl} target="_blank" rel="noreferrer">{copy.footer.enter}<span>↗</span></a>
      </div>

      <nav className="footer-directory" aria-label="Footer navigation">
        {groups.map((group) => (
          <section key={group.title}>
            <h2>{group.title}</h2>
            {group.links.map((link) => <Link key={`${group.title}-${link.href}`} href={link.href}>{link.label}</Link>)}
          </section>
        ))}
      </nav>

      <div className="footer-meta">
        <span>{copy.footer.legal}</span>
        <span>© 2026 NUMERON</span>
        <span>WORLD 01 / PUBLIC SITE</span>
      </div>
    </footer>
  );
}
