type PinnedOutcome<P> = { kind: "pinned"; body: P } | { kind: "unavailable" };
type RouteOutcome<R> = { kind: "route"; result: R; usable: boolean } | { kind: "error"; error: unknown };

/** Keep a verified static scene eligible while its ordinary route is loading. */
export async function chooseSceneBootstrap<P, R>({
  pinned, signal: owner, acceptPinned, loadRoute, acceptRoute, graceMs = 2_500,
}: {
  pinned: Promise<P | null> | null;
  signal: AbortSignal;
  acceptPinned: (body: P) => boolean;
  loadRoute: (signal: AbortSignal) => Promise<R>;
  acceptRoute: (result: R) => boolean;
  graceMs?: number;
}): Promise<{ kind: "pinned"; body: P } | { kind: "route"; result: R }> {
  owner.throwIfAborted();
  if (!pinned) return { kind: "route", result: await loadRoute(owner) };
  const routeController = new AbortController();
  const routeSignal = AbortSignal.any([owner, routeController.signal]);
  let onAbort: (() => void) | undefined;
  let graceTimer: ReturnType<typeof setTimeout> | undefined;
  const aborted = new Promise<never>((_, reject) => {
    onAbort = () => reject(owner.reason);
    owner.addEventListener("abort", onAbort, { once: true });
  });
  const pending: Promise<PinnedOutcome<P>> = pinned
    ? pinned.then(body => body && !owner.aborted && acceptPinned(body)
      ? { kind: "pinned" as const, body } : { kind: "unavailable" as const })
      .catch(() => ({ kind: "unavailable" as const }))
    : Promise.resolve({ kind: "unavailable" as const });
  const use = (outcome: RouteOutcome<R>) => {
    owner.throwIfAborted();
    if (outcome.kind === "error") throw outcome.error;
    return { kind: "route" as const, result: outcome.result };
  };
  try {
    const warm = await Promise.race([pending, aborted, new Promise<null>(resolve => {
      graceTimer = setTimeout(() => resolve(null), graceMs);
    })]);
    clearTimeout(graceTimer);
    owner.throwIfAborted();
    if (warm?.kind === "pinned") return warm;
    const route: Promise<RouteOutcome<R>> = Promise.resolve().then(() => {
      routeSignal.throwIfAborted();
      return loadRoute(routeSignal);
    }).then(result => ({ kind: "route" as const, result, usable: acceptRoute(result) }))
      .catch(error => ({ kind: "error" as const, error }));
    let first = await Promise.race([pending, route, aborted]);
    if (first.kind === "unavailable") first = await Promise.race([route, aborted]);
    owner.throwIfAborted();
    if (first.kind === "pinned") {
      routeController.abort("verified-static-scene-ready");
      return first;
    }
    if (first.kind === "route" && first.usable) return use(first);
    const late = await Promise.race([pending, aborted]);
    owner.throwIfAborted();
    return late.kind === "pinned" ? late : use(first);
  } finally {
    clearTimeout(graceTimer);
    if (onAbort) owner.removeEventListener("abort", onAbort);
  }
}
