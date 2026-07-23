#[test_only]
module obelisk_node_registry::node_registry_tests {
    use obelisk_node_registry::node_registry;
    use sui::coin;
    use sui::sui::SUI;
    use sui::test_scenario;

    const OPERATOR: address = @0xA11CE;

    fun key(byte: u8): vector<u8> {
        vector[
            byte, byte, byte, byte, byte, byte, byte, byte,
            byte, byte, byte, byte, byte, byte, byte, byte,
            byte, byte, byte, byte, byte, byte, byte, byte,
            byte, byte, byte, byte, byte, byte, byte, byte,
        ]
    }

    #[test]
    fun register_rotate_and_revoke_returns_stake() {
        let mut scenario = test_scenario::begin(OPERATOR);
        let (admin, mut registry) = node_registry::create_for_testing(scenario.ctx());
        let stake = coin::mint_for_testing<SUI>(2_000_000, scenario.ctx());
        let first_key = key(7);
        let node_id = node_registry::derive_node_id(first_key);
        let mut cap = node_registry::register_for_testing(
            &mut registry,
            stake,
            first_key,
            b"127.0.0.1:7020",
            b"test-az-a",
            128,
            8,
            1_000,
            scenario.ctx(),
        );
        assert!(node_registry::is_active(&registry, node_id), 0);
        assert!(node_registry::node_generation(&registry, node_id) == 1, 1);

        node_registry::rotate_key_for_testing(
            &mut registry,
            &mut cap,
            key(9),
            2_000,
            scenario.ctx(),
        );
        assert!(node_registry::node_generation(&registry, node_id) == 2, 2);

        let refund =
            node_registry::revoke_for_testing(&mut registry, cap, 3_000, scenario.ctx());
        assert!(coin::value(&refund) == 2_000_000, 3);
        assert!(!node_registry::is_active(&registry, node_id), 4);
        assert!(node_registry::is_retired(&registry, node_id), 5);
        coin::burn_for_testing(refund);
        node_registry::destroy_for_testing(admin, registry);
        scenario.end();
    }

    #[test, expected_failure(abort_code = 2, location = obelisk_node_registry::node_registry)]
    fun registration_rejects_understake() {
        let mut scenario = test_scenario::begin(OPERATOR);
        let (admin, mut registry) = node_registry::create_for_testing(scenario.ctx());
        let stake = coin::mint_for_testing<SUI>(999_999, scenario.ctx());
        let cap = node_registry::register_for_testing(
            &mut registry,
            stake,
            key(1),
            b"node:7020",
            b"test-az-a",
            1,
            1,
            1_000,
            scenario.ctx(),
        );
        // Expected abort occurs before these cleanup calls.
        let refund =
            node_registry::revoke_for_testing(&mut registry, cap, 2_000, scenario.ctx());
        coin::burn_for_testing(refund);
        node_registry::destroy_for_testing(admin, registry);
        scenario.end();
    }

    #[test, expected_failure(abort_code = 8, location = obelisk_node_registry::node_registry)]
    fun revoked_identity_cannot_reset_reputation_by_registering_again() {
        let mut scenario = test_scenario::begin(OPERATOR);
        let (admin, mut registry) = node_registry::create_for_testing(scenario.ctx());
        let public_key = key(3);
        let stake = coin::mint_for_testing<SUI>(2_000_000, scenario.ctx());
        let cap = node_registry::register_for_testing(
            &mut registry,
            stake,
            public_key,
            b"node:7020",
            b"test-az-a",
            8,
            2,
            1_000,
            scenario.ctx(),
        );
        let refund =
            node_registry::revoke_for_testing(&mut registry, cap, 2_000, scenario.ctx());
        coin::burn_for_testing(refund);

        let second_stake = coin::mint_for_testing<SUI>(2_000_000, scenario.ctx());
        let second_cap = node_registry::register_for_testing(
            &mut registry,
            second_stake,
            public_key,
            b"node:7020",
            b"test-az-a",
            8,
            2,
            3_000,
            scenario.ctx(),
        );
        // Expected abort occurs before these cleanup calls.
        let second_refund =
            node_registry::revoke_for_testing(&mut registry, second_cap, 4_000, scenario.ctx());
        coin::burn_for_testing(second_refund);
        node_registry::destroy_for_testing(admin, registry);
        scenario.end();
    }
}
