const MIR2_REMOTE_ASSET_BASE_URL = (
  process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? ""
).trim().replace(/\/+$/, "");

// Same-origin public/ asset roots. next.config.ts `rewrites()` already maps any
// missing file under these prefixes through /api/r2-proxy to the R2 CDN release,
// so returning the local path is safe even when the file is absent locally:
// served directly when present, proxied from R2 when not.
const REMOTE_BACKED_PREFIXES = [
  "/original-map/",
  "/original-ui/",
  "/generated/",
  "/bevy-entity-atlases/",
  "/Sound/",
];

export function originalAssetPath(path: string) {
  if (path.startsWith("/api/remote-asset/")) {
    return path;
  }
  if (/^(?:https?:)?\/\//i.test(path) || /^(?:data|blob):/i.test(path)) {
    return path;
  }
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;

  // When the file lives under a rewrite-backed prefix, return the local path
  // directly. Serving locally is instant for files we already ship, and the
  // existing fallback rewrite transparently proxies genuinely-missing files from
  // R2. Avoiding the /api/remote-asset wrapper keeps per-frame <img> src updates
  // from being aborted mid-flight (ERR_ABORTED), which previously made DOM-mode
  // entity sprites flicker. Files outside these prefixes (e.g. /original-effects)
  // have no same-origin rewrite, so keep proxying them when an R2 base exists.
  if (REMOTE_BACKED_PREFIXES.some((prefix) => normalizedPath.startsWith(prefix))) {
    return normalizedPath;
  }
  return MIR2_REMOTE_ASSET_BASE_URL
    ? `/api/remote-asset${normalizedPath}`
    : normalizedPath;
}

