# Classic late-route bounded implementation — 2026-10-04

This delivery changes exactly six repository files. It repairs two named original NPC scripts and supplies a bounded ordinary-protocol route controller with independent evidence verification. It does **not** assert that stone tomb, Zuma or red moon gameplay has passed a live Gateway journey, natural Boss drop, fresh-level progression, native screenshot or human acceptance gate.

Repository worktree: `C:/Users/Administrator/.codex/worktrees/maps/mir2-player-journey/mir2-web3`, branch `codex/classic-map-closure-20261004`, actual HEAD `38a64de2985c82008a596a34768f36185980f62e`. Root reported a later integration baseline `17b44e08f4253bfc0905dfb951e978d82bf78ada`; this worktree was not reset or relabelled as that revision. The prior eleven v27 files and prior resource receipts remain immutable. The new implementation receipt records their actual hashes.

## Exact repository write set

1. Existing `apps/simulation/src/runtime/npc_script.rs`: source-bound Stone/BigTaoist legacy quest compatibility and StoneHeart carrier compatibility, plus two source-proof unit tests.
2. New `apps/simulation/tests/classic_late_route_gates.rs`: thirteen prepared local ordinary ClientPacket tests.
3. New `apps/web/scripts/quest-agent/classic-late-route-policy.mjs`: exact source identity, public commands, source routes, pair identity and finite attempt limits.
4. New `apps/web/scripts/quest-agent/run-classic-late-route.mjs`: ordinary WSS/loopback transport, live route/NPC/combat/pickup/book/save controller.
5. New `apps/web/scripts/quest-agent/test-classic-late-route.mjs`: forty-four positive and negative controller/verifier fixtures.
6. New `apps/web/scripts/quest-agent/verify-classic-late-route-evidence.mjs`: separate trace/source/UID/transition/save validation and bounded acceptance results.

`source-diff.patch` contains all six files, including complete additions. It does not contain the previous eleven-file v27 patch. Git reverse-apply check against the actual current worktree is recorded; root still owns forward integration and consumer compatibility on its own baseline.

## Repaired behavior and source difference

The changes are **Candidate playability repairs using inferred legacy intent**, not a claim of exact modern C# CHECKQUEST semantics. Original raw source and imported manifests are unchanged. Modern C# treats ACTIVE as a current quest and other CHECKQUEST tokens as completed; the supplied Jev-era numeric conditions are inconsistent with both that implementation and the current numeric-stage mapping.

Only these original named lines receive compatibility:

| Script | Section / original line | Reviewed intent |
|---|---|---|
| `MongchonProvince/StoneTemple/Stone` | `@MAIN`, line 4, `CHECKQUEST 135 0` | Current accepted quest: InProgress or ReadyToTurnIn. Available, Completed and absent are not active. |
| Same Stone script | `@Check2`, line 15, `CHECKQUEST 153 1` | Completed quest only. |
| Same Stone script | `@stonetomba`, line 39, `CHECKITEM StoneHeart 1` | A valid owned canonical `crystal-item-1080` in Bag1/Bag2, with real nonzero item UID. |
| Same Stone script | `@stonetomba`, line 41, `TAKEITEM StoneHeart 1` | Remove exactly one valid carrier before the original MOVE; do not move if no exact debit can occur. Original item1080 stack size is one. |
| `WoomyonWoods/TaoistVillage/BigTaoist` | `@Next1`, line 15, `CHECKQUEST 146 1` | Completed quest only. |

The source key, relative path, full raw SHA256 and full serialized imported execution representation SHA256 must all match. A known script with changed proof fails closed before dispatch; unrelated scripts retain the existing generic condition/item implementation and numeric mapping (Available0, InProgress1, Ready2, Completed3). No global reinterpretation, quest rebalance, drop modification, portal, profile change, runtime reachability relaxation or protocol schema change is included.

Original Stone text SHA256: `249ff6c6810c7f06cd52f551c2534a2e24e7cead835d6948b72101146cf154c2`; full imported script SHA256: `f86465391544e7633c1999aea856636266456d650bbd0d56af5d66322dc39d0d`.

Original BigTaoist text SHA256: `911a5edd4dc638eedf54819d0314cbcf2b438f32ba02d6d951091146ece02ebd`; full imported script SHA256: `e186e61b616cfb28f48611eeca19aba568cf9d9c6f39362ed932b133bcef3111`.

The old name-only StoneHeart fixture was invalid at StartGame. The corrected tests use lawful item1080 carriers, and a separate test obtains that same carrier from the normal q135 FinishQuest reward, then logs out and logs in again. That reward test begins from prepared ReadyToTurnIn state; it is not a complete q135 player journey. A valid different imported item cannot substitute for StoneHeart. Stone's actual numeric level window is 22–42, despite source prose saying 22–43.

## RED / GREEN evidence

All first failures remain in this directory. They are not erased or counted as gameplay passes.

- `rust-red-01` and `rust-red-02`: initial fixture failures from invalid name-only item carrier and alias. These are fixture defects, not evidence of NPC fixes.
- `rust-red-03`: corrected valid carriers against the unchanged old `npc_script.rs`, **7 failed / 1 passed**. It exposes the actual named quest/script gate failures.
- `rust-green-01`: compile failure because sha2 0.11's digest lacks LowerHex. Corrected inside the authorized existing file.
- `rust-green-02`: **8 / 8 pass** after the named source and carrier repairs.
- `rust-source-proof-01`: compile failure from private added execution-state metadata used by an existing parent-module struct update. Fixed to `pub(super)` in the same authorized file; no outside literal edit.
- `rust-source-proof-02`: **2 / 2 source-proof tests pass**, including changed raw text, path, key, parsed representation, and unchanged unrelated numeric semantics.
- `rust-green-03`: **11 / 12 pass**; the sole failure was a nonexistent test potion template. The fixture was corrected to the actual imported TownTeleport item.
- `rust-final-targeted-01`: that corrected negative item test **1 / 1 pass**.
- `rust-needmove-01`: preserved D10061 defect test **1 / 1 pass**.
- `rust-final-clean-01`: final unchanged six-file source, **13 / 13 pass** in one clean invocation, 191.70 seconds.
- `node-source-first-failure-01`: source route loader initially confused map index with map filename. Corrected using the source map-index join; no source topology was modified.
- `node-tests-03`: final controller/source/evidence tests **44 / 44 pass**. Earlier 33- and 43-case passing logs remain.
- `derived-plan-01.json`: source-validated static 76-hop plan, explicitly `plannedOnly=true`, `executed=false`. This command made no network connection.

Rust used the previously authorized phase2 cargo cache, `-j 2`, and test threads2. No root Gateway cache was used. These tests compile the simulation and use local in-memory sessions with normal ClientPacket calls; they are prepared-state protocol gates, not public Gateway traces or raw durable store inspection.

## Runner scope and acceptance boundaries

The six source chains comprise 76 directed transitions: ordinary stone tomb outward/return, Zuma outward/return, and red moon outward/return. A three-distinct-account Warrior/Wizard/Taoist cohort would comprise 228 planned transitions. No cohort was run here.

The runner requires a root-issued paired deployment attestation and its exact file SHA256. The attestation binds v27/208 runtime maps, all ten actual data source hashes, observed live health revision, Gateway executable SHA256, natural QA kill multiplier1, profile multiplier1, debug mutation disabled and this named legacy policy. Executable/config identity remains explicit root deployment attestation; it is not inferred from a revision string. WSS is supported directly; WS is restricted to explicit loopback. Credentials are read from a private scenario file and redacted deeply from evidence.

The main route uses normal movement and observed live map transfers only. Stone entry uses the real current NPC object, observed dialog body/link, normal selectNpcDialog, destination29,17 in D710A and a one-item debit. The Great Tao defect lane walks to the real D10051 source178,53 and records absence of entry instead of silently repairing it. Normal Logout/StartGame receipts are mandatory for an observed run; raw durable store bytes remain unverified.

Boss attempts are bounded to zero through three per target, a maximum of twelve attempts overall. The ordinary route has an unchanged 120-minute maximum deadline; a configured spawn wait can never extend that deadline. WhiteBoar, EvilSnake, ZumaTaurus and RedMoonEvil use their exact nested original DropPaths (122, 140, 179 and 155 entries respectively), not empty same-name tables or WhiteBoar0. Original/profile probabilities remain intact. No-drop, absent spawn, death and failed combat remain outcomes. No QA item grant, event spawn, HP reduction, deterministic drop or reset-until-pass occurs.

Pickup evidence binds a real owned ground item to the ordinary kill ledger and a new inventory UID/count. Book evidence requires lawful class/level, unknown skill, actual UseItem ACK, NewMagic, exact one-book debit and retained skill through normal relogin. Prepared or shop-bought books are not classified as natural acquisition. Learning a book does not assert skill activation, damage or cooldown acceptance.

Several public trace fields remain insufficient for strict natural acceptance. The verifier preserves this distinction:

- A map packet and correct destination prove a transition, but not a directly observed committed source coordinate when the owner receives only the transition. Such hops stay `sourcePositionAccepted=false`; route acceptance stays pending.
- Source respawn identity cannot be copied from static manifest onto a live Boss. Missing public respawn identity keeps `naturalSpawnAccepted=false`.
- Ground drops currently omit exact source-kill object linkage. Missing public `sourceMonsterObjectId` keeps `naturalAcquisitionAccepted=false`, even when ground pickup and skill learning otherwise pass.
- Owned equipment retention is checked, but complete equipment progression and naturally acquired skill activation are separate pending gates.

The synthetic verifier fixtures prove rejection behavior and evidence shape only. They are not used as actual network, drop, resource or human acceptance receipts.

## Source defects preserved

`D10051(178,53) -> D10061(20,25)` is original NeedMove=true. The supplied NPC/MapCoords bindings contain no ordinary inbound mechanism for this edge. The local prepared ordinary Walk test reaches source178,53 and remains in D10051. BigTaoist tests prepare a lawful local character inside D10061 solely to isolate dialog predicates; they do not assert ordinary entry into that room. This implementation adds no D10061 entrance.

The original Ancient branch has only internal walking edges: D710A↔D711A↔D712A↔D713A. There is **no authored outer walking exit**. Stone's normal item-gated entrance does not imply a walking return. Internal six-hop traversal and an owned lawful escape item must be validated separately on the later paired Gateway; no invented return portal is included.

`D71653` remains the one declared source-proven resource-only room: native resources209, runtime whitelist208. It is excluded from this runner and has no invented inbound mechanism. Frozen v27 reachability validation remains unchanged.

## Reproduction and later root-owned gate

From this project root, the focused local commands already executed are:

```powershell
$env:CARGO_TARGET_DIR = 'C:/mir2-playtest-releases/20261004-native-r17/map-implementation/phase2-208-runtime-209-resources/cargo-test-target'
cargo test -p mir2-simulation --test classic_late_route_gates -j 2 -- --test-threads=2
cargo test -p mir2-simulation --lib classic_late_route_source_proof_tests -j 2
node --test apps/web/scripts/quest-agent/test-classic-late-route.mjs
node apps/web/scripts/quest-agent/run-classic-late-route.mjs --plan
```

After root integration and a separately approved paired Gateway deployment, the following is an example invocation shape, **not executed in this round**. Private scenario credentials, exact class/character/level provenance, current v27 data files and paired receipt must already be prepared legitimately. `--map-pack-root` is the directory of original `.map.gz` collision inputs, not the rendered native PNG manifest.

```powershell
node apps/web/scripts/quest-agent/run-classic-late-route.mjs --input '<private-scenario.json>' --output '<NEW-run-directory>' --pair-receipt '<root-paired-gateway-receipt.json>' --pair-sha256 '<verified-file-sha256>' --map-pack-root '<approved-original-map-gzip-directory>'
node apps/web/scripts/quest-agent/verify-classic-late-route-evidence.mjs --input '<NEW-run-directory>' --output '<NEW-independent-verdict.json>'
```

Root owns the next actual three-class ordinary route run, live conditional NPC journey, bounded natural Boss/gear/book acquisition, exact missing provenance evidence, owned escape-item return, durable store inspection, client pairing, native visual acceptance and any release. This delivery performed no commit, push, release build, package, Gateway startup/restart, service change, upload or publication.
