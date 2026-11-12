import { fetchPrewarmAsset } from "./asset-prewarm-request";

// The six families used by Native QuestDisplayLanguage::font_asset. Small
// fallback families go first; the two CJK bodies share the same Worker fills
// as CSS/Bevy readers. No alternative font or locale is substituted.
export const BEVY_RUNTIME_FONT_PATHS = [
  "/original-ui/fonts/NotoSans-Regular.ttf",
  "/original-ui/fonts/NotoSansDevanagari-Regular.ttf",
  "/original-ui/fonts/NotoSansThai-Regular.ttf",
  "/original-ui/fonts/NotoSansArabic-Regular.ttf",
  "/original-ui/fonts/NotoSansCJKsc-Regular.otf",
  "/original-ui/fonts/NotoSansTC.ttf",
] as const;

/** Warm complete bodies while the renderer downloads; never gate its boot. */
export async function prewarmBevyRuntimeFonts({
  signal,
  fetchImpl,
}: { signal?: AbortSignal; fetchImpl?: typeof fetch } = {}) {
  const completed: string[] = [];
  const failed: Array<{ path: string; errorName: string }> = [];
  let next = 0;
  let bytes = 0;
  const consume = async () => {
    while (!signal?.aborted) {
      const path = BEVY_RUNTIME_FONT_PATHS[next++];
      if (!path) break;
      try {
        const result = await fetchPrewarmAsset(path, {
          signal, fetchImpl,
          // The Worker can finish a shared font cache fill before returning
          // headers. Keep its foreground reader alive within the existing
          // runtime download budget; progressing bodies keep the idle policy.
          headerTimeoutMs: 120_000,
        });
        if (signal?.aborted) break;
        if (!result.response.ok || !result.body) {
          failed.push({ path, errorName: `HTTP${result.response.status}` });
          continue;
        }
        bytes += result.body.byteLength;
        completed.push(path);
      } catch (error) {
        if (signal?.aborted) break;
        failed.push({ path, errorName: error instanceof Error ? error.name : "Error" });
      }
    }
  };
  // Bounded background work starts only after the first playable frame and
  // renderer policy/support checks. It cannot consume six image lanes at once.
  await Promise.all([consume(), consume()]);
  return { requested: BEVY_RUNTIME_FONT_PATHS.length, completed, failed, aborted: signal?.aborted === true, bytes };
}
