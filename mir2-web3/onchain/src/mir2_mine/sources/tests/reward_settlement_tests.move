#[test_only]
module mir2_mine::reward_settlement_tests {
    use mir2_mine::reward_settlement;
    use sui::test_scenario;

    const ADMIN: address = @0xA;

    #[test]
    fun one_batch_per_game_epoch_and_claim_key_is_unset_initially() {
        let mut scenario = test_scenario::begin(ADMIN);
        let ctx = scenario.ctx();
        let (cap, mut registry) = reward_settlement::create_for_testing(ctx);
        let batch_id = b"batch-1";
        let node_id = b"guild-a";
        reward_settlement::publish_batch(
            &mut registry,
            &cap,
            batch_id,
            b"mir2",
            7,
            x"abababababababababababababababababababababababababababababababab",
            1_000,
            2,
            42,
        );
        assert!(reward_settlement::contains_batch(&registry, batch_id), 0);
        assert!(!reward_settlement::is_claimed(&registry, batch_id, node_id), 1);
        // Tables intentionally contain the published audit record and cannot be destroyed here.
        transfer::public_transfer(cap, ADMIN);
        reward_settlement::share_for_testing(registry);
        scenario.end();
    }

    #[test, expected_failure(abort_code = 2, location = mir2_mine::reward_settlement)]
    fun duplicate_game_epoch_aborts() {
        let mut scenario = test_scenario::begin(ADMIN);
        let ctx = scenario.ctx();
        let (cap, mut registry) = reward_settlement::create_for_testing(ctx);
        reward_settlement::publish_batch(
            &mut registry,
            &cap,
            b"batch-1",
            b"mir2",
            7,
            x"abababababababababababababababababababababababababababababababab",
            1_000,
            2,
            42,
        );
        reward_settlement::publish_batch(
            &mut registry,
            &cap,
            b"batch-2",
            b"mir2",
            7,
            x"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
            2_000,
            2,
            43,
        );
        abort 99
    }
}
