export const WALLET_QR_IMAGE_LIFECYCLE = "mir2:wallet-qr-image-lifecycle";
type QrImageLifecycle = { id: number; phase: "ready" | "retired" };
let nextImageId = 0;

/** Notify only after a decoded QR image has had a chance to paint. No approval URL leaves the component. */
export function observeWalletQrImage(image: HTMLImageElement) {
  const id = ++nextImageId;
  let frame: number | undefined, ready = false, retired = false;
  const emit = (phase: QrImageLifecycle["phase"]) => window.dispatchEvent(
    new CustomEvent<QrImageLifecycle>(WALLET_QR_IMAGE_LIFECYCLE, { detail: { id, phase } }),
  );
  const loaded = () => {
    if (retired || ready || frame !== undefined || !image.complete || image.naturalWidth <= 0) return;
    frame = window.requestAnimationFrame(() => {
      // The first frame gives the QR a paint opportunity before network work.
      frame = window.requestAnimationFrame(() => {
        frame = undefined;
        if (retired || !image.complete || image.naturalWidth <= 0) return;
        ready = true;
        emit("ready");
      });
    });
  };
  image.addEventListener("load", loaded);
  loaded();
  return () => {
    if (retired) return;
    retired = true;
    image.removeEventListener("load", loaded);
    if (frame !== undefined) window.cancelAnimationFrame(frame);
    if (ready) emit("retired");
  };
}

type FontResult = { requested: number; completed: string[]; failed: unknown[]; aborted: boolean; bytes: number };
type PreparationOptions = {
  target: Pick<Window, "addEventListener" | "removeEventListener">;
  eligible: () => boolean;
  workerReady: () => boolean;
  readFont: (options: { signal: AbortSignal }) => Promise<FontResult>;
  onState?: (state: "Start" | "Complete" | "Skipped", detail: Record<string, string | number | boolean>) => void;
};

/** Prime the existing shared Worker fill; this foreground reader never owns or cancels other consumers. */
export function prepareFontAfterWalletQrImage({ target, eligible, workerReady, readFont, onState }: PreparationOptions) {
  let current: { id: number; owner: AbortController; timer?: ReturnType<typeof setTimeout>; since: number } | undefined;
  const report = (state: Parameters<NonNullable<PreparationOptions["onState"]>>[0], detail: Record<string, string | number | boolean>) => {
    try { onState?.(state, detail); } catch { /* Metrics cannot affect login or preparation. */ }
  };
  const retire = () => {
    if (!current) return;
    clearTimeout(current.timer);
    current.owner.abort();
    current = undefined;
  };
  const available = (check: () => boolean) => {
    try { return check() === true; } catch { return false; }
  };
  const lifecycle = (event: Event) => {
    const detail = (event as CustomEvent<QrImageLifecycle>).detail;
    if (!detail || !Number.isSafeInteger(detail.id) || detail.id <= 0) return;
    if (detail.phase === "retired") { if (current?.id === detail.id) retire(); return; }
    if (detail.phase !== "ready" || current?.id === detail.id) return;
    retire();
    if (!available(eligible)) return;
    const entry = { id: detail.id, owner: new AbortController(), since: performance.now(), timer: undefined as ReturnType<typeof setTimeout> | undefined };
    current = entry;
    const begin = () => {
      if (current !== entry || entry.owner.signal.aborted) return;
      if (!available(eligible)) { retire(); return; }
      if (!available(workerReady)) {
        if (performance.now() - entry.since >= 8_000) {
          report("Skipped", { reason: "configured-worker-unavailable" }); retire(); return;
        }
        entry.timer = setTimeout(begin, 100);
        return;
      }
      report("Start", { stage: "wallet-qr-visible" });
      void Promise.resolve().then(() => {
        entry.owner.signal.throwIfAborted();
        return readFont({ signal: entry.owner.signal });
      }).then(result => {
        if (current !== entry || entry.owner.signal.aborted) return;
        report("Complete", { requested: result.requested, ok: result.completed.length, failed: result.failed.length, bytes: result.bytes, aborted: result.aborted });
      }).catch(() => { /* Native readers and the shared fill retain their normal recovery. */ });
    };
    begin();
  };
  target.addEventListener(WALLET_QR_IMAGE_LIFECYCLE, lifecycle);
  return () => { target.removeEventListener(WALLET_QR_IMAGE_LIFECYCLE, lifecycle); retire(); };
}
