# MagicShield lifecycle fix notes

Date: 2026-09-10
Source HEAD: `e7fd008b287607f15e2ab6f67519d77ea71ebeb2`
Worktree: `C:/mir2-shield-fix-20260910`
Branch: `codex/r4-shield-lifecycle-20260910`

## Scope and diagnosis

The implementation was independently checked against `shield-diagnosis.md` and the frozen source before editing. The confirmed fault chain was:

- shared Zone actor ids were left in owner-facing `AddBuff`, `RemoveBuff`, `PauseBuff`, and `ObjectEffect` packets;
- MagicShield expiry removed Buff 24 without emitting Crystal's MagicShield Down effect 7;
- lethal Zone paths did not remove the shield before death;
- the Windows effect renderer appended every Up effect 6 and had no Down cleanup;
- `ObjectPlayer.effect` carried late-AOI state but was not consumed by `NativeEffects`.

The patch is restricted to the four authorized files. It does not change MP penalty/rate logic or other skills.

## Changes

### Gateway owner identity

`apps/gateway/src/routing.rs`

- Extends the final owner-facing normalization pass to `AddBuff`, `RemoveBuff`, `PauseBuff`, and `ObjectEffect`.
- Rewrites only packets delivered to the owner from the shared Zone id to the local SelfPlayer id.
- Leaves observer broadcast packets on the global Zone id.
- Keeps the personal `active_buffs` mirror updated from the Zone Add/Remove/Pause events.
- Expands the existing shared runtime test with distinct owner local and Zone ids and an AOI observer.

### Zone lifecycle

`apps/simulation/src/runtime/zone/runtime.rs`

- Adds an idempotent native MagicShield removal helper that emits `RemoveBuff(24)` followed by `ObjectEffect Down(7)`.
- Uses it for lethal hazard, physical PvP, magic PvP, and pending native monster-hit paths before `ObjectDied`.
- Emits Down when Buff 24 expires.
- Does not clear an unrelated later `player.effect` value.

`apps/simulation/src/runtime/zone/packets.rs`

- Projects active Buff 24 as `ObjectPlayer.effect=6` for late AOI entry.
- Clears mirrored shield state on RemoveBuff, Down, and death while preserving unrelated later effects when a delayed Down arrives.

### Windows renderer

`apps/game-client/platform-windows/src/effects.rs`

- Uses one stable `magic-shield-{object_id}` effect key, making repeated Up idempotent.
- Processes Down before tile lookup and removes all bookkeeping for that shield key.
- Recovers a missing persistent loop from `ObjectPlayer.effect=6`.
- Records a per-object Down suppression gate so a stale `effect=6` render snapshot cannot recreate the shield after a newer Down.
- A newer Up clears suppression. Leaving AOI clears the old object-incarnation suppression, allowing a later `ObjectPlayer.effect=6` to restore the loop.
- Adds ordering coverage for stale-0 then Up, stale-6 then Down, new Up, and AOI leave/re-entry.

## Validation

All completed Cargo checks used Rust `+1.95.0`, `-j1`, one test thread, and were run serially at below-normal process priority.

1. Simulation lifecycle:

   `CARGO_TARGET_DIR=C:/mir2-three-class-r4-gateway-target cargo +1.95.0 test --locked -j1 -p mir2-simulation --lib magic_shield_lifecycle -- --test-threads=1 --nocapture`

   Result: **PASS**, 2 passed, 0 failed, 1736 filtered out. Build/test elapsed approximately 2m24s.

2. Windows effect lifecycle and recovery:

   `CARGO_TARGET_DIR=C:/Users/Administrator/AppData/Local/Temp/mir2-trade-completion-windows-340f7ca3b1f44af6a6ae18576bb183e1 MIR2_NATIVE_ASSET_ROOT=C:/mir2-three-class-effects-r2-20260910 cargo +1.95.0 test --locked -j1 --bin mir2-platform-windows magic_shield_ -- --test-threads=1 --nocapture`

   Result: **PASS**, 2 passed, 0 failed, 621 filtered out; tests finished in 0.02s. Build elapsed 13m16s because local worktree crates needed recompilation and relinking.

3. Gateway owner/observer ids and mirror:

   `CARGO_TARGET_DIR=C:/mir2-three-class-r4-gateway-target cargo +1.95.0 test --locked -j1 -p mir2-gateway --lib shared_in_process_runtime_mirrors_pending_zone_self_buff_to_session_snapshot -- --test-threads=1 --nocapture`

   Result: **PASS**, 1 passed, 0 failed, 732 filtered out; test finished in 3.92s. Build/test elapsed 2m15s.

4. Static patch check:

   `git diff --check`

   Result: **PASS**. The worktree contains only the four authorized modified files.

An earlier Gateway build was interrupted immediately when the user reported frame-rate contention; it produced no test result and left no compiler/linker process. One Windows attempt was also stopped before linking after it was found to use the Gateway target cache. The final passing Windows result above uses the established R4 Windows cache.

## Known recovery limits

- Live owner HUD behavior is covered by normalized Add/Remove/Pause events and the personal-session buff mirror. A reconnect/world snapshot `active_buffs` list is not converted into `StatusHud` bootstrap events by the current client pipeline. Therefore full MagicShield reconnect HUD restoration is not claimed by this patch.
- An Up event received while the actor has no entry in `zone_tiles` is still discarded by the generic renderer path. Down does not require a tile and always clears/suppresses the loop. The tested recovery covers stale snapshot ordering, a newer Up, and AOI leave/re-entry; it does not claim arbitrary packet-loss or packet-reordering recovery.
- No runtime process, UI, account data, save data, or package was changed or launched.

## Review artifacts

- Patch: `C:/mir2-three-class-r4-20260910-evidence/shield-fix.patch`
- Concise test log: `C:/mir2-three-class-r4-20260910-evidence/shield-fix-tests.log`
