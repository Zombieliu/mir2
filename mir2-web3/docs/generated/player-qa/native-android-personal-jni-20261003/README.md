# Native Android personal metadata — actual offline Java/JNI checkpoint

Full Windows alignment goal: **Active**. Complete NI-12 and NI-13: **PARTIAL**.
Frozen Windows denominator: `3d735745f1117d42a7859e87604a106351dca935`.
Observed remote Windows remains `6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4`;
it does not replace the denominator. No Windows, backend, shared rule or renderer edit.

## Current result and exact source

v33 source: `0bfa1632c375a11dd54d6c70783edcff652af59e`.
Three Android source commits touch five distinct files from published parent
`ecba88bd63e70fa9ec90b938a9779344cabca395`.
Java injects a bounded, immutable offline event plan through the actual JNI entry,
Rust host/queue applies it, and the existing shared GameShop/Storage consumers
receive it. The new scenes do not manually prefill the received models.

API 31 arm64 emulator, serial emulator-5554, 2340×1080 landscape, density440.
This is a native GameActivity/Bevy APK, not a WebView. Gateway URL is deliberately
empty: no remote webpage version or real server acceptance is claimed.
The diagnostic sender is enabled only in the separately identified UiPreview APK.
Passing its scene parameter to the normal Debug APK still leaves the real login
screen at “Test server not configured,” with no diagnostic sender or model.

| Bounded observation | Actual result |
| --- | --- |
| Java → JNI → shared GameShop | 105 rows, last gIndex2104/stock3; wallet777 Gold/33 Credit; visible first page |
| Java → JNI → shared Storage | 160 items, last ID90159/slot159/count5; size160, expansion/password metadata |
| Storage tab II / same-PID resume | Actual tab changes; tab II and received data retained after resume |
| GameShop same-PID resume | Received data/page1 retained; exactly one Java event batch |
| GameShop next arrow, tap and 600ms hold | **FAIL**: both remained page1/14 |
| Locked Storage JNI | Actual shared model stays locked; no unlock event/password sent |
| Locked password screenshot | **NOT ACCEPTED**: three empty captures; window reports SECURE and IME visible |
| Whole phone UI | **FAIL**: storage overlaps inventory; GameShop tiny targets/clipped names; login too small |
| Zero renderer errors | **FAIL**: four distinct v33 PIDs record 52 GL506 +52 uninitialized-color, no observed fatal/panic |
| Real login / HTTPS / WSS / purchase / storage transfer / device | **OPEN**, not attempted here |

The locked shared receipt proves read-model state, not a visible secure-password
or keyboard-layout acceptance. Back hides the keyboard but is not password-modal
cancellation; the subsequent capture also stays empty. FLAG_SECURE was not removed
or bypassed. Empty bytes are retained as failed captures, not counted as screenshots.

## Original pixels and limitations

Eight valid v33 images were inspected, with three historical v31/v32 images.
The originals are unedited; no cropped, annotated or reconstructed image is
substituted. First-page visibility is not proof that all105 rows or all160 slots
can be operated. JNI counts/identity/last slot are checked in the shared-model
receipt, separately from pixels.

- [v33 GameShop received first page](raw/v33/ui-gameshop-jni/0-cold.png)
- [v33 arrow tap remained first page](raw/v33/ui-gameshop-jni/8-tap.png)
- [v33 arrow hold remained first page](raw/v33/ui-gameshop-jni/16-swipe.png)
- [v33 received Storage, overlapping inventory](raw/v33/ui-storage-jni/0-cold.png)
- [v33 actual Storage tab II](raw/v33/ui-storage-jni/8-tap.png)
- [v33 Storage tab II after same-PID resume](raw/v33/ui-storage-jni/16-home.png)
- [v33 normal APK rejects preview intent](raw/v33/ui-normal-intent-isolation/0-cold.png)

The zero GL count in this particular GameShop PID is not a renderer fix or trend.
Storage, locked Storage and normal login record20/20/12 respectively. The two
Bevy rendering files and protected authentication/queue/runtime bytes remain
identical to the published parent. The previously asked dependency/GLES expansion
has not been authorized or implemented.

## Failures retained, not relabeled

v31 source `0f9c49935a983ba72ac43b845b199abeaf2b5ea7` and v32 source
`47e8ea47f0975494478be88eb49be154b6199cbe` both built and ran but their
JNI scene ended in ConnectionLost. v32's actual telemetry records Applied world
data,105 catalog entries and160 storage items, followed by the existing render
identity guard: “render receipt does not match the authenticated player.”
The fixture used UIWarrior in the selected roster but OFFLINE JAVA JNI in the
received owner. v33 aligns only that offline roster identity; production checks
are unchanged. Both failed packages retain their own source/hash/evidence.

v31/v32 also delivered ResizeStorage before a full world snapshot. Existing
shared runtime ordering deliberately lets the full base model replace earlier
scalar metadata. No production ordering bug or runtime fix is claimed.
v33 sends an independent ResizeStorage after the world/items base; its later
event is verified in Java tests and the actual JNI receipt.

Compiled reds: initial scene/model cases0/2, introduced NPC resource regression
385/1, final fixture identity0/1 and separate expansion-event Java0/1.
The introduced resource regression was fixed by initializing the new diagnostic
resource, without weakening the old test. Final red-to-green evidence remains.
Java39 checked-JSONException compile errors, an incomplete cargo-ndk argument
and one initial patch-format error are preparation failures, not product bugs.
The patch-format error survives only in the tool transcript, not an invented raw
file. Actual arrow failures, overlapping windows, GL errors and protected
capture failures remain open after all source tests passed.

The first evidence checker incorrectly expected an English javac total; the
original compiler output says “39 个错误”. Only that evidence assertion was
corrected. This preparation failure remains in the tool transcript, not a
fabricated raw log or a product red-to-green claim.
The first staging guard also miscounted the separately tracked manifest by one;
the guard was corrected to derive its total from the payload list plus the
four QA metadata files and five edited docs. No payload was removed or altered.
The initial index check then exceeded the default1MiB child-process buffer while
reading an original PNG Git blob. The verifier's read buffer is now32MiB; the
original pixels and their manifest hashes remain unchanged.

## Exact committed-source gates and packages

| v33 gate | Result |
| --- | --- |
| Android normal / UiPreview, serialized | 359 /387 passed |
| Shared native-player UI | 1260 passed,10 pre-existing ignored |
| Shared runtime | 292 passed,1 pre-existing ignored |
| Java Debug / UiPreview, recompiled offline | 52 /52 passed,6 classes each |
| Actual arm64 API31 cargo-ndk check | Both variants pass |

Seven gates per version bind before/after hashes to47 Git input blobs. All three
versions' logs/XML remain separate. MockWebServer TLS/WSS tests are fixtures,
not real login. The current eight actual UI images belong only to v33.

Both v33 APKs were installed with -r, installed bytes equal the source-bound APK
SHA256, and original firstInstallTime is retained. This checks upgrade flags and
package identity, not a content hash of saves or complete save migration.

| APK kept outside Git | SHA256 |
| --- | --- |
| mir2-native-personal-ingress-debug-v33.apk | 364b606076d4aab1518877e836bc25cb02a8c3bee90a7596e642de00f72b8b20 |
| mir2-native-personal-ingress-preview-v33.apk | 47ad2fc0948e7920406ac470a111cd27054d0f082bd3f414f57771b3cbfbb9fd |

Local artifact root is recorded in raw/v33/package-baseline.json. Each native
AArch64 ELF equals that variant's stripped build output. Selected6647 PNGs and
three metadata files match the approved local pack; full assets/audio/update are
not accepted. The diagnostic marker exists only in the preview native library.
No APK, native library, resource pack, cache, credential or signing key is tracked.

## Complete goal audit and next work

The full acceptance matrix is unchanged: complete phone HUD/all windows,
IME/compact/multitouch/languages, all NI-12/13 online transactions and NI-14–20,
real authenticated HTTPS/WSS → character/StartGame → authoritative Zone/movement/
save/reconnect, full resources/audio/update, physical device and human acceptance
remain open. This leaf closes only the actual offline metadata JNI bridge gap.

Next: isolate the GameShop arrow hit-test failure and complete phone presentation
for GameShop/Storage, retaining shared controllers and server-owned decisions.
Then continue the complete host/protocol matrix and approved real-network gates.
Do not treat the new offline receipt as real authentication or native parity done.

Both original worktrees' HEAD/branch/Git status equal before records across all
three versions; untracked contents were not recursively hashed. No deployment,
real save mutation, bypass, force push, reset/stash/clean, AVD wipe or PR merge.
Before publication, remote Android/PR253 were freshly verified at ecba88bd,
open/Draft, base codex/playtest-registration. This evidence commit and source
still require a normal fast-forward publication and independent post-push check;
post-publication observations stay separate from these pre-publication records.

Run `node verify-evidence.mjs working`, `index` or `HEAD` here. The manifest
binds404 exact payloads, including empty captures; the verifier checks hashes,
version-specific source/gates/XML, install records, JNI/negative logs, untouched
rule boundaries and explicitly incomplete full-goal flags. Only the exact raw
directory is excluded from whitespace formatting; its original bytes are hashed.
