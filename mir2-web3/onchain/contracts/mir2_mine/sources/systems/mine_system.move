/// mine_system — the authoritative batch-mining entrypoint (DESIGN §3.2).
///
/// One `mine_batch` tx settles N swings: nonce replay guard, protocol fee -> treasury,
/// regen check, `sui::random` payout roll, per-epoch emission cap, then persists mine
/// stones / ore balance / nonce and emits events for the Indexer -> Relayer -> Sim path.
///
/// The chain is the single source of truth for stones, ore ownership and the RNG
/// (DESIGN §2-A/§2-D). NO economic constants are baked in: `per_swing_fee` and
/// `epoch_emission_cap` are admin-set StorageValues read with safe defaults (fee 0,
/// large cap) until the M5 sign-off, so an un-initialised contract still works.
///
/// Structure: `mine_batch` (entry) only owns the `sui::random` roll; all invariant logic
/// (nonce/fee/cap/persist/events) is in the RNG-free `settle_no_rng`, which is what the
/// Move unit tests drive — Sui's randomness verifier forbids `#[test]` functions from
/// calling any randomness-using function, so the full RNG path is covered by the testnet
/// TS smoke instead.
module mir2_mine::mine_system {
    use sui::sui::SUI;
    use sui::coin::{Self, Coin};
    use sui::clock::Clock;
    use sui::random::{Self, Random, RandomGenerator};

    use mir2_mine::mir2_mine_schema::Schema;
    use mir2_mine::mir2_mine_dapp_system as dapp;
    use mir2_mine::mir2_mine_errors as errors;
    use mir2_mine::mir2_mine_events as events;
    use mir2_mine::mir2_mine_ore_kind as ore_kind;
    use mir2_mine::mir2_mine_ore_kind::OreKind;
    use mir2_mine::mir2_mine_mine_config as mine_config;
    use mir2_mine::mir2_mine_mine_config::MineConfig;

    #[test_only]
    use sui::test_scenario::{Self as ts};
    #[test_only]
    use sui::clock;
    #[test_only]
    use mir2_mine::mir2_mine_init_test;
    #[test_only]
    use mir2_mine::admin_system;

    /// Non-binding default emission cap when the admin has not set one (M5 sets the real
    /// value). Large enough to never bind in M1, small enough to never overflow.
    const DEFAULT_EPOCH_EMISSION_CAP: u64 = 1_000_000_000_000;

    /// Map a mine_set to its ore kind (DESIGN §3.1: set 1 = BlackIron sink, set 2 = Gold).
    /// Real per-set ore tables are M5/M6; M1 keeps a minimal deterministic mapping.
    fun ore_kind_of(mine_set: u8): OreKind {
        if (mine_set == 2) { ore_kind::new_gold() } else { ore_kind::new_black_iron() }
    }

    /// Regenerable node (DESIGN §2-C): if the mine is empty and its regen time has passed,
    /// refill to `max_stones` and emit `mine_regened`. RNG-free + `public(package)` so it
    /// is unit-testable.
    public(package) fun maybe_regen(schema: &mut Schema, mine_id: u64, max_stones: u32, now_ms: u64) {
        let left = schema.stones_left().try_get(mine_id).destroy_with_default(0u32);
        if (left == 0) {
            let next = schema.next_regen_ms().try_get(mine_id).destroy_with_default(0u64);
            if (now_ms >= next) {
                schema.stones_left().set(mine_id, max_stones);
                schema.next_regen_ms().set(mine_id, 0);
                events::mine_regened_event(mine_id, max_stones);
            }
        }
    }

    /// Settle `swings` swings against `mine_id` in one tx. `entry` + non-`public` on
    /// purpose: Sui's randomness rule forbids `public` functions from taking `&Random`
    /// (a public one would let a caller compose the roll and abort on a bad outcome).
    entry fun mine_batch(
        schema: &mut Schema,
        mine_id: u64,
        swings: u64,
        nonce: u64,
        fee: Coin<SUI>,
        random: &Random,
        clock: &Clock,
        ctx: &mut TxContext,
    ) {
        let now_ms = clock.timestamp_ms();

        // Need the config up front for regen + the roll rates.
        let cfg_opt = schema.config().try_get(mine_id);
        errors::MineNotFound_error(cfg_opt.is_some());
        let cfg: MineConfig = cfg_opt.destroy_some();

        maybe_regen(schema, mine_id, mine_config::get_max_stones(&cfg), now_ms);
        let stones_before = schema.stones_left().try_get(mine_id).destroy_with_default(0u32);

        // The only randomness-using step (DESIGN §2-D).
        let mut generator = random::new_generator(random, ctx);
        let (consumed, ore) = roll(
            &mut generator,
            swings,
            stones_before,
            mine_config::get_hit_rate(&cfg),
            mine_config::get_drop_rate(&cfg),
        );

        settle_no_rng(schema, mine_id, swings, nonce, fee, stones_before, consumed, ore, now_ms, ctx);
    }

    /// Roll `swings` swings against `stones` available, returning (stones_consumed, ore).
    /// Private (uses the RNG generator). hit_rate / drop_rate are 0..100 percentages
    /// (Crystal parity ~25 / ~10). One ore unit per successful drop; purity/amount
    /// weighting is M5/M6.
    fun roll(
        generator: &mut RandomGenerator,
        swings: u64,
        stones: u32,
        hit_rate: u8,
        drop_rate: u8,
    ): (u32, u64) {
        let mut left = stones;
        let mut consumed: u32 = 0;
        let mut ore: u64 = 0;
        let mut i: u64 = 0;
        while (i < swings && left > 0) {
            left = left - 1;
            consumed = consumed + 1;
            let r1 = random::generate_u8_in_range(generator, 0, 99);
            let r2 = random::generate_u8_in_range(generator, 0, 99);
            if (r1 < hit_rate && r2 < drop_rate) {
                ore = ore + 1;
            };
            i = i + 1;
        };
        (consumed, ore)
    }

    /// RNG-free settlement of a roll result. Validates nonce/fee/cap, charges the fee to
    /// the treasury, persists stones/ore/nonce, arms the regen timer on depletion, and
    /// emits the events. `public(package)` (callable only inside this package; external
    /// callers must use the `entry mine_batch`) and free of `sui::random` so the full
    /// invariant set is unit-testable. `consumed` must be <= `stones_before` (the roll
    /// guarantees this).
    public(package) fun settle_no_rng(
        schema: &mut Schema,
        mine_id: u64,
        swings: u64,
        nonce: u64,
        mut fee: Coin<SUI>,
        stones_before: u32,
        consumed: u32,
        ore: u64,
        now_ms: u64,
        ctx: &mut TxContext,
    ) {
        dapp::ensure_no_safe_mode(schema);
        errors::BadSwings_error(swings > 0);
        let miner = ctx.sender();

        // 1) Replay guard (idempotency #1): nonce must be exactly prev + 1.
        let prev_nonce = schema.miner_nonce().try_get(miner).destroy_with_default(0u64);
        errors::BadNonce_error(nonce == prev_nonce + 1);

        // 2) Mine must exist.
        let cfg_opt = schema.config().try_get(mine_id);
        errors::MineNotFound_error(cfg_opt.is_some());
        let cfg: MineConfig = cfg_opt.destroy_some();

        // 3) Protocol fee -> treasury (DESIGN §5). per_swing_fee defaults to 0 until M5.
        let per_fee = schema.per_swing_fee().try_get().destroy_with_default(0u64);
        let required = per_fee * swings;
        errors::InsufficientFee_error(coin::value(&fee) >= required);
        if (required > 0) {
            let pay = coin::split(&mut fee, required, ctx);
            let admin_default = *schema.dapp__admin().get();
            let treasury = schema.treasury_addr().try_get().destroy_with_default(admin_default);
            sui::transfer::public_transfer(pay, treasury);
            let cur = schema.treasury_mist().try_get().destroy_with_default(0u64);
            schema.treasury_mist().set(cur + required);
        };
        sui::transfer::public_transfer(fee, miner);

        // 4) Per-epoch emission cap (DESIGN §2-C). Defaults large until M5.
        let cap = schema.epoch_emission_cap().try_get().destroy_with_default(DEFAULT_EPOCH_EMISSION_CAP);
        let emitted = schema.emitted_this_epoch().try_get().destroy_with_default(0u64);
        errors::EmissionCapReached_error(emitted + ore <= cap);
        schema.emitted_this_epoch().set(emitted + ore);

        // 5) Persist: stones, ore balance, nonce. Arm the regen timer on depletion.
        let left = stones_before - consumed;
        schema.stones_left().set(mine_id, left);
        if (left == 0) {
            let regen_ms = (mine_config::get_regen_secs(&cfg) as u64) * 1000;
            schema.next_regen_ms().set(mine_id, now_ms + regen_ms);
        };
        let kind = ore_kind_of(mine_config::get_mine_set(&cfg));
        if (ore > 0) {
            let bal = schema.ore_balance().try_get(miner, kind).destroy_with_default(0u64);
            schema.ore_balance().set(miner, kind, bal + ore);
        };
        schema.miner_nonce().set(miner, nonce);

        // 6) Emit for the Indexer -> Relayer -> Sim path (DESIGN §4).
        events::mine_settled_event(miner, mine_id, swings, kind, ore, left, required, nonce);
        if (left == 0) {
            events::mine_depleted_event(mine_id);
        };
    }

    // Full end-to-end test of the `entry mine_batch` (roll + settle). In-module so it can
    // call the non-`public` entry, holding only `&Random` (from `create_for_testing`).
    // hit/drop = 100/100 makes the roll deterministic: every swing drops one ore.
    #[test]
    fun mine_batch_end_to_end_rolls_and_settles() {
        let admin = @0xAD;
        let player = @0xB0B;
        // `random::create_for_testing` requires the system address @0x0 as sender.
        let mut scenario = ts::begin(@0x0);
        random::create_for_testing(scenario.ctx());
        ts::next_tx(&mut scenario, admin);
        mir2_mine_init_test::deploy_dapp_for_testing(&mut scenario);
        let mut schema = ts::take_shared<Schema>(&scenario);
        admin_system::set_mine(&mut schema, 1, 0, 1, 10, 300, 100, 100, scenario.ctx());
        ts::next_tx(&mut scenario, player);
        let random = ts::take_shared<Random>(&scenario);
        let clk = clock::create_for_testing(scenario.ctx());
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_batch(&mut schema, 1, 5, 1, fee, &random, &clk, scenario.ctx());

        assert!(*schema.get_stones_left(1) == 5, 0); // 10 - 5
        assert!(*schema.get_ore_balance(player, ore_kind::new_black_iron()) == 5, 1); // 100/100
        assert!(*schema.get_miner_nonce(player) == 1, 2);

        clock::destroy_for_testing(clk);
        ts::return_shared(random);
        ts::return_shared(schema);
        ts::end(scenario);
    }
}
