export const SCENE_ASSET_STALLED_RETRY_DELAY_MS = 30_000;
export const SCENE_ASSET_MAX_STALLED_RETRIES = 1;

const LEGACY_RETRY_QUERY_KEYS = ["mir2ImgRetry", "mir2ImgRetryTs"] as const;

export type SceneAssetStalledRetryDecision = "wait" | "retry" | "fail" | "skip";

export function stableSceneAssetUrl(url: string, pageUrl: string): string | null {
  try {
    const parsed = new URL(url, pageUrl);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return null;
    }

    for (const key of LEGACY_RETRY_QUERY_KEYS) {
      parsed.searchParams.delete(key);
    }

    const pageOrigin = new URL(pageUrl).origin;
    return parsed.origin === pageOrigin
      ? `${parsed.pathname}${parsed.search}${parsed.hash}`
      : parsed.toString();
  } catch {
    return null;
  }
}

export function buildSceneAssetCandidateUrls({
  url,
  pageUrl,
  remoteAssetBaseUrls,
  isRemoteBackedPath,
}: {
  url: string;
  pageUrl: string;
  remoteAssetBaseUrls: string[];
  isRemoteBackedPath: (path: string) => boolean;
}): string[] {
  const candidates: string[] = [];
  const add = (candidate: string | null) => {
    if (candidate && !candidates.includes(candidate)) {
      candidates.push(candidate);
    }
  };
  const primary = stableSceneAssetUrl(url, pageUrl);
  add(primary);

  if (!primary) {
    return candidates;
  }

  try {
    const parsed = new URL(primary, pageUrl);
    if (!isRemoteBackedPath(parsed.pathname)) {
      return candidates;
    }

    for (const baseUrl of remoteAssetBaseUrls) {
      const normalizedBase = baseUrl.replace(/\/+$/, "");
      if (!normalizedBase) continue;
      const remoteUrl = new URL(`${normalizedBase}/${parsed.pathname.replace(/^\/+/, "")}`);
      remoteUrl.search = parsed.search;
      remoteUrl.hash = parsed.hash;
      if (remoteUrl.origin === parsed.origin && remoteUrl.pathname === parsed.pathname) {
        continue;
      }
      add(stableSceneAssetUrl(remoteUrl.toString(), pageUrl));
    }
  } catch {
    return candidates;
  }

  return candidates;
}

export function sceneAssetRetryUrl(candidates: string[], retryOrdinal: number): string | null {
  if (!candidates.length) return null;
  const index = (Math.max(1, Math.floor(retryOrdinal)) - 1) % candidates.length;
  return candidates[index] ?? candidates[0] ?? null;
}

export function sceneAssetStalledRetryDecision({
  elapsedMs,
  retryCount,
  loadState,
}: {
  elapsedMs: number;
  retryCount: number;
  loadState?: string;
}): SceneAssetStalledRetryDecision {
  if (loadState === "retrying" || loadState === "true") {
    return "skip";
  }
  if (elapsedMs < SCENE_ASSET_STALLED_RETRY_DELAY_MS) {
    return "wait";
  }
  return retryCount < SCENE_ASSET_MAX_STALLED_RETRIES ? "retry" : "fail";
}
