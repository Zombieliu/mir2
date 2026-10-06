# Native Android GameShop metadata ingress — 2026-10-02

Outcome: the bounded **source producer/projection leaf passes**; full NI-12 and
the Windows-alignment goal are **not complete**. Exact clean source commit:
`92033b910cc9ddd996c416ec4024af4625e78b31`, parent
`bffed177fb6dab5b2b9afe973028b04710d93331`.

The full frozen denominator remains
`3d735745f1117d42a7859e87604a106351dca935`. Before this source commit, the live
Windows branch still pointed at `6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4`;
its backward move is not permission to lower the denominator or rewrite Windows.

## Root cause and scope

The actual Java receive whitelist omitted `GameShopInfo` / `GameShopStock`,
and Android HostState had no corresponding producer. The shared runtime and
GameShop read model already supported both messages and pre-catalog stock.

Exactly six source files change: shared `game_shop.rs`; Android
`game_shop_ingress.rs`, `lib.rs`, `shared_shell.rs`; Java
`GatewaySession.java` and `GatewaySessionTest.java`.
The two packet projections and four scalar helpers preserve frozen Windows
function-body tokens, ignoring only whitespace outside string literals.
Their current Windows bodies also matched the frozen bodies before extraction.
This is pure read-model reuse, not a new transaction/server rule implementation.

The Java transport forwards only these two public metadata packets during an
authenticated listed-character Start/accepted-world lifetime, under its existing
16KiB UTF-8 boundary. Before authoritative owner binding, Android stages metadata
without bootstrapping a player or freezing an unbootstrapped tooltip viewer.
It accepts only the exact self ID/name/map snapshot, preserves personal metadata
across same-character map changes, and clears it on rejected Start/terminal reset.
An unexpected owner swap without personal reset fails closed.

FIFO admission is bounded to 1024 messages and 4MiB of retained serialized source
plus projected JSON; each packet is at most16KiB and projected row at most64KiB.
These are serialized-data bounds, not a measured process-memory figure.
Backpressure retains the exact front and later messages for retry. Unsupported
NPC/admin/purchase/receipt domains are not promoted to catalog packets.

Catalog and stock never grant purchases, deduct wallet balances, deliver Mail or
release an exact pending purchase. The legacy shared GameShop rules before the
new projection block remain byte-identical to the parent. The runtime consumer,
native critical/receipt queues, Windows host and renderer are unchanged.

[Six-file/Windows projection binding](raw/source-and-projection-bindings.json),
[committed source binding](raw/source-commit-binding.json), and
[boundary/original-checkout audit](raw/final-source-boundary-audit.json).

## Failure-first and exact committed-source gates

Two Java regressions compiled and failed against the unchanged producer:
missing packet-first forwarding and missing oversized-metadata disconnection.
Exact red test and gateway source, XML and output are retained. After the fix,
those two and the accepted-owner map-transition regression pass.

Final gates are separately source-bound at command start/end:

| Gate | Result |
| --- | --- |
| Android normal Rust | 343 passed |
| Android diagnostic Rust | 368 passed |
| Shared native-player UI | 1257 passed + 10 existing ignored |
| Runtime | 292 passed + 1 existing ignored |
| Forced fresh Java Debug / UiPreview | 44 passed per variant, five test classes each |
| Actual arm64-v8a / API31 check | Both variants pass |

The new ingress tests use a **105-row + 105-stock synthetic source-shaped
fixture**, not a newly fetched/live catalog. They cover stock-before-info,
all rows, field aliases/types, owner/Hero/context, snapshot atomicity, tooltip
viewer, personal/scene reset, count/byte bounds and exact FIFO retry.
Host tests exercise the real receive/bind/native-queue producer and terminal
reset systems; they do not run Android JNI or render a shop. The existing
`native_mail_shop_storage_payloads_update_preserve_reject_and_reset` runtime
test also verifies105-row accumulation and stock through its actual consumer.
The exact pending-receipt runtime test remains green.

[Initial compiled failures](raw/java-red-debug.xml),
[red output](raw/java-red-debug.log),
[red test source](raw/java-red-test-source.java.txt),
[committed Android normal](raw/bound-rust-normal.log),
[shared gate](raw/bound-shared-native-player.log),
[runtime gate](raw/bound-runtime.log), and
[final Java output](raw/bound-java-full.log).

The first API31 helper invocation failed before compiling because it ran from
the repository root without a Cargo.toml. Its original result/output is preserved
and classified as **check preparation**, not a product regression. Running from
the actual Android crate passes for both variants. Existing compiler warnings
and ignored tests are not suppressed or relabeled as newly passed coverage.

## Open acceptance and publication

No new APK was built or installed in this leaf. No screenshots, live account,
actual Java-to-JNI transport, server catalog/price/image parity, purchase,
Credit/Mail settlement, Zone/save or physical-device loop are accepted.
The previous v30 APKs and emulator evidence belong to their original
`2c9ad6f96` source; they do **not** contain this source leaf.

The two actual renderer files still byte-match published `277ab0a65`.
No GPU fix or new render measurement is made. The
[prior v30 renderer failure](../native-android-npc-quests-20261002/README.md)
remains FAIL; its38 GL0x506 entries are historical, not a current measurement.
Only an API31 emulator is attached, not a physical phone.

Before publication, remote Android and Draft PR253 were still at
`2c9ad6f96ebc47e07056abbf5a6117c278836083`; the earlier v30 evidence commit
`bffed177f` remained local after large-transfer failures. This new source is a
descendant of that commit. A local commit or upload attempt is not proof of
remote publication. [Pre-publication refs](raw/remote-branches-before.txt) and
[Draft PR snapshot](raw/pr-before.txt) are time-bound read-only facts.

Full phone UI, remaining NI-13–20, approved real login/HTTPS/WSS/JNI, player
actions, shared Zone/save, complete resources, audio/updater and device/human
acceptance remain OPEN. Full NI-12 stays PARTIAL and the whole goal stays Active.
Next source leaf: NI-13 authoritative storage contents/lock/expansion model;
exact package/JNI/online validation and pending publication remain required too.

Original checkout comparison covers Git HEAD/branch/status only, not recursive
hashes of untracked content. No original checkout, Windows, production, real save,
authentication, signing key or global network configuration is changed.

## Durable bytes

`manifest.json` binds all original `raw/` payloads and `diagnosis.json`.
`raw/** -text` preserves binary/CRLF bytes in Git. Verify from repository root:

```sh
node mir2-web3/docs/generated/player-qa/native-android-game-shop-ingress-20261002/verify-evidence.mjs working
node mir2-web3/docs/generated/player-qa/native-android-game-shop-ingress-20261002/verify-evidence.mjs index
node mir2-web3/docs/generated/player-qa/native-android-game-shop-ingress-20261002/verify-evidence.mjs HEAD
```
