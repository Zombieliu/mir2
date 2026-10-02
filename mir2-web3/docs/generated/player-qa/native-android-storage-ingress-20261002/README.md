# Native Android Storage ingress — NI-13 source checkpoint

Full alignment goal: **Active**. Complete NI-13: **PARTIAL**, not accepted.

Exact source: `81eb08cdca3390262e1eff3756afd4d09a16d005`, parent `600d8374a22e712c3e4f56c96525dd323b8efba8`.
Frozen full Windows denominator: `3d735745f1117d42a7859e87604a106351dca935`.
Do not downgrade the denominator to the currently observed remote Windows ref
`6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4`. Windows was not modified.

## What was fixed

The shared Storage model and native consumers already existed. Android Java
did not forward its public metadata and the Rust host had no owner-bound
producer for snapshots, item-only refreshes or password/expansion results.
Two new compiled TLS tests failed before any fix: **2 tests, 2 failures**.
The same cases pass in both freshly compiled variants after the six-file repair.

Ten pure function bodies retain the frozen Windows tokens (only non-string
whitespace is ignored). The existing shared slot/current-count item metadata
helper is reused. Android's packaged-frame lookup is added separately, without
inventing frame dimensions, transfer/password decisions or a second rule set.
The current Windows snapshot has an equivalent closure block, while slot/item
metadata now delegate to the existing shared inventory helper. These three
structural differences are recorded rather than called identical frozen bodies.

Java forwards only UserStorage, StorageUnlockResult, StoragePasswordResult and
ResizeStorage in an authenticated listed-character Start/world phase. Rust
stages packet-first metadata until the exact owner snapshot, refuses unreset
owner/name switches, rejects other-context/hero packets, preserves personal
metadata across same-owner maps, and clears it at rejected Start or a terminal
boundary. Storage metadata cannot bootstrap a player or bypass render-ready.

Storage arrays keep all 160 slots, including sparse slot 159. UserStorage updates
only items, not size/password/unlocked state. Drafts are never copied from a
snapshot/result. Existing runtime semantics still own unlock/removal/expansion.
StoreItemV2/TakeBackItemV2 are excluded from the new gameplay metadata path and
continue exclusively on their existing correlated receipt channel. The shared
runtime consumer, original storage rules, auth and native critical queue are
byte-identical to the parent.

Bounds: raw packets 16 KiB UTF-8, snapshots 1 MiB, projected message 512 KiB,
32 retained messages, 2 MiB of retained serialized JSON. This is not a measured
resident-memory bound. Native admission failure retains the exact FIFO front
for a later frame; it does not replay a transfer request or drop a critical
result. Overflow follows the existing host disconnect/DataReset path. This
bounded host retry differs from Windows' direct producer fallback; it does not
change the native queue or receipt rules.

## Exact committed-source gates

| Gate | Result |
| --- | --- |
| Android normal, serialized | 359 passed, 0 failed |
| Android ui-preview, serialized | 384 passed, 0 failed |
| Shared native-player UI | 1260 passed, 10 pre-existing ignored |
| Shared runtime | 292 passed, 1 pre-existing ignored |
| Java Debug / UiPreview | 47 / 47 passed, 5 classes per variant |
| Actual arm64 API 31 check | Both variants pass |

Seven post-commit command records bind their start/end hashes to all six exact
source blobs. Original pre-commit runs and the targeted 29-test run remain
separate. Java uses offline, no-build-cache and rerun-tasks; it actually
recompiled both test variants. The Java transport is TLS MockWebServer + OkHttp
WSS fixtures, **not a real Gateway/account acceptance test**. Headless Rust host
tests exercise the actual receive/bind/native-queue producer, **not Android JNI
or visible Warehouse UI**. Existing runtime Storage consumer/receipt tests pass.

Two overly strict baseline comparisons and the wrong preview task name failed
during evidence preparation. Their classifications/raw compared bodies are
preserved. They are not product red-to-green claims. No post-fix recorded test
failed. Compiler warnings and pre-existing ignored cases remain visible.

## Still open

No new APK was built, installed or measured for this source. No new emulator
screenshot, actual Java-to-JNI trace, real HTTPS/WSS login, warehouse transfer,
password/expansion operation, reconnect/save or physical-device acceptance.
The installed v30 package is historical and does not contain this leaf; do not
relabel its hash or render evidence. No GPU fix or new renderer error count was
made; the prior zero-error gate remains FAIL. A dependency/GLES expansion question
from the previous investigation remains unanswered and was not implemented.

NI-12 full catalog UI/purchase/Credit/Mail settlement, full NI-13 visible/storage
loop and transfer-to-shared-runtime integration, NI-14–20, complete mobile UI,
IME/compact/multi-touch/languages, authoritative Zone/movement/save, all resource,
audio/update and physical-device/human gates remain in the full matrix.
The next phase includes exact NI-12/13 packages and actual host/emulator evidence;
safe remaining source leaves do not erase any acceptance requirement.

Both original worktrees retain their recorded HEAD, branch and Git status.
Untracked contents were not recursively hashed. No production deployment,
real save mutation, authentication bypass, force push, stash/reset/clean,
AVD wipe, APK/cache/credential/signing-key commit, Windows edit or PR merge.

## Evidence and publication

`manifest.json` hashes the exact raw payloads plus the machine-readable diagnosis.
`raw/source-commit-binding.json` records the six-file Git scope and source blobs.
`raw/frozen-storage-functions.json`, `raw/source-boundary-audit.json`,
red XML/log and post-commit gate logs/XML keep the observations reproducible.

Run `node verify-evidence.mjs working`, `index` or `HEAD` from this directory.
Modes verify raw bytes, Git blobs, frozen bodies, source gates, Java/red counts,
unchanged rule/protected boundaries and explicit false acceptance fields.
Raw command logs retain their original trailing spaces/blank endings; only the
exact raw directory is excluded from whitespace-format checks, and its bytes
are verified by the manifest instead. All edited source/docs still get checked.

Before publishing this leaf, Android remote and Draft PR253 were freshly
verified at `600d8374a22e712c3e4f56c96525dd323b8efba8`; this also delivered the
previous v30/NI-12 evidence backlog. Source81eb and this evidence still need
a normal fast-forward push and fresh remote/PR verification. Post-publication
status is recorded separately in the ignored build-evidence directory and
must not be inferred from the local commit or rewritten into old raw evidence.
