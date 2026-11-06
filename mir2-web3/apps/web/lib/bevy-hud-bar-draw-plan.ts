import type { BevyQuestUiPresentation, BevyQuestUiSnapshot } from "./bevy-quest-ui";

export type HudBarName = "experience" | "weight";
export type HudBarRect = { left: number; top: number; width: number; height: number };
export type HudBarPlanCommit = {
  lifetime: string; experienceToken: string; experienceSequence: number;
  weightToken: string; weightSequence: number;
};
export type HudBarSpritePlan = {
  token: string; sequence: number; state: "draw" | "known-empty" | "withdrawn";
  current: number | null; maximum: number | null; slot: HudBarRect | null;
  image: string | null; source: HudBarRect | null; destination: HudBarRect | null;
};
export type HudBarDrawPlan = {
  version: 1; generation: number; revision: number; lifetime: string;
  experience: HudBarSpritePlan | null; weight: HudBarSpritePlan | null;
};
export type CurrentHudBarPlans = { lifetime: string; experience: HudBarSpritePlan | null; weight: HudBarSpritePlan | null };
export type HudBarPlanRuntime = {
  getMir2HudBarDrawPlanVersion?: () => number;
  getMir2HudBarDrawPlan?: () => string;
};

const paths = {
  experience: ["original-ui/Prguse/8.png"],
  weight: ["original-ui/Prguse/76.png", "original-ui/UI_32bit/473.png", "original-ui/UI_32bit/472.png"],
} as const;
const nativeSize = { experience: { width: 1004, height: 8 }, weight: { width: 76, height: 12 } } as const;
const opaque = (value: unknown): value is string => typeof value === "string" && value.length > 0
  && value.length <= 128 && /^[\x21-\x7e]+$/.test(value);
const sequence = (value: unknown): value is number => Number.isSafeInteger(value) && (value as number) >= 0;
const near = (a: number, b: number) => Number.isFinite(a) && Number.isFinite(b) && Math.abs(a - b) <= 0.75;
const rect = (value: unknown, allowEmpty = false): value is HudBarRect => {
  if (!value || typeof value !== "object") return false;
  const r = value as HudBarRect;
  return [r.left, r.top, r.width, r.height].every(Number.isFinite)
    && r.left >= 0 && r.top >= 0 && r.width >= 0 && r.height > 0
    && (allowEmpty || r.width > 0) && r.width <= 16384 && r.height <= 16384;
};
const sameRect = (a: HudBarRect, b: HudBarRect) => near(a.left, b.left) && near(a.top, b.top)
  && near(a.width, b.width) && near(a.height, b.height);

/** The callable version and getter are one exact capability, never a truthy status hint. */
export function supportsHudBarDrawPlan(runtime: HudBarPlanRuntime | null): boolean {
  try { return runtime?.getMir2HudBarDrawPlanVersion?.() === 1 && typeof runtime.getMir2HudBarDrawPlan === "function"; }
  catch { return false; }
}

export type HudBarVisualClock = { lifetime: string; experienceKey: string; weightKey: string;
  experienceSequence: number; weightSequence: number };
export function createHudBarVisualClock(): HudBarVisualClock {
  return { lifetime: "", experienceKey: "", weightKey: "", experienceSequence: 0, weightSequence: 0 };
}

/** Stable r/g/p identity, independent of a bar plan or temporary draw eligibility. */
export function hudBarCommonIdentity(runtimeLifetime: number, generation: number,
  playerObjectId: string | null): string | null {
  if (!sequence(runtimeLifetime) || !sequence(generation) || generation === 0
    || !playerObjectId || !/^\d{1,40}$/.test(playerObjectId)) return null;
  const lifetime = `r${runtimeLifetime}-g${generation}-p${playerObjectId}`;
  return opaque(lifetime) ? lifetime : null;
}

/** Only the bar's own inputs advance its token; movement and sibling changes do not. */
export function nextHudBarPlanCommit(clock: HudBarVisualClock,
  runtimeLifetime: number, playerObjectId: string | null, snapshot: Pick<BevyQuestUiSnapshot,
    "generation" | "inGame" | "hostVisible" | "presentation" | "dialog" | "player" | "experienceBarSlot" | "weightBarSlot">,
): HudBarPlanCommit | null {
  const lifetime = hudBarCommonIdentity(runtimeLifetime, snapshot.generation, playerObjectId);
  if (!lifetime) return null;
  if (clock.lifetime !== lifetime) {
    clock.lifetime = lifetime; clock.experienceKey = ""; clock.weightKey = "";
    clock.experienceSequence = 0; clock.weightSequence = 0;
  }
  const common = { inGame: snapshot.inGame, hostVisible: snapshot.hostVisible,
    presentation: snapshot.presentation, dialogOpen: snapshot.dialog.isOpen, dialogInput: snapshot.dialog.hasInput };
  const experienceKey = JSON.stringify({ ...common, current: snapshot.player.experience,
    maximum: snapshot.player.maxExperience, slot: snapshot.experienceBarSlot });
  const weightKey = JSON.stringify({ ...common, current: snapshot.player.currentWeight,
    maximum: snapshot.player.maxWeight, slot: snapshot.weightBarSlot });
  if (experienceKey !== clock.experienceKey) {
    if (clock.experienceSequence >= Number.MAX_SAFE_INTEGER) return null;
    clock.experienceKey = experienceKey; clock.experienceSequence++;
  }
  if (weightKey !== clock.weightKey) {
    if (clock.weightSequence >= Number.MAX_SAFE_INTEGER) return null;
    clock.weightKey = weightKey; clock.weightSequence++;
  }
  return { lifetime, experienceToken: `e-${clock.experienceSequence}`, experienceSequence: clock.experienceSequence,
    weightToken: `w-${clock.weightSequence}`, weightSequence: clock.weightSequence };
}

function parseBar(name: HudBarName, value: unknown): HudBarSpritePlan | null {
  if (!value || typeof value !== "object") return null;
  const bar = value as HudBarSpritePlan;
  if (!opaque(bar.token) || !sequence(bar.sequence)
    || !["draw", "known-empty", "withdrawn"].includes(bar.state)) return null;
  if (bar.state === "withdrawn") return bar.current === null && bar.maximum === null
    && bar.slot === null && bar.image === null && bar.source === null && bar.destination === null ? bar : null;
  const size = nativeSize[name];
  if (!Number.isSafeInteger(bar.current) || !Number.isSafeInteger(bar.maximum)
    || (bar.current as number) < 0 || (bar.maximum as number) <= 0
    || (name === "weight" && ((bar.current as number) > 65535 || (bar.maximum as number) > 65535))
    || !rect(bar.slot) || !rect(bar.source, true) || !rect(bar.destination, true)
    || !near(bar.source.left, 0) || !near(bar.source.top, 0)
    || !near(bar.source.height, size.height) || bar.source.width > size.width
    || bar.source.left + bar.source.width > size.width
    || !near(bar.destination.left, bar.slot.left) || !near(bar.destination.top, bar.slot.top)
    || !near(bar.destination.height, bar.slot.height)
    || bar.destination.width > bar.slot.width + 0.75) return null;
  if (bar.state === "known-empty") return bar.source.width === 0 && bar.destination.width === 0 && bar.image === null ? bar : null;
  return bar.source.width > 0 && bar.destination.width > 0 && paths[name].some(path => path === bar.image) ? bar : null;
}

export function readHudBarDrawPlan(runtime: HudBarPlanRuntime | null): HudBarDrawPlan | null {
  if (!supportsHudBarDrawPlan(runtime)) return null;
  try {
    const raw = runtime!.getMir2HudBarDrawPlan!();
    if (typeof raw !== "string" || raw.length > 2048) return null;
    const plan = JSON.parse(raw) as HudBarDrawPlan;
    if (!plan || plan.version !== 1 || !sequence(plan.generation) || plan.generation === 0
      || !sequence(plan.revision) || !opaque(plan.lifetime)) return null;
    return { ...plan, experience: parseBar("experience", plan.experience), weight: parseBar("weight", plan.weight) };
  } catch { return null; }
}

/** An older applied revision remains current when this bar's exact visual identity remains current. */
export function currentHudBarPlans(plan: HudBarDrawPlan | null, snapshot: BevyQuestUiSnapshot | null,
  hostRevision: number, heartbeatFresh: boolean): CurrentHudBarPlans | null {
  if (!plan || !snapshot || !snapshot.hudBarPlan || !heartbeatFresh
    || plan.generation !== snapshot.generation || plan.revision > snapshot.revision
    || plan.revision > hostRevision || plan.lifetime !== snapshot.hudBarPlan.lifetime
    || !snapshot.inGame || !snapshot.hostVisible || !snapshot.presentation) return null;
  const bar = (name: HudBarName): HudBarSpritePlan | null => {
    const value = plan[name];
    const commit = snapshot.hudBarPlan!;
    const token = name === "experience" ? commit.experienceToken : commit.weightToken;
    const seq = name === "experience" ? commit.experienceSequence : commit.weightSequence;
    const slot = name === "experience" ? snapshot.experienceBarSlot : snapshot.weightBarSlot;
    const current = name === "experience" ? snapshot.player.experience : snapshot.player.currentWeight;
    const maximum = name === "experience" ? snapshot.player.maxExperience : snapshot.player.maxWeight;
    if (!value || value.state === "withdrawn" || value.token !== token || value.sequence !== seq
      || value.current !== current || value.maximum !== maximum || !value.slot || !slot
      || !sameRect(value.slot, slot) || slot.left + slot.width > snapshot.presentation.logicalWidth + 0.75
      || slot.top + slot.height > snapshot.presentation.logicalHeight + 0.75) return null;
    return value;
  };
  return { lifetime: plan.lifetime, experience: bar("experience"), weight: bar("weight") };
}

type ImageHandle = CanvasImageSource & { close?: () => void };
type AssetEntry = { controller: AbortController; state: "loading" | "ready" | "failed";
  image: ImageHandle | null };
export type HudBarCanvasInput = { canvas: HTMLCanvasElement | null; anchor: Pick<HTMLElement, "getAttribute"> | null;
  sourceGeometry?: boolean;
  current: number | null | undefined; maximum: number | null | undefined;
  slot: HudBarRect | null; readSlot?: () => HudBarRect | null; plan: HudBarSpritePlan | null };
export type HudBarCanvasUpdate = { lifetime: string | null; presentation: BevyQuestUiPresentation | null;
  experience: HudBarCanvasInput; weight: HudBarCanvasInput;
  readLivePlans: () => CurrentHudBarPlans | null };

const defaultLoad = async (path: string, signal: AbortSignal): Promise<ImageHandle> => {
  const response = await fetch(`/${path}`, { signal, cache: "force-cache" });
  if (!response.ok) throw new Error(`HUD image ${response.status}`);
  return createImageBitmap(await response.blob());
};

/** Owns only two native-size passive canvases and four generation-scoped asset attempts. */
export class HudBarCanvasController {
  private lifetime: string | null = null;
  private cache = new Map<string, AssetEntry>();
  private current: HudBarCanvasUpdate | null = null;
  private owners = { experience: false, weight: false };
  private stamps = { experience: "", weight: "" };
  private retiredStamps = { experience: "", weight: "" };
  private paintedKeys = { experience: "", weight: "" };
  constructor(private readonly onOwner: (name: HudBarName, owns: boolean) => void,
    private readonly loadImage: (path: string, signal: AbortSignal) => Promise<ImageHandle> = defaultLoad) {}

  update(input: HudBarCanvasUpdate) {
    if (this.lifetime !== input.lifetime) {
      this.releaseImages(); this.lifetime = input.lifetime;
      this.clear("experience", this.current?.experience.canvas ?? input.experience.canvas);
      this.clear("weight", this.current?.weight.canvas ?? input.weight.canvas);
      this.stamps.experience = ""; this.stamps.weight = "";
      this.retiredStamps.experience = ""; this.retiredStamps.weight = "";
    }
    this.current = input;
    this.render("experience", input.experience, input.presentation);
    this.render("weight", input.weight, input.presentation);
  }

  private clear(name: HudBarName, canvas: HTMLCanvasElement | null) {
    this.paintedKeys[name] = "";
    if (canvas) {
      canvas.style.visibility = "hidden";
      try {
        const size = nativeSize[name];
        if (canvas.width !== size.width) canvas.width = size.width;
        if (canvas.height !== size.height) canvas.height = size.height;
        const ctx = canvas.getContext("2d");
        if (ctx) {
          ctx.globalAlpha = 1; ctx.globalCompositeOperation = "source-over";
          ctx.imageSmoothingEnabled = false; ctx.clearRect(0, 0, size.width, size.height);
        }
      } catch { /* The output remains synchronously hidden and DOM resumes. */ }
    }
    if (this.owners[name]) { this.owners[name] = false; this.onOwner(name, false); }
  }

  private render(name: HudBarName, input: HudBarCanvasInput, presentation: BevyQuestUiPresentation | null) {
    const { canvas, anchor, plan } = input;
    const stamp = `${this.lifetime ?? ""}:${plan?.token ?? ""}:${plan?.sequence ?? ""}`;
    if (this.retiredStamps[name] && this.retiredStamps[name] === stamp) {
      this.clear(name, canvas); return;
    }
    if (plan && this.retiredStamps[name] !== stamp) this.retiredStamps[name] = "";
    if (stamp !== this.stamps[name]) {
      this.clear(name, canvas); this.stamps[name] = stamp;
    }
    const pair = name === "experience" ? ["data-experience", "data-max-experience"]
      : ["data-current-weight", "data-max-weight"];
    const rawCurrent = anchor?.getAttribute(pair[0]);
    const rawMaximum = anchor?.getAttribute(pair[1]);
    let measuredSlot: HudBarRect | null;
    try { measuredSlot = input.readSlot ? input.readSlot() : input.slot; }
    catch { this.clear(name, canvas); return; }
    const liveMatches = input.sourceGeometry === true || rawCurrent !== null && rawCurrent !== undefined && rawMaximum !== null && rawMaximum !== undefined
      && /^\d+$/.test(rawCurrent) && /^\d+$/.test(rawMaximum)
      && Number(rawCurrent) === input.current && Number(rawMaximum) === input.maximum;
    if (!canvas || !this.lifetime || !presentation || !plan || !liveMatches || !plan.slot || !measuredSlot
      || plan.current !== input.current || plan.maximum !== input.maximum
      || !rect(plan.slot) || !sameRect(plan.slot, measuredSlot)
      || plan.slot.left + plan.slot.width > presentation.logicalWidth + 0.75
      || plan.slot.top + plan.slot.height > presentation.logicalHeight + 0.75
      || !this.liveOwns(name, input)) {
      this.clear(name, canvas); return;
    }
    const size = nativeSize[name];
    if (canvas.width !== size.width || canvas.height !== size.height) this.clear(name, canvas);
    const paintKey = JSON.stringify({ stamp, state: plan.state, slot: plan.slot, measuredSlot,
      image: plan.image, source: plan.source, destination: plan.destination,
      cssScale: presentation.stageCssScale });
    if (this.owners[name] && this.paintedKeys[name] === paintKey && canvas.style.visibility === "visible") return;
    let ctx: CanvasRenderingContext2D | null;
    try { ctx = canvas.getContext("2d"); }
    catch { this.clear(name, canvas); return; }
    if (!ctx) { this.clear(name, canvas); return; }
    // Parent stage owns the one CSS scale. Source crop remains in native pixels.
    canvas.style.left = `${plan.slot.left}px`; canvas.style.top = `${plan.slot.top}px`;
    canvas.style.width = `${plan.slot.width}px`; canvas.style.height = `${plan.slot.height}px`;
    try {
      ctx.globalAlpha = 1; ctx.globalCompositeOperation = "source-over";
      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, size.width, size.height);
      if (plan.state === "known-empty") {
        if (this.liveOwns(name, input)) this.take(name, canvas, paintKey);
        else this.clear(name, canvas);
        return;
      }
      if (plan.state !== "draw" || !plan.image || !plan.source || !plan.destination) {
        this.clear(name, canvas); return;
      }
      const image = this.asset(name, plan.image);
      if (!image) { this.clear(name, canvas); return; }
      const dx = (plan.destination.left - plan.slot.left) * size.width / plan.slot.width;
      const dy = (plan.destination.top - plan.slot.top) * size.height / plan.slot.height;
      const dw = plan.destination.width * size.width / plan.slot.width;
      const dh = plan.destination.height * size.height / plan.slot.height;
      if (![dx, dy, dw, dh].every(Number.isFinite) || dw <= 0 || dh <= 0) { this.clear(name, canvas); return; }
      ctx.drawImage(image, plan.source.left, plan.source.top, plan.source.width, plan.source.height,
        dx, dy, dw, dh);
      if (this.liveOwns(name, input)) this.take(name, canvas, paintKey);
      else this.clear(name, canvas);
    } catch { this.clear(name, canvas); }
  }

  private take(name: HudBarName, canvas: HTMLCanvasElement, paintKey: string) {
    canvas.style.visibility = "visible";
    this.paintedKeys[name] = paintKey;
    if (!this.owners[name]) { this.owners[name] = true; this.onOwner(name, true); }
  }

  private liveOwns(name: HudBarName, input: HudBarCanvasInput): boolean {
    const current = this.current;
    const plan = input.plan;
    if (!current || current[name] !== input || !this.lifetime || !plan) return false;
    let live: CurrentHudBarPlans | null;
    try { live = current.readLivePlans(); }
    catch { return false; }
    const selected = live?.[name];
    const same = (a: HudBarRect | null, b: HudBarRect | null) => a === b || Boolean(a && b
      && a.left === b.left && a.top === b.top && a.width === b.width && a.height === b.height);
    if (live?.lifetime !== this.lifetime || !selected || selected.token !== plan.token
      || selected.sequence !== plan.sequence || selected.state !== plan.state
      || selected.current !== plan.current || selected.maximum !== plan.maximum
      || selected.image !== plan.image || !same(selected.slot, plan.slot)
      || !same(selected.source, plan.source) || !same(selected.destination, plan.destination)) return false;
    const pair = name === "experience" ? ["data-experience", "data-max-experience"]
      : ["data-current-weight", "data-max-weight"];
    try {
      const rawCurrent = input.anchor?.getAttribute(pair[0]);
      const rawMaximum = input.anchor?.getAttribute(pair[1]);
      const slot = input.readSlot ? input.readSlot() : input.slot;
      return (input.sourceGeometry === true || rawCurrent !== null && rawCurrent !== undefined && rawMaximum !== null && rawMaximum !== undefined
        && /^\d+$/.test(rawCurrent) && /^\d+$/.test(rawMaximum)
        && Number(rawCurrent) === input.current && Number(rawMaximum) === input.maximum)
        && Boolean(slot && plan.slot && sameRect(slot, plan.slot));
    } catch { return false; }
  }

  private asset(name: HudBarName, path: string): ImageHandle | null {
    if (!paths[name].some(candidate => candidate === path) || !this.lifetime) return null;
    let entry = this.cache.get(path);
    if (!entry) {
      const controller = new AbortController();
      entry = { controller, state: "loading", image: null };
      this.cache.set(path, entry);
      const expectedLifetime = this.lifetime;
      const expectedToken = this.current?.[name].plan?.token;
      const expectedSequence = this.current?.[name].plan?.sequence;
      const ownedEntry = entry;
      this.loadImage(path, controller.signal).then(image => {
        if (this.lifetime !== expectedLifetime || this.cache.get(path) !== ownedEntry) {
          image.close?.(); return;
        }
        ownedEntry.image = image; ownedEntry.state = "ready";
        const input = this.current;
        if (input?.lifetime === expectedLifetime && input[name].plan?.image === path
          && input[name].plan?.token === expectedToken && input[name].plan?.sequence === expectedSequence) {
          this.render(name, input[name], input.presentation);
        }
      }, () => {
        if (this.lifetime === expectedLifetime && this.cache.get(path) === ownedEntry) {
          ownedEntry.state = "failed";
          const input = this.current;
          if (input?.lifetime === expectedLifetime && input[name].plan?.image === path
            && input[name].plan?.token === expectedToken && input[name].plan?.sequence === expectedSequence) {
            this.clear(name, input[name].canvas);
          }
        }
      });
    }
    return entry.state === "ready" ? entry.image : null;
  }

  private releaseImages() {
    for (const entry of this.cache.values()) { entry.controller.abort(); entry.image?.close?.(); }
    this.cache.clear();
  }

  /** Network/raw-authority edges may withdraw before React's next commit. */
  withdraw(name: HudBarName) {
    const plan = this.current?.[name].plan;
    if (plan) this.retiredStamps[name] = `${this.lifetime ?? ""}:${plan.token}:${plan.sequence}`;
    this.clear(name, this.current?.[name].canvas ?? null);
  }

  dispose() {
    this.releaseImages(); this.lifetime = null;
    this.clear("experience", this.current?.experience.canvas ?? null);
    this.clear("weight", this.current?.weight.canvas ?? null);
    this.current = null; this.stamps.experience = ""; this.stamps.weight = "";
    this.retiredStamps.experience = ""; this.retiredStamps.weight = "";
  }
}
