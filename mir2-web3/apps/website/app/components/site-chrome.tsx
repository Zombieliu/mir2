import Link from "next/link";
import { getDownloadCopy } from "@/lib/download-copy";
import { getProductCopy } from "@/lib/product-copy";
import { getPromotionLink } from "@/lib/promotion-links";
import { getCopy, getLocaleMeta, locales, type Locale, type SiteCopy } from "@/lib/site-copy";

type SiteChromeProps = {
  locale: Locale;
  copy: SiteCopy;
  gameUrl: string;
  explorerUrl: string;
  currentPage?: "home" | "watch" | "membership" | "download";
  currentPath?: string;
};

type HeaderProps = SiteChromeProps & {
  liveNow?: boolean;
};

function currentPageSuffix(currentPage: SiteChromeProps["currentPage"], currentPath?: string) {
  if (currentPath) return currentPath;
  if (currentPage === "watch") return "/watch";
  if (currentPage === "membership") return "/membership";
  if (currentPage === "download") return "/download";
  return "";
}

function LocaleMenu({ locale, copy, currentPage = "home", currentPath }: Pick<SiteChromeProps, "locale" | "copy" | "currentPage" | "currentPath">) {
  return (
    <details className="language-menu">
      <summary aria-label={copy.nav.locale}>{getLocaleMeta(locale).short}</summary>
      <div className="language-options">
        {locales.map((item) => (
          <Link key={item} href={`/${item}${currentPageSuffix(currentPage, currentPath)}`} lang={item} aria-current={item === locale ? "page" : undefined}>
            <span>{getLocaleMeta(item).short}</span>
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
  const download = `/${locale}/download`;
  const productCopy = getProductCopy(locale);
  const downloadCopy = getDownloadCopy(locale);
  const promotion = getPromotionLink(locale);

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
          <Link href={`/${locale}/updates`}><span>03</span>{productCopy.chrome.updates}</Link>
          <Link href={`/${locale}/community`}><span>04</span>{productCopy.chrome.community}</Link>
        </div>
      </details>

      <Link className="nav-direct nav-token" href={membership} aria-current={currentPage === "membership" ? "page" : undefined}>
        {copy.nav.token}<span aria-hidden="true">◆</span>
      </Link>

      <Link className="nav-direct" href={download} aria-current={currentPage === "download" ? "page" : undefined}>
        {downloadCopy.nav}<span aria-hidden="true">↓</span>
      </Link>

      <a className="nav-direct" href={promotion.href} data-track="navigation_promotions">
        {promotion.label}<span aria-hidden="true">↗</span>
      </a>

      <a className="nav-direct" href={explorerUrl}>
        {copy.nav.atlas}<span aria-hidden="true">↗</span>
      </a>
    </>
  );
}

export function SiteHeader({ locale, copy, gameUrl, explorerUrl, currentPage = "home", currentPath, liveNow = false }: HeaderProps) {
  return (
    <header className="site-header">
      <Link className="wordmark" href={`/${locale}`} aria-label={`Legend of Rebirth · ${copy.nav.home}`}>
        <span className="wordmark-main">NUMERON</span>
        <span className="wordmark-sub">LEGEND OF REBIRTH</span>
      </Link>

      <nav className="primary-nav" aria-label={copy.nav.menu}>
        <NavigationLinks locale={locale} copy={copy} explorerUrl={explorerUrl} currentPage={currentPage} />
      </nav>

      <div className="header-actions">
        {liveNow ? <Link className="header-live" href={`/${locale}/watch#live`}><i />{copy.nav.watch}</Link> : null}
        <LocaleMenu locale={locale} copy={copy} currentPage={currentPage} currentPath={currentPath} />
        <details className="mobile-menu">
          <summary aria-label={copy.nav.menu}><span>{copy.nav.menu}</span><i /><i /></summary>
          <nav className="mobile-menu-panel" aria-label={copy.nav.menu}>
            <NavigationLinks locale={locale} copy={copy} explorerUrl={explorerUrl} currentPage={currentPage} />
            <a className="mobile-play" href={gameUrl} data-track="mobile_play">{copy.hero.primary}<span>↗</span></a>
          </nav>
        </details>
        <a className="header-play" href={gameUrl} data-track="header_play">
          {copy.hero.primary}<span aria-hidden="true">↗</span>
        </a>
      </div>
    </header>
  );
}

export function SiteFooter({ locale, copy, gameUrl, explorerUrl, currentPage = "home", currentPath }: SiteChromeProps) {
  const home = `/${locale}`;
  const watch = `/${locale}/watch`;
  const membership = `/${locale}/membership`;
  const download = `/${locale}/download`;
  const productCopy = getProductCopy(locale);
  const downloadCopy = getDownloadCopy(locale);
  const promotion = getPromotionLink(locale);
  const groups = [
    {
      title: copy.footer.game,
      links: [
        { label: copy.nav.home, href: home },
        { label: copy.nav.token, href: membership },
        { label: downloadCopy.nav, href: download },
        { ...promotion, hardNavigation: true },
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
      title: productCopy.chrome.community,
      links: [
        { label: productCopy.chrome.community, href: `/${locale}/community` },
        { label: productCopy.chrome.updates, href: `/${locale}/updates` },
        { label: productCopy.chrome.roadmap, href: `/${locale}/roadmap` },
        { label: productCopy.chrome.press, href: `/${locale}/press` },
        { label: productCopy.chrome.support, href: `/${locale}/support` },
        { label: productCopy.chrome.status, href: `/${locale}/status` },
      ],
    },
    {
      title: productCopy.chrome.legal,
      links: [
        { label: productCopy.chrome.account, href: `/${locale}/account` },
        { label: productCopy.chrome.privacy, href: `/${locale}/legal/privacy` },
        { label: productCopy.chrome.terms, href: `/${locale}/legal/terms` },
        { label: productCopy.chrome.refunds, href: `/${locale}/legal/refunds` },
        { label: productCopy.chrome.aiData, href: `/${locale}/legal/ai-data` },
      ],
    },
    {
      title: copy.footer.regions,
      links: locales.map((item) => ({ label: getCopy(item).languageName, href: `/${item}${currentPageSuffix(currentPage, currentPath)}` })),
    },
  ];

  return (
    <footer className="site-footer">
      <div className="footer-lead">
        <Link className="footer-wordmark" href={home}>NUMERON</Link>
        <p>{copy.footer.note}</p>
        <a className="footer-enter" href={gameUrl} data-track="footer_play">{copy.footer.enter}<span>↗</span></a>
      </div>

      <nav className="footer-directory" aria-label={copy.footer.note}>
        {groups.map((group) => (
          <section key={group.title}>
            <h2>{group.title}</h2>
            {group.links.map((link) => "hardNavigation" in link && link.hardNavigation
              ? <a key={`${group.title}-${link.href}`} href={link.href} data-track="footer_promotions">{link.label}</a>
              : <Link key={`${group.title}-${link.href}`} href={link.href}>{link.label}</Link>)}
          </section>
        ))}
      </nav>

      <div className="footer-meta">
        <span>{copy.footer.legal}</span>
        <span>© 2026 NUMERON</span>
        <span>WORLD 01 / {copy.footer.worldPulse}</span>
      </div>
    </footer>
  );
}
