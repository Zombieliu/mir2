# Classic later-route evidence — 2026-10-08

This is bounded Candidate evidence. Prepared doorway tests are separate from a
continuous player journey, original-C# execution, natural Boss loot, an installed
Windows client, and final human acceptance. P2 is not 100% complete on these tests.

## Current source and controller

`platinum_176` is version 27, with 208 typed runtime map-whitelist entries. Native
resource scope includes 209 maps; `D71653` is a source-only room and has no authored
player entrance. These counts were previously verified in
[the map closure evidence](generated/player-qa/classic-map-closure-20261004/phase2-208-209/README.md).
They are not proof that every map has been played.

The controller now reads `mapWhitelist[].fileName`, rejects duplicate/invalid
entries, and pins the actual profile SHA-256
`7ee7a6de1c253e8ba3831e3455d6ba0c59bc9dc00ef90841947d5ec58989395d`.
The respawn manifest SHA-256 is
`907db052030a97036e404429373dd3ad3e0e838e279613080e92c3be28bf3ff2`;
the change from its prior pin consists of the five original map flags used by
the shared experience/pet work. Other source hashes remain checked by the
controller's `SOURCE_HASHES` constant.

Public WSS uses an explicit, exact `webOrigin` attested by the root deployment
receipt. Node 22's actual WebSocket upgrade sends that Origin; the server's Origin
allowlist remains enforced. A loopback HTTP upgrade fixture checks the observed
header. Trace and report retain the same Origin and paired receipt hash.

Controller tests: **51 passed**, command
`node --test apps/web/scripts/quest-agent/test-classic-late-route.mjs`.
Local log: `C:/mir2-build/p2-classic-controller-20261008-05.log`.
Static `--plan`: passed; `executed=false` and gameplay acceptance remain false.
Local log: `C:/mir2-build/p2-classic-plan-20261008-01.log`.

The public ground-item adapter uses the actual serialized
`GroundDropLootSnapshot::InventoryItem` kind, `inventoryItem`, and the
`exactItem.uidAssigned` payload. Nested protocol `UserItem` fields remain
`unique_id`, `item_index`, and `count`. The obsolete synthetic `item` kind had
hidden a real controller mismatch; it is now rejected by the verifier. An
already assigned ground UID must equal the acquired UID. A fresh monster drop
instead has `uidAssigned:false` and UID zero until ordinary pickup; its new bag
UID is observable, but the controller cannot claim a pre-existing ground UID or
an exact source-kill receipt. Source respawn/kill identities still need separate
authoritative evidence when absent from public snapshots.

## Actual prepared doorway test

`apps/simulation/tests/classic_late_route_story.rs` covers the 76 directed map
pairs in the Stone Tomb, Zuma, and Red Moon round-trip plans for Warrior, Wizard,
and Taoist: 228 prepared stories. Each starts an explicitly prepared level-50
actor adjacent to a real original doorway. The test then requires:

1. Full packaged original Zone collision, with bounded terrain; no unbounded
   fallback and no wall removal.
2. An accepted shared-Zone ordinary Walk, owner UserLocation, and authoritative
   SaveTransform at the original source cell.
3. The first actual source-bound canonical movement selected by the existing
   gateway bridge, with matching destination MapInformation, map index, and
   exact original destination coordinate.
4. Unchanged item custody, normal LogOutSuccess, a saved destination, and normal
   StartGame with that saved map/coordinate.

Fixture configuration is reused to avoid repeatedly importing/hash-initializing
test data. Every case still starts a new authenticated runtime from its declared
prepared save. This is not a continuous 76-hop journey, and the shared Zone in
this fixture does not model a surrounding live crowd of monsters/players.

The first run retained a compile failure for `MapInformation.index` (the actual
field is `map_index`). The second run passed the Warrior's 28 Stone Tomb
directions but failed because the test selected the first authored Zuma doorway
without the original ValidPoint gate. No implementation or terrain was changed
to make that record pass.

| Original D5068 source | D5071 destination | Original 50×50 terrain |
| --- | --- | --- |
| 91,11 | 60,16 | Outside bounds |
| 92,38 | 34,44 | Wall |
| 90,70 | 9,77 | Outside bounds |
| 92,44 | 8,10 | Valid original entrance |

Crystal `PlayerObject.CheckMovement` skips a movement when its destination fails
`Map.ValidPoint` (PlayerObject.cs around 4533; Map.cs 637 checks bounds and cell
validity). The simulation correctly retains the fourth doorway. The test and
controller now prefer an authored source that the real runtime publishes.
The original three bad records remain unchanged.

Complete prepared doorway run: **228/228 passed** (one aggregate test), with
Warrior, Wizard, and Taoist each completing all 76 directed doorway cases,
including normal save/relogin at each destination. The aggregate test finished
in 3660.44 seconds; it was not skipped or shortened after the retained failures.
Command:

```powershell
$env:CARGO_TARGET_DIR = 'C:/mir2-build/p6-server-20261008'
cargo +1.89.0 test -p mir2-simulation --test classic_late_route_story -- --test-threads=1 --nocapture
```

Local logs are `C:/mir2-build/p2-late-doorway-story-20261008-01.log` (compile),
`-02.log` (retained source-choice failure), and `-03.log` (complete green run).

## Separate gates still required

The existing `classic_late_route_gates` fixtures exercise 13 prepared ordinary
NPC/item cases: source-bound StoneHeart quest/level/count gates, exact one-item
debit, wrong object/distance rejection, Big Taoist pages, learned-book NewMagic
and one debit, normal relog, and rejection of the unbound Great Taoist entrance.
Their prepared items and levels do not establish natural acquisition. The
current adjacent rerun passed **13/13** in 342.60 seconds, using normal
`ClientPacket` NPC/item paths; local log:
`C:/mir2-build/p2-late-gates-adjacent-20261008-01.log`.

`D10051 (178,53) -> D10061` is a supplied `NeedMove` without a matching default
NPC page. It remains unbound. `D710A`–`D713A` have internal authored connections
but no outer walking exit; an owned escape item is a distinct lawful return
mechanism. Do not invent an entrance/exit to close an evidence count.

Public ordinary routes require root-issued paired executable/source evidence,
ordinary test accounts, live source positions and transitions, save/relogin,
and the controller's independent trace verifier. Natural WhiteBoar/EvilSnake,
ZumaTaurus, and RedMoonEvil combat, source-kill-linked equipment/book pickups,
learned-book activation, and installed Windows visuals remain separate tests.
A non-drop stays a non-drop; no QA spawn, relocation, damage/drop multiplier,
or prepared book may be counted as natural loot.

## Public controller command template

The root coordinator prepares the private scenario JSON and attests the deployed
pair. Keep account credentials outside the repository and do not print them.
Use a new evidence directory for every attempt. Replace the placeholder paths
and hashes with the actual root-controlled files before execution.

```powershell
node apps/web/scripts/quest-agent/run-classic-late-route.mjs --input '<private-scenario-json>' --output '<new-evidence-directory>' --map-pack-root 'apps/web/lib/generated/crystal-map-pack' --pair-receipt '<root-paired-receipt-json>' --pair-sha256 '<actual-receipt-sha256>'
node apps/web/scripts/quest-agent/verify-classic-late-route-evidence.mjs --input '<evidence-directory>' --output '<new-verdict-json>'
```

Private scenario fields are `gatewayUrl`, `webOrigin`, `className`, `accountId`,
`password`, `characterName`, `characterIndex`, `lane`, and
`startingState:{preparedLevel,level}`. Main routes use
`lane:"main-round-trips"`; optional natural combat uses
`maxKillsPerTarget:1` (maximum 3), `spawnWaitMs:3600000`, and
`timeoutMs:7200000` (maximum 120 minutes). Prepared levels must be declared as
prepared. The other bounded lanes are `stone-entry` and `great-tao-defect`.

The attestation schema is `mir2.classic-paired-gateway-attestation.v1`, with the
real `sourceRevision`, `gatewayExecutableSha256`, matching `gatewayUrl` and
`webOrigin`, profile ID/version, all ten current source hashes, original
drop multipliers equal to 1, `debugWorldMutationEnabled:false`, and
`namedLegacyPolicy:"source-bound-stone-big-taoist-v1"`.
The controller also reads live `/health` and rejects a revision mismatch.
The child worker has not issued this deployment attestation or used public
account credentials.

Three-class public aggregation requires three distinct authenticated account
fingerprints and the same pair. The verifier continues to keep complete-world,
fresh-level journey, runtime gameplay, and native visual acceptance false when
those separate gates have not been demonstrated.
