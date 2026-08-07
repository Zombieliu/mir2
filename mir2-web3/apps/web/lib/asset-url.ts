const MIR2_REMOTE_ASSET_BASE_URL = (
  process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? ""
).trim().replace(/\/+$/, "");

export function originalAssetPath(path: string) {
  if (path.startsWith("/api/remote-asset/")) {
    return path;
  }
  if (/^(?:https?:)?\/\//i.test(path) || /^(?:data|blob):/i.test(path)) {
    return path;
  }
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  return MIR2_REMOTE_ASSET_BASE_URL
    ? `/api/remote-asset${normalizedPath}`
    : normalizedPath;
}
