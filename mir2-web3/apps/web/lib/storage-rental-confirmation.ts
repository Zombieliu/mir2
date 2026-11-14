import { sameSocialReplyOwner, type SocialReplyOwner } from "./social-incoming-replies";
import { npcPurchaseSignedDecimal } from "./npc-purchase-client";

export const STORAGE_RENTAL_GOLD = 1_000_000;
export const STORAGE_RENTAL_DAYS = 10;
export type StorageRentalFacts = Readonly<{
  owner: SocialReplyOwner; serviceRevision: number; hasExpandedStorage: boolean;
  expiryObservation: number | string; storageSize: number; gold: number;
}>;
export type StorageRentalProof = Readonly<{ id: number; facts: StorageRentalFacts; shared: boolean }>;
function ready(f: StorageRentalFacts | null): f is StorageRentalFacts {
  return !!f && Number.isSafeInteger(f.serviceRevision) && f.serviceRevision > 0
    && typeof f.hasExpandedStorage === "boolean" && (Number.isSafeInteger(f.expiryObservation) || npcPurchaseSignedDecimal(f.expiryObservation))
    && Number.isSafeInteger(f.storageSize) && f.storageSize > 0
    && Number.isSafeInteger(f.gold) && f.gold >= STORAGE_RENTAL_GOLD;
}
/** Confirmation holds the observed rental state; only the server changes size/expiry/gold. */
export class StorageRentalConfirmation {
  private sequence = 0;
  private offer: StorageRentalProof | null = null;
  open(facts: StorageRentalFacts | null, shared: boolean): StorageRentalProof | null {
    if (!ready(facts) || this.offer || this.sequence >= Number.MAX_SAFE_INTEGER) return null;
    return this.offer = Object.freeze({ id: ++this.sequence,
      facts: Object.freeze({ ...facts, owner: Object.freeze({ ...facts.owner }) }), shared });
  }
  get pending(): StorageRentalProof | null { return this.offer; }
  allows(p: StorageRentalProof, current: StorageRentalFacts | null): boolean {
    return this.offer === p && ready(current) && sameSocialReplyOwner(p.facts.owner, current.owner)
      && p.facts.serviceRevision === current.serviceRevision
      && p.facts.hasExpandedStorage === current.hasExpandedStorage
      && p.facts.expiryObservation === current.expiryObservation && p.facts.storageSize === current.storageSize;
  }
  claim(p: StorageRentalProof, current: StorageRentalFacts | null, command: Record<string, unknown>): boolean {
    if (!this.allows(p, current) || Object.keys(command).length !== 2
      || command.type !== "chat" || command.message !== "@ADDSTORAGE") return false;
    this.offer = null;
    return true;
  }
  cancel(p: StorageRentalProof): boolean {
    if (this.offer !== p) return false;
    this.offer = null;
    return true;
  }
  retire(): void { this.offer = null; }
}
