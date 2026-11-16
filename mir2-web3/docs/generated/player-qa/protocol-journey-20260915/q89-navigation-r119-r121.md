# q89 navigation and retreat recovery, R119–R121

These are public-protocol controller repairs, not completed native or Crystal visual acceptance.

- R119 permits only a live, revalidated D2031 → D2032 movement door when the expedition profile lacks that map edge. Ordinary route selection and static collision remain authoritative. Its live attempt escaped to town before using the door, then stopped at the strict origin-map check.
- R120 returns to D2031 by ordinary travel before validating the exact fallback door. Its nine-minute live attempt ended after a successful emergency escape D2031 → map 0; q89 stayed 2/3 Priest, 3/3 Shi and 2/3 CursedZombie. The kiting wrapper incorrectly treated that safe map change as fatal.
- R121 abandons the stale ranged action after a confirmed, alive authoritative map change, records `retreatMapChanged`, and replans. Missing player/map, death, and unrelated navigation exceptions still fail. The natural Wizard save resumed on unchanged Gateway R54; completion is pending.

Root-reviewed regressions:

| Revision | Passed | Failed / cancelled / skipped | SHA-256 of raw log |
|---|---:|---|---|
| R119 | 625 | 0 / 0 / 0 | `5807997AF7581AE18B30D34E8E3138B4E9C7F5C4515FD7656551905581290D54` |
| R120 | 626 | 0 / 0 / 0 | `FEF0DA901F403A9A3E301881CBB3D77EC395BE017BC6631B23B756E38D7D7C3B` |
| R121 | 630 | 0 / 0 / 0 | `58AAA799BA4997E264D66A6697F3F18309AA94B0A1D1F9F355C86BA6C72F4C36` |

Raw logs: [R119](quest-agent-tests-r119-root.log), [R120](quest-agent-tests-r120.log), [R121](quest-agent-tests-r121.log).

Latest independently observed natural checkpoint (2026-09-15 14:23 UTC): Warrior Lv30, 55/55 mandatory plus 4/4 milestones; Wizard and Taoist Lv25, each 45/55 plus 3/4. Total 155/177 units (87.57%), with 20 mandatory tasks and two milestone claims remaining. Partial kill counts are not completed units. Taoist q89 is 0/3 + 3/3 + 0/3. Wizard q89 is 2/3 + 3/3 + 2/3.

The 375-minute route budget is a design target, not measured full 0→30 playtime. `accepted=false`, `visualAccepted=false`, `formalCandidate=false`, `globalParityPercent=null`.
