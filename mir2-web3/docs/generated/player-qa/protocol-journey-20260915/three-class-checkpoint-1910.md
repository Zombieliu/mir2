# Checkpoint validation — 2026-09-15 19:10

Read-only validation of `gateway/accounts.json` (last write observed 19:06:07), the three class private metadata files, newcomer quest configuration, and the native soak report. Passwords, hashes, tokens, and raw auth material are intentionally omitted.

## Account mapping and quest progress

The private metadata resolves to these normal accounts:

| Class | accountId | name | level | mandatory route | milestone claims | revision |
|---|---|---|---:|---:|---:|---:|
| Warrior | `jpa3693f66` | `JWa3693f66` | 30 | 55/55 | 4/4 | 7434 |
| Wizard | `jpdebac6d4` | `JWdebac6d4` | 25 | 45/55 | 3/4 | 7120 |
| Taoist | `jp3f4a4e1f` | `JT3f4a4e1f` | 23 | 39/55 | 2/4 | 8290 |

The 55-ID denominator is the `guidance.category == recommended` set from each class’s `*-1-30-newcomer-v1.json`; the files contain 116 total quest definitions, which are not used as the mandatory denominator.

Milestone quest IDs checked were `2100015`, `2100020`, `2100025`, and `2100030`. Explicit current statuses are Warrior: `completed, completed, completed, completed` (4/4); Wizard: `completed, completed, completed, absent` (3/4); Taoist: `completed, completed, absent, absent` (2/4). The class-specific level limits explain absent higher milestones at the Wizard level 25 and Taoist level 23 checkpoints.

Quest checkpoint records:

- Warrior: q89 completed 9/9; q99 completed 20/20.
- Wizard: q89 in progress 6/9; q99 completed 6/6.
- Taoist: q89 absent; q99 in progress 3/6.

## Save-state values

| Class | map | position/direction | HP/max | MP/max | gold | inventory slots | belt slots | equipment slots | skills | quest records |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Warrior | 0 | 331,272 / DownRight | 419/419 | 116/116 | 135652 | 4 | 2 | 9 | 2 | 60 |
| Wizard | 0 | 277,538 / Up | 100/100 | 244/398 | 42016 | 17 | 4 | 7 | 2 | 50 |
| Taoist | 0 | 275,210 / DownRight | 159/159 | 85/158 | 1598 | 10 | 3 | 8 | 2 | 43 |

The requested Wizard checkpoint matches exactly: level 25, 45/55 mandatory, 3/4 milestones, q89 6/9, revision 7120, map 0, HP 100, MP 244, gold 42016.

The requested Taoist checkpoint matches level 23, 39/55 mandatory, 2/4 milestones, q99 3/6, revision 8290, map 0, HP 159, MP 85, gold 1598, MP-drug quantity 3, and SoulFireBall XP 193. The initial belt-only reading showed `Amulet` quantity 76, but the equipped item array contains another 24 of the same item index 712; aggregate Taoist Amulet quantity is therefore 100 (24 equipped + 76 belt), matching the expected checkpoint. No requested Taoist item/vital value disagrees after aggregating equipment and belt/inventory.

## Native soak boundary

Source: `C:\mir2-native-run-acceptance-20260915\soak-r20-r54\observer\soak-30m.json`.

- Target identity verified as `mir2-platform-windows.exe`; 30.005 minutes actual duration, 180 runtime and 180 FX samples, parse errors 0, malformed tagged lines 0.
- Runtime/FX timestamps strictly increasing and aligned; maximum gap 10010 ms versus 30000 ms allowed.
- Maximum active effects 54 with cap 96; maximum retained total 731; maximum additive cache entries 134; both observed-decrease flags false; post-warmup retained growth false.
- Windows event queries found 0 relevant events.
- RSS warmup completed; final working set 912,576,512 bytes, limit 1,138,810,880 bytes, within warmup 125%; post-warmup delta 1,527,808 bytes and slope 76,882.51 bytes/minute.
- Health checks remained HTTP 200/OK; process samples remained alive/responding for all recorded samples.

The formal gate is `FAIL` with the sole reason `successful-reconnect-not-observed`. These stability metrics do not establish a formal soak PASS or a held-right claim. No AutoRunD execution is inferred from this evidence.
