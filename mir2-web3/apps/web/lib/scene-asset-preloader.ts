export type SceneAssetPreloadResult = {
  key: "scene-assets";
  ready: boolean;
  interactionReady: boolean;
  visualReady: boolean;
  status: "ready" | "timeout" | "loading";
  total: number;
  loaded: number;
  failed: number;
  pending: number;
  durationMs: number;
  failedUrls: string[];
};

export type SceneAssetPreloadOptions = {
  allowPartialReady?: boolean;
  minLoaded?: number;
  concurrency?: number;
  resolveCandidates?: (url: string) => string[];
  loadCandidate?: (url: string, timeoutMs: number) => Promise<boolean>;
  now?: () => number;
};

const DEFAULT_MINIMUM_LOADED = 24;
const DEFAULT_CONCURRENCY = 8;

/**
 * Preloads a bounded set of scene images. Once enough images are available for
 * interaction, queued work is abandoned so the first playable scene keeps the
 * network for visible assets instead of launching hundreds of speculative
 * Image requests at once.
 */
export async function preloadSceneAssetUrls(
  inputUrls: string[],
  timeoutMs: number,
  options: SceneAssetPreloadOptions = {},
): Promise<SceneAssetPreloadResult> {
  const urls = Array.from(new Set(inputUrls));
  const now = options.now ?? defaultNow;
  const startedAt = now();
  const minimumLoaded = Math.min(urls.length, options.minLoaded ?? DEFAULT_MINIMUM_LOADED);
  const concurrency = Math.max(1, Math.min(urls.length || 1, Math.floor(options.concurrency ?? DEFAULT_CONCURRENCY)));
  const resolveCandidates = options.resolveCandidates ?? ((url: string) => [url]);
  const loadCandidate = options.loadCandidate ?? preloadSceneImageCandidate;
  let loaded = 0;
  let completed = 0;
  let nextIndex = 0;
  let finished = false;
  const failedUrls: string[] = [];

  return await new Promise<SceneAssetPreloadResult>((resolve) => {
    const build = (timedOut: boolean): SceneAssetPreloadResult => {
      const partialReady =
        options.allowPartialReady === true && minimumLoaded > 0 && loaded >= minimumLoaded;
      const pending = Math.max(0, urls.length - completed);
      const ready = (pending === 0 && failedUrls.length === 0) || partialReady;
      return {
        key: "scene-assets",
        ready,
        interactionReady: ready,
        visualReady: pending === 0 && failedUrls.length === 0,
        status: ready ? "ready" : timedOut ? "timeout" : "loading",
        total: urls.length,
        loaded,
        failed: failedUrls.length,
        pending,
        durationMs: Math.round(now() - startedAt),
        failedUrls: failedUrls.slice(0, 20),
      };
    };
    const finish = (timedOut: boolean) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      resolve(build(timedOut));
    };
    const launchNext = () => {
      if (finished) return;
      if (options.allowPartialReady === true && minimumLoaded > 0 && loaded >= minimumLoaded) {
        finish(false);
        return;
      }
      const index = nextIndex;
      if (index >= urls.length) {
        if (completed >= urls.length) finish(false);
        return;
      }
      nextIndex += 1;
      const url = urls[index];
      const remainingMs = Math.max(1, timeoutMs - (now() - startedAt));
      void preloadSceneImage(url, remainingMs, resolveCandidates, loadCandidate).then((result) => {
        completed += 1;
        if (result.loaded) loaded += 1;
        else failedUrls.push(result.url);
        launchNext();
      });
    };
    const timer = setTimeout(() => finish(true), Math.max(1, timeoutMs));
    if (urls.length === 0) {
      finish(false);
      return;
    }
    for (let worker = 0; worker < concurrency; worker += 1) {
      launchNext();
    }
  });
}

async function preloadSceneImage(
  url: string,
  timeoutMs: number,
  resolveCandidates: (url: string) => string[],
  loadCandidate: (url: string, timeoutMs: number) => Promise<boolean>,
): Promise<{ url: string; loaded: boolean }> {
  const startedAt = defaultNow();
  const candidates = Array.from(new Set(resolveCandidates(url)));

  for (const candidate of candidates) {
    const remainingMs = timeoutMs - (defaultNow() - startedAt);
    if (remainingMs <= 0) break;
    if (await loadCandidate(candidate, Math.max(250, remainingMs))) {
      return { url, loaded: true };
    }
  }

  return { url, loaded: false };
}

function preloadSceneImageCandidate(url: string, timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    const image = new Image();
    let settled = false;
    const timer = window.setTimeout(() => finish(false), timeoutMs);

    const finish = (loaded: boolean) => {
      if (settled) return;
      settled = true;
      window.clearTimeout(timer);
      resolve(loaded);
    };

    image.onload = () => {
      if (typeof image.decode === "function") {
        image
          .decode()
          .then(() => finish(image.naturalWidth > 0))
          .catch(() => finish(image.naturalWidth > 0));
        return;
      }
      finish(image.naturalWidth > 0);
    };
    image.onerror = () => finish(false);
    image.decoding = "async";
    image.src = url;
    if (image.complete) finish(image.naturalWidth > 0);
  });
}

function defaultNow() {
  return typeof performance === "undefined" ? Date.now() : performance.now();
}
