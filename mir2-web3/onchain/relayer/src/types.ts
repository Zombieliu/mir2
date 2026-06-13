/**
 * Relayer types — the on-chain event shape (as returned by Sui `queryEvents`) and the
 * normalized commands the Relayer injects into the Sim (DESIGN §4, ROADMAP §5/§6).
 *
 * The command schema is the M2↔M3 contract: M3 adds matching `WorldCommand` variants
 * (`GrantOnchainOre` / `CreditGoldFromOre`) in apps/simulation/src/world_runtime.rs. The
 * Sim applies these only on chain-confirmed events (server stays authoritative), keyed by
 * `idempotencyKey` (idempotency place #3).
 */

/** A Sui event as returned by `SuiClient.queryEvents`, narrowed to what we use. */
export interface ChainEvent {
  id: { txDigest: string; eventSeq: string };
  /** Fully-qualified Move type, e.g. `0x..::storage_event::SetRecord<..MineSettledEvent..>`. */
  type: string;
  /** Dubhe wraps records as { name, key1, key2, value }. `value` holds the business event. */
  parsedJson: DubheRecordJson | null;
  timestampMs?: string | null;
}

export interface DubheRecordJson {
  name?: string;
  key1?: unknown;
  key2?: unknown;
  value?: unknown;
}

/** `(tx_digest, event_seq)` — the relayer's dedup key (idempotency place #2). */
export type EventId = `${string}:${string}`;

export const eventIdOf = (e: ChainEvent): EventId => `${e.id.txDigest}:${e.id.eventSeq}`;

/** Ore varieties, mirrored from the on-chain `OreKind` enum (DESIGN §3.1). */
export type OreKind =
  | 'BlackIron'
  | 'Gold'
  | 'Silver'
  | 'Copper'
  | 'Platinum'
  | 'Ruby'
  | 'Nephrite'
  | 'Amethyst';

/** Grant chain-confirmed ore to a player's bag + drive render (from `mine_settled`). */
export interface GrantOnchainOre {
  kind: 'GrantOnchainOre';
  account: string; // "sui:0x.."
  oreKind: OreKind;
  amount: string; // u64 as string (no precision loss)
  mineId: string;
  stonesLeft: number;
  nonce: string;
  idempotencyKey: EventId;
}

/** A mine became empty — render hint (from `mine_depleted`). */
export interface MineDepleted {
  kind: 'MineDepleted';
  mineId: string;
  idempotencyKey: EventId;
}

/**
 * Credit game gold for burned ore (from `ore_redeemed`). NOTE: the ore→gold RATE is an M5
 * economic decision — NOT applied here. The relayer passes the burned ore through; the Sim
 * applies the admin-set rate downstream (no economic constant in the bridge).
 */
export interface CreditGoldFromOre {
  kind: 'CreditGoldFromOre';
  account: string; // "sui:0x.."
  oreKind: OreKind;
  oreAmount: string; // u64 as string; gold = rate * oreAmount (applied at M5)
  idempotencyKey: EventId;
}

export type NormalizedCommand = GrantOnchainOre | MineDepleted | CreditGoldFromOre;
