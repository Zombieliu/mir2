# Android original weight-bar package repair — 2026-10-01

Bounded NI-20 resource leaf; the Windows-completeness goal remains Active.
Frozen Windows: `3f5e61533235921369bc13a7760b4a56b0e467e5`.
Source parent: `9e29d7b67bead922ead99ae4179ae2e452f06e3e`.
The source/staging gates below precede the clean v13 package/runtime follow-up.
Exact APK source: `cfac6ecd4bb66ac7c4b0a2625ff178f1703335a0`; version13 /
`0.1.10-weight-bars`. Both actual packages install and the offline BAG weight
bar is now visible. Phone HUD weight/full UI/live/device remain unaccepted.
The exact v12 packages and missing471/473 images remain in the
[previous evidence](../native-android-player-ingress-20261001/README.md).

## Cause and bounded repair

The shared Windows HUD/BAG weight code uses original UI_32bit frames470..473.
Both actual v12 APKs omitted all four: numeric weight66/100 was visible, but
HUD473 and BAG471 produced real missing-image errors. This is Android staging,
not a request to replace the shared weight/UI rules or recolor fallback bars.

- `stage-shared-weight-assets.mjs` reads the four PNGs and metadata directly
  from the frozen Git blobs, copying them into a **new** ignored diagnostic pack.
  All13240 prior source files stay unchanged;13239 copied files are byte-identical.
  Only the cloned aggregate manifest changes. This is not a new raw-library
  export or image edit. PNGs, source metadata and the pack stay outside Git.
- Metadata version3 has library extent474 and exported count4:470/471 are84x6,
  472/473 are76x12, all offsets0. The Android guard requires these four, not474
  dense bitmap files. Other valid source frames remain bounded by library extent.
- Gradle checks integer/unique indices, exact paths, metadata/manifest agreement,
  required dimensions and zero offsets, PNG existence/byte bounds/signature/IHDR
  **before** decoding, then actual decoded size. Sync includes original PNGs
  and UI_32bit metadata. No shared renderer, wallet, gameplay, auth or server rules
  change; Windows workspaces and branch are untouched.

## Measured source gates

| Gate | Actual result |
| --- | --- |
| Sparse weight resource gate | 20/20: one valid input plus19 negatives |
| Existing Items/StateItem geometry gate | 13/13 on the supplemented pack |
| Existing MagIcon/MagIcon2 gate | 7/7 on the supplemented pack |
| Java Debug /uiPreview | 36/36 each, five suites each,0 failures/errors/skips |
| Actual Gradle Sync byte audit | 6647 PNGs and three metadata files match inputs |
| Bounded independent read-only source review | No remaining P0/P1/P2 reported |

`source-audit.mjs`/`source-results.json` bind the three source-file hashes, actual
control logs and XML, and staged image/metadata equality. Java/staging was run
with `--rerun-tasks`:44/44 tasks actually executed. The first Java command had a
wrong log-output directory and **did not start Gradle**; it is not counted. The
corrected actual run is `java-and-stage.log`.

`guard-before.log` retains actual BUILD SUCCESSFUL with a missing library,
followed by the failure-first assertion. `guard-after.log` is the intermediate
18/18 result; `guard-final.log` is the final20/20 including oversized-header and
wrong-signature controls. Each final weight control's actual Gradle log is kept.
Missing libraries/meta/PNG, malformed JSON, duplicate/fractional/missing indices,
wrong geometry/path, invalid PNG and manifest mismatches reject at this specific
guard, not due to an unavailable toolchain. All fixture edits are isolated in
ignored target; input byte digests remain unchanged.

Original manifest SHA-256:
`e14940711b944e54baa41254fca3f4b8e0ca0ad3e5d92e76e12a366823ca7735`.
Supplement manifest SHA-256:
`ea2a4cc8bf0a9e91f22213d4744587b0298d691021dcf405f51c553fd8632d3d`.
Frozen UI_32bit metadata SHA-256:
`9459d5a272a4a41ee0485a553ec3ce8f57996d5840cd99c8eae9b1dab5f14db9`.
Four individual original PNG hashes/dimensions are in `asset-identity.json`.

The reviewer compared source/frozen bytes/logs without running builds/tests or
operating a device. No Rust/shared gameplay source changed: v12 Rust257/preview270/
shared1203+8ignored results are historical, not freshly rerun for this resource leaf.
Gradle dependencies were resolved offline; wrapper network traffic was not traced.

## Remaining acceptance and publication

Next bind clean v13 source, both actual APK hashes/contents/installations, then
inspect original HUD/BAG weight bars in the actual emulator. Staging success alone
does not accept those images, startup performance, full original resource release,
all-phone controls/layout/IME, chat/NPC/services/remaining Windows models, real
HTTPS/WSS player flow or a physical phone. Existing v12 startup/Launcher failures
remain recorded, not erased by the resource fix.

Only emulator-5554 is connected. No approved live environment/account or physical
device exists for this run; empty Gateway diagnostics cannot accept online play.
The v12 evidence commit9e29d7b67 is currently local: ordinary SSH/HTTPS and verified
SSH443 attempts failed in transport. Fresh GitHub ref/PR reads still confirm only
f04a74399; PR253 stays Open/Draft, basecodex/playtest-registration. No publication
success, forced push, PR merge/base change, production deployment, save modification,
APK/raw-image/key commit or original workspace mutation is claimed here.

## Later exact v13 APK and actual original BAG image

Both builds have the exact clean source above and empty Git status before/after,
without source changes between them. They are Gradle Debug/uiPreview diagnostics
with a release-profile Rust library, not store Release or production signing.
Actual APK/config verifies arm64/min31/target35, normal UI_PREVIEW=false and
isolated `com.mir2.web3.uipreview` true. Both use empty Gateway URLs, no Web page.

Local root: `apps/game-client/platform-android/target/weight-bars-v13-20261001/final-apks`.

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| `mir2-native-weight-bars-debug-v13.apk` | 386299417 | `9b1bcb15e65b9d1737b5cbf30ef0df102e6a9bc6057ca2f770bab77f9f7c6437` |
| `mir2-native-weight-bars-preview-v13.apk` | 390021621 | `7efead879a7a27f22279e702f3f414fea620e37fbbadec8d775daf2eb6ec99a1` |

`verify-package.mjs`/`package-v13.json` compare6647 original PNGs and three
metadata files per actual APK to the supplemented diagnostic input. All four
weight PNGs **and metadata** additionally match frozen Windows Git blob bytes;
all three tested source hashes match the committed APK build inputs. World/entity
inputs remain the unchanged diagnostic packs, not a complete public resource release.

Only emulator-5554 /sdk_gphone64_arm64 /Android12 API31 is connected, physical
1080x2340 /landscape2340x1080 /density440. Both installs use `-r` and preserve data.
Three original captured frames were manually inspected:

- `preview-inventory.png`: original yellow BAG471 weight fill is restored for
  the explicit offline66/100, with available34, BAG gold12,352 and phone HUD12352.
  No weight resource error remains in the captured process log. Thumb/status/chat/
  belt lanes are unobscured. Other ratios/colors/zero/overweight are not tested here.
- `preview-character.png`: original male body and frame30 weapon remain visible,
  with the same explicit HP80/200 and MP20/100 sidebar. No new stats-tab/close,
  dragging or touch interaction was performed in this v13 capture set; v12's
  earlier interactions remain their own source checkpoint. Armour has no StateImage.
- `debug-login-recovered.png`: empty shared login, **Test server not configured**.
  It remains small for the phone; no credentials, account login or server state.

Phone HUD still has **no visually accepted weight row/bar** in these scenes.
473 exists and no missing-resource error occurs, but that is not a displayed HUD
weight acceptance. Only the original BAG471 image is accepted in this visual leaf.
Credit503 is a diagnostic source marker, not a rendered credit or purchase result.

### Simulator interruption and recovery remain failures

The first normal start timed out13463ms; before any capture, the emulator left
ADB and its host process disappeared. The original macOS report at21:39:21 records
qemu-system-aarch64 SIGABRT in a gRPC callback. This proves a **host** exit, not
an Android game crash, and does not establish its deeper root cause.
`emulator-exit-summary.json` retains the bounded exception/stack summary; the
original168352-byte `.ips` is still under macOS DiagnosticReports, not committed.

The background recovery did not persist. A retained-session boot then waited
at the SDK crash-upload consent dialog; CUA could not expose the standalone SDK
application. After gracefully stopping only that new waiting process, the same
Mir2_API_31_ARM64 AVD was launched with documented per-launch
`-crash-report-mode never -no-metrics`, no snapshot load/save. No AVD/GPU/auth/global
configuration edit, userdata wipe, Pixel5 launch or source/save cleanup occurred.
Normal installed version13 survived the restart and was reverified. SDK no-upload
handling automatically removed its queued temporary minidump; **no recoverable
copy of that SDK dump is verified**. The original macOS report and retained boot/
failure records remain; no crash upload was authorized.

Recovered start waits are ok638ms normal,546ms BAG and433ms CHAR. These do not
erase the earlier timeout/host exit or constitute first-frame latency/soak/stability
acceptance. Captured own-PID logs4438/4622/4748 have219/375/220 line entries and0
ERROR/fatal/panic/signal or missing-weight matches; this is a bounded capture only.
Both diagnostic apps are stopped afterward; the same recovered test AVD remains
running in its retained execution session. No global log clear/data wipe.

`runtime-audit.mjs`/`runtime-v13.json` bind installed versions, source, device,
capture dimensions/hashes, bounded logs, first/recovered starts and explicit
manual observations. Full phone HUD/UI/input/resources/startup/online/device
remain false/open. Next NI-09 incoming chat and HUD weight, then NPC/services and
all remaining Windows leaves; the goal is not complete.

Ordinary v13 source pushes also failed. The retained HTTPS1.1 attempt sent4.25MiB
but received no successful result before timeout; **100% sent is not published**.
Final observed ref/PR files distinguish this from a successful push, still f04a74399
Open/Draft with the same base. Sourcecfac6ecd4 and v12 evidence9e29d7b67 remain
local pending transport recovery. No forced push, branch cleanup, merge or deployment.
