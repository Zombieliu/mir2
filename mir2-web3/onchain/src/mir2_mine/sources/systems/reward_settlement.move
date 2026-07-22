/// Settlement for verified shared-compute rewards.
///
/// The game control plane computes a deterministic reward Merkle root from N-of-M verified
/// execution receipts. A treasury operator publishes that root and pays a claim only after the
/// off-chain relayer validates the corresponding proof. Guild nodes never hold this capability.
module mir2_mine::reward_settlement {
    use sui::balance::{Self, Balance};
    use sui::coin::{Self, Coin};
    use sui::event;
    use sui::sui::SUI;
    use sui::table::{Self, Table};

    const E_BAD_ID: u64 = 0;
    const E_BAD_AMOUNT: u64 = 1;
    const E_EPOCH_EXISTS: u64 = 2;
    const E_BATCH_NOT_FOUND: u64 = 3;
    const E_CLAIM_EXISTS: u64 = 4;
    const E_BATCH_EXHAUSTED: u64 = 5;
    const E_TREASURY_EMPTY: u64 = 6;

    public struct RewardAdminCap has key, store {
        id: UID,
    }

    public struct RewardRegistry has key {
        id: UID,
        treasury: Balance<SUI>,
        epochs: Table<RewardEpochKey, bool>,
        batches: Table<vector<u8>, RewardBatch>,
        claims: Table<RewardClaimKey, bool>,
    }

    public struct RewardEpochKey has copy, drop, store {
        game_id: vector<u8>,
        epoch: u64,
    }

    public struct RewardClaimKey has copy, drop, store {
        batch_id: vector<u8>,
        node_id: vector<u8>,
    }

    public struct RewardBatch has store {
        game_id: vector<u8>,
        epoch: u64,
        merkle_root: vector<u8>,
        total_reward: u64,
        remaining_reward: u64,
        allocation_count: u32,
        finalized_control_height: u64,
    }

    public struct RewardBatchPublishedEvent has copy, drop {
        batch_id: vector<u8>,
        game_id: vector<u8>,
        epoch: u64,
        merkle_root: vector<u8>,
        total_reward: u64,
        allocation_count: u32,
        finalized_control_height: u64,
    }

    public struct RewardClaimPaidEvent has copy, drop {
        batch_id: vector<u8>,
        game_id: vector<u8>,
        epoch: u64,
        node_id: vector<u8>,
        recipient: address,
        amount: u64,
    }

    fun init(ctx: &mut TxContext) {
        let (cap, registry) = create(ctx);
        transfer::transfer(cap, ctx.sender());
        transfer::share_object(registry);
    }

    fun create(ctx: &mut TxContext): (RewardAdminCap, RewardRegistry) {
        (
            RewardAdminCap { id: object::new(ctx) },
            RewardRegistry {
                id: object::new(ctx),
                treasury: balance::zero(),
                epochs: table::new(ctx),
                batches: table::new(ctx),
                claims: table::new(ctx),
            },
        )
    }

    /// Anyone may fund the SUI payout treasury. Publishing and paying remain capability-gated.
    public fun fund(registry: &mut RewardRegistry, payment: Coin<SUI>) {
        balance::join(&mut registry.treasury, coin::into_balance(payment));
    }

    /// Publish one immutable reward root for one game/epoch.
    public fun publish_batch(
        registry: &mut RewardRegistry,
        _cap: &RewardAdminCap,
        batch_id: vector<u8>,
        game_id: vector<u8>,
        epoch: u64,
        merkle_root: vector<u8>,
        total_reward: u64,
        allocation_count: u32,
        finalized_control_height: u64,
    ) {
        assert!(!batch_id.is_empty() && !game_id.is_empty() && merkle_root.length() == 32, E_BAD_ID);
        assert!(total_reward > 0 && allocation_count > 0, E_BAD_AMOUNT);
        let epoch_key = RewardEpochKey { game_id: game_id, epoch };
        assert!(!table::contains(&registry.epochs, epoch_key), E_EPOCH_EXISTS);
        assert!(!table::contains(&registry.batches, batch_id), E_EPOCH_EXISTS);
        table::add(&mut registry.epochs, epoch_key, true);
        table::add(
            &mut registry.batches,
            batch_id,
            RewardBatch {
                game_id,
                epoch,
                merkle_root,
                total_reward,
                remaining_reward: total_reward,
                allocation_count,
                finalized_control_height,
            },
        );
        event::emit(RewardBatchPublishedEvent {
            batch_id,
            game_id,
            epoch,
            merkle_root,
            total_reward,
            allocation_count,
            finalized_control_height,
        });
    }

    /// Pay a proof-checked claim. The relayer checks the Rust-generated Merkle proof before this
    /// call, and this module provides on-chain no-double-claim, budget, treasury, and audit fences.
    public fun pay_verified_claim(
        registry: &mut RewardRegistry,
        _cap: &RewardAdminCap,
        batch_id: vector<u8>,
        node_id: vector<u8>,
        recipient: address,
        amount: u64,
        ctx: &mut TxContext,
    ) {
        assert!(!node_id.is_empty(), E_BAD_ID);
        assert!(amount > 0, E_BAD_AMOUNT);
        assert!(table::contains(&registry.batches, batch_id), E_BATCH_NOT_FOUND);
        let claim_key = RewardClaimKey { batch_id, node_id };
        assert!(!table::contains(&registry.claims, claim_key), E_CLAIM_EXISTS);
        let batch = table::borrow_mut(&mut registry.batches, batch_id);
        assert!(batch.remaining_reward >= amount, E_BATCH_EXHAUSTED);
        assert!(balance::value(&registry.treasury) >= amount, E_TREASURY_EMPTY);
        batch.remaining_reward = batch.remaining_reward - amount;
        let game_id = batch.game_id;
        let epoch = batch.epoch;
        table::add(&mut registry.claims, claim_key, true);
        let payout = coin::from_balance(balance::split(&mut registry.treasury, amount), ctx);
        transfer::public_transfer(payout, recipient);
        event::emit(RewardClaimPaidEvent {
            batch_id,
            game_id,
            epoch,
            node_id,
            recipient,
            amount,
        });
    }

    public fun treasury_balance(registry: &RewardRegistry): u64 {
        balance::value(&registry.treasury)
    }

    public fun contains_batch(registry: &RewardRegistry, batch_id: vector<u8>): bool {
        table::contains(&registry.batches, batch_id)
    }

    public fun is_claimed(
        registry: &RewardRegistry,
        batch_id: vector<u8>,
        node_id: vector<u8>,
    ): bool {
        table::contains(&registry.claims, RewardClaimKey { batch_id, node_id })
    }

    #[test_only]
    public fun create_for_testing(ctx: &mut TxContext): (RewardAdminCap, RewardRegistry) {
        create(ctx)
    }

    #[test_only]
    public fun share_for_testing(registry: RewardRegistry) {
        transfer::share_object(registry);
    }

    #[test_only]
    public fun destroy_for_testing(cap: RewardAdminCap, registry: RewardRegistry) {
        let RewardAdminCap { id: cap_id } = cap;
        object::delete(cap_id);
        let RewardRegistry { id, treasury, epochs, batches, claims } = registry;
        balance::destroy_zero(treasury);
        table::destroy_empty(epochs);
        table::destroy_empty(batches);
        table::destroy_empty(claims);
        object::delete(id);
    }
}
