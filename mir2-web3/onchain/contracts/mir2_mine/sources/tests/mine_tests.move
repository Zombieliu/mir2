#[test_only]
module mir2_mine::mine_tests {
    use sui::test_scenario::{Self as ts, Scenario};
    use sui::coin;
    use sui::sui::SUI;

    use mir2_mine::mir2_mine_schema::Schema;
    use mir2_mine::mir2_mine_init_test;
    use mir2_mine::mir2_mine_ore_kind;
    use mir2_mine::admin_system;
    use mir2_mine::mine_system;
    use mir2_mine::redeem_system;

    const ADMIN: address = @0xAD;
    const PLAYER: address = @0xB0B;
    const TREASURY: address = @0x77;

    // Deploy the dapp; leaves a fresh tx with sender = ADMIN and the Schema shared.
    fun begin(): Scenario {
        let mut scenario = ts::begin(ADMIN);
        mir2_mine_init_test::deploy_dapp_for_testing(&mut scenario);
        scenario
    }

    // Admin seeds mine #1 (map 0, set 1 = BlackIron), regen 300s.
    fun seed_mine(scenario: &mut Scenario, schema: &mut Schema, max_stones: u32) {
        admin_system::set_mine(schema, 1, 0, 1, max_stones, 300, 25, 10, scenario.ctx());
    }

    fun black_iron(): mir2_mine_ore_kind::OreKind {
        mir2_mine_ore_kind::new_black_iron()
    }

    // settle_no_rng(schema, mine_id, swings, nonce, fee, stones_before, consumed, ore, now_ms, ctx)
    // is the RNG-free core of mine_batch; tests drive it with a chosen roll result.

    #[test]
    fun settle_credits_ore_and_nonce() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut schema, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());

        assert!(*schema.get_stones_left(1) == 5, 0); // 10 - 5 consumed
        assert!(*schema.get_ore_balance(PLAYER, black_iron()) == 5, 1);
        assert!(*schema.get_miner_nonce(PLAYER) == 1, 2);
        assert!(*schema.get_emitted_this_epoch() == 5, 3);

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    fun settle_zero_ore_credits_nothing() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut schema, 1, 5, 1, fee, 10, 5, 0, 0, scenario.ctx());

        assert!(*schema.get_stones_left(1) == 5, 0);
        assert!(*schema.get_miner_nonce(PLAYER) == 1, 1);
        assert!(*schema.get_emitted_this_epoch() == 0, 2);
        assert!(schema.borrow_ore_balance().try_get(PLAYER, black_iron()).is_none(), 3);

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    fun deplete_arms_regen_then_refills() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 2);
        ts::next_tx(&mut scenario, PLAYER);

        // Consume both stones at now = 0 -> deplete, regen armed for 300s.
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut schema, 1, 5, 1, fee, 2, 2, 2, 0, scenario.ctx());
        assert!(*schema.get_stones_left(1) == 0, 0);
        assert!(*schema.get_next_regen_ms(1) == 300_000, 1); // 0 + 300s

        // Regen at/after the timer refills to max.
        mine_system::maybe_regen(&mut schema, 1, 2, 300_000);
        assert!(*schema.get_stones_left(1) == 2, 2);

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    fun maybe_regen_noop_before_timer() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 2);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut schema, 1, 2, 1, fee, 2, 2, 2, 0, scenario.ctx()); // deplete, regen at 300s
        mine_system::maybe_regen(&mut schema, 1, 2, 299_999); // before the timer -> no-op
        assert!(*schema.get_stones_left(1) == 0, 0);

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    fun settle_charges_fee_to_treasury() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 10);
        admin_system::set_per_swing_fee(&mut schema, 100, scenario.ctx()); // 100 MIST/swing
        admin_system::set_treasury_addr(&mut schema, TREASURY, scenario.ctx());
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::mint_for_testing<SUI>(1000, scenario.ctx()); // 1000 >= 100*5
        mine_system::settle_no_rng(&mut schema, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());

        assert!(*schema.get_treasury_mist() == 500, 0); // 100 * 5

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    fun redeem_burns_ore() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut schema, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());

        redeem_system::redeem(&mut schema, black_iron(), 3, scenario.ctx());
        assert!(*schema.get_ore_balance(PLAYER, black_iron()) == 2, 0); // 5 - 3

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    #[expected_failure]
    fun settle_nonce_replay_aborts() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee1 = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut schema, 1, 3, 1, fee1, 10, 3, 3, 0, scenario.ctx());
        let fee2 = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut schema, 1, 3, 1, fee2, 7, 3, 3, 0, scenario.ctx()); // reuse nonce 1

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    #[expected_failure]
    fun settle_zero_swings_aborts() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut schema, 1, 0, 1, fee, 10, 0, 0, 0, scenario.ctx());

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    #[expected_failure]
    fun settle_insufficient_fee_aborts() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 10);
        admin_system::set_per_swing_fee(&mut schema, 1000, scenario.ctx());
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx()); // 0 < 1000 * 5
        mine_system::settle_no_rng(&mut schema, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    #[expected_failure]
    fun settle_emission_cap_aborts() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 10);
        admin_system::set_epoch_emission_cap(&mut schema, 3, scenario.ctx()); // cap 3
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        // ore 5 > cap 3 -> abort.
        mine_system::settle_no_rng(&mut schema, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    #[expected_failure]
    fun redeem_insufficient_ore_aborts() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        seed_mine(&mut scenario, &mut schema, 10);
        ts::next_tx(&mut scenario, PLAYER);
        let fee = coin::zero<SUI>(scenario.ctx());
        mine_system::settle_no_rng(&mut schema, 1, 5, 1, fee, 10, 5, 5, 0, scenario.ctx());
        redeem_system::redeem(&mut schema, black_iron(), 10, scenario.ctx()); // only 5

        ts::return_shared(schema);
        ts::end(scenario);
    }

    #[test]
    #[expected_failure]
    fun set_mine_requires_admin() {
        let mut scenario = begin();
        let mut schema = ts::take_shared<Schema>(&scenario);
        ts::next_tx(&mut scenario, PLAYER); // not the admin
        admin_system::set_mine(&mut schema, 1, 0, 1, 10, 300, 25, 10, scenario.ctx());

        ts::return_shared(schema);
        ts::end(scenario);
    }
}
