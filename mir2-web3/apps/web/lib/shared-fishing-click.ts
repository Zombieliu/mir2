/** Data-only adapters for the optional shared Rust fishing-click ABI. */
export type FishingClickCell = Readonly<{ x: number; y: number }>;
export type FishingClickTargetsInput = Readonly<{ origin: FishingClickCell; direction: number }>;
export type FishingClickWalkCandidate = Readonly<{ direction: number; cell: FishingClickCell }>;
export type FishingClickTargets = Readonly<{
  walkCandidates: readonly [FishingClickWalkCandidate, FishingClickWalkCandidate, FishingClickWalkCandidate];
  waterTarget: FishingClickCell;
}>;
export type FishingClickInput = FishingClickTargetsInput & Readonly<{
  requestedWalk: boolean;
  autoRoute: boolean;
  walkBlocked: readonly [boolean | null, boolean | null, boolean | null];
  rodPresent: boolean | null;
  water: Readonly<{ cell: FishingClickCell; light: number }> | null;
  facingMatches: boolean | null;
  standing: boolean | null;
  fishing: boolean | null;
  transformType: number | null;
  nowMs: number;
  lastCastMs: number;
}>;
export type FishingClickDecision = Readonly<{ type: "none" }>
  | Readonly<{ type: "turn"; direction: number; delayMs: 200 }>
  | Readonly<{ type: "cast"; lastCastMs: number }>;
export type FishingClickWasmModule = {
  fishing_click_abi_version?: () => number;
  fishing_click_targets?: (json: string) => string;
  fishing_click_decision?: (json: string) => string;
};

const MAX_JSON_BYTES = 4096;
const TARGET_KEYS = ["origin", "direction"] as const;
const DECISION_KEYS = ["origin", "direction", "requestedWalk", "autoRoute", "walkBlocked", "rodPresent",
  "water", "facingMatches", "standing", "fishing", "transformType", "nowMs", "lastCastMs"] as const;

function exactKeys(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}
function integer(value: unknown, min: number, max: number): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= min && value <= max;
}
function nullableBool(value: unknown): value is boolean | null { return value === null || typeof value === "boolean"; }
function cell(value: unknown): FishingClickCell | null {
  if (!exactKeys(value, ["x", "y"])) return null;
  const x = value.x, y = value.y;
  if (!integer(x, -2147483648, 2147483647) || !integer(y, -2147483648, 2147483647)) return null;
  return Object.freeze({ x, y });
}
function targetsInput(value: unknown): FishingClickTargetsInput | null {
  if (!exactKeys(value, TARGET_KEYS)) return null;
  const origin = cell(value.origin), direction = value.direction;
  if (!origin || !integer(direction, 0, 255)) return null;
  return Object.freeze({ origin, direction });
}
function decisionInput(value: unknown): FishingClickInput | null {
  if (!exactKeys(value, DECISION_KEYS)) return null;
  const origin = cell(value.origin), direction = value.direction;
  const requestedWalk = value.requestedWalk, autoRoute = value.autoRoute, blocked = value.walkBlocked;
  const rodPresent = value.rodPresent, facingMatches = value.facingMatches;
  const standing = value.standing, fishing = value.fishing, transformType = value.transformType;
  const nowMs = value.nowMs, lastCastMs = value.lastCastMs, rawWater = value.water;
  if (!Array.isArray(blocked) || blocked.length !== 3) return null;
  const firstBlocked = blocked[0], secondBlocked = blocked[1], thirdBlocked = blocked[2];
  if (!origin || !integer(direction, 0, 255) || typeof requestedWalk !== "boolean" || typeof autoRoute !== "boolean"
    || !nullableBool(firstBlocked) || !nullableBool(secondBlocked) || !nullableBool(thirdBlocked) || !nullableBool(rodPresent)
    || !nullableBool(facingMatches) || !nullableBool(standing) || !nullableBool(fishing)
    || !(transformType === null || integer(transformType, -32768, 32767))
    || !integer(nowMs, 0, Number.MAX_SAFE_INTEGER) || !integer(lastCastMs, 0, Number.MAX_SAFE_INTEGER)) return null;
  let water: FishingClickInput["water"] = null;
  if (rawWater !== null) {
    if (!exactKeys(rawWater, ["cell", "light"])) return null;
    const waterCell = cell(rawWater.cell), light = rawWater.light;
    if (!waterCell || !integer(light, 0, 255)) return null;
    water = Object.freeze({ cell: waterCell, light });
  }
  const walkBlocked = Object.freeze([firstBlocked, secondBlocked, thirdBlocked]) as FishingClickInput["walkBlocked"];
  return Object.freeze({ origin, direction, requestedWalk, autoRoute, walkBlocked, rodPresent, water,
    facingMatches, standing, fishing, transformType, nowMs, lastCastMs });
}
function boundedJson(text: string): boolean {
  return text.length <= MAX_JSON_BYTES && new TextEncoder().encode(text).byteLength <= MAX_JSON_BYTES;
}
function requestJson(input: FishingClickTargetsInput | FishingClickInput): string {
  return JSON.stringify({ version: 1, ...input });
}

// Capture both functions and recheck their identities around each call. Input
// snapshots also prevent a reentrant ABI/getter from changing the request facts.
// This is data custody only; a returned Cast does not authorize host dispatch.
function readResult<T extends FishingClickTargetsInput>(module: FishingClickWasmModule,
  key: "fishing_click_targets" | "fishing_click_decision", input: T,
  normalize: (value: unknown) => T | null): { result: unknown; input: T } | null {
  const snapshot = normalize(input);
  if (!snapshot) return null;
  const request = requestJson(snapshot);
  const abi = module.fishing_click_abi_version, getter = module[key];
  if (typeof abi !== "function" || typeof getter !== "function" || abi.call(module) !== 1) return null;
  if (!boundedJson(request) || module.fishing_click_abi_version !== abi || module[key] !== getter
    || abi.call(module) !== 1) return null;
  const before = normalize(input);
  if (!before || requestJson(before) !== request || module.fishing_click_abi_version !== abi || module[key] !== getter) return null;
  const text = getter.call(module, request);
  if (typeof text !== "string" || !boundedJson(text) || module.fishing_click_abi_version !== abi
    || module[key] !== getter || abi.call(module) !== 1) return null;
  const after = normalize(input);
  if (!after || requestJson(after) !== request || module.fishing_click_abi_version !== abi || module[key] !== getter) return null;
  return { result: JSON.parse(text) as unknown, input: snapshot };
}

export function readSharedFishingClickTargets(module: FishingClickWasmModule,
  input: FishingClickTargetsInput): FishingClickTargets | null {
  try {
    const read = readResult(module, "fishing_click_targets", input, targetsInput);
    if (!read || !exactKeys(read.result, ["version", "ok", "walkCandidates", "waterTarget"])
      || read.result.version !== 1 || read.result.ok !== true) return null;
    const candidates = read.result.walkCandidates, waterTarget = cell(read.result.waterTarget);
    if (!waterTarget || !Array.isArray(candidates) || candidates.length !== 3) return null;
    const parsed: FishingClickWalkCandidate[] = [];
    for (const candidate of candidates) {
      if (!exactKeys(candidate, ["direction", "cell"]) || !integer(candidate.direction, 0, 7)) return null;
      const candidateCell = cell(candidate.cell);
      if (!candidateCell) return null;
      parsed.push(Object.freeze({ direction: candidate.direction, cell: candidateCell }));
    }
    if (parsed[0].direction !== read.input.direction) return null;
    const walkCandidates = Object.freeze([parsed[0], parsed[1], parsed[2]]) as FishingClickTargets["walkCandidates"];
    return Object.freeze({ walkCandidates, waterTarget });
  } catch { return null; }
}

export function readSharedFishingClickDecision(module: FishingClickWasmModule,
  input: FishingClickInput): FishingClickDecision | null {
  try {
    const read = readResult(module, "fishing_click_decision", input, decisionInput);
    if (!read || !exactKeys(read.result, ["version", "ok", "decision"])
      || read.result.version !== 1 || read.result.ok !== true) return null;
    const decision = read.result.decision;
    if (exactKeys(decision, ["type"]) && decision.type === "none") return Object.freeze({ type: "none" });
    if (exactKeys(decision, ["type", "direction", "delayMs"]) && decision.type === "turn"
      && integer(decision.direction, 0, 7) && decision.direction === read.input.direction && decision.delayMs === 200)
      return Object.freeze({ type: "turn", direction: decision.direction, delayMs: 200 });
    if (exactKeys(decision, ["type", "lastCastMs"]) && decision.type === "cast"
      && integer(decision.lastCastMs, 0, Number.MAX_SAFE_INTEGER) && decision.lastCastMs === read.input.nowMs)
      return Object.freeze({ type: "cast", lastCastMs: decision.lastCastMs });
    return null;
  } catch { return null; }
}
