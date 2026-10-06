# Native Android transaction-receipt checkpoint (2026-09-11)

## Scope

- Branch: `codex/android-player-journey`
- Implementation commit: `e66595d34`
- Android-only files in the isolated worktree; no Windows/backend change,
  deployment, account bypass, or real save mutation.

## Implemented

- `GatewaySession` now forwards only the existing authoritative transaction
  result shapes: `gameShopReceipt`, `StoreItemV2`, `TakeBackItemV2`,
  `ChangePassword`, and `ChangePasswordBanned`.
- The Java boundary limits a receipt to 16 KiB UTF-8. Unrelated packets are
  not copied into the transaction path.
- `MainActivity` delivers the selected envelope into the Bevy update thread,
  which classifies it again and uses the existing bounded exact-request
  GameShop, Storage, and password-result adapters.
- Pending request correlation, duplicate/unmatched quarantine, overflow
  handling, fail-closed unknown state, and no-replay behavior remain owned by
  the existing shared reducer bridge.

## Evidence

- Android Rust tests: 111 passed, 0 failed.
- Java TLS `GatewaySessionTest`: 10 passed, 0 skipped/failed/errors. The new
  socket test observes Storage and GameShop receipts and proves an unrelated
  `ObjectChat` packet is not routed as a transaction receipt.
- arm64 API 31 target check passed with Rust 1.95.0 and NDK 26.1.10909125.
- Normal debug APK package gate passed from implementation commit
  `e66595d34`: 330,078,293 bytes, SHA-256
  `412fe084900bdb88ca05555e0f98a9f026033e5ee5642c76c3f8f046c7c85a90`.
- Platform-only `cargo fmt --check` and `git diff --check` passed.

The TLS test is deterministic protocol evidence, not a real Gateway/account
acceptance. APK, assets, caches, logs and debug signing material remain local
and ignored.

## Remaining boundary

This closes the existing transaction-result return path only. Ordinary
authoritative movement, combat, entity, inventory, NPC, chat and map-update
packets still need a bounded packet projection into shared runtime models.
Approved WSS/account login through the full visible journey and physical
Android device acceptance remain unrun and open.
