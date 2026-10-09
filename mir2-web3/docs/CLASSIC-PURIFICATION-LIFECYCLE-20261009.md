# Original Purification lifecycle Candidate — 2026-10-09

Implemented and locally tested for the current same-map lifecycle. Exact-source
Linux CI, publication, natural TCP/WebSocket gameplay and native/human acceptance
are separate gates. Simultaneous live transfer of both original actors to the
same destination during the 500ms cast remains open; this Candidate conservatively
cancels that case and is not complete Crystal parity.

Original `HumanObject.cs`4440–4446 queues the effect on the caster's ActionList
for cast+500ms. Completion at6249 checks the actual current target Node, map,
friendship and original level chance. `PlayerObject.cs`4751 supplies its own
friendly-target rules: self, group, Guild, a non-enemy Guild, or lawful PK/BrownTime
conditions. This differs from treating Purification as an ordinary hostile spell.

The current live-only pending record binds both typed actor/online/life identities,
keeps the absolute deadline and applies friendship and level chance at completion.
Death, revival, logout, reused session identity, another owner map and cold restore
cannot rebind an old cast. Transactions clone the live queue, while checkpoint and
serialization schemas remain unchanged. Self-cast admission now also requires the
actual current typed owner.

Completion removes every imported Source Debuff, including Curse12,
RhinoPriestDebuff53 and Blindness57, while preserving beneficial buffs. It clears
actual poison/control/slow leases and delayed-explosion state. Typed owned-Human
poison cleanup matches the exact current player identity, preserving unrelated
in-flight damage and new-life leases with the same object ID. Status packets reach
the current target observers. No movement, attack, damage or spell clock is shortened.

| Verification | Actual result and limit |
| --- | --- |
| Focused library | 17 pass: 15 new Source cases and 2 existing Purification cases. |
| Existing pet/Human Source regression | 38 pass with its original test-support feature; not natural Human PK acceptance. |
| Original shared_zone | Root ran all209 cases:201 pass /8 fail. The Purification story now passes, including its later assertions; the other8 failures stay RED. |
| Original shared_monster_hell_ai | Root ran all17 cases:17 pass,0 fail. |
| Legacy fixture corrections | Only Purification's actual +499/+500 completion and monotonic follow-up clocks changed. Original business assertions remain. |
| CI gate | The release workflow retains every existing security/appearance/classic/Source/PostgreSQL/siege gate and adds Purification, its original Zone story and all17 Hell-AI tests. Not yet dispatched for this Candidate. |
| Online/native/human acceptance | Not run; not published. Prepared Rhino/Blindness projections are explicitly not natural monster-producer acceptance. |

Early real failures remain in the raw archive: immediate-effect assumptions,
incorrect Curse targeting and spell clocks, empty serialization assertions,
level-chance expectations, self admission and the original bleed fixture. They
are not counted as passes. The entire Zone suite remains RED and no P1–P7 or
broader Goal completion is claimed.

[Complete original output, inspected Crystal source, current Rust and actual diff](generated/player-qa/purification-20261009/candidate-01/original-evidence.zip)
is442661 bytes, SHA256
`c6a15fadfab0a771330646902b3d223fbfd58ba664b7d5000b0d021a4352f504`.
[Entry pins and Root review](generated/player-qa/purification-20261009/candidate-01/EVIDENCE.json)
bind all six tested source files to parent97a942a66dc184aeb0731e02dbf59c22414a1513.
The archive is the actual pre-commit tested source; no future commit ID is invented.

Next, transfer the original opaque cast record only across a trusted live map
transition, preserving both proofs and absolute time. Original Crystal Teleport
keeps Node and ActionList; cold/login/despawn paths revoke them. Destination Join
must not erase the target's real Curse before a test reaches499ms, otherwise a
false cleanup pass would hide the map-transfer gap.
