# Frozen first same-map EXPOwner implementation evidence

Implementation baseline: 3e1f9167097ff239faeffdde809a17b2c0009b6c. R17 gameplay/failure comparison: 6032ef8b3e27dd97bad0b20c8676ef9f185db64b.
Twelve authorized files are frozen. Root may integrate unrelated map work; this patch excludes it.
No commit, release build, upload, live-store access, deployment, or F-drive installation occurred.

The complete patch uses ordinary repository-root paths (mir2-web3/...). source-manifest.json commits
on-disk bytes for the exact write set; run-receipts.json has explicit normalized replay argv,
observed per-target counts, filtered/ignored counts, original log paths, hashes and retained failures.
These are repeatable commands, not claims that the original PowerShell transcript was logged.

## Results

Corrected semantic RED: 16 tests, 6 passed / 10 failed. Environmental AI98 RED: 1 failed.
Final public ownership GREEN: 21/21. Final private rules: 6/6. Checkpoint adjacent: 15/15.
Public fixture assertions include actual HP/damage, actor identity, EXP recipient, quest target
and ground item/owner identity. This does not prove authenticated TCP/WSS or human/native acceptance.
One adjacent Hell purification test fails identically at line524 in unchanged R17 and current.
It is retained as an independent unresolved fixture/eligibility issue, not relabeled as all green.

| Retained log | Passed | Failed |
| --- | ---: | ---: |
| shared-monster-ownership-red-01-20261004.log | 5 | 11 |
| shared-monster-ownership-red-02-20261004.log | 6 | 10 |
| shared-monster-ownership-environment-red-03-20261004.log | 0 | 1 |
| shared-monster-ownership-master-baseline-04-20261004.log | 1 | 0 |
| shared-monster-ownership-green-01-20261004.log | 21 | 0 |
| shared-monster-ownership-private-green-02-20261004.log | 5 | 0 |
| shared-monster-ownership-checkpoint-adjacent-03-20261004.log | 15 | 0 |
| shared-monster-ownership-adjacent-summon-ai-04-20261004.log | 20 | 1 |
| shared-monster-ownership-adjacent-summon-ai-05-20261004.log | 92 | 0 |
| shared-monster-ownership-hell-r17-baseline-06-20261004.log | 0 | 1 |
| shared-monster-ownership-current-return-07-20261004.log | 6 | 0 |
| shared-monster-ownership-final-green-08-20261004.log | 21 | 0 |
| shared-monster-ownership-lib-group_experience_tests-20261004.log | 5 | 0 |
| shared-monster-ownership-lib-summon_damage_tests-20261004.log | 1 | 0 |
| shared-monster-ownership-lib-soulfire_practice_tests-20261004.log | 9 | 0 |
| shared-monster-ownership-lib-entity_combat-tests-20261004.log | 25 | 0 |
| shared-monster-ownership-lib-reward_rate_tests-20261004.log | 2 | 0 |
| shared-monster-ownership-shared-zone-poison-20261004.log | 8 | 0 |
| shared-monster-ownership-shared-zone-summon-20261004.log | 9 | 0 |
| shared-monster-ownership-gateway-kill_award-20261004.log | 3 | 0 |
| shared-monster-ownership-gateway-world_checkpoint-20261004.log | 4 | 0 |
| shared-monster-ownership-gateway-shared_drop_aoi_tests-20261004.log | 2 | 0 |
| shared-monster-ownership-gateway-guild_kill_source_tests-20261004.log | 2 | 0 |

The current-return-07 public target executed zero tests (21 filtered). Only final-green-08
counts as the final 21-test public result. The stopped adjacent-04 run left eleven targets unexecuted;
adjacent-05 executes exactly those eleven (92/92). No ignored tests occur in these scopes.

## Remaining source-parity and acceptance gates

- Cross-map online spawned identity, teleport vs logout/despawn, global revocation and routing remain manager work.
- Same-map fresh Join revocation is an identity safety boundary, not full Crystal global Node parity.
- Ground Owner remains existing 18000ms, not original 60000ms; no duration changes this round.
- Existing same-zone equal group formula and generic Boss contribution policy are preserved only for scope.
  No explicit human acceptance of the Boss difference or group source parity is implied.
- AI6/58/113 clearing and zero periodic-poison exceptions are private trusted rules; selection and zero-poison
  full-chain behavior are not implemented/accepted. Normal positive poison producers and tick gates are unchanged.
- PK LastHitter, pet PvP and remaining species, ordinary two-account TCP/WSS and native frontend are separate gates.
- User daily/weekly EXP remains unchanged.
- Zone schema/root v5 authenticates owner/deadline/forced impact. Legacy v1-v4 ignores forward fields.
  A future rollback to old R17 must restore a matching pre-upgrade world checkpoint; old binary rejects v5.

## Located dead-level-up risk; not an end-to-end pass

Crystal HumanObject.cs:845-865 refills HP/MP on LevelUp but SetHP does not clear existing Dead.
Current apps/simulation/src/runtime/leveling.rs:218 fills HP, Gateway routing.rs:10945-10952
synchronizes vitals on LevelChanged, and zone/runtime.rs:1394-1404 derives Dead solely from HP0.
A dead solo poison owner's new Zone award could therefore trigger unintentional resurrection and status clearing.
The read-only personal/durable/Gateway consumption trace has no subsequent alive rejection, but outbound alone
does not prove actual XP delivery, persistence or safe death state. This round deliberately leaves that code
untouched. Root will design/run a separate complete authoritative dead-XP/level-up Gate before expanding it.

The repository scoped report is docs/CLASSIC-EXPERIENCE-OWNERSHIP-20261004.md. Source and evidence are frozen;
no next-round implementation is started by this worker.
