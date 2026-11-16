export type SocialRequestKind = "group" | "marriage" | "divorce" | "mentor" | "guild";
export type SocialReplyOwner = Readonly<{
  connectionGeneration: number; sessionGeneration: number; sceneRevision: number;
  playerObjectId: number; mapFileName: string; socket: object;
}>;
export type SocialIncomingRequest = Readonly<{
  kind: SocialRequestKind; name: string; epoch: number; level?: number;
}>;
export type SocialReplyProof = Readonly<{
  request: SocialIncomingRequest; owner: SocialReplyOwner; acceptInvite: boolean;
}>;
const replyType: Record<SocialRequestKind, string> = {
  group: "groupInvite", marriage: "marriageReply", divorce: "divorceReply", mentor: "mentorReply",
  guild: "guildInvite",
};
const kinds: SocialRequestKind[] = ["group", "marriage", "divorce", "mentor", "guild"];

// The counter survives a host remount/HMR. It is a local UI lease, never a server request ID.
function nextEpoch(): number | null {
  const host = globalThis as typeof globalThis & { __mir2SocialReplyEpoch?: number };
  const last = host.__mir2SocialReplyEpoch ?? 0;
  if (!Number.isSafeInteger(last) || last < 0 || last >= Number.MAX_SAFE_INTEGER) return null;
  return host.__mir2SocialReplyEpoch = last + 1;
}
export function sameSocialReplyOwner(a: SocialReplyOwner | null, b: SocialReplyOwner | null): boolean {
  return !!a && !!b && a.socket === b.socket && a.connectionGeneration === b.connectionGeneration
    && a.sessionGeneration === b.sessionGeneration && a.sceneRevision === b.sceneRevision
    && a.playerObjectId === b.playerObjectId && a.mapFileName === b.mapFileName;
}
function validOwner(o: SocialReplyOwner): boolean {
  return [o.connectionGeneration, o.sessionGeneration, o.sceneRevision, o.playerObjectId]
    .every(n => Number.isSafeInteger(n) && n > 0) && !!o.socket
    && typeof o.mapFileName === "string" && o.mapFileName.length > 0;
}
export function socialReplyCommand(p: SocialReplyProof): { type: string; acceptInvite: boolean } {
  return { type: replyType[p.request.kind], acceptInvite: p.acceptInvite };
}

/** One current incoming request per kind. A reply is claimed only at the final socket boundary. */
export class SocialIncomingReplies {
  private pending = new Map<SocialRequestKind, { request: SocialIncomingRequest; owner: SocialReplyOwner }>();
  private reservations = new Map<SocialReplyProof, { entered: boolean }>();

  receive(kind: SocialRequestKind, name: unknown, level: unknown, owner: SocialReplyOwner | null): boolean {
    if (!kinds.includes(kind) || !owner || !validOwner(owner) || typeof name !== "string"
      || !name.trim() || name.length > 256 || name.includes("\0")) return false;
    const epoch = nextEpoch();
    if (epoch === null) return false;
    const request = Object.freeze({ kind, name, epoch,
      ...(typeof level === "number" && Number.isSafeInteger(level) && level >= 0 && level <= 65535 ? { level } : {}) });
    this.dismiss(kind);
    this.pending.set(kind, { request, owner: Object.freeze({ ...owner }) });
    return true;
  }
  list(owner: SocialReplyOwner | null): SocialIncomingRequest[] {
    return kinds.flatMap(kind => {
      const row = this.pending.get(kind);
      return row && sameSocialReplyOwner(row.owner, owner) ? [row.request] : [];
    });
  }
  reserve(kind: SocialRequestKind, epoch: number, acceptInvite: boolean, owner: SocialReplyOwner | null): SocialReplyProof | null {
    const row = this.pending.get(kind);
    if (!row || !sameSocialReplyOwner(row.owner, owner) || row.request.epoch !== epoch
      || typeof acceptInvite !== "boolean" || [...this.reservations.keys()].some(p => p.request === row.request)) return null;
    const proof = Object.freeze({ request: row.request, owner: row.owner, acceptInvite });
    this.reservations.set(proof, { entered: false });
    return proof;
  }
  allows(proof: SocialReplyProof, owner: SocialReplyOwner | null, command: Record<string, unknown>): boolean {
    const lease = this.reservations.get(proof), row = this.pending.get(proof.request.kind);
    return !!lease && !lease.entered && row?.request === proof.request && sameSocialReplyOwner(proof.owner, owner)
      && Object.keys(command).length === 2 && command.type === replyType[proof.request.kind]
      && command.acceptInvite === proof.acceptInvite;
  }
  claim(proof: SocialReplyProof, owner: SocialReplyOwner | null, command: Record<string, unknown>): boolean {
    if (!this.allows(proof, owner, command)) return false;
    this.reservations.get(proof)!.entered = true;
    this.pending.delete(proof.request.kind);
    return true;
  }
  cancelDefinitelyUnsent(proof: SocialReplyProof): void {
    if (this.reservations.get(proof)?.entered === false) this.reservations.delete(proof);
  }
  finish(proof: SocialReplyProof): void { this.reservations.delete(proof); }
  dismiss(kind: SocialRequestKind): void {
    this.pending.delete(kind);
    for (const p of this.reservations.keys()) if (p.request.kind === kind) this.reservations.delete(p);
  }
  retire(): void { this.pending.clear(); this.reservations.clear(); }
}
