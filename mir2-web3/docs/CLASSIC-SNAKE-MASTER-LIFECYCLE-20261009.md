# Snake Totem immediate-master Candidate — 2026-10-09

Implemented and locally tested; exact-source Linux CI, Gateway publication,
ordinary public/native gameplay and human acceptance are separate pending gates.
This fixes the shared owned-monster path used by the original Archer
SummonSnakes skill. It does not certify all Taoist pets or Human PK.

## Behavior and source rules

The real SnakeTotem child previously had its immediate Master set to the Totem,
but attack validation accepted only a direct Human Master. Its attack therefore
never acquired valid authority. The new optional Monster Master reference binds
the current parent incarnation, child incarnation and causal Human online/life
identity. Direct-Human saved pet records retain their existing serialization.
Acquisition, launch and delayed impact independently recheck these bindings.

Children use their immediate Totem's All/Both targeting relationship and SlaveList.
They are not added to the Human's direct Pets collection, focus/recall behavior
or the Human's peer-pet threats. Following uses the Totem's Back position.
Logout, cold restore, parent/child reuse or parent death revoke the child chain
and pending impacts; raw object IDs cannot rebind authority.

Original SnakeTotem sets a preliminary birth action time of +1000ms, but its
normal base Spawn overwrites it to +2000ms. The original readiness comparison
is strict `>`: eligibility starts at birth+2001ms. The implementation now follows
that actual lifecycle, preserves the 300ms strike impact, consumes it once,
and applies paralysis only after a current lawful positive hit. No production
action, damage, spell or movement clock is shortened.

Original CharmedSnake.Die calls base.Die first, which clears its Master. The
corpse explosion therefore uses a current, unowned Monster relationship; an
ordinary wild target is ineligible without the original Hallucination/Rage
relationship. The current AI63 incarnation and actual HP0/dead state are required.
Former causal-Human permission never authorizes the corpse burst.

Experience and drop ownership use the immediate Monster Master. Its WinExp
does not award the causal Human. Child damage cannot mint or renew Human XP,
gold or custody; an earlier living Human first hitter retains its lawful claim,
and a dead first claimant is cleared under the source rule.

The source references are original Crystal `SnakeTotem.cs`, `CharmedSnake.cs`,
`MonsterObject.cs` and `MapObject.cs`. Their complete inspected bytes are retained
in the evidence archive alongside the actual changed Rust source.

## Actual local verification

| Gate | Result and scope |
| --- | --- |
| Final private regression, run31 | 64 pass: 11 immediate-master cases, 47 original owned-pet cases and 6 experience-owner cases. |
| Ordinary Zone API, run32 | 5 pass without a test-support feature. Real normal summon, delayed strike/paralysis and accepted Wizard ThunderBolt parent death are exercised in process. No public WebSocket or rendered-native claim. |
| Existing shared pet PK API, run33 | 38 pass with its original test-support feature. This is bounded regression, not natural Human PK acceptance. |
| Full original shared_zone, run34 | 200 pass / 9 fail. The two original Snake attack/paralysis failures are fixed without changing shared_zone.rs. The remaining nine original-rule/fixture failures stay RED and are enumerated in the baseline audit. |
| Current checkpoint test | Corrected the old literal version6 to the actual unchanged version7 and added a v7 save/restore roundtrip requiring revoked XP/forced-impact authority and the unchanged pending deadline. Earlier run20 failure is retained. |
| Formatting | Existing broad formatting differences remain RED in run29 and the experience file in run30; only new changed lines were formatted. No broad unrelated source formatting was performed. |

Each final test receipt verifies the same current source hashes before and after
execution. The original shared_zone fixture and checkpoint implementation remain
byte-identical to parent faef. The new Linux release step runs the immediate
Master API/private cases and the original owned-pet/experience regression while
retaining all earlier security, movement, classic Source/PostgreSQL and siege gates.
That workflow has not yet been dispatched for this uncommitted Candidate.

Earlier compile errors, wrong projected-death fixture assumptions, actual delayed
parent-death burst leaks and the stale checkpoint assertion remain in the raw
archive. They are not counted as passing. Final accepted parent death and the
private real HP0/cached corpse tests are separate from the filtered fake packet.

## Evidence and remaining work

[Complete original outputs, runners, inspected Crystal source, current Rust and
baseline logs](generated/player-qa/snake-master-20261009/candidate-01/original-evidence.zip)
is 782337 bytes, SHA256
`bb395eced0270531b236dcc8d74179b9542d4c4b2a582d6b8d3d49c239735850`.
[Entry hashes and actual final receipts](generated/player-qa/snake-master-20261009/candidate-01/EVIDENCE.json)
bind the dirty tested source to parent faef; no future commit ID is invented.

P4 still requires broad natural Human PK, nested-pet XP/drop and other lifetime
acceptance, plus the source NPC world-action path described in
[the P7 audit](CLASSIC-P7-SOURCE-WORLD-AUDIT-20261009.md).
The remaining nine original Zone stories need separate source-correct fixtures
and all later assertions must run; first-failure explanations are not passes.
The original twelve-hour deadline remains missed and its heartbeat paused.
Neither the broader Goal nor P1–P7/Mentor is marked complete.
