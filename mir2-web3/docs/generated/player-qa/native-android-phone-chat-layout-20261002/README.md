# Native Android focused chat layout — v15, 2026-10-02

Bounded G4/NI-09 repair; the complete Windows-alignment goal remains **Active**.
Frozen Windows: `3f5e61533235921369bc13a7760b4a56b0e467e5`.
Exact clean APK source: `d009f240247261146c1053b5ec8cdd934724cad8`.
Parent: `b0d49df63a9ce6205c325a1617626d50d9c1b48e`.

## Diagnosis, change and source controls

The [original v14 failure](../native-android-received-chat-20261001/preview-chat.png)
is retained unchanged: focused chat displayed the system line, but hid the latest
peer line. Android requested `rows*18 +2*48 +12`, while its four gaps require
`+16`. Short available IME height also selected the oldest requested rows rather
than the newest rows that could fit. This is geometry, not a missing packet or
a shared filtering-rule change.

New actual shared-child node tests reproduced both defects **before** the repair:
`chat-before.log` has2 passed/4 failed. The first fixture compilation attempted
private HostState fields; `chat-test-setup-error.log` retains that E0451 separately,
not as a product failure. Only after corrected tests reproduced the defect did
the phone-only adapter add the4px and calculate capacity from actual `lines_bottom`.
It keeps the newest suffix of the existing shared, already-filtered slice.
Texts, draft, shared source actions/settings, authentication, networking, packet
ordering, server rules and desktop layout are unchanged. Version15 distinguishes
the new APK; no original artwork was recolored or replaced.

| Current source gate | Actual result |
| --- | --- |
| New actual-node regressions |6 passed,0 failed |
| Android normal /actual preview feature |273 /287 passed,0 failed,0 ignored |
| Java Debug /uiPreview |37 each, five suites each,0 failures/errors/skips |
| Java task execution |42/42 actually executed with `--rerun-tasks` |
| API31 arm64 normal /preview checks |Both passed |
| Bounded independent three-file read-only source review |No new P0/P1/P2; no tests/build/device executed by reviewer |

The density/UiScale/source-size regression checks96 combinations, plus ordinary
two-row, clamped-newest-row, IME resize/restore, zero available history and
unfocused one/two-row cases. It verifies actual child display and bounds, not only
a requested panel height. It does **not** prove all extremely short windows:
the existing262-high/top8/IME160 case can still place filter controls above safe
top (`filter_top=-14`). This remains open, not a regression introduced here.
The unchanged shared runtime can still evict ChatLine under critical/ACK pressure
(NI-19); this leaf is not end-to-end lossless chat.

`audit-v15.mjs source` records12 exact source hashes, Rust/Java results and source
binding. Run all scripts **from repository root**, with the exact ignored target
below as the QA argument. Shared/Windows full suites were not rerun in this
phone-only leaf; earlier results remain historical evidence, not new acceptance.

## Exact installed packages

Version15 /`0.1.12-chat-layout`, minimum API31, target35, arm64-v8a.
Both clean before/after builds are bound to d009f2402 in `package-v15.json`.
Each actual APK contains6647 selected Items/StateItem/MagIcon/MagIcon2/UI_32bit PNG
files and three metadata files matching approved input bytes; the four weight
originals/meta also match frozen Git. This is not a complete aligned resource
release. Rust release cdylibs are inside Gradle Debug diagnostics, not store
Release, production signing or a remotely loaded Web page.

Ignored APK directory, relative to repository root:
`mir2-web3/apps/game-client/platform-android/target/phone-chat-v15-20261002/final-apks/`.

| APK | Bytes | SHA-256 |
| --- | --- | --- |
| `mir2-native-chat-layout-debug-v15.apk` |386272009 |`18bff3e98696f1bee77698a606febd84831b73a6057afa63ef33c11363ba4cf0` |
| `mir2-native-chat-layout-preview-v15.apk` |390041901 |`8b13152429d7999ee731ab73f50ab47a0e8331a0b23722dfbcfb6dbed40160e3` |

Both `install -r` return Success and installed version15 is verified. Only the
existing dedicated Mir2_API_31_ARM64 /emulator-5554 was used: Android12
sdk_gphone64_arm64, density440 (2.75), actual landscape captures2340x1080.
No emulator restart, GPU/SDK/global settings change, data wipe or Pixel5 action.
Both owned apps were force-stopped at the end; data and the same running AVD
were retained. Gateway URL remains empty; ordinary login visibly says
`Test server not configured.` No credentials, real login or server save was used.

## Actual original-frame and touch checks

All seven original SDK screenshots were manually inspected, not cropped/edited.
Their hashes, resumed Activity, PID logs and real actions are in `runtime-v15.json`.
These manual semantic observations are distinct from the script's file-integrity
checks; the script is not OCR. Two compile-time OFFLINE packets enqueue once.

| Actual step | Observed result |
| --- | --- |
| [Normal entry](debug-login.png) |Empty original login; no configured Gateway, no bypass |
| [Focused chat with keyboard](preview-chat.png) |PASS for this defect: system and latest peer rows both visible above actual keyboard |
| [Android Back](preview-chat-keyboard-hidden.png) |Keyboard disappears, app and both rows remain |
| [Tap draft again](preview-chat-reopen-attempt.png) |**FAIL**: actual1000,978 tap leaves keyboard hidden |
| [Tap Set](preview-chat-settings-tap.png) |Original FILTER modal opens via actual1955,970 tap, not a settings Intent fixture |
| [Tap CHAT BOX](preview-chat-settings-tab.png) |Actual1180,229 tap selects transparency page |
| [Tap Cancel](preview-chat-settings-cancel.png) |Actual1105,874 tap closes modal and restores both rows |

No typing/send/echo/Apply/toggle/trade/scroll/soak or full settings-acceptance claim.
The new keyboard reactivation failure remains open and is the next priority;
displaying both received rows does not complete chat interaction.

## Logs and failures retained

Debug PID6671 capture149 line entries has0 error/fatal/panic/signal matches.
Preview PID6769 cumulative captures have223/417/498/628/749/870 line entries.
Each retains the same **six GL0x506 framebuffer-completeness error line entries**
at startup (three unique full timestamp/message strings). These are not36 newly
reported entries across six copies. No fatal/panic/signal match was captured, but
the **zero-error log gate FAILS**, and startup/render stability is not accepted.
Root cause is not verified; do not dismiss these merely because later frames
rendered or label the renderer accepted.

`runtime-audit-first-failure.log` retains the original zero-error assertion failure.
The final report emits the measured errors and continues returning exit1 for that
failed gate; it does not filter them away. One invocation used the wrong working
directory and could not resolve the old screenshot; that separate setup error is
in `runtime-audit-wrong-cwd.log`, not a product error or pass. Winit/GLES warnings
and earlier v14 clipped-row/v13 timeout/emulator-host crash evidence remain intact.

SDK start waits402/383ms report ok; these are not first-frame, soak or performance
measurements. Original row failure is fixed in this exact specimen, while the
keyboard reopen and zero-error gate failures remain. Next repair/retest the
reopen path, extreme IME controls and phone HUD weight, then NPC/services and the
remaining frozen Windows UI/gameplay denominator. Real approved HTTPS/WSS,
Shared Zone player-loop/save, full resources, physical phone and final human
acceptance remain OPEN; no completeness percentage is inferred from test counts.

Only scoped source/evidence are to be normally pushed to `codex/android-shared-sync`
with PR253 kept Open/Draft and its base unchanged; no Windows push/merge/deployment.
APK/cache/unpacked originals/signing keys/passwords are not committed. Later doc
commits do not change either APK's exact d009f2402 source.
