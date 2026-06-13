/// mine_system — the authoritative batch-mining entrypoint (DESIGN §3.2), Dubhe 1.2.x.
///
/// One `mine_batch` tx settles N swings: nonce replay guard, protocol fee -> treasury,
/// regen check, `sui::random` payout roll, per-epoch emission cap, then persists mine
/// stones / ore balance / nonce and emits events for the Indexer -> Relayer -> Sim path.
///
/// SK1 migration to the dappHub/UserStorage model:
///  - global mine state (config/stones/treasury/emission) lives in `DappStorage`;
///  - per-miner state (ore_balance/miner_nonce) lives in the miner's `UserStorage`;
///  - the **miner = `dapp_service::canonical_owner(user_storage)`**, NOT `ctx.sender()` —
///    so an ephemeral session wallet may sign `mine_batch` on the owner's behalf (the
///    framework's `set_record` authorizes owner OR active session) and the OWNER is
///    credited. See `onchain-sk/SK0-FINDINGS.md`.
///  - events are plain `sui::event::emit` structs (1.2.x emit helpers are framework-
///    internal); field shapes are preserved for the relayer.
///
/// NO economic constants are baked in: `per_swing_fee` and `epoch_emission_cap` are
/// admin-set globals read with safe defaults (fee 0, large cap) until the M5 sign-off.
///
/// Structure: `mine_batch` (entry) only owns the `sui::random` roll; all invariant logic
/// (nonce/fee/cap/persist/events) is in the RNG-free `settle_no_rng`, which the Move unit
/// tests drive — Sui's randomness verifier forbids `#[test]` functions from calling any
/// randomness-using function, so the full RNG path is covered by the testnet TS smoke.
module mir2_mine::mine_system {
    use sui::sui::SUI;
    use sui::coin::{Self, Coin};
    use sui::clock::Clock;
    use sui::random::{Self, Random, RandomGenerator};
    use sui::event;

    use dubhe::dapp_service::{Self, DappStorage, UserStorage};
    use dubhe::dapp_system;

    use mir2_mine::dapp_key::DappKey;
    use mir2_mine::config;
    use mir2_mine::stones_left;
    use mir2_mine::next_regen_ms;
    use mir2_mine::ore_balance;
    use mir2_mine::miner_nonce;
    use mir2_mine::per_swing_fee;
    use mir2_mine::epoch_emission_cap;
    use mir2_mine::emitted_this_epoch;
    use mir2_mine::treasury_addr;
    use mir2_mine::treasury_mist;
    use mir2_mine::ore_kind::{Self, OreKind};
    use mir2_mine::error;
    use mir2_mine::migrate;

    #[test_only]
    use sui::test_scenario::{Self as ts};
    #[test_only]
    use sui::clock;
    #[test_only]
    use mir2_mine::admin_system;

    /// Non-binding default emission cap when the admin has not set one (M5 sets the real
    /// value). Large enough to never bind in M1, small enough to never overflow.
    const DEFAULT_EPOCH_EMISSION_CAP: u64 = 1_000_000_000_000;

    // ── Events (plain Sui events; the relayer reads `type` + the parsed fields) ──
    public struct MineSettledEvent has copy, drop {
        miner: address,
        mine_id: u64,
        swings: u64,
        ore_kind: OreKind,
        ore_amount: u64,
        stones_left: u32,
        fee_paid: u64,
        nonce: u64,
    }
    public struct MineDepletedEvent has copy, drop { mine_id: u64 }
    public struct MineRegenedEvent has copy, drop { mine_id: u64, stones_left: u32 }

    // ── Default-reading helpers (the generated `get` aborts when a key is absent) ──
    fun stones_or0(ds: &DappStorage, mine_id: u64): u32 {
        if (stones_left::has(ds, mine_id)) { stones_left::get(ds, mine_id) } else { 0 }
    }
    fun next_regen_or0(ds: &DappStorage, mine_id: u64): u64 {
        if (next_regen_ms::has(ds, mine_id)) { next_regen_ms::get(ds, mine_id) } else { 0 }
    }
    fun per_fee_or0(ds: &DappStorage): u64 {
        if (per_swing_fee::has(ds)) { per_swing_fee::get(ds) } else { 0 }
    }
    fun cap_or_default(ds: &DappStorage): u64 {
        if (epoch_emission_cap::has(ds)) { epoch_emission_cap::get(ds) } else { DEFAULT_EPOCH_EMISSION_CAP }
    }
    fun emitted_or0(ds: &DappStorage): u64 {
        if (emitted_this_epoch::has(ds)) { emitted_this_epoch::get(ds) } else { 0 }
    }
    fun balance_or0(us: &UserStorage, kind: OreKind): u64 {
        if (ore_balance::has(us, kind)) { ore_balance::get(us, kind) } else { 0 }
    }
    fun nonce_or0(us: &UserStorage): u64 {
        if (miner_nonce::has(us)) { miner_nonce::get(us) } else { 0 }
    }
    fun treasury_or(ds: &DappStorage, fallback: address): address {
        if (treasury_addr::has(ds)) { treasury_addr::get(ds) } else { fallback }
    }

    /// Map a mine_set to its ore kind (DESIGN §3.1: set 1 = BlackIron sink, set 2 = Gold).
    fun ore_kind_of(mine_set: u8): OreKind {
        if (mine_set == 2) { ore_kind::new_gold() } else { ore_kind::new_blackiron() }
    }

    /// Regenerable node (DESIGN §2-C): if the mine is empty and its regen time has passed,
    /// refill to `max_stones` and emit `MineRegenedEvent`. RNG-free + `public(package)`.
    public(package) fun maybe_regen(ds: &mut DappStorage, mine_id: u64, max_stones: u32, now_ms: u64) {
        if (stones_or0(ds, mine_id) == 0) {
            if (now_ms >= next_regen_or0(ds, mine_id)) {
                stones_left::set(ds, mine_id, max_stones);
                next_regen_ms::set(ds, mine_id, 0);
                event::emit(MineRegenedEvent { mine_id, stones_left: max_stones });
            }
        }
    }

    /// Settle `swings` swings against `mine_id` in one tx. `entry` + non-`public` on
    /// purpose: Sui's randomness rule forbids `public` functions from taking `&Random`.
    /// `user_storage` is the MINER's storage — a session wallet may sign on the owner's
    /// behalf and the framework credits the owner.
    entry fun mine_batch(
        dapp_storage: &mut DappStorage,
        user_storage: &mut UserStorage,
        mine_id: u64,
        swings: u64,
        nonce: u64,
        fee: Coin<SUI>,
        random: &Random,
        clock: &Clock,
        ctx: &mut TxContext,
    ) {
        let now_ms = clock.timestamp_ms();

        error::MineNotFound(config::has(dapp_storage, mine_id));
        let (_map_id, _mine_set, max_stones, _regen_secs, hit_rate, drop_rate) =
            config::get(dapp_storage, mine_id);

        maybe_regen(dapp_storage, mine_id, max_stones, now_ms);
        let stones_before = stones_or0(dapp_storage, mine_id);

        // The only randomness-using step (DESIGN §2-D).
        let mut generator = random::new_generator(random, ctx);
        let (consumed, ore) = roll(&mut generator, swings, stones_before, hit_rate, drop_rate);

        settle_no_rng(
            dapp_storage, user_storage, mine_id, swings, nonce, fee, stones_before, consumed, ore, now_ms, ctx,
        );
    }

    /// Roll `swings` swings against `stones` available, returning (stones_consumed, ore).
    /// hit_rate / drop_rate are 0..100 percentages. One ore unit per successful drop.
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
    /// the treasury, persists stones (global) / ore + nonce (per-user), arms the regen
    /// timer on depletion, and emits events. `public(package)` + RNG-free so the full
    /// invariant set is unit-testable. `consumed` must be <= `stones_before`.
    public(package) fun settle_no_rng(
        dapp_storage: &mut DappStorage,
        user_storage: &mut UserStorage,
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
        dapp_system::ensure_not_paused<DappKey>(dapp_storage);
        dapp_system::ensure_latest_version<DappKey>(dapp_storage, migrate::on_chain_version());
        error::BadSwings(swings > 0);

        // Owner attribution: the UserStorage's canonical owner (session-signed or not).
        let miner = dapp_service::canonical_owner(user_storage);

        // 1) Replay guard (idempotency #1): nonce must be exactly prev + 1.
        error::BadNonce(nonce == nonce_or0(user_storage) + 1);

        // 2) Mine must exist; pull its settlement-relevant config.
        error::MineNotFound(config::has(dapp_storage, mine_id));
        let (_map_id, mine_set, _max_stones, regen_secs, _hit_rate, _drop_rate) =
            config::get(dapp_storage, mine_id);

        // 3) Protocol fee -> treasury (DESIGN §5). per_swing_fee defaults to 0 until M5.
        let per_fee = per_fee_or0(dapp_storage);
        let required = per_fee * swings;
        error::InsufficientFee(coin::value(&fee) >= required);
        if (required > 0) {
            let pay = coin::split(&mut fee, required, ctx);
            let treasury = treasury_or(dapp_storage, dapp_service::dapp_admin(dapp_storage));
            sui::transfer::public_transfer(pay, treasury);
            let new_treasury_mist = treasury_mist_or0(dapp_storage) + required;
            treasury_mist::set(dapp_storage, new_treasury_mist);
        };
        // Refund any remainder to the signer (the gas/fee payer — owner OR session wallet).
        sui::transfer::public_transfer(fee, ctx.sender());

        // 4) Per-epoch emission cap (DESIGN §2-C). Defaults large until M5.
        let cap = cap_or_default(dapp_storage);
        let emitted = emitted_or0(dapp_storage);
        error::EmissionCapReached(emitted + ore <= cap);
        emitted_this_epoch::set(dapp_storage, emitted + ore);

        // 5) Persist: stones (global), ore balance + nonce (per-user). Arm regen on depletion.
        let left = stones_before - consumed;
        stones_left::set(dapp_storage, mine_id, left);
        if (left == 0) {
            next_regen_ms::set(dapp_storage, mine_id, now_ms + (regen_secs as u64) * 1000);
        };
        let kind = ore_kind_of(mine_set);
        if (ore > 0) {
            let new_balance = balance_or0(user_storage, kind) + ore;
            ore_balance::set(user_storage, kind, new_balance, ctx);
        };
        miner_nonce::set(user_storage, nonce, ctx);

        // 6) Emit for the Indexer -> Relayer -> Sim path (DESIGN §4).
        event::emit(MineSettledEvent {
            miner,
            mine_id,
            swings,
            ore_kind: kind,
            ore_amount: ore,
            stones_left: left,
            fee_paid: required,
            nonce,
        });
        if (left == 0) {
            event::emit(MineDepletedEvent { mine_id });
        };
    }

    fun treasury_mist_or0(ds: &DappStorage): u64 {
        if (treasury_mist::has(ds)) { treasury_mist::get(ds) } else { 0 }
    }

    // Full end-to-end test of the `entry mine_batch` (roll + settle). In-module so it can
    // call the non-`public` entry, holding only `&Random`. hit/drop = 100/100 makes the
    // roll deterministic: every swing drops one ore. Sender == the UserStorage owner.
    #[test]
    fun mine_batch_end_to_end_rolls_and_settles() {
        let player = @0xB0B;
        // `random::create_for_testing` requires the system address @0x0 as sender.
        let mut scenario = ts::begin(@0x0);
        random::create_for_testing(scenario.ctx());

        ts::next_tx(&mut scenario, player);
        let mut dapp_storage = dapp_system::create_dapp_storage_for_testing<DappKey>(scenario.ctx());
        let mut user_storage = dapp_system::create_user_storage_for_testing<DappKey>(player, scenario.ctx());
        admin_system::set_mine(&mut dapp_storage, 1, 0, 1, 10, 300, 100, 100, scenario.ctx());

        ts::next_tx(&mut scenario, player);
        let random = ts::take_shared<Random>(&scenario);
        let clk = clock::create_for_testing(scenario.ctx());
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_batch(&mut dapp_storage, &mut user_storage, 1, 5, 1, fee, &random, &clk, scenario.ctx());

        assert!(stones_left::get(&dapp_storage, 1) == 5, 0); // 10 - 5
        assert!(ore_balance::get(&user_storage, ore_kind::new_blackiron()) == 5, 1); // 100/100
        assert!(miner_nonce::get(&user_storage) == 1, 2);

        clock::destroy_for_testing(clk);
        ts::return_shared(random);
        dapp_service::destroy_dapp_storage(dapp_storage);
        dapp_service::destroy_user_storage(user_storage);
        ts::end(scenario);
    }
}
