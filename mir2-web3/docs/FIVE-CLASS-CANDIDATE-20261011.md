# Five-class Candidate — 2026-10-11

This round adds the normal Assassin and Archer content through level 50 to the
Windows/shared-zone Candidate. It does not certify all-level Crystal parity,
interactive Windows gameplay, or the unrelated outstanding P1–P7 work.

## Implemented scope

- `platinum_fiveclass` explicitly admits all five classes, 345 items and 97
  skills: the existing 63 skills plus 16 Assassin and 18 Archer skills. The
  three-class `platinum_176` profile remains the default for existing callers.
- Normal character creation, equipment, skill books, NPC purchases, positive
  book drop paths and the newcomer practice steps admit the two additional
  classes. Source books without an ordinary route have explicit, bounded
  project drop overrides rather than silently appearing in every drop table.
- Shared-zone skill admission checks real learned skills, mana, weapon state,
  cooldown and authoritative target identity. Delayed arrows, traps, summons,
  buffs, incoming damage and secondary targets use the existing single-writer
  zone lifecycle. Disconnect/rejoin does not replay an old target incarnation.
- MoonLight/DarkBody buffs preserve Source millisecond lifetimes, including
  half-second boundaries. Typed damage settlement separates defended damage
  from the damage actually clipped by remaining HP. The original three-class
  branches keep their existing behavior.
- Windows animation, visual effects, combat audio, ordinary skill presentation,
  supplies and equipment guidance support Assassin and Archer. The nine
  existing locales include the new guidance strings and pass validation.

The content bundle contains 208 runtime maps, 176 monsters, 101 NPC definitions
and 51 drop definitions. The package requires the 209 native resources,
including the existing resource-only map; these counts are inventory facts,
not proof of interactive travel through every map.

## Source assets and explicit limits

The actual Source pixel checks pass for 118 player libraries and 122,384 body
frames, 34 skills, 1,508 effect frames and 401 summoned-actor frames. Fifty-one
new WAV files are present. The genuine Source set lacks `M94-0`, `M94-1`,
`M107-0`, `M107-1`, `M137-7` and `M140-7`; the closure records these absences
and does not invent replacement Source audio.

The additional skills above level 50 remain excluded: MoonMist, SummonToad,
NapalmShot, OneWithNature and Portal. Do not describe this round as full
all-level five-class acceptance.

The finite Git media list contains 1,960 files. Generated player PNGs are
verified and staged for local packaging but are not recursively added to Git.
The normal package still checks genuine full-pack identities, fonts, native
maps and the complete selected-profile asset closure.

## First full merged Windows run — retained failure

The real full01 result is912 pass/75 fail/7 original external ignores. Its
[original source, command, result and logs](../artifacts/qa-evidence/five-class-candidate-20261011/d917-native-red01.json)
are retained, with no release acceptance. The checkout lacked its normally
generated native keyed-map index, so asset_root failed and local frames/maps
were unavailable to many tests. The original Node builder now generates
map0 with7432 real entries, missing241 under the unchanged2969 source budget.
The old, retained compiled binary passes a single asset-root diagnostic; this
does not qualify current repaired Source.

Two newly authored DoubleSlash tests expected idle100ms. The original Crystal
Player Standing descriptor and unchanged Native catalog are500ms; only those
two values are corrected. Active two-phase timings, queue/sequence, movement,
held input and camera assertions remain unchanged. Current-source full02
and newest NPC history integration precede any release build/publication.

## Second actual R25 merge — 2026-10-12 checkpoint

The actual d917 merge retains Source CanMove consumption, Turn delay, all
shared-buff real millisecond/pause clocks, bounded terrain loading, map
availability, lightweight outcome tick, ordered health, Native queue
ownership, mail feature isolation and eleven real Source pet WAV inputs.

Selected tests pass simulation124 and Gateway18, plus CanMove9, shared buff
clocks5, owner health16/32, zero-health7, outcome3, transfer4 and default-feature
monthly8. The latter includes both new classes buying the real card SKU,
claiming its mail, consuming its permanent UID and retaining one debit and
one expiry across normal logout/relogin. Two original billing fault-injection
cases require test-support and are not counted in the local eight-case run.
The unchanged original Linux workflow still executes them before release.

[Exact results and original logs](../artifacts/qa-evidence/five-class-candidate-20261011/d917-backend-checks01.json)
retain the initial120/4 failure. Three strict cold BoneFamiliar fixtures now
use genuine map0 terrain with equivalent geometry; their root/HP/control/
visibility/idempotence assertions are unchanged. The old Taoist Hiding tick
assumption is explicitly superseded by R25 real-ms projection and now also
checks real serialization and expiry. Production recovery is unchanged.

Native movement waits for both DoubleSlash phases and complete committed
BackStep Jump, including a Jump queued after Struck. New local prediction
cannot replace its camera window while that Source action is pending. Five
typed-packet combination tests are written; actual Native execution, fresh
clean-source builds, signed delivery and paired publication remain pending.
The original266 Native Rust file formatting check failed and is retained;
eight formatter-only files were corrected and the identical check now passes.

## First actual R24 merge — 2026-10-12 checkpoint

The first working merge retains the R24 owner HP/MP/death, monthly card
and commercial mail code. The former schema8 label was incorrect: actual
Zone canonical/runtime versions remain7/7 and Gateway/factory versions3/2. Normal no-target/direction casts use shared
Zone admission. Six Source material-free spells no longer attempt an empty
economic consume; PoisonSword still requires genuine powder. A real Focus
shot uses the viewport/health FIFO, so queue saturation cannot bypass ordering
or repeat mana/practice settlement. Map bootstrap discards that stale arrow
while retaining committed personal skill progression.

Actual selected checks pass simulation124/Gateway18, Windows bridge32 and
health20, locale13 and game-data75. The data total includes 67 library tests
and two integration groups of 5 and 3, rather than 75 library tests. One data
benchmark and four original locale performance probes stay ignored. The
wrong-package health filter that executed zero tests is excluded. Original
compile errors, the earlier Focus queue failure, the map-fixture abort and its
uncaptured diagnostic are retained; the final normal 18-case run has no Source
drift. Repeated and overlapping results are not summed.

[Exact runs and original evidence](../artifacts/qa-evidence/five-class-candidate-20261011/r24-first-merge-checks.json)
bind the actual working merge, not a future final release commit. Public
readback now verifies R25/feed20/client d917 and Gateway4006; the second actual
merge must preserve their movement/SourceCanMove, real buff clocks, Native
ingestion/mail boundaries and eleven real pet WAV files. Fresh final checks,
clean-source original compiler/build, signed package and paired publication
remain open. The older table below is the pre-R24 five-class evidence.

## Executed automated checks (pre-R24 history)

| Check | Result |
| --- | --- |
| Game-data crate library and integrations | 75 pass; one original benchmark ignored |
| Five-class simulation | 118 pass |
| Five-class Gateway receipts, reconnect and settlement | 12 pass |
| Native entity lifecycle and attack relations | 22 pass |
| Shared player combat and relationship modes | 7 pass |
| Runtime animation | 28 pass |
| Client audio | 26 pass |
| Client supplies and equipment guidance | 21 pass |
| Windows five-class bridge and effects | 32 pass |
| Updater library | 81 pass; four original ignores |
| R2 publisher / publication preparer | 23 / 11 pass |
| Origin deployment script fixtures | 20 pass |
| CDN Worker fixtures | 23 pass |
| Producer / delivery verifier capacity fixtures | 10 / 12 pass |
| Bootstrap content gates (pure fixtures) | 77 pass |

Each Cargo run has an exact source snapshot, guarded Cargo receipt, log hash and
unchanged-input result. The smaller repeated filters overlap the listed
suites and are not counted twice. See
[the check index](../artifacts/qa-evidence/five-class-candidate-20261011/automated-checks.json).

Five adjacent failed tests were repaired without altering production behavior
to satisfy their old assertions: two tests must perform real death/revival,
one pet test needs a real threat target, and the range/magic tests must wait
for authoritative projectile impact. Archer's new Source timing intentionally
changes its old immediate-hit fixture. FireBall's delayed settlement was
already present in the baseline. Original failures and the static baseline
comparison are retained; a baseline run is not claimed.

## Integration with the deployed client

Fresh read-only HTTPS checks on 2026-10-11 show that the public client is already
R24, signed feed sequence 19. Its clean Source is
`87fd85a0d5a819e5b38c90d192421f9239f65678`, later than this round's
initial parent. Preserve its shop, mail, month-card, billing and authoritative
owner-health fixes in a real merge before building this release. New five-class
modules also require explicit integration of owner-health operation receipts;
a clean textual merge alone does not prove runtime compatibility. All tests
above describe the pre-merge Source. No R24 rollback or publication is approved.

## Remaining release gates

1. Finish the normal updater/bootstrap integration. The expanded resource set
   exceeds the old 200,000-file / 32 MiB metadata limits. Both producer and
   signed consumer must retain measured, finite bounds; old installed clients
   need a reachable compatible engine-upgrade route.
2. Commit/push the exact Source and run the unchanged Linux release gates plus
   the added five-class checks. Existing failures must be diagnosed, not hidden.
3. Build the real Windows Release MSVC EXE from clean committed Source with the
   existing strict attestation builder and independently reviewed Cargo guard.
4. Stage and verify the explicit `platinum_fiveclass` package, sign the invited
   Candidate with the pinned existing CMS identity, then prepare a genuine
   installer and paired Gateway release. Public Authenticode acceptance is a
   separate gate; the internal invited signer is not a public publisher.
5. Verify the immutable uploaded files and signed discovery pointers, perform
   a fresh session-drain check, then deploy the paired release with recovery
   and readback checks. No Gateway restart has occurred in this round yet.
6. Run native interactive creation, movement, learned-skill use, supplies,
   reconnect and maps on all five classes when native control is available.
   Native computer APIs are currently unavailable, so no screenshot or human
   acceptance has been fabricated.

Root owns integration, production inputs, common documentation and rollout.
The active asset/client/content reviewers are read-only unless explicitly
reassigned. This round does not complete or resume an expired autonomous Goal.
