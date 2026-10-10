# Classic Shared Zone Baseline Audit — 2026-10-09

## Scope and conclusion

Read-only audit of the retained `shared_zone` runs; this document is the only authorized write. No new build, test, native client, service, account, save or Git operation was performed to produce it.

Both runs report **198 passed / 209 total; 11 failed**, exit 101. Failure names and first panic locations match exactly. These failures reproduce before the current P1 target/movement cancellation changes; this does not make the suite green or close P4.

- Run 16: `gateway-lifecycle-20261008-01/mir2-web3`, documentation HEAD `88a36e7a2f79b54163fc3849fb48d00d498d4cda`, plus the current uncommitted P1 implementation.
- Run 17: clean `gateway-lifecycle-baseline-20261008-01/mir2-web3`, HEAD `7fea5cdba8cd160d4f2c5536f14fb7ce98c6bfbc` (clean status established in the preceding read-only audit).
- Prior comparison found no committed shared Zone/test/data change between 7fea and 88a. `shared_zone.rs`, owned-pet/Shinsu/SnakeTotem/Vampire modules and simulation Cargo configuration match between lanes; the current P1 Zone cancellation change is isolated from these failure paths.
- `apps/simulation/tests/shared_zone.rs` SHA-256 in both lanes: `a0ecdff3eef071174e72adf285321661f96fdaff538eda091564b82c7859781c`.

## Retained execution evidence

Logs and accompanying `.log.json` receipts are under `C:/mir2-build/p1-target-lifecycle-20261009-01/`.

| Run | Log | Completed UTC | SHA-256 | Result |
| --- | --- | --- | --- | --- |
| 16 | `16-zone-adjacent.log` | 2026-10-09 01:20:14 | `01648efb76029a6e6245cf2236df9745acc281c6658894fbd21359a1d4254bd5` | 198 pass / 11 fail; 2.84s |
| 17 | `17-zone-baseline.log` | 2026-10-09 01:41:48 | `f2582c419c54820e4c3949550c540dedd0c42599a65b3841a3d005e736bb4868` | 198 pass / 11 fail; 2.60s |

Both hashes were re-read and match their receipts. Both commands use `cargo +1.95.0 test --locked --offline --jobs 2 -p mir2-simulation --test shared_zone -- --test-threads=1`, without a feature override. Run 17 additionally sets `--config profile.test.package.mir2-simulation.codegen-units=147`; this is a compilation setting. No feature, concurrent-test or data difference was found that explains this identical failure set.

The receipts bind `apps/simulation/src/runtime/zone/runtime.rs` to `ca2fc26c042c7c3246b1d252362cde3217829b4cbfd0a91d3ea9a4491e759929` (16) and `3460313383fa0e802a0bb7639014738bd60d6e28c455a2dca369384f9ea7c225` (17). These are execution-time hashes, not a promise that an actively edited working tree remains frozen.

## Identical first-failure inventory

All locations in this table are in `apps/simulation/tests/shared_zone.rs`.

| Test | First panic line | Classification |
| --- | --- | --- |
| `level50_taoist_purification_clears_shared_poison_and_curse_state` | 13961 | Old immediate-poison expectation |
| `zone_native_archer_summon_snakes_spawns_static_totem_profile` | 14038 | Existing immediate-Master identity gap |
| `zone_native_charmed_snake_hit_applies_paralysis_poison` | 14038 | Same gap; poison assertion never reached |
| `zone_native_holy_deva_uses_ranged_summon_attack_against_hostile_monster` | 9181 | Birth deadline equality / moving-target fixture |
| `zone_native_monster_combat_kill_and_drop_are_authoritative` | 4768 | Old drop-owner duration |
| `zone_native_pet_enhancer_buffs_owned_summon_and_increases_damage` | 9364 | Missing prior threat acquisition |
| `zone_native_player_fire_bounce_chains_projectiles_and_damage` | 7003 | Cold recovery incorrectly treated as live resume |
| `zone_native_player_poison_cloud_spawns_ground_spell_and_poisons_monsters` | 7510 | Missing travel delay |
| `zone_native_summon_attacks_hostile_monster_for_owner_without_hitting_players` | 9091 | Attack expected before birth lock expires |
| `zone_native_summon_shinsu_spawns_owned_pet_and_attacks_hostile_monster` | 9268 | Missing prior threat acquisition |
| `zone_native_vampire_spider_explodes_on_expiry_and_vampires_nearby_target` | 9605 | Old Master-dependent death-burst expectation |

## Nine first fixture differences: boundaries and next steps

Repository references below are relative to `mir2-web3/`; Crystal references are relative to the inspected original source root `E:/mir2/Crystal/`. Line numbers were checked against the current files. A first-failure explanation is not evidence that later assertions pass.

1. **Purification preparation.** The test expects `ObjectPoisoned` in the cast output at 20ms (`shared_zone.rs:13947`, `13961`). Original `Server/MirObjects/HumanObject.cs:4365`, `4374` schedules Poisoning at +500ms. Current `apps/simulation/src/runtime/zone/runtime.rs:5755` and `apps/simulation/src/runtime/zone/runtime/owned_pet_combat.rs:2987` preserve that delay.
   - Next: require no poison at 519ms and real poison at 520ms, then perform curse/Purification checks on monotonic times. The existing curse preparation is 25ms (`shared_zone.rs:13981`) and Purification 40ms (`13995`); both must follow the delayed poison instead of rewinding the fixture clock. Retain poison removal, curse removal and late-observer assertions.

2. **Monster drop custody.** `shared_zone.rs:4777` still requires `ownership_remaining_ticks == Some(100)`. Current `apps/simulation/src/runtime/zone/runtime.rs:12770`, `12772` supplies the real owner and 200 ticks. Original gold uses `OwnerTime = Envir.Time + Settings.Minute` at `Server/MirObjects/MonsterObject.cs:1183`; `Server/Settings.cs:16` defines a minute as 60,000ms (item path: `MonsterObject.cs:1155`).
   - Next: update only the stale duration expectation to 200 and verify the real 60-second ownership boundary. Preserve award amount 6, drop ID 9200, owner ID 101, genuine death, corpse collision, late join and claim/custody assertions.

3. **FireBounce recovery.** `shared_zone.rs:6998` calls public checkpoint deserialization and `7003` expects the old caster's second bounce. `apps/simulation/src/runtime/zone/runtime/checkpoint.rs:977`, `979` deliberately revoke online authority; `apps/simulation/src/runtime/zone/runtime/experience_ownership.rs:93`, `100` clear its online presence. `apps/simulation/src/runtime/zone/runtime.rs:10041` rejects the stale owned caster proof.
   - Next: continue the positive bounce on the original live Zone at the saved 1120ms impact deadline. Independently restore the authenticated bytes and require no old-caster projectile/hit/award. Keep saved due time, remaining-bounce count, original damage and duplicate-consumption checks. Do not regrant authority from serialized IDs or weaken proof validation.

4. **PoisonCloud travel.** `shared_zone.rs:7461` places the caster at 324,270 and `7459` the target at 334,270. The cast is at 20ms, but `7508` ticks at 520ms. Original `Server/MirObjects/HumanObject.cs:4553`, `4556` and current `apps/simulation/src/runtime/zone/runtime.rs:7155` use +500ms +50ms per tile: the real deadline is **1020ms**.
   - Next: verify no `ObjectSpell`/damage at 1019ms, then real cloud, primary hit/poison and unaffected out-of-area monster at the correct deadline. Original damage/poison behavior also requires review after this first failure; `Server/MirObjects/SpellObject.cs:150` handles both. Do not assume the later 70%/poison-value assertions pass.

5. **Skeleton birth.** Birth is 510ms (`shared_zone.rs:9071`), but an attack is demanded at 1110ms (`9090`). Original `Server/MirObjects/MonsterObject.cs:734`, `736` sets +2000ms and `650`, `657` requires strictly later. Current `apps/simulation/src/runtime/zone/runtime.rs:11085`, `11087` admits from 2511ms.
   - Next: retain no attack through 2510ms, establish the real wild threat and check launch/impact after 2511ms at the actual Source hit delay. Keep pet attacker ID, DC-range, victim HP and no player-hit assertions.

6. **HolyDeva equality.** Birth is 1510ms (`shared_zone.rs:9152`); the test demands attack at exactly 3510ms (`9180`). The same original/current birth lock requires **3511ms or later**. Its wild target also moves during the sparse tick sequence, so the fixed launch point 336,270 must be checked against the actual captured target position.
   - Next: preserve no early range attack, tick the real AI without altering speed/damage, and verify typed target ID plus exact launch geometry and delayed hit after the strict boundary. Changing the timestamp alone does not prove the rest of this test passes.

7. **Shinsu threat acquisition.** `shared_zone.rs:9267` performs its first post-spawn AI tick at 2511ms and immediately expects Show. Current `apps/simulation/src/runtime/zone/runtime.rs:2197` evaluates Shinsu state before wild AI at `2204`; the wild target is recorded at `11775`. Pet eligibility requires that real threat or owner target (`apps/simulation/src/runtime/zone/runtime/owned_pet_combat.rs:1594`). Original `Server/MirObjects/Monsters/Shinsu.cs:28`, `30`, `35`, `36` requires Target and adds another 1000ms Show action lock.
   - Next: acquire the threat before checking Show; the existing `apps/simulation/src/runtime/zone/runtime/owned_pet_combat_tests.rs:717`, `754` uses ticks 2510 → 2511 → 3512. Preserve the 2510 birth boundary, 3511 Show-lock boundary, line geometry and delayed damage; never grant pets unrelated wild targets to make the fixture pass.

8. **PetEnhancer preparation.** `shared_zone.rs:9357` spawns a wild monster, then `9363` immediately expects Show on the first 2511ms tick. It has the same acquisition/order issue as item 7; the enhancer Magic/AddBuff assertions already precede this failure.
   - Next: establish real threat before Show with monotonic Source ticks, then retain all buff-stat, enhanced damage, victim-health and owner-safety checks. This audit has not rerun the enhanced damage assertion.

9. **Vampire death phase.** `shared_zone.rs:9605` expects an expiry explosion to damage a normal unmastered wild monster; `9616` then expects healing of the former owner. Original `Server/MirObjects/Monsters/VampireSpider.cs:73`, `75` first calls base.Die; `Server/MirObjects/MonsterObject.cs:1000` clears Master. Ordinary wild targets then require Hallucination/Rage (`MonsterObject.cs:2522`, `2537`, `2539`), and healing requires non-null Master (`VampireSpider.cs:103`). Current `apps/simulation/src/runtime/zone/runtime/vampire_ai.rs:442`, `452`, `491`, `521`, `666` preserves this separation.
   - Next: keep genuine expiry death and assert this ordinary wild receives no burst and the old owner receives no heal. Add/retain a separate real lawful target case for positive burst damage and the live-bite healing case. Do not restore the former Human's authority to a Master-null corpse.

## Two Snake failures: real P4 gap remains open

Both tests reach a real spawned CharmedSnake but time out waiting for its attack in `shared_zone.rs:14025`, `14038`. Extending the timeout or lowering combat guards cannot solve this.

- `apps/simulation/src/runtime/zone/runtime/snake_totem_ai.rs:198`, `200`, `201` creates a child with causal human owner but immediate `master_object_id = totem_id`; insertion/packet projection at `207`, `209` does not install a valid typed parent binding.
- `apps/simulation/src/runtime/zone/runtime.rs:11865` requires `owned_pet_source_has_live_master`; `apps/simulation/src/runtime/zone/runtime/owned_pet_combat.rs:995` requires a typed pet life. Registration at `1322`, `1332` and lookup at `1345`, `1351` accept only a direct Human Master. Calling that existing registration helper for a Totem child would still fail.
- Original `Server/MirObjects/Monsters/SnakeTotem.cs:140` explicitly sets `monster.Master = this`; `146` also sets MasterTotem. Victim eligibility in `Server/MirObjects/MonsterObject.cs:2527`, `2533` uses that immediate Master's pets/target relation. Replacing it with the causal Human changes Source semantics.
- Additional blocked boundary: current `snake_totem_ai.rs:205` uses child +1000ms. Original `SnakeTotem.cs:143` initially does the same, but `149` calls Spawn and base `MonsterObject.cs:736` overwrites it with +2000ms; `657` is strict `>`.

Next bounded P4 implementation should add a typed immediate-Monster-Master path binding parent and child incarnations plus causal Human online/life identity. Retain existing generic owned-source rejection (`apps/simulation/src/runtime/zone/runtime/entity_combat.rs:1320`) and the direct-Human pet path. Integrate real child acquisition/launch/impact; fix its true birth deadline in the same bounded slice.

Use a dedicated integration file for positive ordinary SummonSnakes → child attack → delayed damage/paralysis, and negatives for absent/dead parent, reused parent ID, stale child incarnation, owner logout/cold revoke and duplicate pending impact. Prove no attack at birth+2000 and valid eligibility only afterward; retain Source paralysis rules (`Server/MirObjects/Monsters/CharmedSnake.cs:195`, `199`). Do not claim Human PK or full nested-pet parity from these two PvE tests.

## Executable follow-up, not executed by this audit

After Root assigns a separate bounded fix lane and its verified scoped Windows toolchain, run each exact test name from the inventory first, preserving a new numbered log for every attempt. Example PowerShell template:

```powershell
$env:CARGO_TARGET_DIR = 'C:/mir2-build/classic-zone-baseline-fix-20261009'
$case = 'zone_native_summon_shinsu_spawns_owned_pet_and_attacks_hostile_monster'
cargo +1.95.0 test --locked --offline --jobs 2 -p mir2-simulation --test shared_zone $case -- --exact --test-threads=1
```

Then run the dedicated Snake lifecycle tests, existing `shared_pet_pvp` Source cases and the complete original `shared_zone` command. Do not add feature flags to disguise baseline failures, shorten Source action windows, remove custody/identity assertions or reissue online authority on cold decode. Record new outcomes before updating parity status.

**Status:** current P1 changes are not the cause of this identical baseline failure set; 16/17 remain RED. Nine first fixture causes are explained, not repaired or proven through later assertions. The two Snake failures and broader P4/P1 acceptance gates remain open.
