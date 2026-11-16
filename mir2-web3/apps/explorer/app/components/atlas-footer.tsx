import { copy, facts, type Locale } from "@/lib/atlas-data.mjs";
import { getPolicyLinks } from "@/lib/atlas-display.mjs";

export function AtlasFooter({ locale }: { locale: Locale }) {
  return (
    <footer className="atlas-policy-footer">
      <div><strong>NUMERON ATLAS</strong><span>{facts[locale][0]}</span></div>
      <p>{copy[locale].record[4]}</p>
      <span>WORLD 01 / 2026</span>
      <nav aria-label={copy[locale].ui[19]}>
        {getPolicyLinks(locale).map(link => <a key={link.href} href={link.href}>{link.label}</a>)}
      </nav>
    </footer>
  );
}
