import type { SocialReplyOwner } from "./social-incoming-replies";

export type GuildPermissionKey = "CanChangeRank" | "CanRecruit" | "CanKick" | "CanStoreItem"
  | "CanRetrieveItem" | "CanAlterAlliance" | "CanChangeNotice" | "CanActivateBuff";
const permissionKeys: readonly GuildPermissionKey[] = ["CanChangeRank", "CanRecruit", "CanKick", "CanStoreItem",
  "CanRetrieveItem", "CanAlterAlliance", "CanChangeNotice", "CanActivateBuff"];
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const integer = (value: unknown, maximum = Number.MAX_SAFE_INTEGER): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= maximum;
const cell = (value: unknown, size: number): value is number => integer(value, size - 1);
const exactKeys = (value: Record<string, unknown>, keys: readonly string[]): boolean =>
  Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));

/** Crystal GuildRankOptions is exactly eight bits; unknown input grants no permissions. */
export function guildPermissionsFromOptions(myOptions: unknown): GuildPermissionKey[] | null {
  return integer(myOptions, 255) ? permissionKeys.filter((_, bit) => (myOptions & (1 << bit)) !== 0) : null;
}

export type RawGuildUserItem = Readonly<Record<string, unknown> & { unique_id: number; item_index: number; count: number }>;
export type RawGuildStorageItem = Readonly<Record<string, unknown> & { item: RawGuildUserItem; userId: number }>;
export type RawGuildStorageSlots = readonly (RawGuildStorageItem | null)[];
type GuildStorageChange = Readonly<{ changeType: number; from: number; to: number; user: number; item: RawGuildStorageItem | null }>;

/** Nested UserItem retains snake_case while the containing GuildStorageItem has camelCase userId. */
function validRawGuildItem(value: unknown): value is RawGuildStorageItem {
  return record(value) && record(value.item) && !Object.hasOwn(value.item, "uniqueId") && !Object.hasOwn(value.item, "itemIndex")
    && integer(value.item.unique_id) && integer(value.item.item_index, 2_147_483_647)
    && integer(value.item.count, 65_535) && value.item.count > 0
    && typeof value.userId === "number" && Number.isSafeInteger(value.userId);
}
function validRawGuildSlots(value: unknown): value is RawGuildStorageSlots {
  if (!Array.isArray(value) || value.length !== 112) return false;
  const identities = new Set<number>();
  for (const row of value) {
    if (row === null) continue;
    if (!validRawGuildItem(row) || identities.has(row.item.unique_id)) return false;
    identities.add(row.item.unique_id);
  }
  return true;
}
function validTuple(kind: SocialWindowOperationKind, from: unknown, to: unknown): boolean {
  switch (kind) {
    case "depositTrade": return cell(from, 80) && cell(to, 10);
    case "retrieveTrade": return cell(from, 10) && cell(to, 80);
    case "guild0": return cell(from, 80) && cell(to, 112);
    case "guild1": return cell(from, 112) && cell(to, 80);
    case "guild2": return cell(from, 112) && cell(to, 112);
    default: return false;
  }
}
function guildStorageChange(value: unknown): GuildStorageChange | null {
  const fields = ["changeType", "from", "to", "user", "item"];
  if (!record(value) || !exactKeys(value, fields) && !(value.typed === true && exactKeys(value, [...fields, "typed"]))
    || !integer(value.changeType, 5) || typeof value.user !== "number" || !Number.isSafeInteger(value.user)
    || value.user < -2_147_483_648 || value.user > 2_147_483_647
    || value.item !== null && !validRawGuildItem(value.item)) return null;
  const requested = value.changeType % 3;
  if (!validTuple(`guild${requested}` as SocialWindowOperationKind, value.from, value.to)) return null;
  // The actual failure and retrieve packets carry None. Deposit and move carry the moved item.
  if (value.changeType >= 3 ? value.item !== null : requested === 1 ? value.item !== null : value.item === null) return null;
  return value as GuildStorageChange;
}

/** Apply only a real server change; no catalog lookup, compaction or optimistic request mutation. */
export function applyGuildStorageChange(raw112: unknown, packet: unknown): RawGuildStorageSlots | null {
  if (!validRawGuildSlots(raw112)) return null;
  const change = guildStorageChange(packet);
  if (!change) return null;
  if (change.changeType >= 3) return raw112;
  const result = [...raw112];
  if (change.changeType === 0) result[change.to] = change.item;
  else if (change.changeType === 1) result[change.from] = null;
  else {
    // A swap needs the current source, rather than inferring missing state from the packet item.
    const source = result[change.from];
    if (!source || source.item.unique_id !== change.item!.item.unique_id
      || source.item.item_index !== change.item!.item.item_index || source.item.count !== change.item!.item.count) return null;
    [result[change.from], result[change.to]] = [result[change.to], result[change.from]];
  }
  return validRawGuildSlots(result) ? result : null;
}

export type SocialWindowOperationKind = "depositTrade" | "retrieveTrade" | "guild0" | "guild1" | "guild2";
export type SocialWindowOperationInput = Readonly<{
  owner: SocialReplyOwner; surface: "trade" | "guild"; localSourceKey: string;
  kind: SocialWindowOperationKind; from: number; to: number; uniqueId: number;
  /** Actual authenticated StartGame character index, never a character-list display index. */
  ownCharacterIndex: number;
  /** Host revision of the complete authoritative world observed before reservation. */
  beforeSnapshotRevision: number;
}>;
export type SocialWindowOperationProof = SocialWindowOperationInput;
export type SocialWindowOperationPending = Readonly<{
  proof: SocialWindowOperationProof; entered: boolean; acked: boolean;
  success: boolean | null; snapshotRevision: number | null;
}>;
export type SocialWindowOperationReceipt = Readonly<{
  matched: boolean; success: boolean | null; pending: SocialWindowOperationPending | null;
}>;
export type SocialWindowItemCommand =
  | { type: "depositTradeItem" | "retrieveTradeItem"; from: number; to: number }
  | { type: "guildStorageItemChange"; changeType: 0 | 1 | 2; from: number; to: number };

function validOwner(owner: SocialReplyOwner | null): owner is SocialReplyOwner {
  return !!owner && record(owner.socket)
    && [owner.connectionGeneration, owner.sessionGeneration, owner.sceneRevision, owner.playerObjectId]
      .every(value => integer(value) && value > 0)
    && typeof owner.mapFileName === "string" && owner.mapFileName.length > 0
    && owner.mapFileName.length <= 256 && !owner.mapFileName.includes("\0");
}
function sameOwner(left: SocialReplyOwner, right: SocialReplyOwner | null): boolean {
  return validOwner(right) && left.socket === right.socket && left.connectionGeneration === right.connectionGeneration
    && left.sessionGeneration === right.sessionGeneration && left.sceneRevision === right.sceneRevision
    && left.playerObjectId === right.playerObjectId && left.mapFileName === right.mapFileName;
}
function validSourceKey(key: unknown): key is string {
  return typeof key === "string" && key.length > 0 && !key.includes("\0");
}

/** Local proof is never a gateway request ID or server authority; it does not appear in the wire command. */
export function socialWindowItemCommand(proof: SocialWindowOperationProof): SocialWindowItemCommand {
  if (proof.kind === "depositTrade" || proof.kind === "retrieveTrade") {
    return { type: proof.kind === "depositTrade" ? "depositTradeItem" : "retrieveTradeItem", from: proof.from, to: proof.to };
  }
  return { type: "guildStorageItemChange", changeType: Number(proof.kind.slice(-1)) as 0 | 1 | 2, from: proof.from, to: proof.to };
}

/** One unresolved item operation across both surfaces. Closing a UI or changing a source never clears it. */
export class SocialWindowOperations {
  private operation: { proof: SocialWindowOperationProof; entered: boolean; acked: boolean;
    success: boolean | null; snapshotRevision: number | null } | null = null;

  get pending(): SocialWindowOperationPending | null {
    return this.operation ? Object.freeze({ ...this.operation }) : null;
  }
  reserve(input: SocialWindowOperationInput): SocialWindowOperationProof | null {
    if (this.operation || !validOwner(input.owner) || !validSourceKey(input.localSourceKey)
      || !validTuple(input.kind, input.from, input.to) || !integer(input.uniqueId)
      || !integer(input.ownCharacterIndex, 2_147_483_647) || !integer(input.beforeSnapshotRevision)
      || input.surface !== (input.kind === "depositTrade" || input.kind === "retrieveTrade" ? "trade" : "guild")) return null;
    const proof = Object.freeze({ ...input, owner: Object.freeze({ ...input.owner }) });
    this.operation = { proof, entered: false, acked: false, success: null, snapshotRevision: null };
    return proof;
  }
  allows(proof: SocialWindowOperationProof, owner: SocialReplyOwner | null, localSourceKey: string, command: unknown): boolean {
    if (!this.operation || this.operation.proof !== proof || this.operation.entered
      || !sameOwner(proof.owner, owner) || localSourceKey !== proof.localSourceKey || !record(command)) return false;
    const expected = socialWindowItemCommand(proof);
    return exactKeys(command, Object.keys(expected)) && Object.entries(expected).every(([key, value]) => command[key] === value);
  }
  /** Call at the final socket boundary, after the host repeats current inventory, permission and lease checks. */
  claim(proof: SocialWindowOperationProof, owner: SocialReplyOwner | null, localSourceKey: string, command: unknown): boolean {
    if (!this.allows(proof, owner, localSourceKey, command)) return false;
    this.operation!.entered = true;
    return true;
  }
  cancelDefinitelyUnsent(proof: SocialWindowOperationProof): boolean {
    if (this.operation?.proof !== proof || this.operation.entered) return false;
    this.operation = null;
    return true;
  }
  /** An exception after claim cannot establish that the request was unsent. Keep the barrier. */
  outcomeUnknown(proof: SocialWindowOperationProof): boolean {
    return this.operation?.proof === proof && this.operation.entered;
  }
  receipt(packetName: unknown, payload: unknown, owner: SocialReplyOwner | null): SocialWindowOperationReceipt {
    const unmatched = (): SocialWindowOperationReceipt => ({ matched: false, success: null, pending: this.pending });
    const operation = this.operation;
    if (!operation || !operation.entered || !sameOwner(operation.proof.owner, owner) || !record(payload)) return unmatched();
    const proof = operation.proof;
    if (payload.from !== proof.from || payload.to !== proof.to) return unmatched();
    let success: boolean;
    if (proof.surface === "trade") {
      if (packetName !== (proof.kind === "depositTrade" ? "DepositTradeItem" : "RetrieveTradeItem")
        || !exactKeys(payload, ["from", "to", "success"]) || typeof payload.success !== "boolean") return unmatched();
      success = payload.success;
    } else {
      if (packetName !== "GuildStorageItemChange") return unmatched();
      const change = guildStorageChange(payload), requested = Number(proof.kind.slice(-1));
      if (!change || change.changeType !== requested && change.changeType !== requested + 3) return unmatched();
      success = change.changeType === requested;
      if (success ? change.user !== proof.ownCharacterIndex : change.user !== 0 && change.user !== proof.ownCharacterIndex) return unmatched();
      if (success && requested !== 1 && change.item!.item.unique_id !== proof.uniqueId) return unmatched();
    }
    if (operation.acked && operation.success !== success) return unmatched();
    operation.acked = true;
    operation.success = success;
    return { matched: true, success, pending: this.pending };
  }
  /** Only a same-owner COMPLETE world snapshot received after the matching ACK may satisfy this barrier. */
  markSnapshot(proof: SocialWindowOperationProof, owner: SocialReplyOwner | null, snapshotRevision: number): boolean {
    const operation = this.operation;
    if (!operation || operation.proof !== proof || !operation.entered || !operation.acked
      || !sameOwner(proof.owner, owner) || !integer(snapshotRevision) || snapshotRevision <= proof.beforeSnapshotRevision
      || operation.snapshotRevision !== null && snapshotRevision <= operation.snapshotRevision) return false;
    operation.snapshotRevision = snapshotRevision;
    return true;
  }
  releaseAfterSnapshot(proof: SocialWindowOperationProof, owner: SocialReplyOwner | null): boolean {
    const operation = this.operation;
    if (!operation || operation.proof !== proof || !operation.entered || !operation.acked || !sameOwner(proof.owner, owner)
      || operation.snapshotRevision === null || operation.snapshotRevision <= proof.beforeSnapshotRevision) return false;
    this.operation = null;
    return true;
  }
  /** Host calls only for actual connection retirement, never on modal/source/scene changes or timeout. */
  retire(): void { this.operation = null; }
}
