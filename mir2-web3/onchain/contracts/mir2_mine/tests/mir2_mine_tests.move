#[test_only]
module mir2_mine::mir2_mine_tests;

use mir2_mine::mir2_mine;

/// M0 smoke: the scaffold package compiles and exports its version symbol.
/// M1 replaces this suite with the `mine_batch` invariant tests (nonce replay,
/// insufficient fee, emission cap, regen, payout, event fields) per ROADMAP §4.
#[test]
fun scaffold_version_is_zero() {
    assert!(mir2_mine::scaffold_version() == 0, 0);
}
