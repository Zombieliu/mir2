/// admin_system — governance entrypoints, gated on the dapp admin (set to the deployer
/// at genesis via `dapp_system::create_dapp`). DESIGN §3.2 / §8, Dubhe 1.2.x.
///
/// Economic parameters live here as admin-set globals. M1 only wires the setters; the
/// real values are decided at the M5 sign-off — nothing economic is hardcoded.
module mir2_mine::admin_system {
    use dubhe::dapp_service::DappStorage;
    use dubhe::dapp_system;
    use mir2_mine::dapp_key::DappKey;
    use mir2_mine::config;
    use mir2_mine::stones_left;
    use mir2_mine::next_regen_ms;
    use mir2_mine::per_swing_fee;
    use mir2_mine::epoch_emission_cap;
    use mir2_mine::emitted_this_epoch;
    use mir2_mine::treasury_addr;

    /// Create or replace a mine's static config and (re)fill it to `max_stones`.
    public fun set_mine(
        dapp_storage: &mut DappStorage,
        mine_id: u64,
        map_id: u32,
        mine_set: u8,
        max_stones: u32,
        regen_secs: u32,
        hit_rate: u8,
        drop_rate: u8,
        ctx: &mut TxContext,
    ) {
        dapp_system::ensure_dapp_admin<DappKey>(dapp_storage, ctx.sender());
        config::set(dapp_storage, mine_id, map_id, mine_set, max_stones, regen_secs, hit_rate, drop_rate);
        stones_left::set(dapp_storage, mine_id, max_stones);
        next_regen_ms::set(dapp_storage, mine_id, 0);
    }

    /// Per-swing protocol fee in MIST. Placeholder 0 until M5.
    public fun set_per_swing_fee(dapp_storage: &mut DappStorage, fee: u64, ctx: &mut TxContext) {
        dapp_system::ensure_dapp_admin<DappKey>(dapp_storage, ctx.sender());
        per_swing_fee::set(dapp_storage, fee);
    }

    /// Per-epoch ore emission cap.
    public fun set_epoch_emission_cap(dapp_storage: &mut DappStorage, cap: u64, ctx: &mut TxContext) {
        dapp_system::ensure_dapp_admin<DappKey>(dapp_storage, ctx.sender());
        epoch_emission_cap::set(dapp_storage, cap);
    }

    /// Roll the epoch: reset the emitted-this-epoch counter.
    public fun reset_epoch(dapp_storage: &mut DappStorage, ctx: &mut TxContext) {
        dapp_system::ensure_dapp_admin<DappKey>(dapp_storage, ctx.sender());
        emitted_this_epoch::set(dapp_storage, 0);
    }

    /// Set the treasury (protocol-fee recipient) address.
    public fun set_treasury_addr(dapp_storage: &mut DappStorage, addr: address, ctx: &mut TxContext) {
        dapp_system::ensure_dapp_admin<DappKey>(dapp_storage, ctx.sender());
        treasury_addr::set(dapp_storage, addr);
    }
}
