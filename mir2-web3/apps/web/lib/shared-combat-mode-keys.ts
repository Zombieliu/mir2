/** Browser transport for the common Native CombatModes reducer. */
export type CombatModeRuntime = {
  getMir2CombatModeKeysVersion?: () => number;
  processMir2CombatModeKeys?: (json: string) => string;
};
export type CombatModeOwner = Readonly<{
  generation: number; connectionGeneration: number; sessionGeneration: number;
  playerObjectId: number; sceneRevision: number; mapFileName: string;
}>;
export type CombatModeCommand = Readonly<{ type: "changeAMode" | "changePMode"; mode: number }>;
export type CombatModeProof = Readonly<{
  requestId: number; owner: CombatModeOwner; revision: number; function: string; command: CombatModeCommand;
}>;
export const CRYSTAL_COMBAT_MODE_FUNCTIONS = Object.freeze([
  "ChangeAttackmode", "AttackmodePeace", "AttackmodeGroup", "AttackmodeGuild", "AttackmodeEnemyguild",
  "AttackmodeRedbrown", "AttackmodeAll", "ChangePetmode", "PetmodeBoth", "PetmodeMoveonly",
  "PetmodeAttackonly", "PetmodeNone", "PetmodeFocusMasterTarget",
]);
export function isCrystalCombatModeFunction(value: string): boolean {
  return CRYSTAL_COMBAT_MODE_FUNCTIONS.includes(value);
}
export function crystalCombatModeStopsDispatch(value: string): boolean {
  return isCrystalCombatModeFunction(value) && value !== "ChangeAttackmode" && value !== "ChangePetmode";
}
/** Issue once per real socket/session lifetime; the WASM module can survive a Page remount. */
export function nextCombatModePhysicalGeneration(): number {
  const scope = globalThis as typeof globalThis & { __mir2CombatModePhysicalGeneration?: number };
  const previous = scope.__mir2CombatModePhysicalGeneration ?? 0;
  if (!Number.isSafeInteger(previous) || previous < 0 || previous >= Number.MAX_SAFE_INTEGER) throw Error("Mode lifetime exhausted");
  return scope.__mir2CombatModePhysicalGeneration = previous + 1;
}
const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value);
const keys = (value: object, expected: string) => Object.keys(value).sort().join() === expected;
const unsigned = (value: unknown, maximum = Number.MAX_SAFE_INTEGER): value is number => Number.isSafeInteger(value) && Number(value) >= 0 && Number(value) <= maximum;
function validOwner(value: unknown): value is CombatModeOwner {
  return record(value) && keys(value, "connectionGeneration,generation,mapFileName,playerObjectId,sceneRevision,sessionGeneration")
    && [value.generation, value.connectionGeneration, value.sessionGeneration].every(v => unsigned(v) && v > 0)
    && unsigned(value.sceneRevision)
    && unsigned(value.playerObjectId, 0xffffffff) && value.playerObjectId > 0
    && typeof value.mapFileName === "string" && value.mapFileName.length > 0
    && new TextEncoder().encode(value.mapFileName).length <= 512 && !/[\u0000-\u001f\u007f-\u009f]/.test(value.mapFileName);
}
function validCommand(value: unknown): value is CombatModeCommand {
  return record(value) && keys(value, "mode,type") && (value.type === "changeAMode" || value.type === "changePMode")
    && unsigned(value.mode, value.type === "changeAMode" ? 5 : 4);
}
function sameOwner(a: CombatModeOwner, b: CombatModeOwner): boolean {
  return a.generation === b.generation && a.connectionGeneration === b.connectionGeneration
    && a.sessionGeneration === b.sessionGeneration && a.playerObjectId === b.playerObjectId
    && a.sceneRevision === b.sceneRevision && a.mapFileName === b.mapFileName;
}
function sameCommand(a: CombatModeCommand, b: CombatModeCommand): boolean {
  return a.type === b.type && a.mode === b.mode;
}
export function supportsCombatModeKeys(runtime: CombatModeRuntime | null): boolean {
  try { return runtime?.getMir2CombatModeKeysVersion?.() === 1 && typeof runtime.processMir2CombatModeKeys === "function"; }
  catch { return false; }
}
type Source = Readonly<{ owner: CombatModeOwner; revision: number; attackMode: number | null; petMode: number | null; enabled: boolean }>;
type Options = {
  runtime: () => CombatModeRuntime | null; read: () => Source | null; now: () => number;
  onWire: (proof: CombatModeProof, command: CombatModeCommand) => "confirmedSend" | "definitelyUnsent" | "outcomeUnknown";
};
/** A claimed request is spent; new key presses continue to use Native throttles. */
export class SharedCombatModeKeys {
  private active: { runtime: CombatModeRuntime; proof: CombatModeProof; entered: boolean } | null = null;
  constructor(private readonly options: Options) {}
  supported(): boolean { return supportsCombatModeKeys(this.options.runtime()); }
  private source(): Source | null {
    try {
      const source = this.options.read();
      return source && validOwner(source.owner) && unsigned(source.revision)
        && (source.attackMode === null || unsigned(source.attackMode, 5)) && (source.petMode === null || unsigned(source.petMode, 4))
        && typeof source.enabled === "boolean" ? source : null;
    } catch { return null; }
  }
  private call(runtime: CombatModeRuntime, operation: Record<string, unknown>): Record<string, unknown> | null {
    try {
      const clockMs = Math.floor(this.options.now());
      if (!unsigned(clockMs) || !supportsCombatModeKeys(runtime) || this.options.runtime() !== runtime) return null;
      const input = JSON.stringify({ version: 1, ...operation, clockMs });
      if (input.length > 8192) return null;
      const output = runtime.processMir2CombatModeKeys!(input);
      if (typeof output !== "string" || output.length > 8192) return null;
      const result: unknown = JSON.parse(output);
      return record(result) && result.version === 1 && result.ok === true ? result : null;
    } catch { return null; }
  }
  private snapshot(runtime: CombatModeRuntime, source: Source): boolean {
    return Boolean(this.call(runtime, { operation: "snapshot", owner: source.owner, revision: source.revision,
      attackMode: source.attackMode, petMode: source.petMode }));
  }
  request(functionId: string): boolean {
    if (!isCrystalCombatModeFunction(functionId) || this.active) return false;
    const source = this.source(), runtime = this.options.runtime();
    if (!source?.enabled || !runtime || !this.snapshot(runtime, source)) return false;
    const result = this.call(runtime, { operation: "request", owner: source.owner, revision: source.revision, function: functionId });
    if (!result || !keys(result, "command,ok,proof,version") || !record(result.proof) || !validCommand(result.command)) return false;
    const value = result.proof;
    if (!keys(value, "command,function,owner,requestId,revision") || !unsigned(value.requestId) || value.requestId === 0
      || !validOwner(value.owner) || !sameOwner(value.owner, source.owner)
      || value.revision !== source.revision || value.function !== functionId || !validCommand(value.command)
      || !sameCommand(value.command, result.command)) return false;
    const proof: CombatModeProof = Object.freeze({ requestId: value.requestId, owner: Object.freeze({ ...value.owner }),
      revision: source.revision, function: functionId, command: Object.freeze({ ...value.command }) });
    this.active = { runtime, proof, entered: false };
    try {
      const outcome = this.options.onWire(proof, proof.command);
      if (outcome === "definitelyUnsent") {
        if (this.active?.entered) this.outcomeUnknown(proof);
        else this.cancelDefinitelyUnsent(proof);
      }
      else if (outcome === "outcomeUnknown") this.outcomeUnknown(proof);
    } catch {
      if (this.active?.entered) this.outcomeUnknown(proof);
      else this.cancelDefinitelyUnsent(proof);
    } finally { if (this.active?.proof === proof) this.active = null; }
    return true;
  }
  allows(proof: CombatModeProof, command: unknown = proof.command): boolean {
    const active = this.active, source = this.source();
    return Boolean(active && !active.entered && active.proof === proof && this.options.runtime() === active.runtime
      && source?.enabled && source.revision === proof.revision && sameOwner(source.owner, proof.owner)
      && validCommand(command) && sameCommand(command, proof.command));
  }
  claim(proof: CombatModeProof, command: unknown): boolean {
    if (!this.allows(proof, command)) return false;
    const active = this.active!;
    const result = this.call(active.runtime, { operation: "claim", owner: proof.owner, revision: proof.revision, proof, command });
    // The reducer may have consumed the proof even if the reply or live fence
    // changed. Never roll back an attempted claim or replay its network command.
    const current = this.allows(proof, command);
    active.entered = true;
    return Boolean(result && keys(result, "entered,ok,version") && result.entered === true && current);
  }
  cancelDefinitelyUnsent(proof: CombatModeProof): boolean {
    const active = this.active;
    if (!active || active.proof !== proof || active.entered) return false;
    const result = this.call(active.runtime, { operation: "definitelyUnsent", owner: proof.owner, proof });
    return Boolean(result && keys(result, "cancelled,ok,version") && result.cancelled === true);
  }
  outcomeUnknown(proof: CombatModeProof): boolean {
    const active = this.active;
    if (!active || active.proof !== proof || !active.entered) return false;
    const result = this.call(active.runtime, { operation: "outcomeUnknown", owner: proof.owner, proof });
    return Boolean(result && keys(result, "entered,ok,outcomeUnknown,version") && result.entered === true && result.outcomeUnknown === true);
  }
  observeReceipt(packet: "ChangeAMode" | "ChangePMode", mode: number): boolean {
    const source = this.source(), runtime = this.options.runtime();
    return Boolean(source && runtime && unsigned(mode, packet === "ChangeAMode" ? 5 : 4)
      && this.call(runtime, { operation: "receipt", owner: source.owner, revision: source.revision, packet, mode }));
  }
}
