import { SelfSpriteImageError, type DecodedSelfSpriteImage, type SelfSpriteImageSource } from "./self-sprite-image";

export type SelfSpriteLayer = SelfSpriteImageSource & {
  role: string; path: string; x: number; y: number;
  shadowX: number; shadowY: number; maskPath: string | null;
};
export type SelfSpritePose = { key: string; layers: readonly SelfSpriteLayer[]; nameplateTop: number };
export type ResidentSelfSpritePose = SelfSpritePose & {
  images: ReadonlyMap<string, DecodedSelfSpriteImage & { key: string }>;
};
type Entry = { frame: SelfSpriteLayer; image: DecodedSelfSpriteImage };
type Flight = { owner: AbortController; generation: number; local: boolean };
type Failure = { count: number; retryAt: number };

export function selfSpriteImageKey(frame: SelfSpriteImageSource & { role?: string }) {
  // Each simultaneously displayed role owns its decoded element, even when a
  // dual-wielding actor uses the same PNG in two layers.
  return JSON.stringify([frame.source, frame.width, frame.height, frame.role]);
}

/** One self actor; frame changes keep requests alive, actor changes stop them. */
export class SelfSpriteResidency {
  private active = false;
  private generation = 0;
  private target: SelfSpritePose | null = null;
  private warm: readonly SelfSpriteLayer[] = [];
  private current: ResidentSelfSpritePose | null = null;
  private presented = new Set<string>();
  private cache = new Map<string, Entry>();
  private flights = new Map<string, Flight>();
  private failures = new Map<string, Failure>();
  private listeners = new Set<() => void>();
  private retryTimer: ReturnType<typeof setTimeout> | undefined;

  private recordFailure(key: string, failure: Failure) {
    this.failures.delete(key);
    this.failures.set(key, failure);
    // Failure history is an LRU too, including failures of data-URI frames.
    // Keep demanded/in-flight entries while bounding retired frame history.
    const pinned = new Set([...(this.target?.layers.map(selfSpriteImageKey) ?? []),
      ...this.warm.map(selfSpriteImageKey), ...this.flights.keys()]);
    for (const older of this.failures.keys()) {
      if (this.failures.size <= 256) break;
      if (!pinned.has(older)) this.failures.delete(older);
    }
  }

  constructor(
    private load: (frame: SelfSpriteLayer, owner: AbortSignal) => Promise<DecodedSelfSpriteImage>,
    private now = Date.now,
    private limits = { entries: 64, bytes: 16 * 1024 * 1024, network: 4, local: 2 },
  ) {}

  getSnapshot = () => this.current;
  subscribe = (notify: () => void) => { this.listeners.add(notify); return () => { this.listeners.delete(notify); }; };
  start() { this.active = true; this.pump(); }
  stop() {
    this.active = false; this.generation++;
    clearTimeout(this.retryTimer); this.retryTimer = undefined;
    for (const flight of this.flights.values()) flight.owner.abort();
    this.flights.clear();
    for (const entry of this.cache.values()) entry.image.release();
    this.cache.clear(); this.failures.clear(); this.presented.clear();
    this.target = null; this.warm = []; this.publish(null);
  }

  setTarget(target: SelfSpritePose | null, warm: readonly SelfSpriteLayer[] = []) {
    this.target = target;
    this.warm = warm.slice(0, 4);
    if (!target?.layers.some(layer => layer.role === "body")) this.publish(null);
    this.commitReady(); this.pump();
  }

  /** Called after React committed the new URLs; the previous DOM lease can end. */
  presentedPose(pose: ResidentSelfSpritePose | null) {
    this.presented = new Set(pose ? [...pose.images.values()].map(image => image.key) : []);
    this.trim(0, 0);
  }

  invalidate = (key: string, displayUrl: string) => {
    const entry = this.cache.get(key);
    if (!entry || entry.image.url !== displayUrl) return;
    this.cache.delete(key); entry.image.release();
    this.recordFailure(key, { count: 1, retryAt: this.now() + 5_000 });
    if (this.current && [...this.current.images.values()].some(image => image.key === key)) this.publish(null);
    this.pump();
  };

  stats() {
    return { active: this.flights.size, network: [...this.flights.values()].filter(x => !x.local).length,
      local: [...this.flights.values()].filter(x => x.local).length, cached: this.cache.size,
      bytes: [...this.cache.values()].reduce((sum, entry) => sum + entry.image.bytes, 0), failures: this.failures.size };
  }

  private publish(pose: ResidentSelfSpritePose | null) {
    if (pose?.key === this.current?.key && (pose === null) === (this.current === null)) return;
    this.current = pose; for (const notify of this.listeners) notify();
  }

  private commitReady() {
    if (!this.active || !this.target?.layers.some(layer => layer.role === "body")) return;
    const images = new Map<string, DecodedSelfSpriteImage & { key: string }>();
    for (const layer of this.target.layers) {
      const key = selfSpriteImageKey(layer), entry = this.cache.get(key);
      if (!entry) return;
      images.set(layer.role, { ...entry.image, key });
      this.cache.delete(key); this.cache.set(key, entry);
    }
    this.publish({ ...this.target, images });
  }

  private protectedKeys() {
    return new Set([...this.presented, ...(this.current ? [...this.current.images.values()].map(image => image.key) : []),
      ...(this.target?.layers.map(selfSpriteImageKey) ?? [])]);
  }

  private trim(bytes: number, entries: number) {
    const pinned = this.protectedKeys();
    let total = this.stats().bytes;
    for (const [key, entry] of this.cache) {
      if (this.cache.size + entries <= this.limits.entries && total + bytes <= this.limits.bytes) break;
      if (pinned.has(key)) continue;
      this.cache.delete(key); total -= entry.image.bytes; entry.image.release();
    }
    return this.cache.size + entries <= this.limits.entries && total + bytes <= this.limits.bytes;
  }

  private pump() {
    if (!this.active) return;
    clearTimeout(this.retryTimer); this.retryTimer = undefined;
    // At most four startup body frames plus the current composed pose. Never use
    // the renderer's exhaustive combat/direction preloadPaths here.
    const demand = new Map<string, SelfSpriteLayer>();
    for (const layer of [...(this.target?.layers ?? []), ...this.warm]) demand.set(selfSpriteImageKey(layer), layer);
    let nextRetry = Infinity;
    for (const [key, frame] of demand) {
      if (this.cache.has(key) || this.flights.has(key)) continue;
      const failure = this.failures.get(key);
      if (failure && failure.retryAt > this.now()) { nextRetry = Math.min(nextRetry, failure.retryAt); continue; }
      const local = frame.source.startsWith("data:");
      const active = [...this.flights.values()].filter(flight => flight.local === local).length;
      if (active >= (local ? this.limits.local : this.limits.network)) continue;
      const flight = { owner: new AbortController(), generation: this.generation, local };
      this.flights.set(key, flight);
      void this.load(frame, flight.owner.signal).then(image => {
        if (!this.active || flight.generation !== this.generation) { image.release(); return; }
        if (this.trim(image.bytes, 1)) { this.cache.set(key, { frame, image }); this.failures.delete(key); }
        else { image.release(); this.recordFailure(key, { count: 3, retryAt: Infinity }); }
        this.commitReady();
      }, error => {
        if (!this.active || flight.generation !== this.generation) return;
        const count = (this.failures.get(key)?.count ?? 0) + 1;
        this.recordFailure(key, { count, retryAt: count >= 3 || error instanceof SelfSpriteImageError && error.terminal
          ? Infinity : this.now() + count * 5_000 });
      }).finally(() => {
        if (this.flights.get(key) !== flight) return;
        this.flights.delete(key); this.pump();
      });
    }
    if (Number.isFinite(nextRetry)) this.retryTimer = setTimeout(() => this.pump(), Math.max(1, nextRetry - this.now()));
  }
}
