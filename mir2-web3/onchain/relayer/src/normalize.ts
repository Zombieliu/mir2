import type { ChainEvent, NormalizedCommand, OreKind } from './types';
import { eventIdOf } from './types';

/**
 * Pure normalization: one chain event -> zero or one normalized command. No I/O, so it is
 * fully unit-testable.
 *
 * SK1 (Dubhe 1.2.x): the systems emit plain `sui::event::emit` structs (the framework's
 * storage-event helpers are `public(package)`), so we switch on the event TYPE
 * (`<pkg>::<module>::MineSettledEvent`) and read the struct fields straight from
 * `parsedJson` — not the old Dubhe `{ name, value: { fields } }` wrapper. The downstream
 * command shapes (the M2↔M3 contract) are unchanged. `MineRegenedEvent` and anything else
 * are not bridged.
 */

/** Map a Sui address to the Sim account id (auth.rs: `account_id == "sui:0x.."`). */
export const toAccountId = (address: string): string => `sui:${address}`;

const str = (v: unknown): string => (v == null ? '' : String(v));
const num = (v: unknown): number => Number(v ?? 0);

/** A Move enum in event JSON is a variant string, `{ variant: 'X' }`, or `{ X: {...} }`. */
function readVariant(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value && typeof value === 'object') {
    const v = value as Record<string, unknown>;
    if (typeof v.variant === 'string') return v.variant;
    const keys = Object.keys(v).filter((k) => k !== 'fields' && k !== 'type');
    if (keys.length === 1 && keys[0]) return keys[0];
  }
  return String(value);
}

/** The struct fields of a plain Sui event (or {} when absent). */
function readFields(parsedJson: ChainEvent['parsedJson']): Record<string, unknown> {
  return parsedJson && typeof parsedJson === 'object' ? parsedJson : {};
}

/** Struct name from the event type, dropping the module path + any generic suffix. */
function eventName(type: string): string {
  const base = type.split('<')[0] ?? type;
  return base.split('::').pop() ?? '';
}

export function normalize(event: ChainEvent): NormalizedCommand | null {
  if (!event.parsedJson) return null;
  const fields = readFields(event.parsedJson);
  const idempotencyKey = eventIdOf(event);

  switch (eventName(event.type)) {
    case 'MineSettledEvent': {
      // Dry batches (ore_amount=0 — every swing missed, or the mine was empty) are
      // FORWARDED: the sim still updates the vein from stones_left and re-broadcasts
      // MineNodeState, which is the client's settlement signal (M4 — dropping these
      // wedges the client's in-flight batch). The grant itself is a no-item no-op.
      const amount = str(fields.ore_amount) || '0';
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
    case 'MineDepletedEvent':
      return { kind: 'MineDepleted', mineId: str(fields.mine_id), idempotencyKey };
    case 'OreRedeemedEvent':
      return {
        kind: 'CreditGoldFromOre',
        account: toAccountId(str(fields.miner)),
        oreKind: readVariant(fields.ore_kind) as OreKind,
        oreAmount: str(fields.amount),
        idempotencyKey,
      };
    default:
      return null; // MineRegenedEvent + non-business events — not bridged
  }
}
