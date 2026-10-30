import Image from "next/image";
import Link from "next/link";
import type { Locale } from "@/lib/site-copy";
import type { ProductCopy } from "@/lib/product-copy";

const proofImages = [
  { src: "/gameplay/character-select.png", position: "50% 45%" },
  { src: "/gameplay/shared-world.png", position: "50% 51%" },
  { src: "/gameplay/skills-and-inventory.png", position: "50% 51%" },
] as const;

export function GameplayProof({ locale, copy }: { locale: Locale; copy: ProductCopy["proof"] }) {
  return (
    <section className="gameplay-proof section" id="gameplay-proof">
      <div className="section-heading proof-heading" data-reveal>
        <div className="eyebrow"><span />{copy.eyebrow}</div>
        <h2>{copy.title.split("\n").map((line) => <span key={line}>{line}</span>)}</h2>
        <p>{copy.copy}</p>
      </div>
      <div className="proof-grid">
        {copy.items.map((item, index) => (
          <article className={`proof-card proof-card-${index + 1}`} key={item.title} data-reveal>
            <div className="proof-media">
              <Image src={proofImages[index].src} alt={`${item.title} · ${copy.verified}`} fill sizes="(max-width: 720px) 100vw, 42vw" style={{ objectPosition: proofImages[index].position }} />
              <span>{copy.verified}</span>
            </div>
            <div className="proof-copy"><small>{item.eyebrow}</small><h3>{item.title}</h3><p>{item.copy}</p></div>
          </article>
        ))}
      </div>
      <Link className="button button-primary proof-action" href={`/${locale}/play`} data-track="gameplay_proof_play"><span>{copy.action}</span><i>↗</i></Link>
    </section>
  );
}
