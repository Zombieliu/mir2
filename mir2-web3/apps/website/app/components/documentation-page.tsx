import Link from "next/link";
import { notFound } from "next/navigation";
import { DocumentationDirectory } from "@/app/components/documentation-directory";
import { SiteFooter, SiteHeader } from "@/app/components/site-chrome";
import { documentation, documentationHref } from "@/lib/documentation";
import { documentationStatus, documentationUpdated, type DocumentationArea, type DocumentationSection } from "@/lib/documentation-types";
import { getCopy, isLocale, type Locale } from "@/lib/site-copy";

function DirectoryNavigation({ locale, area, slug }: { locale: Locale; area: DocumentationArea; slug?: string }) {
  return (
    <>
      {(["guide", "developers"] as const).map((item) => (
        <div className="docs-nav-group" key={item}>
          <Link className="docs-nav-heading" href={`/${locale}/${item}`} aria-current={area === item && !slug ? "page" : undefined}>{documentation[item].title}<span aria-hidden="true">↗</span></Link>
          {area === item ? <ol>{documentation[item].articles.map((article) => <li key={article.slug}><Link href={`/${locale}/${item}/${article.slug}`} aria-current={slug === article.slug ? "page" : undefined}>{article.title}</Link></li>)}</ol> : null}
        </div>
      ))}
      <Link className="docs-support-link" href={`/${locale}/support`}>需要帮助？前往支持页 ↗</Link>
    </>
  );
}

function ArticleSection({ section, locale }: { section: DocumentationSection; locale: Locale }) {
  const List = section.ordered ? "ol" : "ul";
  return (
    <section id={section.id} className="docs-section" aria-labelledby={`heading-${section.id}`}>
      <h2 id={`heading-${section.id}`}><a href={`#${section.id}`}>{section.title}<span aria-hidden="true">#</span></a></h2>
      {section.paragraphs?.map((paragraph) => <p key={paragraph}>{paragraph}</p>)}
      {section.items ? <List>{section.items.map((item) => <li key={item}>{item}</li>)}</List> : null}
      {section.table ? <div className="docs-table-wrap" role="region" aria-label={section.title} tabIndex={0}><table><thead><tr>{section.table.headers.map((header) => <th key={header} scope="col">{header}</th>)}</tr></thead><tbody>{section.table.rows.map((row) => <tr key={row[0]}>{row.map((cell, index) => index === 0 ? <th key={index} scope="row">{cell}</th> : <td key={index}>{cell}</td>)}</tr>)}</tbody></table></div> : null}
      {section.code ? <figure className="docs-code"><figcaption>{section.code.language}</figcaption><pre tabIndex={0}><code>{section.code.value}</code></pre></figure> : null}
      {section.note ? <aside className="docs-note"><strong>请留意</strong><p>{section.note}</p></aside> : null}
      {section.links ? <ul className="docs-related-links">{section.links.map((link) => <li key={link.href}>{link.href === "@promotions" || link.href.startsWith("http") || link.href.startsWith("/") ? <a href={documentationHref(locale, link.href)}>{link.label}<span aria-hidden="true"> ↗</span></a> : <Link href={documentationHref(locale, link.href)}>{link.label}<span aria-hidden="true"> →</span></Link>}</li>)}</ul> : null}
    </section>
  );
}

export function DocumentationPage({ locale: candidateLocale, area, slug }: { locale: string; area: DocumentationArea; slug?: string }) {
  if (!isLocale(candidateLocale)) notFound();
  const locale = candidateLocale;
  const collection = documentation[area];
  const article = slug ? collection.articles.find((item) => item.slug === slug) : undefined;
  if (slug && !article) notFound();
  const articleIndex = article ? collection.articles.indexOf(article) : -1;
  const previous = collection.articles[articleIndex - 1];
  const next = collection.articles[articleIndex + 1];
  const path = `/${area}${slug ? `/${slug}` : ""}`;
  const copy = getCopy(locale);
  const gameUrl = `/${locale}/play`;
  const explorerUrl = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace("{locale}", locale) ?? `/${locale}/explore`;

  return (
    <div className="documentation-page">
      <a className="docs-skip" href="#docs-content">跳到正文</a>
      <SiteHeader locale={locale} copy={copy} gameUrl={gameUrl} explorerUrl={explorerUrl} currentPath={path} />
      <div className="docs-layout">
        <aside className="docs-sidebar"><nav aria-label="文档导航" lang="zh-CN"><DirectoryNavigation locale={locale} area={area} slug={slug} /></nav><div className="docs-sidebar-foot">NUMERON / DOCUMENTATION<br />中文核实稿 · v0.1</div></aside>
        <main id="docs-content" className="docs-main" lang="zh-CN" dir="ltr" tabIndex={-1}>
          <details className="docs-mobile-navigation"><summary>浏览文档目录</summary><nav aria-label="移动端文档导航"><DirectoryNavigation locale={locale} area={area} slug={slug} /></nav></details>
          <nav className="docs-breadcrumb" aria-label="面包屑"><Link href={`/${locale}`}>官网</Link><span aria-hidden="true">/</span><Link href={`/${locale}/${area}`} aria-current={article ? undefined : "page"}>{collection.title}</Link>{article ? <><span aria-hidden="true">/</span><span aria-current="page">{article.title}</span></> : null}</nav>
          {locale !== "zh-CN" ? <aside className="docs-language-note"><p lang="en">This first edition is available in Simplified Chinese. Translations are pending review.</p><p>当前正文为简体中文核实稿，其他语言尚待审核；不作为已审核的翻译条款。</p><Link href={`/zh-CN${path}`}>切换至中文入口 →</Link></aside> : null}
          <header className="docs-hero"><div className="eyebrow"><span />{collection.eyebrow}</div><h1>{article?.title ?? collection.title}</h1><p className="docs-lead">{article?.description ?? collection.description}</p><div className="docs-meta"><span>最后核对 <time dateTime={documentationUpdated}>{documentationUpdated}</time></span><span>中文核实稿 · v0.1</span></div></header>
          {article ? (
            <>
              <div className="docs-article-status"><span className={`docs-badge docs-badge-${article.status}`}>{documentationStatus[article.status]}</span><p>{article.environment}</p></div>
              <nav className="docs-inline-toc" aria-label="本页目录"><strong>本页内容</strong>{article.sections.map((section) => <a key={section.id} href={`#${section.id}`}>{section.title}</a>)}</nav>
              <article className="docs-article" aria-label={article.title}>{article.sections.map((section) => <ArticleSection key={section.id} section={section} locale={locale} />)}</article>
              <nav className="docs-pagination" aria-label="章节翻页">{previous ? <Link href={`/${locale}/${area}/${previous.slug}`}><small>← 上一篇</small><strong>{previous.title}</strong></Link> : <Link href={`/${locale}/${area}`}><small>← 返回</small><strong>{collection.title}</strong></Link>}{next ? <Link href={`/${locale}/${area}/${next.slug}`}><small>下一篇 →</small><strong>{next.title}</strong></Link> : <Link href={`/${locale}/${area === "guide" ? "developers" : "guide"}`}><small>继续探索 →</small><strong>{area === "guide" ? "开发者文档" : "玩家指南"}</strong></Link>}</nav>
            </>
          ) : (
            <>
              <div className="docs-introduction"><p>{collection.intro}</p><p>本文档区分已上线、测试中与规划中。状态只适用于页面注明的环境；测试规则可能调整，请同时阅读当前测试公告。</p><div className="docs-index-actions"><Link className="button button-primary" href={`/${locale}/${area}/${collection.articles[0].slug}`}><span>开始阅读</span><i aria-hidden="true">→</i></Link><Link href={`/${locale}/${area === "guide" ? "developers" : "guide"}`}>{area === "guide" ? "我想参与开发" : "我想开始游玩"} ↗</Link></div></div>
              <DocumentationDirectory entries={collection.articles.map((item) => ({ href: `/${locale}/${area}/${item.slug}`, title: item.title, description: item.description, status: documentationStatus[item.status] }))} />
            </>
          )}
          <div className="docs-feedback"><span>文档有误或需要帮助？</span><Link href={`/${locale}/support`}>通过现有支持入口反馈 ↗</Link></div>
        </main>
      </div>
      <SiteFooter locale={locale} copy={copy} gameUrl={gameUrl} explorerUrl={explorerUrl} currentPath={path} />
    </div>
  );
}
