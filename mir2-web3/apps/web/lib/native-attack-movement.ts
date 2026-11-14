import type { CombatProof } from "./bevy-combat-input";
import { sameOwner, type WorldFishingOwner, type WorldFishingSourceRecord } from "./world-fishing-source";

export const NATIVE_ATTACK_MOVEMENT_CAPABILITY = "nativeAttackOutcomeV1";
type Terminal = "launched" | "notLaunched" | "unavailable" | "unknown";

export type NativeAttackLegacyRecoveryHint = Readonly<{
  owner: WorldFishingOwner;
  previousProof: CombatProof;
  requestId: string;
  objectId: number;
  notBeforeMs: number;
  code: "unsupported" | "executionUnavailable";
}>;
type AttackAttempt = {
  proof: CombatProof;
  source: WorldFishingSourceRecord;
  sentAt: number;
  until: number;
  requestId: string;
  objectId: number;
  terminal: Terminal | null;
  body: WorldFishingSourceRecord | null;
  uncertainSend: boolean;
};

// These two Gateway DTOs are bounded flat objects. Preserve the original keys
// so duplicate or escaped duplicate fields cannot qualify a capability or ACK.
function flatFrame(raw: string): Record<string, unknown> | null {
  if (raw.length > 512) return null;
  const text = raw.trim();
  if (!text.startsWith("{")) return null;
  const values: Record<string, unknown> = Object.create(null);
  const field = /\s*("(?:[^"\\]|\\.)*")\s*:\s*("(?:[^"\\]|\\.)*"|0|[1-9][0-9]*|true|false)\s*([,}])/y;
  field.lastIndex = 1;
  try {
    for (let count = 0; count < 6; count += 1) {
      const row = field.exec(text);
      if (!row) return null;
      const key = JSON.parse(row[1]) as string;
      if (Object.hasOwn(values, key)) return null;
      values[key] = JSON.parse(row[2]);
      if (row[3] === "}") return field.lastIndex === text.length ? values : null;
    }
  } catch { return null; }
  return null;
}

export class NativeAttackMovementChannel {
  private declared = false;
  private supported = false;
  private mode: "initial" | "legacy" | "typed" = "initial";
  private mixedBodySource = false;
  private sequence = 0;
  private last: AttackAttempt | null = null;

  constructor(private readonly socket: object) {}

  declare() { this.declared = true; }

  get usesTypedWire() { return this.supported && this.mode !== "legacy"; }

  enterLegacy(): boolean {
    if (this.mode === "typed") return false;
    this.mode = "legacy";
    return true;
  }

  observeAlternateBodyProducer() {
    if (this.mode === "typed") this.mixedBodySource = true;
    else this.mode = "legacy";
  }

  current(owner: WorldFishingOwner | null) {
    return owner && this.last && sameOwner(owner, this.last.source.owner) ? this.last : null;
  }

  enter(proof: CombatProof, source: WorldFishingSourceRecord, objectId: number, now: number): string | null {
    const owner = source.owner, prior = this.current(owner);
    if (!this.usesTypedWire || this.sequence >= Number.MAX_SAFE_INTEGER
      || owner.socket !== this.socket || !source.animationKnown || !source.self || source.self.dead
      || proof.identity.connectionGeneration !== owner.connectionGeneration
      || proof.identity.sessionGeneration !== owner.sessionGeneration || proof.identity.playerObjectId !== owner.playerObjectId
      || proof.command.type !== "attack" || proof.command.objectId !== objectId || proof.targetId !== objectId
      || !Number.isSafeInteger(objectId) || objectId <= 0 || objectId > 0xffff_ffff
      || !Number.isSafeInteger(now) || !Number.isSafeInteger(proof.movementBlockMs) || proof.movementBlockMs <= 0
      || (prior && (prior.proof === proof || now < prior.until
        || prior.terminal !== "launched" && prior.terminal !== "notLaunched"))) return null;
    // A correlated launch settles execution even if its presentation packet
    // never arrives. Once the original deadline permits a later attack, the
    // old body may still be in flight. Disable fast body qualification on
    // this physical socket before admitting any later attack in that case.
    if (this.last?.terminal === "launched" && !this.last.body) this.mixedBodySource = true;
    const requestId = String(++this.sequence);
    const wire = JSON.stringify({ type: "attackV2", protocolVersion: 2, requestId, objectId });
    this.mode = "typed";
    this.last = { proof, source, requestId, objectId, sentAt: now, until: now + proof.movementBlockMs,
      terminal: null, body: null, uncertainSend: false };
    return wire;
  }

  finish(proof: CombatProof, now: number, uncertain: boolean): boolean {
    const attempt = this.last;
    if (!attempt || attempt.proof !== proof) return false;
    attempt.until = Math.max(attempt.until, now + proof.movementBlockMs);
    attempt.uncertainSend ||= uncertain;
    return true;
  }

  observeBody(owner: WorldFishingOwner, previous: WorldFishingSourceRecord | null,
    body: WorldFishingSourceRecord | null, receivedAt: number) {
    const attempt = this.current(owner);
    if (!attempt || this.mixedBodySource || receivedAt < attempt.sentAt) return;
    if (!body?.animationKnown || body.sourceKey !== attempt.source.sourceKey || body.revision <= attempt.source.revision
      || body.actionEvidence?.family !== "attackMelee" || body.actionEvidence.receivedAtMs !== receivedAt
      || body.actionEvidence === previous?.actionEvidence || attempt.terminal === "notLaunched"
      || (attempt.body && attempt.body.actionEvidence !== body.actionEvidence)) {
      // An owned but incomplete or extra raw body ends fast qualification for
      // this socket. Exact typed results can still pace later new attacks.
      this.mixedBodySource = true;
      return;
    }
    attempt.body = body;
  }

  receipt(attempt: AttackAttempt): WorldFishingSourceRecord | null {
    if (attempt !== this.last || this.mixedBodySource || attempt.terminal !== "launched"
      || attempt.uncertainSend
      || attempt.body?.continuityRevision !== attempt.source.continuityRevision) return null;
    return attempt.body;
  }

  observeFrame(raw: string, owner: WorldFishingOwner | null): NativeAttackLegacyRecoveryHint | null {
    const frame = flatFrame(raw);
    if (!frame || frame.protocolVersion !== 2) return null;
    const size = Object.keys(frame).length;
    if (frame.type === "attackCapabilitiesV2" && size === 5 && this.declared
      && frame.capability === NATIVE_ATTACK_MOVEMENT_CAPABILITY && frame.supported === true && frame.scope === "connection") {
      this.supported = true;
      return null;
    }
    if (frame.type !== "attackResultV2") return null;
    const attempt = this.current(owner);
    if (!attempt || attempt.terminal || frame.requestId !== attempt.requestId || frame.objectId !== attempt.objectId) return null;
    const terminal = frame.result, code = frame.code;
    if (size === 5 && (terminal === "launched" || terminal === "notLaunched")
      || size === 6 && terminal === "unavailable"
        && (code === "unsupported" || code === "executionUnavailable" || code === "authenticationRequired" || code === "requestIdNotFresh")
      || size === 6 && terminal === "unknown"
        && (code === "outcomeUnknown" || code === "previousOutcomeUnknown")) {
      attempt.terminal = terminal;
      if (terminal === "notLaunched" && attempt.body) this.mixedBodySource = true;
      // A no-entry result permits a policy decision for a fresh action on a
      // verified replacement socket. This channel stays typed and never retries.
      if (terminal === "unavailable" && (code === "unsupported" || code === "executionUnavailable") && !attempt.body) {
        return Object.freeze({ owner: attempt.source.owner, previousProof: attempt.proof, requestId: attempt.requestId,
          objectId: attempt.objectId, notBeforeMs: attempt.until, code });
      }
    }
    return null;
  }
}

const channels = new WeakMap<object, NativeAttackMovementChannel>();
export function nativeAttackMovementChannel(socket: object): NativeAttackMovementChannel {
  const existing = channels.get(socket);
  if (existing) return existing;
  const channel = new NativeAttackMovementChannel(socket);
  channels.set(socket, channel);
  return channel;
}
