# Purification live map transfer and Player powder admission — 2026-10-09

Implemented and tested as a bounded server Candidate. This closes the previously
open same-Node map-transfer case from source5860815944 and fixes carried-powder
preflight for a real hostile Player target. Exact-source release CI and deployment
for this new slice remain separate gates; native and human acceptance remain open.

The current published pair is gameR23/sourcefaefb69671272843958f68781b4f8ca2f4014522,
signedfeed18 and Gatewayfaef, with71 ordinary public checks and20 normal logout
acknowledgements. [Its actual stage, promote, Windows transport and final readback](https://github.com/Zombieliu/mir2/blob/f93ed2121046962f9c39dfc62f7696e90b0cc458/mir2-web3/docs/CLASSIC-TARGET-MOVEMENT-PUBLISHED-R23-20261009.md)
is pinned to the already pushed publication commitf93ed212. This server Candidate
does not require a new Windows package or change that publication receipt.

## Resulting behavior

Crystal `MapObject.Teleport` retains the same live Node and ActionList. The Manager
now moves an opaque, non-serialized value between its actual source and destination
Zones. It contains the original pending Purification actions, both actor proofs,
the actual buff map and absolute expiry, and the actual Hidden flag. Adoption
requires the complete current owner after the destination life/Node refresh.
It does not sign a new proof, infer state from a packet, renew a deadline or revive
a logout/cold/replaced-life action. Target-first and caster-first real map changes
both preserve cast+500ms; different destinations and stale actors remain rejected.

The real Player preflight now uses the same read-only target/resource predicate as
live dispatch before Gateway consumes a carried powder. Dispatch checks again
before changing MP, direction or action/cooldown clocks. Legal Poisoning consumes
one powder; immediate repeats and forbidden safe/friendly/dead/stale-point/range/MP
targets consume none. Normal LogOut and a fresh Login preserve the expected count.
All original attack-mode, group, Guild, safe-zone and spell/status clocks remain.

Root additionally reproduced Hiding flag loss on true map Join, then fixed actual
Hidden state and the first destination ObjectPlayer projection. Accepted Hiding
and MassHiding retain their original expiry and clear once at that exact deadline.
This state fix does not claim inherited visible-buff/audience parity: current
Hiding.Visible and live observer Add/RemoveBuff handling still differ from Source.

## Actual verification

| Check | Actual result and limit |
| --- | --- |
| Ordinary Manager |14 pass; real accepted spells, true map changes, original500/600ms clocks, death/rejoin/cold/ID conflicts, repeated hops and Hiding expiry. Prepared maps and actor stats are explicit. |
| Purification library |23 pass, including21 private cases and two adjacent existing cases. |
| Existing pet/Human and Hell-AI |38+17 pass with the original test-support feature; not natural Human PK acceptance. |
| Ordinary optimized Gateway adapter |4 pass through normal Login/StartGame/Magic/UseItem/LogOut/relogin; the real map0 TownTeleport case retains its500ms completion. In-process, not sockets/native acceptance. |
| Broader library |305 pass/8 fail. A separate clean pinned5860815944 checkout yields299 pass/the exact same8 failures. New cases account for the six extra passes; the suite stays RED. |
| Original shared_zone |201 pass/8 fail, matching the retained earlier failure set. No business assertions are removed or ignored. |
| Historical source586 Linux CI |Actual37901046490 succeeds in both original jobs. Genuine complete GitHub digest, ELF/size/hash and clean source metadata pass after strict206 recovery of a partial download. This is not CI for the new working diff. |
| New release CI |Workflow retains every original security/appearance/Source/PostgreSQL/siege gate, and adds ordinary Manager plus optimized Gateway map/powder tests. Final exact commit dispatch is pending. |
| Release/native/human |New server slice not yet deployed; full P1–P7/Mentor and frontend acceptance remain open. |

The default in-process inventory adapter saves at normal LogOut; these cases do
not prove a production PostgreSQL pre-ACK CAS. The existing Curse/stat double
aggregation observed during the normal scroll story is a separate open parity
issue. Rhino/Blindness prepared projections are not natural monster acceptance.

[All original positive and negative outputs, Source C#, tested Rust, diff and independent review](generated/player-qa/purification-map-20261009/candidate-01/original-evidence.zip)
contain180 entries,636709 bytes, SHA256
`d5a4fabf08dfaef0d1a934525d2f27f6837b58a7a4c49beee03d8a8507d2ee37`.
[Exact pins and gate boundaries](generated/player-qa/purification-map-20261009/candidate-01/EVIDENCE.json)
bind the actual pre-commit source to parent5860815944; no future commit ID is invented.
[Historical source586 Linux CI and strict artifact recovery](generated/player-qa/purification-20261009/linux-ci-01/EVIDENCE.json)
retain genuine original output in a separate360024-byte archive.

Earlier debug scroll misses, wrong fixture/schema assumptions and the pre-ACK
default-store assertion stay recorded. Optimized success does not convert those
results into passes or extend the original spell window. Both broader RED suites,
the original Hiding RED and the read-only audience finding remain inspectable.
