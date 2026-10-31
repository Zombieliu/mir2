"use client";

import Image from "next/image";
import { useMemo, useState } from "react";
import type { ProductCopy } from "@/lib/product-copy";

const captures = [
  { id: "client-character", category: "systems", title: "Character Select", image: "/gameplay/character-select.png", duration: "CAPTURE 01" },
  { id: "bichon-world", category: "world", title: "Bichon Province", image: "/gameplay/shared-world.png", duration: "CAPTURE 02" },
  { id: "skills-inventory", category: "gameplay", title: "Skills & Inventory", image: "/gameplay/skills-and-inventory.png", duration: "CAPTURE 03" },
] as const;

type Filter = "all" | (typeof captures)[number]["category"];

export function HighlightGallery({ copy }: { copy: ProductCopy["watch"] }) {
  const [filter, setFilter] = useState<Filter>("all");
  const [copied, setCopied] = useState<string | null>(null);
  const visible = useMemo(() => filter === "all" ? captures : captures.filter((capture) => capture.category === filter), [filter]);
  const filters: Array<{ key: Filter; label: string }> = [
    { key: "all", label: copy.all }, { key: "gameplay", label: copy.gameplay }, { key: "systems", label: copy.systems }, { key: "world", label: copy.world },
  ];

  async function share(id: string, title: string) {
    const url = `${window.location.origin}${window.location.pathname}#${id}`;
    if (navigator.share) {
      await navigator.share({ title, url }).catch(() => undefined);
      return;
    }
    await navigator.clipboard.writeText(url);
    setCopied(id);
    window.setTimeout(() => setCopied(null), 1800);
  }

  return (
    <div className="highlight-archive">
      <div className="highlight-filters" role="group" aria-label={copy.archiveTitle}>
        {filters.map((item) => <button key={item.key} type="button" aria-pressed={filter === item.key} onClick={() => setFilter(item.key)}>{item.label}</button>)}
      </div>
      <div className="highlight-proof-grid">
        {visible.map((capture) => (
          <article id={capture.id} key={capture.id} className="highlight-proof-card">
            <a className="highlight-proof-image" href={capture.image} target="_blank" rel="noreferrer" data-track={`highlight_open_${capture.id}`}>
              <Image src={capture.image} alt={`${capture.title} · ${copy.still}`} fill sizes="(max-width: 720px) 100vw, 32vw" />
              <span>{capture.duration}</span><i>↗</i>
            </a>
            <div><small>{copy.still}</small><h3>{capture.title}</h3><a href={capture.image} target="_blank" rel="noreferrer">{copy.footage} ↗</a><button type="button" onClick={() => void share(capture.id, capture.title)}>{copied === capture.id ? copy.copied : copy.share}</button></div>
          </article>
        ))}
      </div>
    </div>
  );
}
