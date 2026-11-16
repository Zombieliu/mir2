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
| Expired shared owner, monthly item UID replay and current Zone pools |4 passed |[log](GATEWAY-MONTHLY-OWNER-DIRECT-FINAL.txt) |
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
  Cargo invocation. Its already-generated dedicated test binary was subsequently
  identified by its four test names and run directly with one test thread; all
  four passed without rebuilding auxiliary executables. The same shared-owner
  suite remains required in isolated Linux CI.
- GATEWAY-BUILD.txt retains the resulting disk-space build failure. Automatic
  approval rejected cleanup of this round's generated build caches without a
  specific reason. Those caches were retained. The ordinary Gateway binary is
  built separately on F: with a private build-temporary directory;
  GATEWAY-BUILD-02.txt records the successful ordinary binary build. Compiler
  output logs retain their original whitespace and failure diagnostics.

## Release and remaining acceptance

A separate-name compatible native and Gateway are running in the existing local
loopback sandbox. Original installed files are kept. Before startup the old local
Gateway and native were already absent; they were not forcibly stopped by this
work. Private settings/accounts were backed up. Personal account data and wallets
remain equal at startup; existing shared activity clocks advance normally.
Only the test listener webhook secret was refreshed; prices and security keys
were retained. Active binary hashes, health, a native connection, startup stages
and the exact HTML/CSP are recorded in [startup](LOCAL-STARTUP-RECEIPT.json).

Source commit9ccec1c214 is pushed and its adjacent monthly-access CI38041347418
passes. Gift CI38041347387 reached the actual PostgreSQL test but stalled; it was
cancelled without claiming a pass. [Original CI log](CI-GIFT-INITIAL-CANCELLED.txt)
is retained. The PostgreSQL fixture now releases its barrier even if bootstrap
panics, retains that failure, and emits non-sensitive phase diagnostics. Its CI
step has a180-second process bound and uncaptured diagnostics. All debit/mail/CAS,
unsaved-recipient state and activation/replay assertions are retained.

Diagnostic CI38042948920 surfaces a fixture bootstrap failure instead of hanging:
two prepared legacy passwords raced migration for one sender; one contender failed
before Gift. [Diagnostic log](CI-GIFT-DIAGNOSTIC-FAILURE.txt) is retained. The fixture
now migrates that credential once through ordinary password authentication before
opening the independent contender repositories. Each contender still uses normal
password login/StartGame, then the same barrier races the actual identical Gift
transactions. No production authentication path or monetary assertion is weakened.

Actual PostgreSQL Gift CI and native human BUY/GIFT visual acceptance remain open.
The Computer Use helper failed to initialize its kernel-assets path after one
reset/retry; no native shop screenshot or human acceptance is claimed. The local
test shortcut points to the compatible native. The installer/public update feed
is unchanged.

Manual route: normal login → GameShop All → monthly card1000 Credits → BUY →
confirm → mail claim → bag use. Separately GIFT → exact friend name → confirm →
recipient mail/claim/use. Check unsupported Gold, low balance, invalid name,
recipient full, cancel, normal logout/relogin and unknown connection loss.


## Concurrent PostgreSQL follow-up — recovery remains receipt-only

CI38043856064 completed both ordinary authenticated writer bootstraps, then one
actual Gift attempt returned a redacted error. [Failure](CI-GIFT-AUTH-SETUP-FINAL-FAILURE.txt)
is retained; it did not pass. Static review identified a reachable cache split:
a competitor may atomically commit between payer and recipient refresh reads.
The pre-persistence `gameShopGiftItemReceiptMissingOutcome` path now refreshes
the payer and accepts only its complete matching immutable GiftV1 receipt.
If absent or inconsistent, it still fails without writing a replacement. No
unknown-outcome retry is added. A known CAS retry now freezes the first resolved
recipient identity, preserving rename/name-reuse rejection.

[Source Gift15](SOURCE-GIFT-CAS-RECOVERY-01.txt) and
[Gateway shop35](GATEWAY-SHOP-CAS-RECOVERY-01.txt) pass. The new local negative
fixture retains a recipient unit while removing the payer receipt and requires
exact error, unchanged state, one debit and one parcel. The isolated PostgreSQL
fixture adds the same negative case after its existing competing-writer, online
recipient and activation assertions. CI diagnostics now print only fixed domain
labels, never arbitrary repository errors or credentials. Actual PostgreSQL CI
is still pending. A concurrency pass alone does not prove which recovery path
was exercised.

The user also exposed a native mail presentation bug: exact `itemStatesJson`
contains the monthly card but a later key-only snapshot clears its icon. Native
mail projection and the application-owned monthly-card metadata fix are in
progress. The active user session and its unclaimed parcel are preserved.

### Canonical Source contender setup

CI38047285888 failed with the safe label `owner-checkpoint-stale`; the previous
redacted error cannot be classified retrospectively. [Exact log](CI-GIFT-CAS-RECOVERY-01-FAILURE.txt)
is preserved. Read-only tracing identifies the normal StartGame forced save:
each successful startup advances the owner revision, so the second legitimate
startup supersedes the first contender before any Gift executes. This is the
correct stale-owner fence, not a reason to rebase a production purchase.

The PG fixture now completes both normal password logins/StartGames first,
reads one complete canonical payer checkpoint from its owned scoped Source,
and restores that complete checkpoint to both already authenticated fixture
runtimes through the existing trusted identity-checked restore API before
the barrier. It asserts payer identity, revision,1000 Credits and no Gift
receipt. The recipient's unsaved gold and transform are untouched. Independent
repositories, locks, Source versions, identical immutable request, debit/mail/
activation and stale-owner negative assertions remain. This prepares Source
contender checkpoints; it is not proof of real-player simultaneous same-account
login. [Local Gift15](SOURCE-GIFT-CANONICAL-SETUP-01.txt) passes; actual PG CI pending.

[Existing monthly9](SOURCE-MONTHLY-CAS-RECOVERY-01.txt),
[ledger5](SOURCE-GIFT-LEDGER-CAS-RECOVERY-01.txt) and
[Gateway build](GATEWAY-CAS-RECOVERY-BUILD-01.txt) pass after the production recovery
change. [Native mail92](NATIVE-MONTHLY-MAIL-BEVY-01.txt) and
[Windows projection8](NATIVE-MONTHLY-MAIL-WINDOWS-01.txt) pass for concrete
attachment projection, packet-to-snapshot retention, instance/count, app
monthly metadata,1813 asset rendering, hint and unknown-date placeholder.
The actual Source sentence localization is receiving an additional exact-template
check before the prepared native replaces the active user client.

## Final monthly-mail presentation Candidate

The user-provided native screenshot shows an unclaimed Gameshop monthly parcel
with a blank green attachment square and missing date. Read-only validation of
the existing sandbox save confirms one exact `itemStatesJson` monthly attachment;
no attachment was lost. Windows projection previously ignored that array and
the next snapshot erased packet metadata. It now prefers the concrete carriers,
preserves identity/count, rejects malformed concrete carriers and resolves the
reserved application monthly index through its server template. This also
provides the claimed bag item's icon and tooltip. Unknown imported item identity
still cannot guess an arbitrary catalogue row. No user parcel was claimed or
changed, and no payment/gift was performed on the user's behalf.

Native mail reader/list render the monthly1813 icon and activation-on-use hint;
unknown DateTime displays a localized 'Not recorded' placeholder without
manufacturing a timestamp. The actual Source purchase and gift body templates
are localized in all nine supported languages only for Gameshop and an exact
reserved monthly attachment with matching quantity. Player names and unrelated
free text remain intact. Existing claim/lock and authority gates are preserved.

[Final Bevy mail94](NATIVE-MONTHLY-MAIL-BEVY-02.txt),
[Windows projection8](NATIVE-MONTHLY-MAIL-WINDOWS-01.txt) and
[final Windows build](NATIVE-MONTHLY-MAIL-BUILD-02.txt) pass. These are automated
projection/assets/rendering checks, not a screenshot of the human native client.

Actual isolated PostgreSQL CI[38050349344](https://github.com/Zombieliu/mir2/actions/runs/38050349344)
at source1429e70e9032689327c345e884c5ca4d528d0396 passes recharge26/monthly9,
billing PG1, Gift15 plus actual PG1, dedicated Gift RPC9, Gateway billing39 and
shared-owner4. Its [complete retained log](CI-GIFT-CANONICAL-SETUP-PASS.txt) includes
both independent Gift outcomes, exact debit/one parcel, online recipient tick,
unsaved gold/transform save, activation replay and the missing-payer-receipt
negative case. All previous failed/hanging diagnostics remain preserved.

The paired local candidates have now been started after the user completed a
normal logout and closed the old client. All six Gateway admission counters
were zero before the switch. The latest private save and settings were backed
up; the complete saved accounts object was identical after the old Gateway
stopped and again after the new Gateway started, before launching the client.
The old tool session's stdin was closed, so its operator shutdown command did
not reach the server. Only the drained sandbox Gateway process was stopped;
its publication marker was empty afterward. This was not a graceful stdin
shutdown. No freeze marker, lease or account record was manually cleared.

New native289764 runs the final8aafbb60db binary; new Gateway285912 runs the
92912f25c1 binary. The existing test Stripe listener, webhook configuration,
client TOML, isolated profile and read-only assets were retained. The sandbox
shortcut now points to the new client. Startup reaches first_main_update;
Gateway health is true, one client socket is connected and zero characters
are active at the recorded observation. The return page bytes and its security
headers match the tested version. No agent purchase, claim, gift, payment or
monthly activation was made.
[Actual switch and startup receipt](MAIL-LOCAL-SWITCH-RECEIPT.json) records the
new process/binary identities, drain and preservation checks. The prepared
receipt and older startup records remain historical evidence.
No public installer, feed pointer, Gateway service or R22 release was changed.
Native human acceptance and public commercial rollout remain open.

[Prepared binary identities](MAIL-CANDIDATE-PREPARED.json) bind native8aafbb60db
to its final tested projection/rendering code and Gateway92912f25c1 to the
verified production Source recovery. Only test/docs/client files differ between
that Gateway source and passing backend CI1429e70e90. Its runtime/Gateway/game-data/
protocol source diff is empty. Native configuration hash is retained. Human switch
authorization was received, but the first preflight still showed the old native
process and1 active/1 WebSocket; no active character was forcibly closed. Actual
normal exit is required before the six admission counters permit the switch.
