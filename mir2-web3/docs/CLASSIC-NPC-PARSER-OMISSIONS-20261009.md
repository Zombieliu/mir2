# Original NPC CHECKHUM parser omissions — 2026-10-09

Status: **implemented and locally tested Candidate; not published or human accepted**.
Base source is4baa66e99374cb6752906f94c6a264e2f0f1416b. Public Gateway staysc704,
Windows client staysR23/sourcefaef/signedfeed18. Root owns integration,
execution, Git and publication; the one-file Gateway fixture worker returned ownership.

## Original rule and resulting behavior

The supplied C# parser omits CHECKHUM with fewer than four ASCII-space tokens.
An otherwise empty CheckList succeeds and displays SAY. Rust previously added
these lines as failed conditions, executing the wrong ELSE branch. The genuine
default149 CHECKHUM1D10071 and MissMi CHECKHUM1EM002 reached arena actions or a
level-failure page. Both now display the original busy SAY.

The interpreter checks original argument count before runtime substitutions;
ARG values containing spaces cannot create missing arguments. Other valid
conditions still control the branch. The unsupported two-argument evaluation
shorthand is removed. The default compiler verifies the actual original min4
clause before sealing this omission. Complete checks remain compiled even with
invalid runtime values; SAY text is preserved.

Exactly one sealed omission is added: @_OnAcceptQuest(149), instruction518
(header516), reason missing-original-checkhum-arguments. All45 earlier
omissions, all26 source texts, original parser text and expanded script stay
unchanged. Root SHA stays e008fc6e5e12cff7bdaacec530193a00d20d1ab9857b3288a140edc6f4c71107;
execution SHA stays5522ab1f667a363e34965f7552f7b482f93954ad53c2dc776c5eed8a03f9f8be.
New contract44f92fb3f80c8fb939f442c62781685a45033bd0392d2aeb7cf5526857832e51
is pinned by the runtime.

Original anchors: Crystal/Server/MirObjects/NPC/NPCSegment.cs:102–106,255–259,
2947–2948,4956–4963; Build/Server/Debug/Envir/SystemScripts/00Default/OnAcceptQuests.txt:1–14
and NPCs/BichonProvince/Event/MissMi-EM000.txt:16–23 under that Envir root.

## Actual local results

| Run | Actual result | Scope |
| --- | --- | --- |
| 02 / 06 | Node4pass/2fail before;6pass/0fail after | compiler omission, retained complete checks/SAY, original rule proof |
| 03 | Rust2pass/4fail before | actual149/MissMi wrong pages and incomplete/expanded arguments |
| 07 | Rust30pass/0fail/0ignored | five parser and25 default-event cases |
| 08 | Rust104pass/2fail/0ignored | original NPC plus Source8/refine10/ordered-pet32 checks |
| 09 | optimized Gateway4pass/0fail/0ignored | real Poisoning rejection/death and both actor-order scroll/Purification chains |
| 10 | clean pinned4baa,0pass/the same2fail | two adjacent failures already exist on the unchanged parent |

MissMi uses genuine loaded NPC1482 onEM000, with an isolated prepared character,
authenticated Login/StartGame and ordinary CallNpcMAIN→declared start link.
The full exported record is unchanged; no transfer/new diagnostic occurs.
149 uses the trusted default hook inside the full Source guard: exact busy
response, one queue consumption and one durable revision. Full live and full
stored-preimage comparisons preserve durable skill-clock envelopes.
These are not natural travel, ordinary AcceptQuest networking or Windows rendering.

All original failures remain. Run01 had test-fixture compile errors; run05
passed5/failed1 because its expected stored record incorrectly used live skill
exports. The corrected fixture compares the entire actual stored preimage,
including its existing cooldown envelope. The two parent failures remain
crystal_npc_buy_item_dead_player_is_silent_and_preserves_state and
crystal_npc_stage5_message_buff_appearance_name_list_and_hero_commands_execute.
No test or assertion is dropped.

## Actual Linux failure and bounded fixture correction

Exact4baa Linux CI37941832277 failed the original optimized Gateway
Purification transfer gate:3pass/1fail. Its two-scroll chain finished48ms after
the real500ms deadline, failing the scenario's precondition before later
effect/identity assertions. Siege succeeded; no qualified package/deploy occurred.
The first log fetch failed with EOF; a fresh original-job request succeeded.
Both requests and the original1201511-byte log are retained, SHA
ea947c8de7e8353198537459a2acb04ca3d5813db5ac661204c38f173170231a.
This is not evidence that Source guard changed Purification.

The fixture reads the actual owned TownTeleport UIDs after real Curse lands
and before Purification. It then sends the original normal UseItem requests
in both actor orders. Every strict500ms, real transfer, identity, original
Curse expiry, one removal and normal-logout assertion remains. No fake time,
extra retry, geometry, skill or gameplay change is introduced.
Optimized local run09 passes4; chains finish153/179ms before their deadlines.
New exact-source Linux CI remains required. The original workflow also adds
the five dedicated parser tests to its existing Source/ordered-pet gates.

## Evidence and open gates

[Receipts and limits](generated/player-qa/npc-parser-20261009/candidate-01/EVIDENCE.json)
and [72 original entries](generated/player-qa/npc-parser-20261009/candidate-01/original-evidence.zip)
are retained. Archive392791B/SHA
3aa7f0bf05ddf96411cb9ba3b45a702d4343263c6eee52e34326888a6a0aafdf;
all original bytes and ZIP CRC verified. Stable test inputs declare17/22 paths;
the clean-parent comparison declares16. These are selected inputs, not a full
Cargo/toolchain/workspace closure.

Complete CHECKHUM still needs successful-loaded map/instance indexing, trusted
full population including dead players, command-bound capture, source-order
updates and admission reservations. Current private-map/count1 evaluation
is not shared parity; instance and invalid-operator behavior remain open.
Ordered world effects need same-Source-CAS outbox, authoritative execution and
durable recovery. The [P7 contract](CLASSIC-P7-SOURCE-WORLD-AUDIT-20261009.md),
full P1–P7/Mentor and natural/native/human gates stay open.
The expired automation remains stopped; no full Goal completion is recorded.
