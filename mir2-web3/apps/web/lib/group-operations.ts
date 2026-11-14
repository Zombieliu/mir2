import type { SocialReplyOwner } from "./social-incoming-replies";

export type GroupOperation = "addMember" | "delMember";
export type GroupOperationSource = Readonly<{
  owner: SocialReplyOwner | null;
  playerName: string | null;
  members: readonly string[];
}>;
export type GroupOperationProof = Readonly<{
  op: GroupOperation;
  name: string;
  owner: SocialReplyOwner;
  playerName: string;
}>;
export type GroupOperationPending = Readonly<{ proof: GroupOperationProof; entered: boolean }>;
export type GroupOperationCommand = Readonly<{ type: GroupOperation; name: string }>;

const MAX_GROUP_MEMBERS = 15;
// This is the group subqueue only, not Native's shared budget for all social operations.
const MAX_GROUP_PENDING = 8;
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const validName = (value: unknown): value is string =>
  typeof value === "string" && value.trim().length > 0 && value.length <= 256 && !value.includes("\0");
// The server resolves an AddMember target with eq_ignore_ascii_case.
const asciiGroupName = (name: string): string => name.replace(/[A-Z]/g, letter => letter.toLowerCase());
const positiveInteger = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value > 0;

// Do not execute DTO getters or toJSON while deciding whether a command may enter.
function field(value: object, key: PropertyKey): unknown {
  const descriptor = Object.getOwnPropertyDescriptor(value, key);
  return descriptor?.enumerable && "value" in descriptor ? descriptor.value : undefined;
}
function captureOwner(value: unknown): SocialReplyOwner | null {
  if (!record(value)) return null;
  const socket = field(value, "socket"), connectionGeneration = field(value, "connectionGeneration");
  const sessionGeneration = field(value, "sessionGeneration"), sceneRevision = field(value, "sceneRevision");
  const playerObjectId = field(value, "playerObjectId"), mapFileName = field(value, "mapFileName");
  if (!record(socket) || !positiveInteger(connectionGeneration) || !positiveInteger(sessionGeneration)
    || !positiveInteger(sceneRevision) || !positiveInteger(playerObjectId) || !validName(mapFileName)) return null;
  return Object.freeze({ socket, connectionGeneration, sessionGeneration, sceneRevision, playerObjectId, mapFileName });
}
function samePhysicalOwner(left: SocialReplyOwner, right: SocialReplyOwner): boolean {
  return left.socket === right.socket && left.connectionGeneration === right.connectionGeneration
    && left.sessionGeneration === right.sessionGeneration && left.playerObjectId === right.playerObjectId;
}
function names(value: unknown, memberRows: boolean): string[] | null {
  if (!Array.isArray(value)) return null;
  const length = Object.getOwnPropertyDescriptor(value, "length")?.value;
  if (!Number.isSafeInteger(length) || length < 0 || length > 8192) return null;
  const result: string[] = [];
  for (let index = 0; index < length; index++) {
    const row = field(value, String(index));
    const name = memberRows ? record(row) ? field(row, "name") : undefined : row;
    if (!validName(name)) return null;
    result.push(name);
  }
  return result;
}
function captureSource(value: GroupOperationSource): { owner: SocialReplyOwner; playerName: string; members: string[] } | null {
  if (!record(value)) return null;
  const owner = captureOwner(field(value, "owner")), playerName = field(value, "playerName");
  const members = names(field(value, "members"), false);
  return owner && validName(playerName) && members ? { owner, playerName, members } : null;
}
function canManage(op: GroupOperation, source: { playerName: string; members: string[] }): boolean {
  return (op !== "addMember" || source.members.length < MAX_GROUP_MEMBERS)
    && (source.members.length === 0 || source.members[0] === source.playerName);
}
function exactCommand(value: unknown, proof: GroupOperationProof): boolean {
  if (!record(value)) return false;
  const prototype = Object.getPrototypeOf(value), keys = Reflect.ownKeys(value);
  return (prototype === Object.prototype || prototype === null) && keys.length === 2
    && keys.includes("type") && keys.includes("name") && !("toJSON" in value)
    && field(value, "type") === proof.op && field(value, "name") === proof.name;
}

/** Local proof never becomes a request ID, permission flag or extra wire field. */
export function groupOperationCommand(proof: GroupOperationProof): GroupOperationCommand {
  return { type: proof.op, name: proof.name };
}

/** Native's name-specific group pending rules, without optimistic roster changes or retries. */
export class GroupOperations {
  private operations = new Map<GroupOperationProof, { entered: boolean }>();

  get pending(): readonly GroupOperationPending[] {
    return Object.freeze([...this.operations].map(([proof, row]) => Object.freeze({ proof, entered: row.entered })));
  }

  reserve(op: GroupOperation, name: unknown, source: GroupOperationSource): GroupOperationProof | null {
    try {
      if (op !== "addMember" && op !== "delMember" || typeof name !== "string") return null;
      const normalized = name.trim(), captured = captureSource(source);
      if (!validName(normalized) || !captured || !canManage(op, captured)
        || this.operations.size >= MAX_GROUP_PENDING
        || [...this.operations.keys()].some(proof => proof.op === op && proof.name === normalized)) return null;
      const proof = Object.freeze({ op, name: normalized, owner: captured.owner, playerName: captured.playerName });
      this.operations.set(proof, { entered: false });
      return proof;
    } catch {
      return null;
    }
  }

  allows(proof: GroupOperationProof, source: GroupOperationSource, wire: unknown): boolean {
    const row = this.operations.get(proof);
    if (!row || row.entered) return false;
    try {
      if (!exactCommand(wire, proof)) return false;
      // Read current authority after inspecting an untrusted DTO, immediately before claim.
      const captured = captureSource(source);
      return !!captured && samePhysicalOwner(proof.owner, captured.owner)
        && proof.owner.sceneRevision === captured.owner.sceneRevision && proof.owner.mapFileName === captured.owner.mapFileName
        && proof.playerName === captured.playerName && canManage(proof.op, captured)
        // A hostile Proxy may synchronously reenter or retire during DTO inspection.
        && this.operations.get(proof) === row && !row.entered;
    } catch {
      return false;
    }
  }

  /** At the final socket boundary. True permits this one attempt; it is not a delivery ACK. */
  claim(proof: GroupOperationProof, source: GroupOperationSource, wire: unknown): boolean {
    const row = this.operations.get(proof);
    if (!row || !this.allows(proof, source, wire) || this.operations.get(proof) !== row || row.entered) return false;
    row.entered = true;
    return true;
  }

  cancelDefinitelyUnsent(proof: GroupOperationProof): boolean {
    if (this.operations.get(proof)?.entered !== false) return false;
    return this.operations.delete(proof);
  }

  /** A throw after claim does not prove absence of a send. Retain the pending operation. */
  outcomeUnknown(proof: GroupOperationProof): boolean {
    return this.operations.get(proof)?.entered === true;
  }

  /** An invitation send has no required terminal packet for the inviter. */
  finishInviteAttempt(proof: GroupOperationProof): void {
    if (proof.op === "addMember" && this.operations.get(proof)?.entered === true) this.operations.delete(proof);
  }

  /** Only actual named packets/full rosters release entered operations, including after a map change. */
  receipt(packetName: unknown, payload: unknown, owner: SocialReplyOwner | null): boolean {
    if (packetName !== "AddMember" && packetName !== "DeleteMember" && packetName !== "GroupMemberInfo") return false;
    try {
      const candidates = [...this.operations].filter(([, row]) => row.entered);
      const captured = captureOwner(owner);
      if (!captured || !record(payload)) return false;
      const roster = packetName === "GroupMemberInfo" ? names(field(payload, "members"), true) : null;
      const subject = packetName === "GroupMemberInfo" ? null : field(payload, "name");
      if (packetName === "GroupMemberInfo" ? roster === null : !validName(subject)) return false;
      let matched = false;
      for (const [proof, row] of candidates) {
        if (this.operations.get(proof) !== row || !samePhysicalOwner(proof.owner, captured)) continue;
        let done: boolean;
        if (proof.op === "addMember") {
          const requested = asciiGroupName(proof.name);
          done = roster ? roster.some(name => asciiGroupName(name) === requested)
            : packetName === "AddMember" && typeof subject === "string" && asciiGroupName(subject) === requested;
        } else {
          done = roster ? !roster.includes(proof.name) : packetName === "DeleteMember" && proof.name === subject;
        }
        if (done) { this.operations.delete(proof); matched = true; }
      }
      return matched;
    } catch {
      return false;
    }
  }

  /** Actual session/connection retirement only: map changes and UI close do not retire pending work. */
  retire(): void { this.operations.clear(); }
}
