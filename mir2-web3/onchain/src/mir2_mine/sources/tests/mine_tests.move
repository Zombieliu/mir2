#[test_only]
module mir2_mine::mine_tests {
    use sui::test_scenario::{Self as ts, Scenario};
    use sui::coin;
    use sui::sui::SUI;

    use dubhe::dapp_service::{Self, DappStorage, UserStorage};
    use dubhe::dapp_system;

    use mir2_mine::dapp_key::DappKey;
    use mir2_mine::admin_system;
    use mir2_mine::mine_system;
    use mir2_mine::redeem_system;
    use mir2_mine::ore_kind::{Self, OreKind};
    use mir2_mine::stones_left;
    use mir2_mine::next_regen_ms;
    use mir2_mine::ore_balance;
    use mir2_mine::miner_nonce;
    use mir2_mine::emitted_this_epoch;
    use mir2_mine::treasury_mist;

    const ADMIN: address = @0xAD;
    const PLAYER: address = @0xB0B;
    const TREASURY: address = @0x77;

    fun black_iron(): OreKind { ore_kind::new_blackiron() }

    // DappStorage admin = ADMIN (the current sender); UserStorage owned by PLAYER. Owned
    // (not shared) test objects — passed by ref, destroyed at the end.
    fun setup(scenario: &mut Scenario): (DappStorage, UserStorage) {
        let ds = dapp_system::create_dapp_storage_for_testing<DappKey>(scenario.ctx());
        let us = dapp_system::create_user_storage_for_testing<DappKey>(PLAYER, scenario.ctx());
        (ds, us)
    }

    fun teardown(ds: DappStorage, us: UserStorage, scenario: Scenario) {
        dapp_service::destroy_dapp_storage(ds);
        dapp_service::destroy_user_storage(us);
        ts::end(scenario);
    }

    // Admin seeds mine #1 (map 0, set 1 = BlackIron), regen 300s, hit 25 / drop 10.
    fun seed_mine(scenario: &mut Scenario, ds: &mut DappStorage, max_stones: u32) {
        admin_system::set_mine(ds, 1, 0, 1, max_stones, 300, 25, 10, scenario.ctx());
    }

    // settle_no_rng(ds, us, mine_id, swings, nonce, fee, stones_before, consumed, ore, now_ms, ctx)
    // is the RNG-free core of mine_batch; tests drive it with a chosen roll result.

    #[test]
    fun settle_credits_ore_and_nonce() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());

        assert!(stones_left::get(&ds, 1) == 5, 0); // 10 - 5 consumed
        assert!(ore_balance::get(&us, black_iron()) == 5, 1);
        assert!(miner_nonce::get(&us) == 1, 2);
        assert!(emitted_this_epoch::get(&ds) == 5, 3);
        teardown(ds, us, scenario);
    }

    #[test]
    fun settle_zero_ore_credits_nothing() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 5, 1, fee, 10, 5, 0, 0, scenario.ctx());

        assert!(stones_left::get(&ds, 1) == 5, 0);
        assert!(miner_nonce::get(&us) == 1, 1);
        assert!(emitted_this_epoch::get(&ds) == 0, 2);
        assert!(!ore_balance::has(&us, black_iron()), 3);
        teardown(ds, us, scenario);
    }

    #[test]
    fun deplete_arms_regen_then_refills() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 2);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 5, 1, fee, 2, 2, 2, 0, scenario.ctx());
        assert!(stones_left::get(&ds, 1) == 0, 0);
        assert!(next_regen_ms::get(&ds, 1) == 300_000, 1); // 0 + 300s

        mine_system::maybe_regen(&mut ds, 1, 2, 300_000);
        assert!(stones_left::get(&ds, 1) == 2, 2);
        teardown(ds, us, scenario);
    }

    #[test]
    fun maybe_regen_noop_before_timer() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 2);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 2, 1, fee, 2, 2, 2, 0, scenario.ctx());
        mine_system::maybe_regen(&mut ds, 1, 2, 299_999); // before the timer -> no-op
        assert!(stones_left::get(&ds, 1) == 0, 0);
        teardown(ds, us, scenario);
    }

    #[test]
    fun settle_charges_fee_to_treasury() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 10);
        admin_system::set_per_swing_fee(&mut ds, 100, scenario.ctx()); // 100 MIST/swing
        admin_system::set_treasury_addr(&mut ds, TREASURY, scenario.ctx());
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::mint_for_testing<SUI>(1000, scenario.ctx()); // 1000 >= 100*5
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());

        assert!(treasury_mist::get(&ds) == 500, 0); // 100 * 5
        teardown(ds, us, scenario);
    }

    #[test]
    fun redeem_burns_ore() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());

        redeem_system::redeem(&ds, &mut us, black_iron(), 3, scenario.ctx());
        assert!(ore_balance::get(&us, black_iron()) == 2, 0); // 5 - 3
        teardown(ds, us, scenario);
    }

    #[test]
    #[expected_failure]
    fun settle_nonce_replay_aborts() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee1 = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 3, 1, fee1, 10, 3, 3, 0, scenario.ctx());
        let fee2 = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 3, 1, fee2, 7, 3, 3, 0, scenario.ctx()); // reuse nonce 1
        teardown(ds, us, scenario);
    }

    #[test]
    #[expected_failure]
    fun settle_zero_swings_aborts() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 0, 1, fee, 10, 0, 0, 0, scenario.ctx());
        teardown(ds, us, scenario);
    }

    #[test]
    #[expected_failure]
    fun settle_insufficient_fee_aborts() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 10);
        admin_system::set_per_swing_fee(&mut ds, 1000, scenario.ctx());
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx()); // 0 < 1000 * 5
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());
        teardown(ds, us, scenario);
    }

    #[test]
    #[expected_failure]
    fun settle_emission_cap_aborts() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 10);
        admin_system::set_epoch_emission_cap(&mut ds, 3, scenario.ctx()); // cap 3
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx()); // ore 5 > cap 3
        teardown(ds, us, scenario);
    }

    #[test]
    #[expected_failure]
    fun redeem_insufficient_ore_aborts() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, mut us) = setup(&mut scenario);
        seed_mine(&mut scenario, &mut ds, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut ds, &mut us, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());
        redeem_system::redeem(&ds, &mut us, black_iron(), 10, scenario.ctx()); // only 5
        teardown(ds, us, scenario);
    }

    #[test]
    #[expected_failure]
    fun set_mine_requires_admin() {
        let mut scenario = ts::begin(ADMIN);
        let (mut ds, us) = setup(&mut scenario);
        ts::next_tx(&mut scenario, PLAYER); // not the admin
        admin_system::set_mine(&mut ds, 1, 0, 1, 10, 300, 25, 10, scenario.ctx());
        teardown(ds, us, scenario);
    }
}
