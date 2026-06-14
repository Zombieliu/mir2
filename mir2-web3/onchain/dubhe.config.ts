import { DubheConfig } from '@0xobelisk/sui-common';

/**
 * MIR2 on-chain smart mine — Dubhe **1.2.x** schema (dappHub / UserStorage model).
 *
 * Migrated from the 1.1.x flat-`Schema` model (SK1) to adopt Dubhe session keys: a system
 * writes a miner's per-user state in their `UserStorage`, which an ephemeral session wallet
 * may sign for on the owner's behalf (no wallet popup). See `onchain-sk/SK0-FINDINGS.md`.
 *
 * Storage split:
 *  - **global** (DappStorage): mine definitions + dynamic stones + treasury/emission.
 *  - **per-user** (UserStorage): `ore_balance` (per ore kind) + `miner_nonce` (replay guard).
 *    The 1.1.x `address` key is now implicit — it is the UserStorage's `canonical_owner`.
 *
 * Events are NOT declared here (1.2.x has no `events` config key and its event-emit helpers
 * are framework-internal `public(package)`); the systems emit plain `sui::event::emit`
 * structs (`MineSettledEvent`/`MineDepletedEvent`/`MineRegenedEvent`/`OreRedeemedEvent`),
 * preserving the field shapes the relayer reads (relayer matches by event type now).
 *
 * NO economic constants are baked in — fee/cap live in admin-set globals with safe
 * placeholders (fee 0, large cap) until the M5 sign-off.
 */
export const dubheConfig: DubheConfig = {
  name: 'mir2_mine',
  description: 'MIR2 on-chain smart mine',

  enums: {
    // Ore varieties. Index 0 = BlackIron (the refining sink).
    OreKind: ['BlackIron', 'Gold', 'Silver', 'Copper', 'Platinum', 'Ruby', 'Nephrite', 'Amethyst'],
  },

  resources: {
    // ── Global mine state (DappStorage), keyed by mine_id ──
    config: {
      global: true,
      keys: ['mine_id'],
      fields: {
        mine_id: 'u64',
        map_id: 'u32',
        mine_set: 'u8', // 1 or 2 — selects the ore/payout table
        max_stones: 'u32',
        regen_secs: 'u32',
        hit_rate: 'u8', // 0..100
        drop_rate: 'u8', // 0..100
      },
    },
    stones_left: { global: true, keys: ['mine_id'], fields: { mine_id: 'u64', stones: 'u32' } },
    next_regen_ms: { global: true, keys: ['mine_id'], fields: { mine_id: 'u64', at_ms: 'u64' } },

    // ── Global governance / treasury singletons (DappStorage) ──
    emitted_this_epoch: { global: true, fields: { value: 'u64' } },
    treasury_mist: { global: true, fields: { value: 'u64' } },
    treasury_addr: { global: true, fields: { value: 'address' } },
    per_swing_fee: { global: true, fields: { value: 'u64' } }, // MIST per swing; 0 until M5
    epoch_emission_cap: { global: true, fields: { value: 'u64' } },

    // ── Per-user state (UserStorage) — owner = the UserStorage's canonical_owner ──
    ore_balance: { keys: ['ore_kind'], fields: { ore_kind: 'OreKind', amount: 'u64' } },
    miner_nonce: { fields: { value: 'u64' } },
  },

  errors: {
    MineNotFound: 'Mine not found',
    MineExhausted: 'Mine has no stones left',
    BadNonce: 'Replay or out-of-order nonce',
    InsufficientFee: 'Attached SUI is less than the required fee',
    EmissionCapReached: 'Epoch emission cap reached',
    NotAdmin: 'Caller does not hold the admin capability',
    BadSwings: 'Swings must be greater than zero',
    InsufficientOre: 'Insufficient ore balance to redeem',
    BadAmount: 'Amount must be greater than zero',
  },
};

export default dubheConfig;
