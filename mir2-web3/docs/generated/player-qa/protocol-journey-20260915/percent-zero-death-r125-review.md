# Percent-zero death / revive R125 final review

Read-only review of the six changed quest-agent files and replacement focused/full logs. No source, live, store, process, or repository-document writes.

The earlier review found a real R125 defect: `finiteNumber(null)` coerced null to zero, so `hasAuthoritativePlayerDeath` could classify a living self with missing exact HP as dead. That root finding is recorded here as corrected, not silently discarded.

The corrected predicate now uses `knownExactHp` and treats `null`, `undefined`, and empty-string HP as unavailable evidence. It admits death only from self `dead:true`, numeric exact `snapshot.playerHp <= 0`, or numeric exact self `hp <= 0`. Rounded `playerHealthPercent` remains excluded from lifecycle decisions.

The six-file review confirms:

- `protocol-observation.mjs` preserves `observedPlayerHp` as the conservative low-health control while adding the null-safe authoritative death predicate.
- `run-protocol-journey.mjs`, `protocol-play.mjs`, and `protocol-combat.mjs` use the predicate for revive dispatch, navigation death checks, quest recovery, and combat alive checks. Cadence and wait guards therefore ignore percent-zero as death without relaxing the existing low-HP escape/recovery behavior.
- `protocol-play.mjs` requires a self `Revived` response through `client.request({type:'townRevive'}, 'Revived')`, then requires a later healthy world snapshot. The corrected mock explicitly asserts the expected response argument is exactly `Revived` and that the healthy snapshot sequence follows it.
- New regressions cover numeric exact zero, null exact HP, exact positive HP plus rounded percent-zero, unrelated `ObjectRevived`, self `Revived`, and the fresh healthy snapshot requirement. The source guard also rejects undefined and empty-string HP; separate assertions for those two values are not claimed.

Replacement retained evidence:

- `percent-zero-death-r125-focused.log`: **181/181 passed**, SHA-256 `E6F06B37B6B3A5707F5684A6A19A7A9B1FC464DE76B4BF89FC3BBDB981748268`.
- `percent-zero-death-r125-full.log`: **647/647 passed**, SHA-256 `6B8770907E048B367B15D6A8EE42F69EBEA1E68CE6507A9BD9869C9F2A0D66E8`.

Review result: **R125 corrected source and retained-log checks pass.** The Taoist failure is addressed at the confirmed control boundary: rounded percent-zero cannot trigger revival, missing exact HP is not death evidence, and only authoritative self death can start the town-revive lifecycle.
