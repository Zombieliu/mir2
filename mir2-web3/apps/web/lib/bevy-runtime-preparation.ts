/** A single immutable binary shared by selection prefetch and the game initializer. */
export class BevyRuntimePreparation {
  private readonly controller = new AbortController();
  private readonly promise: Promise<WebAssembly.Module>;
  private readonly headerObservers = new Set<() => void>();
  private headersReceived = false;

  constructor(
    readonly key: string,
    compile: (signal: AbortSignal, onResponseHeaders: () => void) => Promise<WebAssembly.Module>,
  ) {
    this.promise = Promise.resolve().then(() => {
      this.controller.signal.throwIfAborted();
      return compile(this.controller.signal, () => {
        if (this.controller.signal.aborted || this.headersReceived) return;
        this.headersReceived = true;
        for (const observer of [...this.headerObservers]) {
          try { observer(); } catch { /* Diagnostics cannot replace the prepared binary. */ }
        }
      });
    });
    // Selection can finish or fail before a game consumer exists. Keep failures
    // observable by that consumer without an unhandled rejection or a new GET.
    void this.promise.catch(() => {});
  }

  take(key: string, ownerSignal: AbortSignal, onResponseHeaders?: () => void) {
    ownerSignal.throwIfAborted();
    if (key !== this.key) { this.dispose(); return undefined; }
    if (this.controller.signal.aborted) return undefined;
    const retire = () => this.dispose();
    ownerSignal.addEventListener("abort", retire, { once: true });
    if (onResponseHeaders) {
      if (this.headersReceived) {
        try { onResponseHeaders(); } catch { /* Coordination cannot alter ownership. */ }
      } else {
        this.headerObservers.add(onResponseHeaders);
      }
    }
    const settled = () => {
      ownerSignal.removeEventListener("abort", retire);
      if (onResponseHeaders) this.headerObservers.delete(onResponseHeaders);
    };
    void this.promise.then(settled, settled);
    return this.promise;
  }

  dispose() {
    this.headerObservers.clear();
    this.controller.abort();
  }
}
