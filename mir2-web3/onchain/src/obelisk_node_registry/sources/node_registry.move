/// Permissionless Sui registry for Obelisk guild compute nodes.
///
/// A stable node id is SHA-256(domain || first Ed25519 public key). Key rotation
/// increments the generation while preserving the node id and staked identity.
module obelisk_node_registry::node_registry {
    use std::hash::sha2_256;
    use sui::balance::{Self, Balance};
    use sui::clock::Clock;
    use sui::coin::{Self, Coin};
    use sui::event;
    use sui::sui::SUI;
    use sui::table::{Self, Table};

    const NODE_ID_DOMAIN: vector<u8> = b"obelisk.guild-node.ed25519.v1\0";
    const INITIAL_MIN_STAKE_MIST: u64 = 1_000_000;

    const E_BAD_PUBLIC_KEY: u64 = 0;
    const E_BAD_METADATA: u64 = 1;
    const E_STAKE_TOO_SMALL: u64 = 2;
    const E_NODE_EXISTS: u64 = 3;
    const E_NODE_NOT_FOUND: u64 = 4;
    const E_STALE_CAPABILITY: u64 = 5;
    const E_NODE_INACTIVE: u64 = 6;
    const E_BAD_SLASH: u64 = 7;
    const E_NODE_RETIRED: u64 = 8;

    public struct RegistryAdminCap has key, store {
        id: UID,
    }

    public struct NodeOwnerCap has key, store {
        id: UID,
        node_id: vector<u8>,
        generation: u64,
    }

    public struct NodeRegistry has key {
        id: UID,
        min_stake_mist: u64,
        treasury: Balance<SUI>,
        nodes: Table<vector<u8>, NodeRecord>,
        retired: Table<vector<u8>, bool>,
    }

    public struct NodeRecord has store {
        operator: address,
        public_key: vector<u8>,
        endpoint: vector<u8>,
        failure_domain: vector<u8>,
        stake: Balance<SUI>,
        max_sessions: u64,
        max_zones: u64,
        generation: u64,
        active: bool,
        updated_at_ms: u64,
    }

    public struct NodeRegisteredEvent has copy, drop {
        node_id: vector<u8>,
        operator: address,
        public_key: vector<u8>,
        endpoint: vector<u8>,
        failure_domain: vector<u8>,
        stake_mist: u64,
        max_sessions: u64,
        max_zones: u64,
        generation: u64,
        observed_at_ms: u64,
    }

    public struct NodeKeyRotatedEvent has copy, drop {
        node_id: vector<u8>,
        operator: address,
        public_key: vector<u8>,
        generation: u64,
        observed_at_ms: u64,
    }

    public struct NodeMetadataUpdatedEvent has copy, drop {
        node_id: vector<u8>,
        endpoint: vector<u8>,
        failure_domain: vector<u8>,
        max_sessions: u64,
        max_zones: u64,
        generation: u64,
        observed_at_ms: u64,
    }

    public struct NodeSlashedEvent has copy, drop {
        node_id: vector<u8>,
        amount_mist: u64,
        remaining_stake_mist: u64,
        active: bool,
        observed_at_ms: u64,
    }

    public struct NodeRevokedEvent has copy, drop {
        node_id: vector<u8>,
        operator: address,
        returned_stake_mist: u64,
        generation: u64,
        observed_at_ms: u64,
    }

    fun init(ctx: &mut TxContext) {
        let (admin, registry) = create(ctx);
        transfer::transfer(admin, ctx.sender());
        transfer::share_object(registry);
    }

    fun create(ctx: &mut TxContext): (RegistryAdminCap, NodeRegistry) {
        (
            RegistryAdminCap { id: object::new(ctx) },
            NodeRegistry {
                id: object::new(ctx),
                min_stake_mist: INITIAL_MIN_STAKE_MIST,
                treasury: balance::zero(),
                nodes: table::new(ctx),
                retired: table::new(ctx),
            },
        )
    }

    /// Direct registration intentionally transfers the owner capability to the
    /// signer so a one-command CLI/Docker bootstrap cannot strand the key object.
    #[allow(lint(self_transfer))]
    public fun register(
        registry: &mut NodeRegistry,
        stake: Coin<SUI>,
        public_key: vector<u8>,
        endpoint: vector<u8>,
        failure_domain: vector<u8>,
        max_sessions: u64,
        max_zones: u64,
        clock: &Clock,
        ctx: &mut TxContext,
    ) {
        let cap = register_internal(
            registry,
            stake,
            public_key,
            endpoint,
            failure_domain,
            max_sessions,
            max_zones,
            clock.timestamp_ms(),
            ctx,
        );
        transfer::transfer(cap, ctx.sender());
    }

    fun register_internal(
        registry: &mut NodeRegistry,
        stake: Coin<SUI>,
        public_key: vector<u8>,
        endpoint: vector<u8>,
        failure_domain: vector<u8>,
        max_sessions: u64,
        max_zones: u64,
        now_ms: u64,
        ctx: &mut TxContext,
    ): NodeOwnerCap {
        assert!(public_key.length() == 32, E_BAD_PUBLIC_KEY);
        assert!(
            !endpoint.is_empty()
                && endpoint.length() <= 255
                && !failure_domain.is_empty()
                && failure_domain.length() <= 128
                && max_sessions > 0
                && max_zones > 0,
            E_BAD_METADATA,
        );
        let mut id_material = NODE_ID_DOMAIN;
        id_material.append(public_key);
        let node_id = sha2_256(id_material);
        assert!(!registry.nodes.contains(node_id), E_NODE_EXISTS);
        assert!(!registry.retired.contains(node_id), E_NODE_RETIRED);
        let stake = coin::into_balance(stake);
        let stake_mist = stake.value();
        assert!(stake_mist >= registry.min_stake_mist, E_STAKE_TOO_SMALL);
        let generation = 1;
        registry.nodes.add(
            node_id,
            NodeRecord {
                operator: ctx.sender(),
                public_key,
                endpoint,
                failure_domain,
                stake,
                max_sessions,
                max_zones,
                generation,
                active: true,
                updated_at_ms: now_ms,
            },
        );
        event::emit(NodeRegisteredEvent {
            node_id,
            operator: ctx.sender(),
            public_key,
            endpoint,
            failure_domain,
            stake_mist,
            max_sessions,
            max_zones,
            generation,
            observed_at_ms: now_ms,
        });
        NodeOwnerCap {
            id: object::new(ctx),
            node_id,
            generation,
        }
    }

    public fun rotate_key(
        registry: &mut NodeRegistry,
        cap: &mut NodeOwnerCap,
        public_key: vector<u8>,
        clock: &Clock,
        ctx: &TxContext,
    ) {
        assert!(public_key.length() == 32, E_BAD_PUBLIC_KEY);
        let record = active_record_mut(registry, cap);
        record.public_key = public_key;
        record.generation = record.generation + 1;
        record.updated_at_ms = clock.timestamp_ms();
        cap.generation = record.generation;
        event::emit(NodeKeyRotatedEvent {
            node_id: cap.node_id,
            operator: ctx.sender(),
            public_key,
            generation: record.generation,
            observed_at_ms: record.updated_at_ms,
        });
    }

    public fun update_metadata(
        registry: &mut NodeRegistry,
        cap: &NodeOwnerCap,
        endpoint: vector<u8>,
        failure_domain: vector<u8>,
        max_sessions: u64,
        max_zones: u64,
        clock: &Clock,
    ) {
        assert!(
            !endpoint.is_empty()
                && endpoint.length() <= 255
                && !failure_domain.is_empty()
                && failure_domain.length() <= 128
                && max_sessions > 0
                && max_zones > 0,
            E_BAD_METADATA,
        );
        let record = active_record_mut(registry, cap);
        record.endpoint = endpoint;
        record.failure_domain = failure_domain;
        record.max_sessions = max_sessions;
        record.max_zones = max_zones;
        record.updated_at_ms = clock.timestamp_ms();
        event::emit(NodeMetadataUpdatedEvent {
            node_id: cap.node_id,
            endpoint,
            failure_domain,
            max_sessions,
            max_zones,
            generation: record.generation,
            observed_at_ms: record.updated_at_ms,
        });
    }

    public fun add_stake(
        registry: &mut NodeRegistry,
        cap: &NodeOwnerCap,
        payment: Coin<SUI>,
        clock: &Clock,
    ) {
        let record = active_record_mut(registry, cap);
        balance::join(&mut record.stake, coin::into_balance(payment));
        record.updated_at_ms = clock.timestamp_ms();
    }

    public fun slash(
        registry: &mut NodeRegistry,
        _admin: &RegistryAdminCap,
        node_id: vector<u8>,
        amount_mist: u64,
        clock: &Clock,
    ) {
        assert!(registry.nodes.contains(node_id), E_NODE_NOT_FOUND);
        let record = registry.nodes.borrow_mut(node_id);
        assert!(record.active, E_NODE_INACTIVE);
        assert!(amount_mist > 0 && amount_mist <= record.stake.value(), E_BAD_SLASH);
        let slashed = balance::split(&mut record.stake, amount_mist);
        balance::join(&mut registry.treasury, slashed);
        let remaining = record.stake.value();
        if (remaining < registry.min_stake_mist) {
            record.active = false;
        };
        record.updated_at_ms = clock.timestamp_ms();
        event::emit(NodeSlashedEvent {
            node_id,
            amount_mist,
            remaining_stake_mist: remaining,
            active: record.active,
            observed_at_ms: record.updated_at_ms,
        });
    }

    public fun revoke(
        registry: &mut NodeRegistry,
        cap: NodeOwnerCap,
        clock: &Clock,
        ctx: &mut TxContext,
    ) {
        let NodeOwnerCap { id, node_id, generation } = cap;
        assert!(registry.nodes.contains(node_id), E_NODE_NOT_FOUND);
        let NodeRecord {
            operator,
            public_key: _,
            endpoint: _,
            failure_domain: _,
            stake,
            max_sessions: _,
            max_zones: _,
            generation: current_generation,
            active: _,
            updated_at_ms: _,
        } = registry.nodes.remove(node_id);
        assert!(generation == current_generation, E_STALE_CAPABILITY);
        registry.retired.add(node_id, true);
        let returned_stake_mist = stake.value();
        let refund = coin::from_balance(stake, ctx);
        transfer::public_transfer(refund, operator);
        object::delete(id);
        event::emit(NodeRevokedEvent {
            node_id,
            operator,
            returned_stake_mist,
            generation,
            observed_at_ms: clock.timestamp_ms(),
        });
    }

    fun active_record_mut(
        registry: &mut NodeRegistry,
        cap: &NodeOwnerCap,
    ): &mut NodeRecord {
        assert!(registry.nodes.contains(cap.node_id), E_NODE_NOT_FOUND);
        let record = registry.nodes.borrow_mut(cap.node_id);
        assert!(record.active, E_NODE_INACTIVE);
        assert!(record.generation == cap.generation, E_STALE_CAPABILITY);
        record
    }

    public fun derive_node_id(public_key: vector<u8>): vector<u8> {
        assert!(public_key.length() == 32, E_BAD_PUBLIC_KEY);
        let mut material = NODE_ID_DOMAIN;
        material.append(public_key);
        sha2_256(material)
    }

    public fun is_active(registry: &NodeRegistry, node_id: vector<u8>): bool {
        registry.nodes.contains(node_id) && registry.nodes.borrow(node_id).active
    }

    public fun is_retired(registry: &NodeRegistry, node_id: vector<u8>): bool {
        registry.retired.contains(node_id)
    }

    public fun min_stake_mist(registry: &NodeRegistry): u64 {
        registry.min_stake_mist
    }

    public fun treasury_balance(registry: &NodeRegistry): u64 {
        registry.treasury.value()
    }

    public fun node_generation(registry: &NodeRegistry, node_id: vector<u8>): u64 {
        assert!(registry.nodes.contains(node_id), E_NODE_NOT_FOUND);
        registry.nodes.borrow(node_id).generation
    }

    #[test_only]
    public fun create_for_testing(ctx: &mut TxContext): (RegistryAdminCap, NodeRegistry) {
        create(ctx)
    }

    #[test_only]
    public fun register_for_testing(
        registry: &mut NodeRegistry,
        stake: Coin<SUI>,
        public_key: vector<u8>,
        endpoint: vector<u8>,
        failure_domain: vector<u8>,
        max_sessions: u64,
        max_zones: u64,
        now_ms: u64,
        ctx: &mut TxContext,
    ): NodeOwnerCap {
        register_internal(
            registry,
            stake,
            public_key,
            endpoint,
            failure_domain,
            max_sessions,
            max_zones,
            now_ms,
            ctx,
        )
    }

    #[test_only]
    public fun rotate_key_for_testing(
        registry: &mut NodeRegistry,
        cap: &mut NodeOwnerCap,
        public_key: vector<u8>,
        now_ms: u64,
        ctx: &TxContext,
    ) {
        assert!(public_key.length() == 32, E_BAD_PUBLIC_KEY);
        let record = active_record_mut(registry, cap);
        record.public_key = public_key;
        record.generation = record.generation + 1;
        record.updated_at_ms = now_ms;
        cap.generation = record.generation;
        event::emit(NodeKeyRotatedEvent {
            node_id: cap.node_id,
            operator: ctx.sender(),
            public_key,
            generation: record.generation,
            observed_at_ms: now_ms,
        });
    }

    #[test_only]
    public fun revoke_for_testing(
        registry: &mut NodeRegistry,
        cap: NodeOwnerCap,
        now_ms: u64,
        ctx: &mut TxContext,
    ): Coin<SUI> {
        let NodeOwnerCap { id, node_id, generation } = cap;
        let NodeRecord {
            operator: _,
            public_key: _,
            endpoint: _,
            failure_domain: _,
            stake,
            max_sessions: _,
            max_zones: _,
            generation: current_generation,
            active: _,
            updated_at_ms: _,
        } = registry.nodes.remove(node_id);
        assert!(generation == current_generation, E_STALE_CAPABILITY);
        registry.retired.add(node_id, true);
        let returned_stake_mist = stake.value();
        object::delete(id);
        event::emit(NodeRevokedEvent {
            node_id,
            operator: ctx.sender(),
            returned_stake_mist,
            generation,
            observed_at_ms: now_ms,
        });
        coin::from_balance(stake, ctx)
    }

    #[test_only]
    public fun destroy_for_testing(admin: RegistryAdminCap, registry: NodeRegistry) {
        let RegistryAdminCap { id: admin_id } = admin;
        let NodeRegistry { id, min_stake_mist: _, treasury, nodes, retired } = registry;
        object::delete(admin_id);
        balance::destroy_zero(treasury);
        table::destroy_empty(nodes);
        table::drop(retired);
        object::delete(id);
    }
}
