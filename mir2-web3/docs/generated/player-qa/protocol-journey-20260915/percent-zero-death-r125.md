# R125: separate rounded low HP from authoritative death

Date: 2026-09-16. `accepted=false`, `visualAccepted=false`.
This fixes the automated public-protocol journey controller, not game balance
or the Windows render state.

Taoist R104 ended with `Timeout waiting for Revived`. Self `ObjectHealth`
percent-zero arrived at sequence 137353 while exact HP was still positive.
The controller sent `townRevive` at 137355 (16:17:02.663 UTC), but subsequent
snapshots showed HP1/dead=false. Actual `Death` arrived at 137367 about 2.2
seconds later; no new valid revival request followed. Four earlier self revival
cycles had succeeded, so substituting `ObjectRevived` was not justified.

R125 retains conservative `observedPlayerHp=0` for potions and emergency escape.
The shared `hasAuthoritativePlayerDeath` predicate uses self dead state or known
exact zero HP for revival, navigation/combat death checks and runner recovery.
Percent-zero alone cannot trigger revival. Root caught and rejected null HP
coercion in the initial predicate; missing or empty HP now provides no evidence.
The revival request still requires self `Revived` plus a later healthy snapshot.

- Focused: **181/181**, SHA256 `E6F06B37B6B3A5707F5684A6A19A7A9B1FC464DE76B4BF89FC3BBDB981748268`.
- Full controller: **647/647**, SHA256 `6B8770907E048B367B15D6A8EE42F69EBEA1E68CE6507A9BD9869C9F2A0D66E8`.
- Final logs and corrected read-only review are retained beside this file.

The normal R54 Taoist run `Taoist.2026-09-15T19-57-11-682Z` resumed without
saved-state edits. Request sequence578 followed an exact HP0/dead=true snapshot
at 577. Self `Revived` arrived at 613; the later snapshot661 showed map0,
HP180 and dead=false. See `percent-zero-death-r125-live-revival.json`.
This proves valid-death recovery at the saved checkpoint. The rounded-zero
dispatch regression is covered by deterministic tests, not a new forced live
near-death scenario. No additional mandatory quest is counted for revival.

Wizard R124 separately cleared two protected Shaman blockers through normal
combat. `q89-spawn-stall-r124-live-proof.json` retains exact same-map HP0/dead
snapshots preceding both clearance events and subsequent `ObjectDied` receipts.
Both remaining routes continue; completed units remain **155/177** at this
observation. Full route, native UI, animation and original paired gates are open.
