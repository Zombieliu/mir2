# Ground Item Identity Phase 1 Report

Status: complete for Phase 1; Phase 2 remains open
Date: 2026-08-25

## Scope delivered

- `ItemState` and `EquipmentState` preserve complete bounded recursive Crystal `UserItem` identity through inventory, belt, storage, hero inventory, equipment, rental, guild storage, mail, trade, auction, refine, save/reload, and StartGame.
- Exact item index, UID, quantity, durability, grade and combat modifiers, Awake/refine/wedding/expiry, shop/GM flags, curse/binding/identified state, rental/seal state, and nested slots survive the committed boundaries.
- Conversion and validation use explicit recursion, node, and collection budgets and fail closed on malformed, unknown, overstacked, orphaned, or conflicting carriers.
- Stack merge compares complete identity. Split refuses embedded identities that cannot be cloned safely. Equip/unequip and shared carrier paths preserve the same exact record.
- Six temporary Candidate aliases were removed from production behavior. Persisted records migrate idempotently to exact Crystal templates:
  - `training-manual` -> FireBall (`crystal-item-990`)
  - `belt-lantern-oil` -> RepairOil (`crystal-item-706`)
  - `repair-powder` -> RareCopperOre (`crystal-item-1135`)
  - `training-splinter` -> Timber (`crystal-item-865`)
  - `quest-wasp-stinger` -> SkyStingerEgg (`crystal-item-876`)
  - `guide-ring-right` -> CopperRing (`crystal-item-404`)
- Migration preserves live UID, quantity, durability, added stats, curse/binding, rental, and seal state, rejects an incompatible exact item index, persists on StartGame/save, and is idempotent on the next load.
- The ordinary Candidate task, combat, hand-in, drop/pickup, save, and relogin fixture now uses canonical server-data items.

## Verification

- `cargo +1.89.0 test --locked -p mir2-simulation --lib --jobs 1 -- --test-threads=1`: 1445 passed; 0 failed.
- `cargo +1.89.0 test --locked -p mir2-simulation --test ordinary_candidate_loop --jobs 1 -- --test-threads=1`: 2 passed; 0 failed.
- `cargo +1.89.0 test --locked -p mir2-game-data --jobs 1 -- --test-threads=1`: 35 unit tests and 3 integration tests passed.
- `cargo +1.89.0 check --locked -p mir2-simulation --jobs 1`: passed.
- `npm --prefix apps/web run typecheck`: passed.
- Exact modified Rust files passed `rustfmt +1.89.0 --edition 2021 --check`.
- `git diff --check`: passed.

The repository-wide `cargo fmt --all -- --check` and the complete Simulation integration-test target were not claimed because an unrelated concurrently modified `apps/simulation/tests/vertical_slice.rs` is syntactically corrupted. That file is excluded from this change set and was not overwritten or staged.

## Independent review

A separate read-only agent performed three review rounds. The first two rounds found and drove fixes for recursive Stage5 committed quantities, metadata-only child UID collisions, Timber weight drift, immediate rental replay, and cross-transaction rental replay. The final result is `P0=0`, `P1=0`, `P2=0`, `SUBMIT=YES`. Shared rental offers now bind each local active transaction to a fresh 128-bit `OsRng` nonce, so an old delivery cannot match a later otherwise-identical rental.

## Remaining Phase 2 boundary

Phase 1 does not claim complete GroundDrop identity or pickup transaction parity. Phase 2 must freeze a canonical item payload at drop creation, stop re-rolling metadata on pickup, connect the production UID authority, and make Zone claim -> account inventory commit -> Zone finalize crash-recoverable and idempotent. Overall Crystal 1:1 parity is therefore not yet 100%.
