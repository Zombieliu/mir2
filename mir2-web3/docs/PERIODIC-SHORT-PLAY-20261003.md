# Periodic short-play rebalance, 2026-10-03

The user requested every hunt target reduced to one tenth and selected
approximately five levels for the complete three-daily set, ten levels for
the complete two-weekly set. This supersedes the original workload/reward
brief. The original ten-hour goal remains historical and is not resumed or
declared complete by this change.

## Resulting rules

- All 20 quests and 26 targets use `ceil(previous_count / 10)`; daily totals
  by band are 20, 19, 15 and 14 kills. Species/maps/navigation are unchanged.
- The server sums actual consecutive Crystal level thresholds from acceptance
  level, using five thresholds for dailies or ten for weeklies. It divides
  the budget across three/two slots using integer prefix shares. Gold is
  unchanged. The complete set at one acceptance level without EXP bonuses
  awards exactly the target level span; monster EXP is extra.
- Each quest still stores its accepted level, absolute u32 EXP and gold.
  EXP buffs apply once at acceptance, retaining existing u32 saturation;
  later levels/buffs do not recalculate the lock or multiply Finish again.
  Level 50's weekly budget is 7,160,000,000; its 3,580,000,000 shares each fit
  the protocol. The full budget remains u64 until after division.
- New acceptance still ends at level 50. Accepted tasks can finish above
  their original band. Relogin above the last authored profile level restores
  the same Crystal threshold used by LevelUp, fixing an observed level-55
  regression (350,000,000 restored instead of 740,000,000).
- Active legacy targets are reconciled downward while retaining raw species
  progress and old locked rewards. Reset Completed rows display current copy
  and refresh their count/descriptions on reacceptance. UTC+8 periods,
  cross-city/cross-band slot claim fences and full-save-before-ACK remain.
- All nine native languages retain reset/acceptance instructions and use the
  canonical new counts in descriptions, summaries, target labels and aliases.

## Focused verification

Ready-count fixtures below are declared mechanical tests. They do not prove
natural kills, travel duration, supplies or frontend human acceptance.

| Check | Result / boundary |
| --- | --- |
| Canonical game-data catalog | 4 passed; every band, cadence, imported species/map and invalid spans |
| Simulation periodic rules/math | 12 passed; all acceptance levels 10–50, rounding, max shares, old reset/reaccept, period and slot fences |
| Adjacent existing recurrence suite | 9 passed; reset/abandon, old claims, clock rollback and repeatable tasks |
| Packet/persistence integration with `test-support` | 21 passed; 1 isolated PostgreSQL test intentionally not rerun |
| Complete-cadence packet cases within that suite | 24 cases: 3 classes × levels 14/24/34/50 × daily/weekly; 60 ordinary Finish claims; exact +5/+10, residual 123, duplicate rejection and full checkpoint relogin equality |
| File store within that suite | Fresh locked daily/weekly multilevel Finish agrees with committed disk state and reopen; existing fault/CAS checks retained |
| Native periodic UI/localization | 11 passed; 1 explicit offline GPU visual fixture not rerun |
| Node periodic controller policy | 29 passed; server objective/reward identity and rollover proofs |
| Mechanical translation review | 26 target checks, 360 description aliases, 234 localized target labels; 86/210 entries change only counts in 9 languages |

Build/test logs and first failed attempts are retained in
`C:/mir2-periodic-short-play-20261003`. Initial failures identified stale test
values and the genuine above-profile relogin threshold defect. Two retries
encountered a locked Windows test binary; an isolated test target and fresh
output name were used without changing application-control settings.

No account or production database was modified by these tests. The previous
isolated PostgreSQL proof remains historical; no persistence schema or CAS
protocol is changed here. Code review independently checked allocation,
legacy descriptions/locks and the narrowly guarded restore fallback.

The signed paired client/gateway update must be built from the committed
revision before promotion. This source checkpoint does not claim deployment.
User preferences and human saves must be preserved at the eventual switch.

The canonical current design is [daily/weekly commissions](DAILY-WEEKLY-QUEST-DESIGN.md).
