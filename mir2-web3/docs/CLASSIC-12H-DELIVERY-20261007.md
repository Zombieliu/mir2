# Classic gameplay delivery — 12-hour resumed work

Started 2026-10-07 15:13:35 UTC. Delivery checkpoint is 2026-10-08
03:13:35 UTC / 11:13 Asia/Shanghai. Work remains active; this document does
not mark the older blocked Goal API record complete.

## Current status — 2026-10-08 00:10 UTC

The new shared gameplay implementation is a local Candidate pending exact-source
CI and paired publication. Public Windows is still R20/source8faedd89fbcdda7d4f814c05f7d2f56979ab5bf5,
signed update16; invited-playtest Gateway remains c6c32381a646dae1067dafdeec57691f606b1779.
Original production realm, real saves and D/F installations are preserved.

| Priority | Implemented and verified in this Candidate | Remaining gates |
| --- | --- | --- |
| P1 movement/combat | Inclusive combat RNG no longer aliases 300/600/2500ms cadence; native owner-Struck2500ms/HP10 walking fallback; native input171 and shared escape14 checks; live owner-wire ID mapping fixes the normal notification path | Strict real seven-monster pressure/attack/escape/quiet cohort is still inconclusive; native held-right-button/human acceptance and paired publication |
| P2 late maps/Boss/loot | Source-pinned controller and actual canonical doorway collision/Walk/save/relogin matrix being executed; original invalid Zuma destinations are rejected and fourth legal doorway is selected | Matrix final receipt; natural three-class full progression, real Boss/source-linked drops/books, original odds, conditional instance entrances and native resources/rendering acceptance |
| P3 XP/drop ownership | Original party16-grid/level/f32 allocation; native kills use trusted Store-to-Zone profile admission; real TCP/WS party story; EXP potion safe-zone pause/resume/expiry and CANGAINEXP source gate; generic Boss owner and causal PK source settlement | Remaining original Hero/Pet group-health refresh, broad natural Boss/PK/drop ownership and published-process recovery acceptance |
| P4 pets/PK | Typed shared player targets, modes/focus/retaliation, group/Guild/safe/Brown/life guards; original Taoist growth, source PetSave, recall/NoPets/logout/cold restore; actual Vampire bite/death and Toad flight.99 focused checks, plus real TCP/WS Source-save/rollback/cold restore story | Source NPC ordered shared-pet/world outbox, unsupported Totem owner chain, native human/public-process acceptance |
| P5 Guild/war | Normal rank insert/options/promotion/kick/leave/disband, notice and rank rename; symmetric paid ordinary wars and authority clock; normal earned Guild XP in the same Source CAS; bounded TCP/WS party+Guild story | Full current integration/CI and competing PostgreSQL writer/clock gate, native management/war acceptance, paired publication |
| P6 mining/refining | Original raw0..5 Belt/6..85 Bag wire,16 material cells, UID custody/fee/check/collect; Source rollback/retry/full-bag/deadline tests10; market9/oven10/shared escape14 regression; native wire3 and Bevy refine19 including unknown-outcome recovery | Final native bridge timeout regression, actual ordinary Carlos workflow in published pair and human visual acceptance |
| P7 map events | Actual default NPC call sites,26 source files/169 expanded sections, exact parser omissions, queue128/budget32, Login/LevelUp/Quest/client/NPC/map/death/script-item hooks, Save/rollback/idempotent recovery and Guild leave same-CAS ordering;24 tests and generator4 | Shared authoritative GIVEPET/REMOVE/CLEAR/MONGEN/MONCLEAR/GROUPTELEPORT/CHECKHUM, ground-hole caller; original18 general event filenames have no identified original scheduler caller and cannot be presented as implemented scheduled events |
| Mentor | Source-owned final-XP bank, original expiry/graduation/logout settlement, bounded recipient-owned once-only credit; complete source rollback/cold/revision tests11 | New normal two-account transport story, paired publication and native human acceptance |

## Evidence scope

Focused verified suites: original math11; source/private potion and late-selection13;
party14; earned Guild9; owned-pet/custom-AI99; source default NPC24;
mentor11; refine custody10. Counts are per named suite, not summed into a parity
percentage. Related cases overlap and isolated prepared scenarios do not prove
normal natural progression.

New ordinary TCP/WS pet story uses actual SummonSkeleton/Amulet consumption,
actual native kill and the normal inventory Source service. BeforePersist
produces no XP ACK or pet growth and restores the full save; retry publishes
before ACK. Each positive GainExp grows the real pet once. Normal logout releases
all old Store Arc owners; a new file load and authenticated StartGame restore
canonical five-field PetInfo. This is actual file-cold restoration in one OS
process, not an independent OS/process restart. Three PK source boundary tests
use real issued deaths, not manually manufactured receipts. A fourth retains
already-saved pet growth across temporarily unavailable online mapping.

Two network defects were found and repaired: first shared player life generation
is legitimately zero; owner live notification copies must map global player ID
back to that socket's trusted SelfPlayer ID, including nested pet MasterID.
Global Zone state, observer identity and refused-send FIFO packets retain their
original IDs. Dedicated live/backpressure tests are still being run.

The first current pressure run returned process success but all three reports
were inconclusive. It is not a behavioral pass. The harness now fails unless
qualification, attack escape and five-second quiet evidence all pass; the next
strict run remains retained as RED/inconclusive. No classifier, source clock,
original stat or acceptance threshold was weakened.

## Source contract and unfinished work

P7 uses actual parsed source with original invalid/missing includes retained as
sealed parser omissions. Missing scheduler callers are not invented from file
names. Source pet/world instructions still mutate the personal compatibility
world; a real shared implementation needs an ordered server-only plan and a
durable outbox in the same source CAS, followed by once-only Zone consumption.
GIVEPET then GIVEEXP/CHECKPET must observe the newly created logical pet; late
unordered packet replay cannot provide that contract. GROUPTELEPORT must route
actual immutable group identities through each Zone lifecycle, not create
RemotePlayer in a private session.

Original Source Boss/drop odds, movement clocks, security/admission limits and
realm capacity remain unchanged. No undisclosed account/item/spawn grants are
used in public verification. Prepared positions, levels, skills and encounters
are labelled. Generated data, dormant helpers, successful builds, isolated test
counts and human acceptance remain distinct.

## Ownership and delivery

Root is the sole runtime/routing integrator and publication owner. The three
bounded workers own read-only reviews/new tests and declared nonconflicting
modules; high-conflict files have one writer. Agent instructions and five parity
progress documents are updated with Candidate and public states separately.
Failures remain in the owned build archives. Sanitized passing logs and hashes
are exported under generated/player-qa/classic-20261008.

Exact-source server CI, clean attested Windows build, strict complete asset
packaging/CMS/CDN stage, same-commit separate promotion and fresh normal session
drain precede any actual public cutover. If players are online, finish staging
and defer the switch; do not kick players to satisfy a deadline. No full100%
claim is made while the remaining gates above are open.
