"use client";

import Link from "next/link";
import { useState } from "react";

type DirectoryEntry = { href: string; title: string; description: string; status: string };

export function DocumentationDirectory({ entries }: { entries: DirectoryEntry[] }) {
  const [query, setQuery] = useState("");
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const matches = entries.filter((entry) => `${entry.title} ${entry.description}`.toLocaleLowerCase().includes(normalizedQuery));
  return (
    <section className="docs-directory" aria-labelledby="directory-heading">
      <div className="docs-directory-heading">
        <h2 id="directory-heading">从这里开始</h2>
        <label className="docs-filter"><span>查找章节</span><input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="输入关键词" /></label>
      </div>
      <p className="docs-result-count" role="status">{normalizedQuery ? `找到 ${matches.length} 篇文档` : `共 ${entries.length} 篇文档 · 按顺序阅读，也可直接选择章节`}</p>
      <div className="docs-card-grid">
        {matches.map((entry, index) => (
          <Link className="docs-card" href={entry.href} key={entry.href}>
            <span className="docs-card-number">{String(index + 1).padStart(2, "0")}<small>{entry.status}</small></span>
            <h3>{entry.title}</h3><p>{entry.description}</p><span className="docs-card-link">阅读文档 <i aria-hidden="true">↗</i></span>
          </Link>
        ))}
      </div>
      {matches.length === 0 ? <p className="docs-empty">没有匹配的章节。试试“登录”“月卡”或“接口”，也可以清空关键词浏览目录。</p> : null}
    </section>
  );
}
