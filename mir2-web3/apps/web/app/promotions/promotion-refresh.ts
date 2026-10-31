export const PROMOTION_REFRESH_INTERVAL_MS = 10_000;

export type PromotionRefreshOptions<Handle = ReturnType<typeof setTimeout>> = {
  refresh: (signal: AbortSignal) => Promise<void>;
  canRefresh: () => boolean;
  schedule?: (callback: () => void, delayMs: number) => Handle;
  cancel?: (handle: Handle) => void;
  intervalMs?: number;
};

/** Poll reads only; the caller owns eligibility and visible error handling. */
export function createPromotionRefresh<Handle = ReturnType<typeof setTimeout>>(
  options: PromotionRefreshOptions<Handle>,
): { request: () => void; stop: () => void } {
  const intervalMs = typeof options.intervalMs === "number"
    && Number.isFinite(options.intervalMs) && options.intervalMs > 0
    ? options.intervalMs : PROMOTION_REFRESH_INTERVAL_MS;
  const schedule = options.schedule ?? ((callback: () => void, delayMs: number) =>
    globalThis.setTimeout(callback, delayMs) as Handle);
  const cancel = options.cancel ?? ((handle: Handle) =>
    globalThis.clearTimeout(handle as ReturnType<typeof setTimeout>));
  let stopped = false;
  let active: AbortController | null = null;
  let timer: { handle?: Handle } | null = null;

  function clearTimer() {
    const previous = timer;
    timer = null;
    if (previous) {
      try { cancel(previous.handle as Handle); } catch { /* Disposal stays best effort. */ }
    }
  }

  function arm() {
    if (stopped || active || timer) return;
    const ticket: { handle?: Handle } = {};
    timer = ticket;
    try {
      ticket.handle = schedule(() => {
        // A cancelled callback already queued by the host cannot start another read.
        if (stopped || timer !== ticket) return;
        timer = null;
        request();
      }, intervalMs);
    } catch {
      if (timer === ticket) timer = null;
    }
  }

  function finish(controller: AbortController) {
    if (active === controller) active = null;
    arm();
  }

  function request() {
    if (stopped || active) return;
    clearTimer();
    let allowed = false;
    try { allowed = options.canRefresh(); } catch { /* Retry eligibility next interval. */ }
    if (stopped) return;
    if (!allowed) { arm(); return; }

    const controller = new AbortController();
    active = controller;
    let pending: Promise<void>;
    try { pending = options.refresh(controller.signal); }
    catch { finish(controller); return; }
    // Handle both outcomes directly: an ignored Promise.finally() can create an
    // unhandled rejected promise even when the original request was handled.
    void Promise.resolve(pending).then(
      () => finish(controller),
      () => finish(controller),
    );
  }

  function stop() {
    if (stopped) return;
    stopped = true;
    clearTimer();
    active?.abort();
  }

  arm();
  return { request, stop };
}
