import { DubheConfig } from '@0xobelisk/sui-common';

/**
 * MIR2 on-chain smart mine — Dubhe schema.
 *
 * Translated from `ONCHAIN-SMART-MINE-DESIGN.md` §3.1 into the **stable** `@0xobelisk`
 * 1.1.x config API (`data` / `schemas` / `events` / `errors`, `StorageMap<>` strings) —
 * the API both canonical examples (constantinople, dms) use. `pnpm dubhe schemagen`
 * generates the Move modules under `contracts/mir2_mine/sources/codegen/`.
 *
 * Authority (DESIGN §2-A): the chain owns mine stones, ore ownership and the payout RNG.
 * NO economic constants are baked in — fee/cap live in admin-set StorageValues with safe
 * placeholders (fee 0, large cap) until the M5 sign-off (ROADMAP §8/§11).
 */
export const dubheConfig = {
  name: 'mir2_mine',
  description: 'MIR2 on-chain smart mine',

  data: {
    // Ore varieties (DESIGN §3.1). Index 0 = BlackIron (the refining sink).
    OreKind: ['BlackIron', 'Gold', 'Silver', 'Copper', 'Platinum', 'Ruby', 'Nephrite', 'Amethyst'],
    // Static config of one mine (mirrors the game's MineZoneRecord + MineSet).
    MineConfig: {
      map_id: 'u32',
      mine_set: 'u8', // 1 or 2 — selects the ore/payout table
      max_stones: 'u32', // full inventory
      regen_secs: 'u32', // regen period (seconds)
      hit_rate: 'u8', // 0..100 (Crystal parity ~25)
      drop_rate: 'u8', // 0..100 (Crystal parity ~10)
    },
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

  events: {
    // One batch of swings settled (Indexer subscribes -> Relayer -> Sim).
    mine_settled: {
      miner: 'address',
      mine_id: 'u64',
      swings: 'u64',
      ore_kind: 'OreKind',
      ore_amount: 'u64',
      stones_left: 'u32',
      fee_paid: 'u64',
      nonce: 'u64',
    },
    mine_depleted: { mine_id: 'u64' },
    mine_regened: { mine_id: 'u64', stones_left: 'u32' },
    // Ore burned for game gold (Relayer -> CreditGoldFromOre in Sim).
    ore_redeemed: { miner: 'address', ore_kind: 'OreKind', amount: 'u64' },
  },

  // NOTE (Dubhe 1.1.x): `schemas` is a FLAT field->type map — one project-level Schema
  // object (`mir2_mine::mir2_mine_schema`) carrying these fields plus built-in dapp fields
  // (dapp__admin, dapp__package_id, dapp__version, …). The nested `{ name: { ...fields } }`
  // form in DESIGN §3.1 / the constantinople example is the older 1.0.x API.
  schemas: {
    // Per-mine static config + dynamic state (key = mine_id).
    config: 'StorageMap<u64, MineConfig>',
    stones_left: 'StorageMap<u64, u32>',
    next_regen_ms: 'StorageMap<u64, u64>',
    // Per (miner, ore_kind) ore balance.
    ore_balance: 'StorageDoubleMap<address, OreKind, u64>',
    // Replay guard: highest settled nonce per miner.
    miner_nonce: 'StorageMap<address, u64>',
    // Emission governance: ore emitted in the current epoch.
    emitted_this_epoch: 'StorageValue<u64>',
    // Treasury accounting (MIST) + fee recipient (DESIGN §5).
    treasury_mist: 'StorageValue<u64>',
    treasury_addr: 'StorageValue<address>',
    // ── Admin-set governance params — placeholders until the M5 sign-off ──
    per_swing_fee: 'StorageValue<u64>', // MIST per swing; 0 until M5
    epoch_emission_cap: 'StorageValue<u64>', // max ore per epoch
  },
} as DubheConfig;

export default dubheConfig;
