# Android NI-14 actual offline mail JNI — 2026-10-03

Only the bounded offline Java/JNI mailbox and shared-feedback consumer gate passes.
NI-14 remains PARTIAL and the complete Windows-alignment goal remains Active.
No real login, server mail operation, custody transfer, settlement, full phone UI,
physical-device or final human acceptance is claimed.

## Source, frozen denominator and delivery

- Product source: `2be65cc9eb43205a4bdd77c808e367de67206829`.
- Independent branch: `codex/android-shared-sync`; [PR253](https://github.com/Zombieliu/mir2/pull/253) stays Draft. Product and later documentation commits are not the same APK source.
- Full frozen Windows denominator: `3d735745f1117d42a7859e87604a106351dca935`; no AP-01–21 scope reduction.
- Remote check 2026-10-03T05:09:12Z: actual Windows source branch `codex/playtest-registration` is `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`, one commit / 83 documentation-evidence files / zero functional-source files after frozen.
  The old `codex/windows-player-journey` remains `6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4`; it is not the latest functional source.
- Exact input hashes, stage APKs, local raw roots, screenshots, device and gates:
  [source-evidence.json](source-evidence.json).
- APKs, native libraries, all screenshots, build caches, credentials and keys remain outside Git.

## DEBUG REPORT

**Symptom and impact.** A synthetic host-owned mail result passed dedicated mail
ingress, then the Android-only scene-effect decoder treated it as a malformed
graphics packet. All four candidate scenes disconnected. After that router was
fixed, the mailbox and shared failure prompts became visible but the strict
read-only diagnostic still never logged.

**Root causes.** The generic graphics dispatch was also fed classified mail.
Separately, the observer read `PreviewRequest.scene`, which is deliberately
retired after four preview frames; Java sends the actual JNI inputs later.
The new diagnostic metadata survives that short request and resets for the next
preview. Neither mail data nor feedback is manually inserted into shared models.

**Fixes.** `0268ad60` filters already classified mail at the Android render-only
dispatch, after unchanged phase/owner/epoch/schema checks. Non-mail malformed
effects retain fail-closed validation. `2be65cc9` retains only the preview scene
in its read-only receipt; the exact mailbox, IDs, inventory and feedback predicate
is unchanged. The shared mail projection, consumer, protocol, auth and settlement
rules are not rewritten.

**Regression proof.** Compiled red/green observer and dispatch tests, followed by
fresh source-bound full gates and newly verified installed APKs. Raw preparation
errors, the interrupted orchestration, failed candidates and images remain
preserved under their own source roots; they are not relabeled as this pass.
A discarded empty-slot-padding hypothesis was disproved by the unchanged shared
inventory projector before any implementation change.

**Scope.** Five cumulative functional files after the prior evidence head:
`OfflinePersonalIngressPreview.java`, its test, `ui_preview.rs`,
`mail_jni_preview_tests.rs`, and `shared_shell.rs`.
This final observer commit changes only the two preview source/test files.
56 other bound inputs and 11 protected complete files match; all older observer
assertions and the strict model predicate remain. The frozen attachment helper
and seven mail command schemas retain their verified hashes. No Android/window
layout redesign or backend/Windows editing occurred.

## Preserved source stages

| Source | Actual Android result | Status |
| --- | --- | --- |
| `0b61463e` | Four synthetic Java/JNI mail-result scenes reach the wrong graphics decoder and disconnect. | FAIL; APKs, 6 originals and logs preserved. |
| `0268ad60` | Router fix shows full mail and shared failure prompts; no false graphics disconnect. Short-lived observer request still prevents the strict marker. | Bounded visible-data proof; strict JNI gate FAIL. |
| `2be65cc9` | Delayed scene survives; all four strict model and same-frame shared UI consumption checks are observed through actual JNI. | Offline JNI gate PASS only. |

The initial diagnostic registration/Java-stream compiled reds, the render-dispatch
compiled 1-pass/1-fail red, and the delayed-observer compiled 0-pass/1-fail red are
preserved. The last red is the same delayed-lifecycle assertion that now passes;
an additional new-preview reset assertion does not replace an old assertion.
No earlier failed gate is silently rewritten.

## Exact-source checks

58 input files bind the clean product source before documentation edits.
Every gate checks source/input stability, and Java tests force rerun with no build
cache rather than counting cached reports.

| Gate | Result |
| --- | --- |
| Android normal Rust | 396 passed |
| Android preview Rust | 429 passed |
| Shared native-player UI | 1292 passed; 10 previously ignored |
| Runtime | 296 passed; 1 previously ignored |
| Java Debug / uiPreview | 77 / 77 passed, six classes each; zero failure/error/skipped |
| API31 arm64 ordinary / preview check | Both passed |

Typed model and request-lifecycle tests are headless observer tests, not JNI.
The synthetic private result never calls `GatewaySession`, creates pending mail,
authenticates or writes a socket. Its success/failure comes from the offline
fixture. The device proof below separately uses the unchanged Activity
`nativeEvent` JNI entry, host inbox, bounded accepted-owner ingress and original
shared consumer.

## APK, selected resources and installation

VersionCode **35**, versionName **0.1.32-gameshop-phone**, min/target SDK **31/35**.
Gateway URL is empty. The labels identify the exact source, not a new version,
store release, signed production distribution or remote Web deployment.

| Variant | Bytes | SHA-256 |
| --- | ---: | --- |
| Normal `mir2-native-mail-observer-debug-2be65cc9.apk` | 387799951 | `7dbef3f63d045a82be37b71d82fb0854bc91b966c3cea3b243b557f05135ce8d` |
| Preview `mir2-native-mail-observer-preview-2be65cc9.apk` | 540980071 | `0f7c1242fc4597841236d1e91f24513affa3a947899b05c4edebbedef06540ce` |

Both files are in
`apps/game-client/platform-android/target/mail-jni-observer-20261003-9LGmkq/final-apks/`.
Paths are relative to `mir2-web3`; exact recorded host paths are in the JSON.
The normal APK is byte-identical to the router stage because this final change
is compiled only into the separate preview variant. No new normal-binary behavior
is inferred from the new source commit label.

Each APK contains the expected native ELF, matching that variant's
`stripped_native_libs` output by bytes; this does not claim successful size stripping.
Each selected Items1003, StateItem5192, MagIcon224, MagIcon2 224 and UI_32bit4 PNGs,
plus three metadata files, matches local resource bytes: 6647 PNGs total.
UI_32bit470–473 also match frozen Git bytes. Entity override pack remains
`android-archer-mount-bow-proof-20260912`, manifest SHA
`929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`.
These selected resources do not certify the complete approved atlas/audio release.

Read-only ZIP inspection explains the size issue without claiming a packaging fix:
preview central directory still references 26321 entries; referenced compressed
bytes grow by 79384, while unreferenced ranges before the central directory grow
to 149188132 bytes, including signing/padding structures. The old library's range
is not referenced by a central-directory entry. Both original signed outputs are
retained; no APK rewriting, stripping or release-size acceptance was attempted.

Both packages were reinstalled with `-r` only, Code35→35.
First-install timestamps remain unchanged and installed APK SHA matches the file.
No app/AVD data was cleared. The existing dedicated AVD's restart in the router
stage used no snapshot load/save and no wipe; its config hash remained unchanged.
The sandbox's ADB-port restriction was a diagnostic permission failure, not a new
emulator crash; the permitted check confirmed the same live emulator.

## Actual API31 JNI and visual observations

Device `emulator-5554`, arm64 Android12/API31, 1080×2340 at density440
(the captured landscape image is 2340×1080). Fingerprint:
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.
One emulator, zero connected physical Android devices were observed.

For each scene Java actually sends five events: STARTING, accepted-owner synthetic
world, host-shaped private result, mailbox refresh, newer mailbox refresh.
The unchanged strict observer validates 256 mail rows × 5 attachments (1280),
every exact u64 mail/attachment ID including MAX, gold77/unclaimed row state,
gold777/credit33 and bag12/UID80000–80011. The original UI consumer removes only
its transient feedback row in that frame; all 256 visible mails remain.
The following newer refresh does not hide that observation.

| Scene | Cold PID | Strict feedback / UI consumption | GL506 / uninitialized color |
| --- | ---: | --- | ---: |
| Claim success | 5507 | PASS; Collect/true/MAX ID, no failure notice | 58 / 58 |
| Claim failure | 5607 | PASS; Collect/false/MAX ID, “Mail claim failed” | 22 / 22 |
| Send success | 5682 | PASS; Send/true/no claim ID, no failure notice | 50 / 50 |
| Send failure | 5756 | PASS; Send/false/no claim ID, shared rejection notice | 0 / 0 |
| Normal package with preview intent | 5831 | PASS isolation; only unconfigured-server login, no fixture | 0 / 0 |

The actual model/UI markers are not “preview ready” or the Java send marker alone.
Evidence parsing only removes ANSI codes and normalizes field-equals whitespace;
incorrect kind, success, counts, wallet, inventory or full IDs remain rejected.
Raw logs are byte-preserved and duplicate Android/Rust log sinks do not count as
two model operations.

All **six final original images** were viewed:
inbox/newer body and page1/26 visible in all mail scenes; claim background/resume
retains PID5507 and inbox; two failure prompts visible; normal preview intent
does not bypass login. No scene-effect false disconnect, Java fatal or Rust panic
was observed in these five cold logs.

**GPU gate FAIL:** 130 GL506 and 130 uninitialized-color messages total.
Earlier 61/8 and current130 error totals have different cold-AVD/process/window/
timing conditions and are not a controlled performance comparison.
This patch is not a GPU or continuous-stability fix.

## Remaining full-goal gates

- NI-14 remains PARTIAL: real account/login, actual Gateway-owned send/claim,
  pending lifecycle, authoritative gold/items/mail settlement and failures.
  The fixture did not open a compose draft/reader, so neither actual draft
  retention nor success-window closure was verified despite the notice text.
- Phone controls, mailbox/reader/compose grouped layout, IME clipping, long text,
  actual touch, multitouch and nine languages remain unaccepted.
  The prior three failed layout repairs require explicit human confirmation
  before resuming that design repair; automatic goal continuation is not it.
- NI-15 Group/Guild/Trade, NI-16 Hero, NI-17 full effects/light/audio,
  NI-18 negotiated native resume, NI-19 every-domain resets/backpressure,
  NI-20 complete versioned assets/cache/update remain open or partial.
- Full AP-01–21 scope includes the remaining render-ready maps/object layers,
  online shared-Zone authoritative movement, save/re-enter persistence,
  supported combat/personal operations, Windows/Android same-server behavior,
  resource delivery, GPU/memory stability, physical-device and final human QA.
- No production deploy, real saves, auth bypass, debug teleport, raw account_id,
  backend transaction-rule rewrite, Windows push or PR merge occurred.
  Protected original workspaces match HEAD/branch/Git-status checkpoints; this is
  not a recursive hash certification of all untracked file contents.

Next leaf: bounded NI-15 Group/Guild/Trade ingress/egress audit, preserve original
shared permissions/offer/settlement rules, then add only the required owner-fenced
host adapters. Do not mark NI-14 or whole goal complete from the offline gate.

## Raw provenance

Three source roots and every existing regular file (359 files, 2525392210 bytes)
are indexed before documentation/push; no symlinks are followed. The index itself
and later delivery observations are outside that snapshot.
Final local index:
`apps/game-client/platform-android/target/mail-jni-observer-20261003-9LGmkq/raw-payload-indices.json`.
SHA-256 `89bd4cc3cc04f04fcedbe72eeadcfd5122c432b79bd8511d3d706cc9329fb1e6`.
The JSON records all three exact raw roots and source/input/APK/image hashes.
Failure and interrupted-build logs remain separately bound to their actual source.
