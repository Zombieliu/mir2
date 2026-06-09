/// admin_system — governance entrypoints, gated on the built-in dapp admin
/// (`schema.dapp__admin`, set to the deployer at genesis). DESIGN §3.2 / §8.
///
/// Economic parameters live here as admin-set StorageValues. M1 only wires the
/// setters; the real values are decided at the M5 sign-off (ROADMAP §8/§11) — nothing
/// economic is hardcoded.
module mir2_mine::admin_system {
    use mir2_mine::mir2_mine_schema::Schema;
    use mir2_mine::mir2_mine_dapp_system as dapp;
    use mir2_mine::mir2_mine_mine_config as mine_config;

    /// Create or replace a mine's static config and (re)fill it to `max_stones`.
    public fun set_mine(
        schema: &mut Schema,
        mine_id: u64,
        map_id: u32,
        mine_set: u8,
        max_stones: u32,
        regen_secs: u32,
        hit_rate: u8,
        drop_rate: u8,
        ctx: &mut TxContext,
    ) {
        dapp::ensure_has_authority(schema, ctx);
        let cfg = mine_config::new(map_id, mine_set, max_stones, regen_secs, hit_rate, drop_rate);
        schema.config().set(mine_id, cfg);
        schema.stones_left().set(mine_id, max_stones);
        schema.next_regen_ms().set(mine_id, 0);
    }

    /// Per-swing protocol fee in MIST. Placeholder 0 until M5.
    public fun set_per_swing_fee(schema: &mut Schema, fee: u64, ctx: &mut TxContext) {
        dapp::ensure_has_authority(schema, ctx);
        schema.per_swing_fee().set(fee);
    }

    /// Per-epoch ore emission cap.
    public fun set_epoch_emission_cap(schema: &mut Schema, cap: u64, ctx: &mut TxContext) {
        dapp::ensure_has_authority(schema, ctx);
        schema.epoch_emission_cap().set(cap);
    }

    /// Roll the epoch: reset the emitted-this-epoch counter.
    public fun reset_epoch(schema: &mut Schema, ctx: &mut TxContext) {
        dapp::ensure_has_authority(schema, ctx);
        schema.emitted_this_epoch().set(0);
    }

    /// Set the treasury (protocol-fee recipient) address.
    public fun set_treasury_addr(schema: &mut Schema, addr: address, ctx: &mut TxContext) {
        dapp::ensure_has_authority(schema, ctx);
        schema.treasury_addr().set(addr);
    }
}
