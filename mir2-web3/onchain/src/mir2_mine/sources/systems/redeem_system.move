/// redeem_system — burn on-chain ore for game gold (DESIGN §3.2 / §4), Dubhe 1.2.x.
///
/// The chain only destroys the ore and emits `OreRedeemedEvent`; the Relayer turns that
/// into a `CreditGoldFromOre` command and the Sim authoritatively credits gold (DESIGN §4).
/// The ore burned belongs to the UserStorage's canonical owner, so a session wallet may
/// redeem on the owner's behalf.
module mir2_mine::redeem_system {
    use sui::event;
    use dubhe::dapp_service::{Self, DappStorage, UserStorage};
    use dubhe::dapp_system;
    use mir2_mine::dapp_key::DappKey;
    use mir2_mine::ore_balance;
    use mir2_mine::ore_kind::OreKind;
    use mir2_mine::error;

    public struct OreRedeemedEvent has copy, drop { miner: address, ore_kind: OreKind, amount: u64 }

    fun balance_or0(us: &UserStorage, kind: OreKind): u64 {
        if (ore_balance::has(us, kind)) { ore_balance::get(us, kind) } else { 0 }
    }

    /// Burn `amount` of the owner's `ore_kind` balance.
    public fun redeem(
        dapp_storage: &DappStorage,
        user_storage: &mut UserStorage,
        ore_kind: OreKind,
        amount: u64,
        ctx: &mut TxContext,
    ) {
        dapp_system::ensure_not_paused<DappKey>(dapp_storage);
        error::BadAmount(amount > 0);
        let miner = dapp_service::canonical_owner(user_storage);
        let bal = balance_or0(user_storage, ore_kind);
        error::InsufficientOre(bal >= amount);
        ore_balance::set(user_storage, ore_kind, bal - amount, ctx);
        event::emit(OreRedeemedEvent { miner, ore_kind, amount });
    }
}
