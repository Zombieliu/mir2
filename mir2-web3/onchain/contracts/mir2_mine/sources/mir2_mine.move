/// mir2_mine — on-chain smart mine for the Legend of Mir 2 (Crystal) web port.
///
/// M0 placeholder package. Milestone M1 (see `docs/ONCHAIN-SMART-MINE-ROADMAP.md` §4)
/// replaces/augments this with the Dubhe `schemagen` output plus `mine_system`,
/// `redeem_system` and `admin_system` per `ONCHAIN-SMART-MINE-DESIGN.md` §3.
///
/// This minimal module exists only so `sui move build` / `sui move test` are green
/// from M0 onward, giving the CI `onchain` job a real compile + test target. It holds
/// no economy logic and hardcodes no economic constants (those are gated on the M5
/// sign-off).
module mir2_mine::mir2_mine;

/// Scaffold version marker. Replaced by the real schema/systems in M1.
const SCAFFOLD_VERSION: u64 = 0;

/// Returns the M0 scaffold version. Exists so the package exports a tested symbol
/// before M1 lands the real `mine_batch` / `redeem` / `admin` systems.
public fun scaffold_version(): u64 {
    SCAFFOLD_VERSION
}
