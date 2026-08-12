const MIR2_REMOTE_ASSET_BASE_URL = (
  process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? ""
).trim().replace(/\/+$/, "");

// These actor libraries are intentionally retained in the Vercel output by
// scripts/prune-vercel-output-assets.mjs. Keep their same-origin URL as the primary
// candidate so a stale/incomplete R2 release cannot shadow newly exported animation
// frames. The scene retry policy will still try configured R2 origins if same-origin
// genuinely fails.
const SAME_ORIGIN_ACTOR_LIBRARY_ROOTS = new Set([
  "Monster",
  "CArmour",
  "CHair",
  "CWeapon",
  "AArmour",
  "AHair",
  "AWeapon",
  "ARArmour",
  "ARHair",
  "ARWeapon",
  "NPC",
]);

export function originalAssetPath(path: string) {
  if (path.startsWith("/api/remote-asset/")) {
    return path;
  }
  if (/^(?:https?:)?\/\//i.test(path) || /^(?:data|blob):/i.test(path)) {
    return path;
  }
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  const originalUiLibraryRoot = normalizedPath.match(/^\/original-ui\/([^/]+)(?:\/|$)/u)?.[1];
  if (originalUiLibraryRoot && SAME_ORIGIN_ACTOR_LIBRARY_ROOTS.has(originalUiLibraryRoot)) {
    return normalizedPath;
  }
  return MIR2_REMOTE_ASSET_BASE_URL
    ? `/api/remote-asset${normalizedPath}`
    : normalizedPath;
}
