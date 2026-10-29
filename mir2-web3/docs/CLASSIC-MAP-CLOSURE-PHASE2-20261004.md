# Classic map closure, phase 2: 208 runtime maps, 209 native resources

Profile `platinum_176` v27 adds the 44 reachable Stone Tomb, Zuma and Red Moon
rooms excluded by v26. Its 208 runtime maps retain every existing 164 map and
all merchant/Sabuk resources. Native coverage includes one additional original
room, `D71653`, through explicit resource-only metadata. Neither this resource
stage nor a count of 209 maps proves complete classic gameplay acceptance.

The reviewed v26/164 phase, receipts and resources remain unchanged. This phase
is based on `38a64de2985c82008a596a34768f36185980f62e`; root owns integration,
Gateway validation and release. No Windows client build, package, installation,
service restart, upload or publication was performed in this worktree.

## Exact admission and source boundary

The newly admitted maps are:

- Stone Tomb: `D716`, `D71601` through `D71625`, `D71650` through `D71652`,
  and `D717` (30).
- Zuma: `D5062` through `D5067`, `D5069`, and `D511` through `D514` (11).
- Red Moon: `D10012`, `D10013`, and `D10032` (3).

New map recommendation ranges copy the adjoining admitted floor/maze rules.
All existing map rules and every unrelated profile field remain unchanged.
Only `EvilSnake` and `WhiteBoar0` are added to the monster whitelist. The source
`D717` respawns use image 49/count 2 and image 48/count 28 respectively; their
source respawn definitions are not edited. Boss lists, drop tables, experience,
NPC scripts and respawn timing are unchanged.

`nativeResourceOnlyMaps: ["D71653"]` is defaulted for older JSON consumers.
The compiled bundle summary separately records `maps=208`,
`nativeResourceOnlyMaps=1`, and `nativeMaps=209`. Its regenerated hashes bind
the actual v27 profile and the same 13 source dependencies.

The only permitted resource room is the canonical `D71653` in this profile.
It must match original map index 235, title `TacticalMaze`, mini/big-map indices
0, no respawns or safe zones, and exactly one outgoing movement to index 209
from `(17,12)` to `(36,34)`, with its original flags. The imported 463-map
movement manifest contains no inbound movement to 235. The original NPC and
map-event manifests do not reference `D71653`; source searches found no ordinary
entry mechanism. No entrance is invented and this room is never runtime
admitted. The original runtime reachability loop and its error are unchanged.

Node and PowerShell scope receipts bind the profile SHA, 208 runtime count,
exact sorted 209-name native list, resource-only list and SHA, and the actual
respawn/NPC/map-event source file SHAs. A final report binds both generated
resource manifest SHAs as well. Static topology retains
`unreachableMaps=["d71653"]`, `unreachableRuntimeMaps=[]`, and the exact
source-room proof. `excludedClassicMaps=["d71653"]` describes runtime exclusion,
not a missing native asset. Explicit custom lists remain exact; selecting the
15-map journey set does not add the orphan or the other profile maps.

Only the named proved room can be classified separately. Wrong/duplicate/path
identities, a forged room, new inbound/NPC/event entry, changed room geometry
or outgoing movement, another unreachable admitted branch, or a hidden
runtime-unreachable claim fail. `completeClassicWorld` and
`runtimeGameplayAccepted` remain boolean false.

## Focused verification

- Node scope/source/topology/CLI fixtures passed 12/12. The combined adjacent
  native pack, routing, gutter, atlas-budget, quest-route and scope run passed
  64/64.
- PowerShell scope/configuration fixtures passed, including actual profile
  identity/source hash binding, exact explicit selection, omitted classic
  branches/resource room, false admission/source proof, another unreachable
  runtime branch and an entirely absent `Monster/049` library.
- Rust tests passed 5 new resource-scope tests, 3 existing natural content-loop
  tests and 4 profile/bundle/source-book unit tests. Adding `D71653` to the
  runtime whitelist still yields the original exact reachability error;
  removing the Sailor scripts still rejects map 5.
- Node and PowerShell scope JSON match. Actual generated resources passed
  PowerShell native, final coverage and actor guards. Bundle `--check` passes
  with 13 unchanged dependencies. Tracked PublicRoot inputs remain unchanged.

The first PowerShell run failed because the new one-room digest incorrectly
called the existing journey-list digest, which requires map 0 and the other
legacy minimum maps. The fix adds a separate digest constrained to an empty
selection or exactly `d71653`; it preserves the existing journey minimum.
`focused-powershell-first.log` and the passing second run are both retained.

Commands from the project root (use the external evidence root below):

```powershell
node --test apps/game-client/platform-windows/scripts/test-candidate-map-coverage.mjs
powershell -NoProfile -File apps/game-client/platform-windows/scripts/test-candidate-map-scope.ps1 -EvidenceRoot <phase2-root>
node --test apps/web/scripts/test-native-keyed-map-pack.mjs apps/web/scripts/test-map-render-routing.mjs apps/web/scripts/test-map-atlas-gutters.mjs apps/web/scripts/test-map-atlas-budget.mjs apps/web/scripts/quest-agent/test-route-manifest.mjs apps/game-client/platform-windows/scripts/test-candidate-map-coverage.mjs
cargo test -p mir2-game-data --test classic_native_resource_scope --test platinum_176_content_loop --target-dir <phase2-root>/cargo-test-target
cargo test -p mir2-game-data --lib platinum_176 --target-dir <phase2-root>/cargo-test-target
node scripts/build-platinum-176-profile-bundle.mjs --check
```

## Actual generation and original actor verification

All assets and evidence are outside the checkout under
`C:/mir2-playtest-releases/20261004-native-r17/map-implementation/phase2-208-runtime-209-resources/`.
`generation-input.json` records exact arguments, scope and process-local temp
directories; no system environment variable or existing guard was altered.
The locked full-pack index SHA remains
`77a9050818df502f65a7b151976ee7d66bd01e94d87680cf0760b925dd10dbee`.

| Result | Actual value |
|---|---:|
| Runtime whitelist / generated native maps | 208 / 209 |
| Keyed/additive entries | 52,333 |
| Native PNG image bytes | 250,336,128 |
| Original-source fallback entries | 41,569 |
| Newly completed ordinary floor frames | 12,217 |
| Completed atlas pages / source frames | 546 / 16,940 |
| Completed atlas image bytes | 149,452,428 |
| Maximum atlas page bytes / pixels | 475,728 / 262,144 |
| Unique map references audited | 69,826 |
| Drawable references covered | 67,993 |
| Drawable gaps / omitted / unexpected maps | 0 / 0 / 0 |
| Original no-draw / out-of-range references | 550 / 1,283 |
| Runtime-unreachable maps / conditional maps | 0 / 16 |

The generator still reports `missingSourceCount=1177`, below its unchanged
2969 budget, and `noDrawReferenceCount=531`. The independent complete-layer
audit classifies ordinary floors and every animation phase, retaining original
empty/out-of-range distinctions and reporting zero drawable gaps. These
counters describe different scopes. Generation took 314.5 seconds, floor
completion 311.9 seconds and the final audit 6.4 seconds, not client load time.

Compared with the immutable 164-map stage, there are 122 additional native
entries, 317 additional atlas source frames, 11 atlas pages and 439 drawable
references. Native plus atlas PNG bytes increased by 3,424,213. This is an
asset-byte delta, not a measured installer/update or resident-GPU-memory delta.

`Monster/049` was exported from the original Crystal `049.Lib` (1,137,879 bytes,
SHA `b52c0eadaf06a015360fbf9d9e68e877b8ca912fbdea6d99d9b1c7785497c4e3`).
All 224 drawable frames match original decoded RGBA, PNG geometry, x/y and
shadow offsets; all mask declarations match the original zero-mask library.
The seven source actions are Standing, Walking, Attack1, Struck, Die, Dead and
reverse Revive. The existing FrameSet catalog already matches, so its bytes
remain unchanged. The candidate actor guard now explicitly requires 049.

The source-exact profile/equipment closure verified 140 libraries and 60,780
drawable frames, with zero missing frames, unknown libraries or verification
failures. Its PNG bytes total 139,993,251. Separately, the native candidate
actor guard passed 128 monster/NPC/gate/MapLinkIcon libraries and 25,597 frames.
These checks have different coverage and are not interchangeable. Inputs were
physical copies of the locked cache; only external actor staging was modified.

Receipt identities:

- Profile v27 SHA: `f50ef1b78e2544df4fd3a65902f6e72f4ec3cbc7693e0ee373d6ee899581e7a9`.
- Expected-map SHA: `b3fc689ed59c7293917ffe4f5df7080a19018779cdd59155c8ac80ef20e8df69`.
- Source-topology SHA: `5cc2488e0c621d56f62d57a20243365c011c5a2df3f63c03a2cf0983793d5ccd`.
- Native manifest SHA: `42cc002dd0f7c211916a976fadcc91af5c16cf2aa851200e464ac80aeeccb5b9`.
- Atlas manifest SHA: `32aa4e7e56550b1067a880bc29ca279f13a699aac6c71e8917154676f0092288`.
- Monster/049 metadata SHA: `6173faeac54c9708d791c1546c0566e97af70a86c12497f7b08b63eb6a961b63`.

## Remaining gates

Root must integrate the source, verify consumers of the defaulted schema,
stage/sign the reviewed assets and measure real update/package size. Ordinary
three-class Gateway travel through the 44 added maps, conditional predicates,
maze movement, Boss encounters, drops and item acquisition, repeated map
handoff, saving/reconnect and bounded image retention remain separate checks.
The source orphan cannot be counted as an ordinary destination. This phase
does not accept gameplay, release a new client, expand to all 463 imported
maps or add later-Mir3 content.
