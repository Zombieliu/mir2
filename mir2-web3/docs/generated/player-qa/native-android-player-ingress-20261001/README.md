# Android shared player stats and wallet ingress — 2026-10-01

Bounded NI-02/08 source checkpoint. The Windows-completeness goal remains Active.
Frozen Windows source: `3f5e61533235921369bc13a7760b4a56b0e467e5`.
The previous v11 APK does **not** contain this implementation. Diagnostic version
12 / `0.1.9-player-ingress` is set, but this source checkpoint has not built or
accepted that APK. Exact clean-source package/emulator evidence follows separately.

## Shared implementation and authority

- `client-bevy::native_player_ingress` is the mechanically extracted Windows
  `NativeUiPlayerCursor`, `WalletState`, five cursor methods and nine helper
  bodies. Windows imports the same implementation. The extraction audit compares
  every field/method/helper against the frozen Git blob, allowing only visibility,
  namespace and whitespace/trailing-comma differences. No Windows branch is pushed.
- Android retains strict world/self/map/position and shared scalar validation,
  then uses that cursor for HP/MP, Crystal stats, XP, exact u32 bag/wear/hand weights,
  authoritative-known-zero weight, class/gender/hair/wing and labels. Missing exact
  weight blocks clear to unknown; other missing partial personal values survive.
  This does not claim all buffs or every Windows personal packet is connected.
- The Android owner adapter is bounded to1MiB snapshots,16KiB packets and64KiB
  typed UI models. A validated numeric-u32 self owner and character name bind it;
  another owner/character or a terminal reset clears its previous personal state.
  Malformed typed values do not partially replace its last model.
- Java forwards only the four public Gained/Lose Gold/Credit packets after a
  validated owner bootstrap, including same-character map loading. UserInformation
  is checked for the pinned owner, non-Hero and same character **before** Java
  mutates or publishes a position. IDs accept only exact Integer/Long JSON tokens,
  including the full u32 range; string/null/fractional tokens cannot round to owner.
  The old pre-snapshot coordinate bootstrap does not authorize personal packets.
- Wallet deltas use the unchanged Windows received-event helper. A missing
  authoritative base is an error, not invented zero. UI plus all known absolute
  wallet fields are retried as the latest pair under backpressure; a retry does
  not apply a delta twice, infer a purchase success or acknowledge item custody.
  Runtime order remains UI, inventory, then wallet patch.
- Scene-only reset keeps this character's personal state but clears map/safe-zone
  labels. Terminal decode/overflow/render rejection now revokes host phase/world,
  pending world/render requests and deferred load, resets all three personal
  domains, cancels the atlas generation and clears live entity/presentation state.
  Entity handling also requires StartingGame/InGame. Same-batch stale world Views
  cannot reopen a disconnected host; legitimate render-ready waiting is retained.
- Feature-only BAG/CHAR specimens traverse the real world validator and shared
  player/inventory adapters. Explicit OFFLINE values exercise HP80/200, MP20/100,
  gold12352, credit503, XP12.5%, weights66/25/9 and29 Crystal stats. These are
  diagnostic received-packet values, not an authenticated account or a game result.

## Measured source gates

| Gate | Current result |
| --- | --- |
| Android Rust full | 257 passed,0 failed/ignored |
| Actual ui-preview feature full | 270 passed,0 failed/ignored |
| Shared native-player-ui | 1203 passed,0 failed,8 existing visual ignores |
| Java Debug / uiPreview | 36 /36 passed,5 XML suites each,0 failures/errors/skips |
| Windows cursor/weight/wallet/appearance filters on Mac | 2+1+2+1 passed; not a Windows device/full-suite gate |
| Real arm64/API31 normal and ui-preview target checks | Both pass; preview feature actually enabled |
| Frozen extraction equivalence / diff whitespace | Pass |

`source-results.mjs` records totals and retained XML; `source-equivalence.json`
records the exact extraction boundary. A read-only final source review found no
remaining P0/P1/P2 blocker in this leaf; it did not run tests or accept an APK.
The same-batch Rust host test checks terminal state on Mac, not the Android-only
entity renderer. Target checks and static review do not substitute for a live
Android entity/render or authenticated player-flow acceptance.

## Failure-first evidence retained

- `before-rust.log`: authoritative weight0 was marked unknown and exact weights
  were dropped by the old projection.
- `before-java.log`: public GainedGold was not forwarded after owner bootstrap.
- `before-owner-java.log`: foreign UserInformation published a changed world
  before Rust could reject it. `after-owner-java.log` is the first limited fix;
  it is **not** the final exact-number guard result.
- `before-owner-number-java.log` is an intermediate run with a42.0 control that
  the test serializer could normalize to the valid integer42; it is not proof of
  the tiny-fraction bug. That control was removed. The subsequent
  `before-owner-tiny-fraction-java.log` and XML identify the actual wire value
  `42.000000000000000001` publishing an invalid world. Final full Java suites
  include the unchanged tiny-fraction negative and full-u32 positive controls.
- `before-terminal-batch-rust.log`: personal rejection left host phase IN_GAME.
  The final regression exercises both the direct stale ObjectWalk and a stale
  IN_GAME View followed by ObjectWalk, with no requests or player-model replay.
- `source-equivalence-first-error.log` retains an audit-normalization error for
  rustfmt signature trailing commas; only normalization was corrected. Source
  body equivalence then passed. Earlier257/270 full gates supersede intermediate
  normal256/preview269 logs; none of these failures is silently labelled green.

## Remaining acceptance

Exact v12 APK/version/hash/resource bytes/installation and actual stat-page
rendering are still unverified at this source checkpoint. Full phone controls,
48dp targets, dragging, compact/IME layouts, launch performance, services, incoming
chat, quests, mail/storage/social/Hero models, complete resources/audio/resume,
approved real HTTPS/WSS player flow and physical-device acceptance remain open.
Only the emulator is connected; no test environment/account has been approved.
No real credentials, saves, production deployment or APK/licensed image/key Git
objects are part of this checkpoint.
