import type { AnchorHTMLAttributes } from "react";
import { isLocale } from "@/lib/atlas-data.mjs";

type AtlasLinkProps = Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> & {
  href: string;
};

const publicOrigin = "https://numeron.obelisk.build";

function publicHref(href: string) {
  const match = href.match(/^\/([^/?#]+)(.*)$/);
  if (!match || !isLocale(match[1])) return href;
  const [, locale, rest] = match;
  const path = !rest || /^[?#]/.test(rest)
    ? `/${locale}/explore${rest}`
    : href;
  return `${publicOrigin}${path}`;
}

export function AtlasLink({ href, ...props }: AtlasLinkProps) {
  // Atlas is proxied under public aliases. Hard navigation keeps its router
  // and prefetch requests from crossing into the marketing application's tree.
  return <a {...props} href={publicHref(href)} />;
}
