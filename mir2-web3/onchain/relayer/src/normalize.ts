import type { ChainEvent, NormalizedCommand, OreKind } from './types';
import { eventIdOf } from './types';

/**
 * Pure normalization: one chain event -> zero or one normalized command. No I/O, so it is
 * fully unit-testable. Returns null for events we don't bridge (the per-storage SetRecord
 * mutations for stones_left/ore_balance/etc. — those are internal to the chain settlement).
 */

/** Map a Sui address to the Sim account id (auth.rs: `account_id == "sui:0x.."`). */
export const toAccountId = (address: string): string => `sui:${address}`;

/** Read the business-event fields from Dubhe's `value` (either `{type, fields}` or flat). */
function readFields(value: unknown): Record<string, unknown> {
  if (value && typeof value === 'object') {
    const v = value as Record<string, unknown>;
    if (v.fields && typeof v.fields === 'object') return v.fields as Record<string, unknown>;
    return v;
  }
  return {};
}

/** A Dubhe enum value is `{ variant: 'BlackIron', ... }` or already the variant string. */
function readVariant(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value && typeof value === 'object' && 'variant' in value) {
    return String((value as { variant: unknown }).variant);
  }
  return String(value);
}

const str = (v: unknown): string => (v == null ? '' : String(v));
const num = (v: unknown): number => Number(v ?? 0);

export function normalize(event: ChainEvent): NormalizedCommand | null {
  const name = event.parsedJson?.name;
  if (!name) return null;
  const fields = readFields(event.parsedJson?.value);
  const idempotencyKey = eventIdOf(event);

  switch (name) {
    case 'mine_settled_event': {
      // No ore credited (a dry batch) -> nothing to grant; the render hint rides on the
      // dedicated mine_depleted/regened events.
      const amount = str(fields.ore_amount);
      if (amount === '' || amount === '0') return null;
      return {
        kind: 'GrantOnchainOre',
        account: toAccountId(str(fields.miner)),
        oreKind: readVariant(fields.ore_kind) as OreKind,
        amount,
        mineId: str(fields.mine_id),
        stonesLeft: num(fields.stones_left),
        nonce: str(fields.nonce),
        idempotencyKey,
      };
    }
    case 'mine_depleted_event':
      return { kind: 'MineDepleted', mineId: str(fields.mine_id), idempotencyKey };
    case 'ore_redeemed_event':
      return {
        kind: 'CreditGoldFromOre',
        account: toAccountId(str(fields.miner)),
        oreKind: readVariant(fields.ore_kind) as OreKind,
        oreAmount: str(fields.amount),
        idempotencyKey,
      };
    default:
      return null; // storage SetRecords (stones_left/ore_balance/miner_nonce/...) — internal
  }
}
