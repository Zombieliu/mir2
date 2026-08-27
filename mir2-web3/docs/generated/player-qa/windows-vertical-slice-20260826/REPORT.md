# Windows Vertical Slice Evidence Report

- Date: 2026-08-26
- Branch: `codex/wn-candidate-recovery`
- Packaged runtime source: `b5c0ecb604946a858bf5d060a2cca306032c0e62`
- Candidate: `WN-CANDIDATE-03-20260826`

## Scope

This report binds the bounded functional Windows/server vertical slice to one
exact attested Release EXE and one exact nonvisual Candidate package. It does
not certify global Crystal parity, strict Windows Candidate 100%, or a
human-accepted release.

## Passed functional evidence

| Gate | Result | Meaning |
| --- | ---: | --- |
| Simulation `vertical_slice` | 8/8 in 283.03 s | The complete current suite passed, including five-class entry/basic play, Bichon quest/drop/reward, the original newcomer quest 1-to-9 route, and shared multiplayer ownership surfaces. |
| Original Bichon quest 1-to-9 focused run | PASS in 216.69 s | A fresh Warrior completes the authoritative quest chain without QA gold, damage multipliers, or direct HP mutation. The route buys and accounts for the original eight small HP drugs and uses safe-zone natural regeneration between field trips. |
| Simulation `shared_zone` | 195/195 in 3.15 s | Current shared-Zone movement, AOI, combat, ownership, checkpoint, and settlement regressions passed. |
| Accuracy-roll regression | 1/1 | An agility-eight target receives both hit and dodge outcomes across successive deterministic ticks. |
| Native Zone high-evasion and replay checks | PASS | The physical-hit correction remains deterministic across identical runs while no longer locking one attacker/target pair into a permanent hit or miss. |
| `ordinary_candidate_loop` | 2/2 | Fresh ordinary-player backend loop through Bichon movement, combat, quest drop/pickup, delivery/reward, save, and new-session relog restoration. |
| Gateway fresh-account persistence | 1/1 | Normal account/login/character/start-game path, authoritative transform persistence, logout/save, and restore in a new Gateway session. |
| Ordered Zone restore regression | PASS | Restore is replayed in order and Zone bindings are refreshed without reintroducing the removed temporary entity. |
| `zone_rpc` library checkpoint group | 21/21 | Focused checkpoint/replay library coverage passed. |
| Clean-HEAD asset generation | 312/312 | Asset generation/checks from a clean source revision completed successfully. |
| Web typecheck | PASS | The Web typecheck gate remained green for this closeout. |
| `platinum_176_combat_milestones` | 15/15, repeated twice | The combat certificate is reproducible and preserves Crystal weapon-skill defence types through delayed packet damage. |

The accuracy correction fixes a real deterministic modulo-resonance defect.
The former tick coefficient was divisible by Scarecrow's agility modulus, so a
given attacker/target pair could remain permanently hit or permanently missed.
The same avalanche-mixed accuracy roll now serves personal-session and native
Zone physical attack paths. The vertical-slice assertions also use the current
authoritative quest item/reward identities and validate the full
`GroundDropClaimedWithTicket` authority surface.

## Exact release artifact

| Field | Recorded value |
| --- | --- |
| Candidate | `WN-CANDIDATE-03-20260826` |
| Repository-relative artifact | `dist/mir2-windows-candidate/WN-CANDIDATE-03-20260826` (ignored build output, not committed) |
| Source revision | `b5c0ecb604946a858bf5d060a2cca306032c0e62` |
| Source worktree at build | clean; source-status digest `924B56D335CCE786AEB34ED9C17512F006F1626212BC536D01723EF573ED4267`; 0 status lines |
| EXE SHA-256 | `9E51CBF3E81D50A182F08CE11D02D9829268881A2124BAFC1D963829CC634E8C` |
| EXE size | 66,665,472 bytes |
| Build completed | `2026-08-26T03:31:28.6100700+00:00` |
| Build attestation SHA-256 | `74F7D06336D486C6430263519282AED02C3B0429C6711FE0829DA7BE08311370` |
| Package manifest SHA-256 | `58F88AD84D1F7F9C9CC1CC44E59932D2A39136FD62FCA1F56CDAB0CF6C861884` |
| Manifest payload aggregate | `6788698E6ED19209D5463B10FF15E5D7972D714C62C6D0093808571C97ABF83A` |
| Manifest coverage | 10,254 payload files / 322,285,374 payload bytes |
| Complete package | 10,258 files / 325,281,417 bytes |
| `VERSION.json` SHA-256 | `037E88048C174AF1FC7BBBE6CA23698FB69EA42A9A3CD3A46B793817C570568C` |
| `RELEASE-STATEMENT.json` SHA-256 | `6770C8F5802423B4B1D290A9453FBFAD663891DE9325725733C382242331DC34` |
| Detached CMS SHA-256 | `B00B93D27D9E722D5C653377D25EA432076732CA1569AEA0B5E484F41D1D0F82` |
| Truth fields | `staged=true`, `builtByPackagingScript=false`, `accepted=false`, `visual=false` |

The four files excluded from the payload manifest are the manifest itself,
`VERSION.json`, the release statement, and its detached signature. Those files
are cross-bound by `VERSION.json` and `RELEASE-STATEMENT.json`; the exclusion is
explicit rather than an uncovered payload gap.

## Nonvisual package and verification evidence

No command in this round used `-Launch`, and no client executable was started.
The package driver and verifier ran from the clean packaged runtime source.
Their source-tree SHA-256 values were
`37B47EF2298201907922233F9A167002D4639E1ADB8DFC58F65C7A0BAFEA5AB0`
and `50461D0977DEAFB3C27800DDFA7704837B3B181E73C40588B12D187E5F1D42EF`.

| Gate | Result |
| --- | --- |
| Package-script PowerShell 5.1 AST | PASS |
| Verifier PowerShell 5.1 AST | PASS |
| Package self-test | PASS; `ADS_SELFTEST=passed`, `REPARSE_SELFTEST=passed` |
| Verifier self-test | PASS; `VERIFY_ADS_SELFTEST=passed` |
| Attested-build self-test | PASS |
| Supply-chain static test | PASS; 15 immutable actions |
| Publisher-contract self-test | PASS |
| Windows `npm.cmd` package invocation | PASS in dry-run and formal branches; prevents PowerShell from resolving `npm` as `pm` |
| Native keyed map generation | PASS; 4,650 emitted, 4,648 keyed, 2 additive, 50,595,486 image bytes; manifest `86478e87d93597432521f3b0345c07eb9bfe9ec489eacbc26774a9983fadfe7e` |
| Formal staging/final-directory verification | PASS; `sourceRepoCheck=checked`, `nonvisual=true`, `launchRequested=false`, 0 failures |
| Copied-artifact verification | PASS; `sourceRepoCheck=unavailable`, `nonvisual=true`, `launchRequested=false`, detached CMS valid, 0 failures |
| Six copied root anchors | 6/6 byte-identical by SHA-256 |

The copied-artifact run reports source checking as unavailable because its
package root is outside the clean verification repository. It still recomputed
all package hashes and verified the detached signature. The clean-package run
checked the exact source revision. The copied EXE, attestation, manifest,
version, statement, and signature hashes all match the clean verified package.

Durable generated evidence:

- `docs/generated/player-qa/windows-package-preflight/WN-CANDIDATE-03-20260826-package-summary.json`
- `docs/generated/player-qa/windows-package-preflight/WN-CANDIDATE-03-20260826-verification.json`
- `docs/generated/player-qa/windows-package-preflight/WN-CANDIDATE-03-20260826-verification-copied-artifact.json`

`WN-CANDIDATE-03-20260826` supersedes Candidate-02 as evidence for the current
runtime because Candidate-02 predates the deterministic accuracy correction.
Candidate-01/02 remain historical recovery records and are not overwritten.

## Signing boundary

`RELEASE-STATEMENT.p7s` is a valid detached CMS signature trusted by thumbprint
`B179E9D6222332C9DB5E960BAECF9990252CFBC7`. The certificate is self-issued as
`CN=Mir2 Internal Candidate 2026-08-26` and is valid from
`2026-08-25T20:45:13Z` through `2026-09-24T20:55:13Z`. This is an internal
Candidate signer only. The EXE's Authenticode status is `NotSigned`; neither an
official publisher certificate nor a formal release-signing ceremony is
claimed.

## Follow-on automated functional gate

After Candidate-03 was packaged, clean revision
`004549e9f15ca6fa4b7fad119cb305fcad7d3230` added a repeatable Windows
functional gate. This follow-on source revision is not represented as a new
packaged EXE in this report; Candidate-03's exact artifact identity above is
unchanged.

The gate ran from a clean worktree from `2026-08-26T06:12:37.0089754Z` through
`2026-08-26T06:24:09.2072237Z`. `SUMMARY.json` has SHA-256
`0590F2CEA720E69FA8755C34A0D22580A3F631647351BBF3C6F4DC136631753B`.

| Fixed automated control | Result | Duration |
| --- | ---: | ---: |
| Windows native host/client contract | 312/312 | 114.166 s including first compile |
| Five-class Bichon functional slice | 10/10 | 440.602 s |
| Ordinary unprivileged candidate loop | 2/2 | 3.377 s |
| Security lifecycle | 18/18 | 33.286 s |
| Shared Zone authority | 195/195 | 3.481 s |
| Gateway logout/reload | 1/1 | 88.898 s including first compile |
| Player Web typecheck | PASS | 8.364 s |

The functional slice adds normal five-class Crystal creation with source start
items and the original Wizard q10-q12 route across Bichon and `0115`, including
twenty real player-owned monster deaths, exact reward semantics, and logout /
new-session reload. The PowerShell 5.1 runner is fail-closed, requires a clean
revision, refuses evidence overwrite, hashes every log, and is wired into a
Windows Candidate CI lane plus a Linux/Windows aggregate job.

The summary reports `automatedFunctionalCoveragePercent=100` only for these
seven declared controls. It deliberately records `globalParityPercent=null`,
`accepted=false`, and `visualAccepted=false` and lists the human/release gates
below as exclusions.

## Clean-runner and Taoist follow-on

Source revision `23ac6012adfd4132896f01642b96ab210320065b` extends the
functional source gate; it is not a new packaged Candidate EXE and does not
change Candidate-03's attested artifact identity above.

Two hosted-runner defects were reproduced and closed. Windows PowerShell 5.1
had promoted Cargo's normal stderr progress to a terminating
`NativeCommandError`; native stderr is now captured while the real exit code
remains fail-closed. A clean checkout also lacked the ignored map-atlas
manifest, so the gate now builds the atlas from repository sources as its first
fixed control instead of depending on a local cache.

The expanded original journey adds Taoist q13-q15: Assistant Jane,
HighPriest Jude, ten real Oma plus ten real RakingCat player-owned deaths,
`OldLoafer`, the retained but not auto-learned `Healing` book, MirGuide Peter,
exact rewards, and logout/new-session reload.

The clean gate ran from `2026-08-26T07:14:04.1368581Z` through
`2026-08-26T07:24:45.0277271Z` and passed 8/8 controls. The functional suite
was 11/11 in 561.116 s; native was 312/312, ordinary 2/2, security 18/18,
shared Zone 195/195, Gateway 1/1, and Web typecheck passed. `SUMMARY.json`
SHA-256 is
`23593A5E4CC564DA9D38729ED4FEE36C6EC93C54D7EEB9629377BDCFACE8EE80`.

Its 100% is only the expanded eight-control automation set. The evidence still
records `globalParityPercent=null`, `accepted=false`, and
`visualAccepted=false`.

## Gateway ordinary-client follow-on

Source revision `f676a2a81f9fae949d6640df747dedf493d913e9` adds a second Gateway
journey; it remains source evidence and is not represented as a replacement
Candidate EXE. Candidate-03's attested executable identity is unchanged.

The journey uses only ordinary `ClientPacket` traffic from fresh account and
character creation through Bichon play. It walks to Village Guide, proves that
remote accept/finish packets fail without the nearby server-owned dialog,
accepts quest 1001 through that dialog, kills a real Field Wasp, receives the
Wasp Stinger proof, picks up visible gold from the drop tile, completes for
exactly 300 gold, two `RareCopperOre`, and `CopperRing`, then logs out and
reloads quest, inventory, equipment, gold, position, and direction through a
new Gateway session. No direct runtime action or QA/admin command is used.

The clean gate ran from `2026-08-26T07:32:31.5299048Z` through
`2026-08-26T07:43:42.0626754Z` and passed 8/8 controls. Map-atlas preparation
passed; native was 312/312, functional 11/11 in 563.256 s, ordinary 2/2,
security 18/18, shared Zone 195/195, Gateway 2/2 in 33.917 s, and Web typecheck
passed. `SUMMARY.json` SHA-256 is
`DE942CEB2D105AD039C3758FF00C7160880BC213BAA4FFC99A6AA826B118C0B6`.

The result is 100% only for the same eight declared automated controls. It
retains `globalParityPercent=null`, `accepted=false`, and
`visualAccepted=false`.

## Assassin q16-q18 follow-on

Source revision `82441f7b1257486d6f2b51206f5cffa4ef20f9b8` adds the original
Assassin q16-q18 instructor journey. It is follow-on source evidence, not a new
packaged Candidate EXE; Candidate-03's exact attested artifact remains
unchanged.

The journey enforces the level-four Assassin class gate, follows Assistant Jane
to HighAssassin Cloud and then MirGuide Peter on map `0`, credits ten real Oma
and ten real RakingCat player-owned deaths, and verifies the original
48/180/48 EXP plus 60/45/60 gold reward sequence. `OldLoafer` and
`FatalSword` remain inventory items, the skill book is not auto-learned, and
quest, inventory, equipment, class, experience, gold, position, direction, and
known-skill state survive logout and a new session.

The clean gate ran from `2026-08-26T08:04:06.0201767Z` through
`2026-08-26T08:17:03.3518161Z` and passed 8/8 controls. Map-atlas preparation
passed; native was 312/312, the expanded functional journey was 12/12 in
670.046 s, ordinary 2/2, security 18/18, shared Zone 195/195, Gateway 2/2, and
Web typecheck passed. `SUMMARY.json` SHA-256 is
`D1A9ACE4920A834541B5798BBE53F38DEE1D37DD261226DD91394C82AA8BC105`.

The 100% result remains limited to the eight declared automated controls. The
evidence still records `globalParityPercent=null`, `accepted=false`, and
`visualAccepted=false`.

## Archer q19-q21 follow-on

Source revision `d01910a1694d45e85dc54eafab6e61c43a063f5f` adds the original
Archer q19-q21 instructor journey. It is follow-on source evidence, not a new
packaged Candidate EXE; Candidate-03's exact attested artifact remains
unchanged.

The journey enforces the level-four Archer class gate, follows Assistant Jane
to Captain Jerald and then MirGuide Peter on map `0`, credits ten real Oma and
ten real RakingCat player-owned deaths, and verifies the original 48/180/48 EXP
plus 60/45/60 gold reward sequence. `OldLoafer` and `Focus` remain inventory
items, the skill book is not auto-learned, and quest, inventory, equipment,
class, experience, gold, position, direction, and known-skill state survive
logout and a new session.

The clean gate ran from `2026-08-26T08:36:39.4707209Z` through
`2026-08-26T08:51:19.8844889Z` and passed 8/8 controls. Map-atlas preparation
passed; native was 312/312, the expanded functional journey was 13/13 in
773.360 s, ordinary 2/2, security 18/18, shared Zone 195/195, Gateway 2/2, and
Web typecheck passed. `SUMMARY.json` SHA-256 is
`BE47F67645A9DF165635C173CF2F04BB85895B5DC6666F8F8DE3E963BC721197`.

The 100% result remains limited to the eight declared automated controls. The
evidence still records `globalParityPercent=null`, `accepted=false`, and
`visualAccepted=false`.

## Gateway original q1-q4 follow-on (2026-08-27)

Clean source revision `e1290bea3de1bdcd1663ee0f823c849c937eff3d`
completes the original Bichon q1-q4 journey through `GatewaySession` and ordinary
client packets. This is follow-on source evidence, not a rebuilt Candidate EXE;
Candidate-03's attested executable and package identity in this report are
unchanged.

The journey uses collision-aware bundled `0.map` movement and proves: q1 grants
and consumes five `CannibalLeaves`, awards 10 EXP and one small HP potion; q2
obtains `GingerTea` from real player-owned Scarecrow deaths, consumes it, and
awards 30 EXP, 200 gold, `GoldenPendant`, and `CopperRing`; q3 exposes the
original `SharpDagger`/`ToughHoaSword`/`StiffWoodenBow` selection and awards the
selected `SharpDagger` plus 10 EXP; q4 uses adjacent neutral-Deer melee and the
real multi-pass `ObjectHarvest`/`ObjectHarvested` path until five 1/2-roll
`DeerMeat` are collected, then consumes them and awards 80 EXP, 20 gold, and
`OldCopperRing`. Completed quests, inventory/equipment, gold, experience, and
authoritative transform survive logout and a newly constructed Gateway session.

A pre-acceptance formal rerun exposed a fifth-corpse failure in which the shared
Gateway action index retained an older incarnation's harvest tombstone. The
implementation now clears stale markers when a Zone-native monster is live or
enters a fresh live-to-dead transition, while a duplicate late death packet does
not reopen the current harvested corpse. Two focused unit regressions passed,
and the strict end-to-end journey completed without skipping or synthesizing a
corpse or quest item.

The clean final gate ran from `2026-08-26T21:16:58.2471471Z` through
`2026-08-26T21:45:46.6861359Z` and passed 8/8 controls:

- map-atlas preparation: PASS, 2,305 sources, 57 pages, content hash
  `732065c9e021a7939b2797dc26b283310eb625c5869972c3da27a072eab0e7a7`;
- native host: 312/312;
- five-class functional journey: 13/13 in 933.42 s;
- ordinary loop: 2/2;
- security lifecycle: 18/18;
- shared Zone: 196/196;
- Gateway persistence: 5/5 in 472.95 s;
- Web respawn-manifest/typecheck: PASS.

The evidence is at
`artifacts/windows-vertical-slice/gateway-q1-q4-20260827-e1290bea3-rerun2/SUMMARY.json`;
its SHA-256 is
`8C942979F9D59178C33BC72D5BAAD0F3986348B76F331EAF6B0C0DF003714849`.
It records `worktreeClean=true`, `automatedFunctionalCoveragePercent=100`,
`globalParityPercent=null`, `accepted=false`, and `visualAccepted=false`.
`MIR2_QA_NATURAL_MOVEMENT_DELAY_MS=10` is scoped only to the Gateway control and
shortens movement waits; combat, probability, harvesting, rewards, persistence,
and native-soak timing are not altered or inferred.

## Web runtime and CI reproducibility follow-on (2026-08-27)

The first PR check exposed an independent Developer Handoff failure: the
current/fallback R2 prefixes did not contain the tracked
`bevy-1813be587ef98bc1` package, and subsequent native runtime changes had not
refreshed the generated manifest. No production R2 publication was performed as
part of this Candidate work.

Current branch source was rebuilt twice on the same evidence host with
runtime-pinned Rust `1.95.0`, the `wasm32-unknown-unknown` target, and
wasm-bindgen `0.2.118`. Both runs generated `bevy-5046abca14947f40` with
manifest SHA-256
`4EC8644042F6926D7D724A7E7E500BA7DAFA1476B49780DF7EFAC7AEEC4806C1` and
identical file hashes. This is same-host repeatability, not cross-machine byte
reproducibility. WebGPU is 27,468,107 raw / 5,954,427 gzip; WebGL2 is
28,836,375 raw / 6,399,104 gzip. Both runtime budgets pass, runtime policy is
5/5, prebuilt downloader tests are 4/4, and the complete Player Web production
build passes 9,650 entity frames, 40,763 original assets, 57 map-atlas pages,
TypeScript, and 13/13 static pages.

Developer Handoff now prefers the SHA-verified prebuilt package but falls back
to the pinned current-source toolchain when the immutable object is unavailable.
The prebuilt path keeps the exact four-file manifest lock; the fallback path
requires its active manifest/file hashes, valid WASM, both size budgets, and the
complete Player Web build to pass, then restores the tracked manifest. This is
follow-on source/CI evidence and does not alter Candidate-03's packaged EXE or
close a deployed runtime, same-EXE UI/live-WSS, DPI, soak, human, or
publisher-certificate gate.

The first exact-head fallback attempt, GitHub Actions run `33020542728`, failed
before runtime compilation. Its wasm-bindgen install launched pinned Cargo but
allowed the child rustc to resolve through the runner's default stable rustup
proxy, which updated concurrently and produced component conflicts. The fixed
step now sets both `RUSTUP_TOOLCHAIN` and `RUSTC` before installation. The
repository developer image also locks Rust `1.95.0`, its wasm32 target, and
wasm-bindgen `0.2.118`; `dev.sh` and `dev.ps1` source-build the current runtime
when the immutable object is absent. Fault-injected Bash and PowerShell wrapper
contracts, real Compose parsing, the developer-release checker, and downloader
4/4 pass locally. Docker Desktop was not running on the evidence host, so the
next exact-head GitHub Linux image/Compose smoke remains a required CI result,
not an inferred pass.

Developer Environment run `33023066209` subsequently proved the Linux source
build but failed an over-strict cross-host hash assertion: Linux generated
`bevy-a314c804ae9919d3`, while this Windows Candidate tracks
`bevy-5046abca14947f40`. Exact-head run `33023784689` then compiled successfully
on Windows, Linux, Apple Silicon macOS, and Intel macOS, but Windows CI generated
host-local `bevy-efe7c0554bdf9a45`; its JS wrappers matched and only the two WASM
hashes differed. Inspection confirms release WASM contains absolute
Cargo-registry source paths, so the prior Windows cross-account zero-diff check
was not a valid source-equivalence gate. The corrected matrix validates each
generated active bundle and restores its host-local manifest on all four hosts
before the final clean-checkout assertion. The fetched immutable prebuilt
package remains strictly SHA-locked. This distinction does not change the
Candidate EXE, its hashes, or any open acceptance gate.

Exact-head Handoff run `33023777224` independently produced the same CI-host
`bevy-efe7c0554bdf9a45`, passed the current-source runtime build and full Player
Web build, and failed only the superseded tracked-manifest byte comparison. The
corrected Handoff uses that path as the active-bundle integrity/budget regression
case and restores the tracked manifest only after the source fallback passes.

## Not closed

The following gates remain open and are not inferred from the passed backend
tests or nonvisual package verification:

- Windows pure-UI execution from account creation through the quest flow using
  this same EXE;
- one continuous authenticated live WebSocket run using this exact final EXE;
- real Windows 125% and 150% OS-DPI behavior;
- a real 30-minute native-client soak with Gateway, client-log, crash-event,
  process-liveness, and memory evidence;
- original-client screenshot comparison and human visual/gameplay-feel
  acceptance;
- an official release certificate and formal Authenticode/release-signing path.

## Verdict

`Windows vertical slice = functionally exercised and artifact-verifiable
internal Candidate evidence; not global or strict Candidate 100%.` The exact
package is staged and cryptographically bound, but its truth fields remain
`accepted=false` and `visualAccepted=false`. The open same-EXE UI, live,
DPI, soak, human, and formal-certificate gates must be completed before an
Accepted claim is made.

## Shared-Zone tick recovery follow-on (2026-08-27)

Hosted q2 initially observed 121 aggregate `ObjectDied` packets but confirmed
zero player-owned kills. The failure was a dual-authority bug: after shared
Zone output was drained, the Gateway also advanced the full private
`SimulationSession` monster/hazard tick. Revision
`fb7cd29e8a0afdd09cd7f3f3592ed5fa1c6c5dff` now advances only personal
compatibility state in shared sessions. Zone owns public monster movement,
combat, death, drops, experience, and hazards; private movement retries and
intelligent-pet pickup from private ECS drops are suppressed.

The current personal spawn table still retains the Crystal respawn schedule and
emits a revive boundary reconciled into Zone state. A Zone-native real
wall-clock/checkpointed respawn scheduler remains an open architecture and
strict-parity item. The q4 Deer search grid was also narrowed from 36-tile to
16-tile spacing; a geometry regression proves all safe western Deer slots are
within the retained observation radius.

Final-source validation passed:

- Gateway unit: 653 passed, 0 failed, 1 ignored in 584.04 s;
- focused shared-authority regression: 1/1;
- bundled-map Deer geometry regression: 1/1;
- ordinary-packet Gateway q1-q4 journey: 1/1 in 455.10 s, with q2 complete
  after three confirmed player-owned kills and q4 complete after seven;
- Windows vertical-slice evidence self-test: 8/8.

`MIR2_QA_NATURAL_MOVEMENT_DELAY_MS=10` was the only QA timing override. No
damage, combat, drop, harvest, or reward multiplier was enabled. The attested
EXE recorded earlier in this report was built from
`4c7e60baa5d85e63858a6fd1717af01c5f893f3d`; it does not contain this fix and
requires an exact-head rebuild/package before promotion.

This follow-on remains 100% only for the declared automated controls. It leaves
`globalParityPercent=null`, `accepted=false`, and `visualAccepted=false` and
does not close same-EXE UI/live WSS, real 125%/150% DPI, a native 30-minute
soak, human original-client visual/gameplay-feel acceptance, the complete
semantic denominator, or formal publisher signing.
