"use client";

import Link from "next/link";
import { useState } from "react";

type DirectoryEntry = { href: string; title: string; description: string; status: string };
type DirectoryGroup = { id: string; title: string; entries: DirectoryEntry[] };

export function DocumentationDirectory({ groups }: { groups: DirectoryGroup[] }) {
  const [query, setQuery] = useState("");
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const matches = groups.map((group) => ({ ...group, entries: group.entries.filter((entry) => `${entry.title} ${entry.description}`.toLocaleLowerCase().includes(normalizedQuery)) })).filter((group) => group.entries.length > 0);
  const total = groups.reduce((count, group) => count + group.entries.length, 0);
  const matchCount = matches.reduce((count, group) => count + group.entries.length, 0);
  return (
    <section className="docs-directory" aria-labelledby="directory-heading">
      <div className="docs-directory-heading">
        <h2 id="directory-heading">按需要查阅</h2>
        <label className="docs-filter"><span>查找章节</span><input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="输入关键词" /></label>
      </div>
      <p className="docs-result-count" role="status">{normalizedQuery ? `找到 ${matchCount} 篇文档` : `${groups.length} 个主题 · ${total} 篇文档`}</p>
      {matches.map((group) => (
        <section className="docs-directory-group" key={group.id} aria-labelledby={`directory-${group.id}`}>
          <h3 id={`directory-${group.id}`}>{group.title}</h3>
          <ul className="docs-directory-list">{group.entries.map((entry) => (
            <li key={entry.href}><Link className="docs-directory-row" href={entry.href}>
              <div><h4>{entry.title}</h4><p>{entry.description}</p></div>
              <small>{entry.status}</small><span aria-hidden="true">↗</span>
            </Link></li>
          ))}</ul>
        </section>
      ))}
      {matchCount === 0 ? <p className="docs-empty">没有匹配的章节。试试“登录”“月卡”或“接口”，也可以清空关键词浏览目录。</p> : null}
    </section>
  );
}
