import { defineConfig } from '@0xobelisk/sui-common';

/**
 * MIR2 on-chain smart mine — Dubhe config.
 *
 * M0 PLACEHOLDER. Milestone M1 (see `docs/ONCHAIN-SMART-MINE-ROADMAP.md` §4) fills the
 * real mine schema and runs `pnpm dubhe schemagen` to generate the Move modules.
 *
 * ⚠️ API DRIFT to reconcile in M1: `ONCHAIN-SMART-MINE-DESIGN.md` §3.1 was written against
 * an older Dubhe API (`schemas` / `data` / `events` / `systems` + a `storage(...)` helper).
 * The installed SDK (`@0xobelisk/sui-common@1.2.0-pre.96`) instead uses:
 *   - `enums`:      Record<string, string[]>                  (e.g. OreKind)
 *   - `components`: entity-attached data ({ fields, keys? })  (per-entity state)
 *   - `resources`:  global storage ({ fields } or MoveType)   (singletons / maps)
 *   - `errors`:     Record<string, string>
 * Events/systems are no longer config keys here (events are emitted from hand-written
 * Move systems). M1 must translate DESIGN §3.1's mine_config/mine_state/ore_balance/
 * miner_nonce/emitted_this_epoch/treasury + OreKind + mine_settled/... into this shape.
 */
export const dubheConfig = defineConfig({
  name: 'mir2_mine',
  description: 'MIR2 on-chain smart mine (M0 placeholder; M1 fills the real schema)',
  enums: {},
  components: {},
  resources: {},
});

export default dubheConfig;
