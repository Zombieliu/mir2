# Android shared player stats and wallet ingress — 2026-10-01

Bounded NI-02/08 source/package/emulator checkpoint. The goal remains Active.
Frozen Windows source: `3f5e61533235921369bc13a7760b4a56b0e467e5`.
Clean APK source: `f04a7439938579480b592b5d4e7381014a612dff`.
The previous v11 APK does **not** contain this implementation. Actual version12 /
`0.1.9-player-ingress` packages and offline stat values are now verified below,
but missing weight-bar imagery, startup and full/live/device acceptance still fail
or remain open. The source gates below were recorded before that clean build.

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

Exact v12 source/package/installation and bounded numeric pages are verified
below; they do not close the missing-weight-bar or whole-interface gate. Full controls,
48dp targets, dragging, compact/IME layouts, launch performance, services, incoming
chat, quests, mail/storage/social/Hero models, complete resources/audio/resume,
approved real HTTPS/WSS player flow and physical-device acceptance remain open.
Only the emulator is connected; no test environment/account has been approved.
No real credentials, saves, production deployment or APK/licensed image/key Git
objects are part of this checkpoint.

## Exact v12 APK and actual emulator follow-up

Both builds have empty Git status before/after and the same full source ID above.
They are Gradle Debug/uiPreview with an optimized Rust release library, **not** a
store Release or production-signing acceptance. MinAPI31/target35/arm64, normal
`UI_PREVIEW=false` and separate `com.mir2.web3.uipreview` true are verified in the
actual APK/config and installed package. Both have `MIR2_GATEWAY_URL=""`.

Local artifact root: `apps/game-client/platform-android/target/player-ingress-v12-20261001/final-apks`.

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| `mir2-native-player-ingress-debug-v12.apk` | 386297658 | `177e51d0b2381002cc215f39954e5c377ef02850dfaeee28f7d9a24f6e42848d` |
| `mir2-native-player-ingress-preview-v12.apk` | 390019866 | `e7e8cbd831af05bf2786c0e046262ef23dc419bfa43b62ff692c4f5bdcd76862` |

`package-v12.json`/`verify-package.mjs` check6195 original item/equipment PNGs,
448 magic-icon PNGs and both Items/StateItem metadata byte-for-byte against the
unchanged approved v9 diagnostic input. UI/world/entity inputs are unchanged;
this bounded6643-frame check is not a complete resource-release claim.

Actual test surface: only `emulator-5554`, `sdk_gphone64_arm64`, Android12/API31,
physical1080x2340 landscape captures2340x1080, density440. Original fingerprint
is retained in `device-fingerprint.txt`; no physical phone is connected.

- `preview-character-stats1.png` visibly contains HP80/200, MP20/100, AC2-7,
  AMC1-4, DC12-24 and the remaining shared diagnostic stat rows. StatsII shows
  XP12.5%, bag66/100, wear25/50 and hand9/25 in its original row order.
- `preview-inventory.png` visibly shows gold12,352, matching phone HUD12352,
  and available weight34. Both closes respond and restore the normal phone HUD;
  side status/chat/six belt targets and both thumb lanes are unobscured. These
  are offline received-packet projection specimens, not account/custody receipts.
- New gender/hair data makes the original male paper doll and source weapon
  visible. The armour specimen has no StateImage: no full equipped appearance
  or all-classes/skins acceptance. Credit503 is confirmed by the queued typed
  marker, **not** by a rendered credit control in this capture set.
- Actual preview logs expose `original-ui/UI_32bit/473.png` (HUD weight bar) and
  `/471.png` (BAG weight bar) missing. All four frozen Windows frames470..473
  are absent from both APKs; numeric rows do not hide this image failure.
  The source Git contains these four originals and sparse metadata (extent474,
  exported4). Next repair stages/guards the originals, not a recolored substitute.
- Normal startup timed out12812ms; character timed out14099ms; BAG was OK4798ms.
  A Pixel Launcher ANR is retained in one portrait and one post-start landscape
  capture. Its **Wait** action was used, not a launcher kill/data wipe. A premature
  character capture stopped at the no-preview-process guard before a tab tap;
  it is retained as the launcher failure, not labelled a character pass.
- Bounded PID logs: normal6221 has184 lines/0 error-or-fatal matches;
  character6451 has814 lines/2 ERROR matches/4 missing-frame mentions;
  inventory6653 has416 lines/3 ERROR matches/6 missing-frame mentions. These
  are asset errors, not a reported app crash or whole-device performance proof.
  Both diagnostic apps were stopped after capture; installed app/emulator data
  were not cleared, no AVD wipe or global log clear was performed.

Nine original PNG captures are retained and manually inspected.
`runtime-v12.json`/`runtime-audit.mjs` bind their dimensions/hashes, actual installed
versions, process logs, source markers and observed results. Full phone UI,
original weight imagery, startup performance, real player flow and device
acceptance remain false/open. The next source/resource version must not be
mistaken for these exact v12 hashes or erase their failures.
