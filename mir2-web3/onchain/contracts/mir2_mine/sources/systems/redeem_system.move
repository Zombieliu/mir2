/// redeem_system — burn on-chain ore for game gold (DESIGN §3.2 / §4).
///
/// The chain only destroys the ore and emits `ore_redeemed`; the Relayer turns that into
/// a `CreditGoldFromOre` command and the Sim authoritatively credits gold (DESIGN §4).
module mir2_mine::redeem_system {
    use mir2_mine::mir2_mine_schema::Schema;
    use mir2_mine::mir2_mine_dapp_system as dapp;
    use mir2_mine::mir2_mine_errors as errors;
    use mir2_mine::mir2_mine_events as events;
    use mir2_mine::mir2_mine_ore_kind::OreKind;

    /// Burn `amount` of the caller's `ore_kind` balance.
    public fun redeem(schema: &mut Schema, ore_kind: OreKind, amount: u64, ctx: &mut TxContext) {
        dapp::ensure_no_safe_mode(schema);
        errors::BadAmount_error(amount > 0);
        let miner = ctx.sender();
        let bal = schema.ore_balance().try_get(miner, ore_kind).destroy_with_default(0u64);
        errors::InsufficientOre_error(bal >= amount);
        schema.ore_balance().set(miner, ore_kind, bal - amount);
        events::ore_redeemed_event(miner, ore_kind, amount);
    }
}
