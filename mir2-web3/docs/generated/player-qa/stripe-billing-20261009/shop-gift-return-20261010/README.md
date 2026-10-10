# BUY, Gift and billing return Candidate — 2026-10-10

This authorized project extension is separate from original Crystal parity.
The existing local price remains1000 Credits for a30-day monthly-card item.
Gift spends the payer's Credits and mails the item to the frozen existing
recipient character. Claim/use activates the recipient's account; purchase
or gift alone never starts the term. Public R22/Gatewaya41/feed17 are unchanged.

## Implemented player behavior

- GameShop opens with Credits selected; an explicit later Gold choice is retained.
  Unsupported currency or insufficient balance has visible localized feedback.
  The checkbox labels are clickable.
- Each product has GIFT. Enter a complete existing character name, then confirm
  recipient, quantity and Credits total. Name overflow is rejected rather than
  truncated. Native dialogs fence world input and use the existing IME/editor.
- BUY and GIFT share one transaction slot. Operation, recipient and all request
  fields must match the terminal receipt. An unknown transport outcome does not
  replay the purchase automatically or downgrade Gift into a self-purchase.
- The dark/gold browser return has nine locales, Arabic RTL, language preference,
  desktop and320px layouts. Query state is cosmetic; the trusted fixed script
  cannot call a payment/account API. Actual credit must be confirmed in game.

## Source and transport authority

Gateway binds a random256-bit server key to the authenticated payer, character,
request ID and complete gift tuple. Repeating the same request within that
connection preserves the original nonce; changed recipient/quantity is rejected.
The bounded1024-entry binding table never evicts an old key into a new purchase.
This connection cache is not a promise of cross-disconnect receipt recovery.

Source commits payer debit/checkpoint, recipient mail, finite stock and exact
monthly units together. Its permanent ledger freezes full request and recipient
identity; replay never resolves a renamed/reused recipient name again. Unknown
file/PG commits remain fenced. Known file failures roll back all effects.
Separate Gift capability is required on the exact existing hosted Source runtime.
An old purchase capability cannot authorize Gift. There is no fallback endpoint
after an economic write might have committed. All Source writers must be upgraded
before public Gift activation.

The positive monthly-price configuration also enables the authenticated repository
poll used by recipient mail refresh. Ordinary shared-Zone Tick notifies an online
recipient. Source snapshots remain lossless; player/spectator views omit both
internal purchase/gift ledgers and their account identities/replay keys.

## Executed bounded checks

Rust1.95.0, locked/offline. Independent executable outputs avoid an unrelated
DeltaForce sharing lock; its process was not closed.

| Check | Result | Log |
| --- | --- | --- |
| ui-core request/receipt/reducer |10 passed |[log](ui-core-game-shop.log) |
| Bevy native-ui shop/modal/editor/queue |41 passed |[log](bevy-game-shop.log) |
| Windows wire/shared transaction lane |8 passed |[log](windows-game-shop.log) |
| Source Gift plus existing monthly items |14+9 passed; actual PG1 ignored locally |[log](SOURCE-GIFT-MONTHLY-FINAL.txt) |
| Immutable gift-ledger merge/name boundaries |5 passed |[log](SOURCE-GIFT-LEDGER-TESTS.txt) |
| Gateway shop, actual dual WS and dedicated/private RPC |35 passed |[log](GATEWAY-GAME-SHOP-FINAL-03.txt) |
| Billing HTTP/WS/provider fixtures/CSP authority |39 passed |[log](GATEWAY-BILLING-FINAL.txt) |
| Generic Session Gift forces typed/capability path |1 passed |[log](GATEWAY-GIFT-SESSION-TESTS.txt) |
| Return-page browser locale/state/layout |27 desktop+27 narrow passed |[receipt](PREVIEW-RECEIPT.json) |
| Windows binary |build passes; prepared hash recorded |[build](WINDOWS-BUILD.txt), [binary](BINARY-RECEIPT.json) |
| Paired Gateway binary |ordinary binary build passes on F:; prepared hash recorded |[build](GATEWAY-BUILD-02.txt), [binary](BINARY-RECEIPT.json) |

Workspace target for final Gateway checks:
`C:/mir2-build/shop-gift-final-20261010-target`.
Client target:`C:/mir2-build/p1-native-20261007`.
Commands:
```text
cargo +1.95.0 test -p mir2-ui-core --lib game_shop --locked --offline
cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui --lib game_shop --locked --offline
cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml --bin mir2-platform-windows game_shop --locked --offline
cargo +1.95.0 test -p mir2-simulation --features test-support --test game_shop_gift --test billing_monthly_card --locked --offline
cargo +1.95.0 test -p mir2-simulation --lib game_shop_gift --locked --offline
cargo +1.95.0 test -p mir2-gateway --lib game_shop --locked --offline
cargo +1.95.0 test -p mir2-gateway --lib billing --locked --offline
cargo +1.95.0 test -p mir2-gateway --lib gateway_session_generic_gift --locked --offline
cargo +1.95.0 build --manifest-path apps/game-client/platform-windows/Cargo.toml --bin mir2-platform-windows --locked --offline
```

The dual WS story uses ordinary registration/password login/character creation,
two real shared-Zone sessions and prepared inactive fixture Credits. It observes
actual bootstrap mailbox/snapshot, ordinary Turn ack and autonomous recipient
Tick. It checks debit once, no sender gift parcel, frozen recipient UID, complete
receipt, repeat/same-ID changed-recipient rejection, claim/use/replay and normal
logout followed by new-socket password authentication. Payer/recipient balances,
one parcel and one30-day term survive. Signed identity is verified by the real
identity service. Connection/session/route counters drain normally.

Prepared fixture wallets are not earned-credit or payment-provider acceptance.
No new actual Stripe payment, live key, public callback or public release occurred.

## Retained failures and correction

- Initial Source compile/value-move failure and feature-disabled replay fixture
  are retained in SOURCE-GIFT-TESTS.txt/02.txt; corrected suite03 and final pass.
- GATEWAY-GIFT-TESTS-02.txt and GATEWAY-GAME-SHOP-FINAL-02.txt retain LNK1104.
  Restart Manager identified DeltaForce as the holder of a previous test exe.
  Independent output paths with reused library caches preserve its application.
- The first WS observation used KeepAlive as an ordering barrier; the socket
  reader may reply before remaining bootstrap mail. Actual mailbox/snapshot and
  business acknowledgements now establish observations without weaker assertions.
- The next WS run found a real replay bug: Gateway regenerated the server nonce.
  Gift now binds the nonce to payer/character/client ID/full tuple. The Source's
  full permanent tuple checks were retained; final socket replay succeeds once.
- Same-socket LogOut returns to character selection and retains authentication;
  it does not mint a new password identity grant. The relogin test now normally
  closes the logged-out socket and authenticates a new socket. Identity assertions
  are retained and signature-verified.
- bevy-game-shop-initial-failure.log retains two invalid old test fixtures;
  the new default Credits fixture and canonical request ID pass all41 checks.
- GATEWAY-MONTHLY-OWNER-TESTS.txt retains a local parallel compiler process failure.
  Its two-job rerun built auxiliary binaries and exhausted the C: build disk;
  no assertion in that additional suite ran. This is not counted as a passing
  check. The unchanged shared-owner suite remains required in isolated Linux CI.
- GATEWAY-BUILD.txt retains the resulting disk-space build failure. Automatic
  approval rejected cleanup of this round's generated build caches without a
  specific reason. Those caches were retained. The ordinary Gateway binary is
  built separately on F: with a private build-temporary directory;
  GATEWAY-BUILD-02.txt records the successful ordinary binary build. Compiler
  output logs retain their original whitespace and failure diagnostics.

## Release and remaining acceptance

A separate-name compatible native is prepared; original installed files are kept.
At the most recent process/port inspection the old local sandbox Gateway and
native were already absent; they were not forcibly stopped by this work.
Local paired startup, real PostgreSQL Gift CI and native human BUY/GIFT visual
acceptance remain open at this preparation checkpoint. Deployment evidence, if
performed, is recorded separately. The installer/public update feed is unchanged.

Manual route: normal login → GameShop All → monthly card1000 Credits → BUY →
confirm → mail claim → bag use. Separately GIFT → exact friend name → confirm →
recipient mail/claim/use. Check unsupported Gold, low balance, invalid name,
recipient full, cancel, normal logout/relogin and unknown connection loss.

