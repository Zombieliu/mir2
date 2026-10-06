# Native Android NPC image / close-touch leaf

2026-10-02. Whole Windows-alignment goal remains **Active**.
Frozen gameplay source: `3d735745f1117d42a7859e87604a106351dca935`.
Exact new Android source: `78e7d2309f6a6b92cea1737c76c5eedfd863e46e`.
No shared, Windows, server, authentication or gameplay rule source changed.

## Actual result and limits

Approved sparse `Items/7` is visible in the received Buy specimen (36×26,
`OFFLINE received item`, price50). The previous synthetic658 was absent from
the approved pack; replacing only the offline specimen is not a real item grant.
Its original PNG hash is `60f5e8a61589c2a4b5c3cdcf64e93a1101110435333cf2e5484da056f0a7ae6a`.

Four actual emulator taps close Buy, Sell, Repair and SpecialRepair. The primary
agent inspected all eight original2340×1080 before/after frames, with shared NPC
window(s) gone and the existing Android HUD restored. The four corresponding
canonical logs each show `ANDROID_NPC_CLOSE_SHARED_LOCAL_UI_EXIT_REQUEST_NOT_LIVE`.
Buy uses(778,158), services(1278,158); installed APK SHA is checked before taps.
No Buy/Confirm/Hold transaction button was tapped, no quote fabricated and no
online purchase/repair/currency/inventory/save success claimed.

- [Buy before](v19/touch-device-v3/npcshop-before.png) → [after](v19/touch-device-v3/npcshop-after.png).
- [Sell before](v19/touch-device-v3/npcshop-sell-before.png) → [after](v19/touch-device-v3/npcshop-sell-after.png).
- [Repair before](v19/touch-device-v3/npcshop-repair-before.png) → [after](v19/touch-device-v3/npcshop-repair-after.png).
- [SpecialRepair before](v19/touch-device-v3/npcshop-srepair-before.png) → [after](v19/touch-device-v3/npcshop-srepair-after.png).

The service × was painted into `Prguse2/351`, not an interactive shared button.
The Android-only transparent target follows the real shared OverlayShop parent,
scale and visibility, and invokes existing `close_all_windows` / Exit lifecycle.
It does not introduce alternate combat, item, trade or save semantics. Guards
reject hidden/detached/old-parent targets and modal/keyboard/trade contexts.
Forty-eight logical-pixel target math is unit-tested; all extreme screen sizes,
drag/picking combinations and physical48dp acceptance are still OPEN.

## Failure-first and source gates

The original v18 close remains unresponsive after both a short tap and a600ms
stationary hold. Its three original frames, commands and PID logs are in
`v18-failed/`; no wipe or retrospective relabeling. Three compiled tests fail
before repair: missing close target, absent approved item frame, stale hidden
target canceling a new request. An intermediate E0369 test-harness compile error
and first label-mismatch failure are retained, not counted as product gates.

Final serial Android gates: **296 normal /315 preview**,0 failed/ignored;
fresh Java **39+39**,0 failure/error/skip,42/42 Gradle tasks executed;
actual arm64/API31 both variants pass. These final logs are in `v19/final-source/`.
Earlier295/314 logs are intermediate pre-parent-guard gates and are not rebound
to78e7. Twenty-eight selected committed input hashes match both package gates.
Read-only reviewer reported P2, reproduced red then fixed; final review found no
remaining P0/P1/P2 in the bounded three-file diff. Reviewer did not build or run
ADB/tests; this is not an independent full-goal acceptance.

## Exact packages and installation

Clean78e7 before/after EACH package, version19/name`0.1.16-npc-touch`,
arm64-v8a, minAPI31/target35, NativeBevy/GameActivity, not WebView.
Rust release cdylib is inside Gradle Debug diagnostic variants, not store Release.
Both Gateway URLs are empty; preview flag is separate. No secrets/signing keys
or APKs are committed. Local APK directory (not portable to another machine):
`mir2-web3/apps/game-client/platform-android/target/npc-touch-v19-20261002/final-apks/`.

| Diagnostic variant | Bytes | SHA-256 |
| --- | ---: | --- |
| mir2-native-npc-touch-debug-v19.apk | 386482753 | `5f7f5c052300ac40ac32ba3ba75c7c254dc463e4b1806a252cd3633acd5aefd2` |
| mir2-native-npc-touch-preview-v19.apk | 390293041 | `6200ab1ef5bc3fd449d23129b28819adde95d9d8014b0e9cd1390b78da73c2dc` |

Selected resources in each APK:6647 PNG+three metadata files match approved
inputs byte-for-byte. This is the previous bounded Items/StateItem/MagIcon/
MagIcon2/UI_32bit denominator, not all maps/entities/effects/audio/update/store.

Only emulator5554, existing Mir2_API_31_ARM64/Android12/API31, native panel
1080×2340/density440 and actual landscape screenshots2340×1080. No phone is
attached. Task apps installed with `-r`; no uninstall/data clear/wipe/AVD change.
Both task apps are stopped after captures. Normal login shows empty credentials
and `Test server not configured`; no real auth/realm/human store was used.

Touch collection first stopped at two read-only setup guards: native portrait
`wm size` vs rotated screenshot, then actual Android `pm path` characters. Their
partial untouched identity outputs stay in `touch-device/` and `touch-device-v2/`.
No activity or gameplay tap occurred in these attempts. Corrected v3 checks the
actual frame dimensions before calibrated taps; this is harness setup repair,
not an app/UI failure erased or a successful product gate.

## Renderer FAIL and remaining full denominator

Nine distinct PID captures contain **135 GL0x506 framebuffer-incomplete** lines:
initial login/Buy/Sell/Repair/SRepair33/6/48/0/0; touch runs6/20/0/22.
Count one latest cumulative log per PID, not both before and after. No missing
image path or fatal/panic was seen in these bounded captures; this is not general
stability. Raw logs retain all errors, and the **zero-error renderer gate FAILS**.
No renderer/backend fix or claim follows from individual zero-error launches.

NI-10 full quest/dialog, NI-11 full receipts/actual authenticated JNI/online,
NI-19 downstream critical/ACK eviction and all other Windows model leaves stay
OPEN/PARTIAL as inventoried. Real HTTPS/WSS/list/StartGame/Zone/save, complete
phone/multitouch/IME/languages/resources and physical/human gates are not passed.
Next: full public quest/dialog ingress using the same Windows/shared projection,
and bounded framebuffer diagnosis; retain exact new source/package binding.

## Reproducible evidence integrity

Run from repo root: `node <this-directory>/curate.mjs`, then
`node <this-directory>/runtime-audit.mjs`; after commit,
`node <this-directory>/git-integrity.mjs HEAD` checks every recorded raw byte in
both working files and actual Git blobs. Explicit `-text` attributes preserve
SDK CRLF before first add. Manifest lists original sizes/hashes. No APK/extracted
asset/cache tree is traversed, and older v17/v18/v16 evidence is not overwritten.
