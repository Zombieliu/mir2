# WN-WEB-PARITY-01 — 12-hour execution checklist

Started: 2026-08-21
Baseline branch/commit: `main` / `119553ff6aabbe05e7bcb4ee977a5470b477a250`
Scope: advance the Windows native client toward current Web functional
completeness. Automated evidence is required for every checked item.

## Safety and truth rules

- [x] Preserve the running Gateway (`PID 11856` at start); never stop it merely
  to work around a build/test lock.
- [x] Preserve the dirty worktree and all unrelated user/agent changes.
- [x] Keep one writer per high-conflict file (`routing.rs`, `overlays.rs`,
  native runtime `lib.rs`).
- [x] Do not use `qa.*`, `event.spawn`, debug teleport or a demo fallback as
  proof of an ordinary-player flow.
- [x] Do not enable authoritative content that says `Enabled=False`.
- [x] Do not call screenshot/model review, mouse automation or computer control
  during the initial non-visual rounds.
- [x] Do not label model-generated, injected or simulated evidence as human
  acceptance.

## Baseline

- [x] Existing Windows Gateway recorded and protected.
- [x] Registry baseline: 127 stable controls / 256 instances / unhandled 0 /
  wrong destination 0 / placeholders 3 / explicit no-op 2 / accepted 0.
- [x] Previous green gates: ui-core 16, native-ui 182, Windows 164, Android host
  10, Big Map 7, shared Zone 177, Web typecheck.
- [x] Refresh all affected client gates after S2 (ui-core 21/21, native-ui
  208/208, runtime 149/149, Windows 168/168, Android host 10/10) and the S3
  focused server gates (Simulation GameShop 29/29, Gateway native GameShop
  6/6, typed Zone RPC compatibility 4/4, native resume 21/21).
- [x] Final moving-worktree closeout refresh: ui-core 30/30, client-bevy
  native-ui 254/254, runtime 166/166, Windows 237/237, Android 36/36,
  Simulation 1,283/1,283, shared Zone 189/189, and Gateway 529 passed /
  0 failed / 1 environment-gated ignored test.
- [x] Preserve at least enough disk for ordinary incremental builds; do not
  create another full isolated Cargo target while the drive is near capacity.

## Round A — Functional contract and runtime hardening

### A1 Native audio lifecycle

- [x] One-shot sound entities despawn after playback.
- [x] Repeated music off/on/re-initialization never leaves more than one loop
  entity (independent-review P1 repaired with an initialization guard).
- [x] 100 Options Apply actions keep sound entities bounded.
- [x] Volume clamp tests cover 0/1/50/100/out-of-range.
- [x] Missing/invalid WAV remains safe and does not consume unrelated effects
  (strict PCM/IEEE-float `fmt` validation preserves `Main -> Login2` fallback).
- [x] Windows package continues to require non-empty legal fallback WAVs.

### A2 Authoritative World Map configuration

- [x] Read `TeleportToNPCCost` from authoritative `Setup.ini` rather than
  treating a Rust literal as the runtime source of truth.
- [x] Support explicit file/server-root overrides and portable runtime
  discovery without compiled developer paths.
- [x] Missing, malformed, negative and excessive costs fail closed or use one
  documented safe fallback.
- [x] Keep the current real `WorldMap.ini Enabled=False` result disabled.
- [x] Record source/provenance in diagnostics without leaking secrets.

### A3 UI typed functional contract

- [x] Every actionable registry entry maps to a typed action or an explicit
  source-verified no-op/visual record.
- [x] Quest, Options, Mail, Shop and Big Map cannot route to Inventory.
- [x] Dangerous confirmations cannot emit duplicate effects from one logical
  commit.
- [x] Windows and Android retain the same reducer semantics.
- [x] Regenerate the registry JSON after source changes (127 controls,
  generated 2026-08-21).

## Round B — Highest-value non-visual playability gaps

- [x] Audit all 127 controls against Windows handlers and Web behavior.
- [x] Convert the top three P1 findings into disjoint code slices.
- [x] Complete at least one end-to-end data panel slice:
  interaction -> typed action -> Gateway command -> server packet -> read model.
- [x] Add disabled/success/rejection/double-submit tests for S1/S2 operations;
  unchanged periodic model refresh is revision-only and no longer releases a
  pending operation; Drop/Move/Merge/Split use explicit inventory-operation
  ACK/NACK correlation.
- [x] Clear transient UI state across Logout/Login without retaining secrets;
  same-frame account-A reset/account-B first-snapshot ordering is covered.
- [x] Keep modal UI from emitting world actions at the typed input boundary;
  the table-driven Windows gate covers every registered world action while
  preserving modal actions such as NPC dialog selection and quest commit.

### B1 Session and inventory authority

- [x] Keep periodic snapshots revision-only; release pending work only from an
  exact ACK/NACK or an operation-specific old-to-new state transition.
- [x] Preserve item template `key` separately from authoritative instance
  `uniqueId`; Drop/Move/Merge/Split never derive an instance id from a name.
- [x] Keep NPC Shop sell/repair and Warehouse deposit/withdraw selections in
  independent state.
- [x] Apply a dedicated runtime SceneReset on `MapChanged` while preserving all
  personal read models and pending operations; runtime 152/152 and the focused
  MapChanged/reset gates pass.

### B2 GameShop authority and client closure

- [x] Add dedicated `gameShopBuy` BrowserCommand -> Crystal
  `ClientPacket::GameShopBuy`; normal clients do not need generic Stage5.
- [x] Revalidate quantity, product, class, currency mode, price, balance,
  attachment count and current stock policy on the server before mutation.
- [x] Deliver both Gold and Credit purchases through Gameshop Mail and split
  exact item state into no more than five StackSize-bounded attachments.
- [x] Fail the whole **real `ClientPacket::CollectParcel`** path without
  mutation when any exact attachment JSON is corrupt/invalid. Both dedicated
  `CollectParcel` and generic compatibility claim now use one authoritative,
  all-or-nothing core; the integration fixture uses real item templates and
  verifies server-assigned unique IDs.
- [x] Commit ordinary `CollectParcel` before emitting GainedGold/GainedItem or
  `ParcelCollected(1)`. Persist failure, bad JSON, full bag, reload, duplicate
  and concurrent claims preserve World/store/File and return failure only.
- [x] Give every new delivery an opaque persistent identity. Keep active local
  IDs stable, re-key only colliding incoming external mail, preserve repeated
  equal-content sends, keep refresh idempotent, and let active unlock state win.
- [x] Derive legacy compatibility identity only from mail ID and immutable
  headers. Ignore mutable flags and claim-cleared payload; same-ID/same-header
  ambiguity safely merges claimed state to prevent duplicate collection, while
  different legacy IDs remain distinct.
- [x] Refuse active-character persistence without authenticated account
  identity; never redirect that save to `demo`.
- [x] Make player-command safety default fail-closed with an unsafe opt-out
  based on the real TCP loopback peer, never a client-supplied proxy header;
  production and staging cannot opt out.
- [x] Use one strict parser for production debug-transfer detection and runtime
  execution; reject missing fields and extra `crystal:map:x:y:*` tail segments.
- [x] Prevent player-WebSocket `qaControl` from bypassing the safety boundary;
  allow it only under the same explicit real-loopback dev/test unsafe mode and
  configured token.
- [x] Require authenticated + active-character state for the real `SendMail`
  path; remove the legacy `demo` / `Scout` identity fallbacks, reject a
  client-forged `stamped=true`, and return deterministic `MailSent(-1)` with
  zero local mutation on validation failure.
- [x] Make cross-character and self `SendMail` atomic across recipient mailbox,
  sender currency and exact attachment debit. The runtime now stages both
  roles in an isolated account-store snapshot, persists File by atomic replace
  or the touched PostgreSQL accounts in one version-checked transaction, then
  commits the shared store/live World and success ACK. Injected
  fail-before-persist/fail-persist and successful reload tests pass. Live
  PostgreSQL execution remains unclaimed on this workstation because no DB was
  reachable; File+PostgreSQL mirror mode uses compensation and is not 2PC.
- [x] Web GameShop and Mail now use dedicated commands instead of generic
  Stage5 fallbacks; Web typecheck passes.
- [x] Remove the normal-player `Transfer controls` / `Quick Jump` debug UI and
  guard against it returning; authoritative portal traversal still has a
  separate internal compatibility path pending a dedicated server command.
- [x] Add a separate native `GameShopModel`; ingest all 105 server
  `GameShopInfo` rows and `GameShopStock`, expose Gold/Credit availability,
  quantity 1..99 and a dedicated purchase intent. Catalog and pre-catalog
  stock buffers are defensively bounded at 512 entries.
- [x] Prove native buy -> currency packet -> Gameshop Mail -> exact claim in an
  authenticated Axum WebSocket black-box ordinary-protocol integration test.
- [x] Persist and consume finite GameShop stock. Individual and global stock,
  currency debit, Gameshop Mail and character revision now commit in one
  durable transaction; StartGame reprojects the remaining stock. Simulation
  passed 1251/1251 plus all integration suites, and an independent transaction
  audit reported P0=0/P1=0. The current 105-row catalog remains unlimited and
  byte-identical; live cross-process PostgreSQL CAS remains a truthful P2 CI
  follow-up because PostgreSQL was unavailable on this workstation.
- [x] Prove the implemented request correlation and dedicated success/failure
  receipt over a real WebSocket connection before treating this wire gate as
  closed; the unchanged Crystal packet set itself only exposes
  currency/mail/chat state transitions.
- [x] Implement the client-only `nativeGameShopReceiptV1` half without
  changing Gateway/Simulation/Protocol: shared typed request/receipt contract,
  exact single-pending correlation, bounded critical runtime ingest, Windows
  capability/forwarding, and Android JSON/queue parity. DataReset marks a lost
  purchase unknown, SceneReset/resume preserves it, and no path auto-replays a
  purchase. The authenticated Axum WebSocket buy/claim proof below now closes
  this local wire gate.

Server producer and local WebSocket evidence recorded on 2026-08-21:

- The production-handler seam parses independent `nativeResumeV1` and
  `nativeGameShopReceiptV1` capabilities, requires one printable 1..64 byte
  `requestId` for opted-in purchases, and rejects a second in-flight request
  before Simulation execution.
- The in-process ordinary-protocol integration executes the typed world buy
  exactly once, separates normal Crystal currency/stock/chat packets before
  the receipt, uses the transaction's real `mailId`, then calls the real
  `ClientPacket::CollectParcel`; exact item quantity/unique ID, claimed reload,
  and duplicate-claim rejection pass.
- Receipt success/failure is derived only from `GameShopPurchaseOutcome`.
  Missing, mismatched, or invalid post-execution outcome is fail-unknown: zero
  receipt, socket close, no pending clear/replay, and no misleading
  `commitFailed`. Capability rejection before execution may return the
  definite `commitFailed` failure.
- Rolling RPC compatibility is explicit: generic non-opted-in GameShop buy
  reaches an old host without a capability probe and accepts an old Execution
  payload as `outcome=None`; the typed-required Gateway-to-Zone path probes the
  actual `nativeGameShopPurchaseV2` capability and sends zero Execute commands
  to an old host; a capable host round-trips the typed outcome.
- `nativeGameShopReceiptV1` and `nativeGameShopPurchaseV2` are intentionally
  different contracts: the former is the native-client-to-Gateway JSON receipt
  opt-in, while the latter gates the versioned Gateway-to-Zone authoritative
  purchase command and its single-endpoint execution rule. Hosts also advertise
  the older `typedGameShopOutcomeV1` compatibility marker for the optional
  `game_shop_purchase_outcome` field on an Execution payload; that legacy marker
  is not accepted as authority to execute `NativeGameShopPurchaseV2`.
- A real `tokio-tungstenite` client now upgrades against an ephemeral Axum
  `/ws`, negotiates `nativeGameShopReceiptV1`, registers, authenticates, creates
  a character, starts the game, buys through `gameShopBuy`, observes the exact
  currency/mail/receipt ordering, collects the receipt-addressed parcel, and
  proves the durable claimed state plus fresh delivered item identity. Gold is
  injected only into the isolated server-side account fixture before StartGame;
  no QA command or client authority bypass is used. The focused black-box test
  and the adjacent exactly-once reload test both pass 1/1. Live PostgreSQL,
  deployed remote Zone owner, and finite-stock wire evidence remain unclaimed;
  the current 105-row catalog intentionally remains unlimited.

At-most-once security closeout recorded on 2026-08-21:

- [x] Upgrade the opted-in native purchase to the versioned
  `NativeGameShopPurchaseV2` Zone operation. Gateway generates a cryptographic
  256-bit server idempotency key for the logical purchase and binds it to the
  trusted Gateway session, account, character and purchase tuple. The client's
  restart-reusable `gs-*` value is receipt correlation only; a new connection
  does not replay it automatically and a newly generated server key is a new
  logical operation.
- [x] Persist the exact typed outcome in the same authoritative character
  transaction as currency debit, finite stock and purchase mail. Repeating the
  exact server key returns the original outcome with zero ordinary mutation
  packets, zero second debit and zero second purchase mail. Reusing a key with
  a different tuple fails closed.
- [x] Keep one account/character-bound hidden ledger across Gateway sessions;
  never delete session A history when session B starts. The mutable hidden
  mail body has a ledger-specific union merge keyed by server idempotency key,
  while ordinary mail body semantics remain immutable. Metadata mismatch,
  duplicate-ledger ambiguity, same-key/different-request or same-key/different-
  outcome conflicts fail closed. Canonical key ordering makes stale A/B merge
  order deterministic.
- [x] Prove the concurrent/stale merge case: session A commits entry A,
  session B loads and commits B, stale A imports/merges B and attempts a stale
  save, reload retains both A and B, and both replay with zero debit/mail. The
  stale full save is revision-rejected rather than overwriting the durable
  store. A separate A -> B -> delayed duplicate A test preserves A after the
  session transition and emits no third mutation.
- [x] Bound the pragmatic hidden ledger at 4,096 entries without eviction.
  Entry 4,097 fails closed and the oldest key remains replayable. This closes
  double-spend integrity but is an explicit **P2 availability limit** until a
  dedicated durable ledger table plus a proven replay-retention policy exists.
- [x] Bind typed mutation execution to one V2-capable Zone endpoint. Health
  probing may select a fallback only before Execute. Once Execute is sent,
  response loss returns unknown commit state with `no endpoint fallback` and
  sends zero Execute requests to the fallback host. A rolling old host without
  V2 gets zero Execute; the V2 wire command also fails legacy decoding before
  runtime execution. Ordinary non-opted-in GameShop remains old-host compatible.
- [x] Treat `CommitFailed` after execution, missing/mismatched typed outcomes
  and response loss as unknown: zero receipt followed by `CloseUnknown`.
  `commitFailed` is serialized only for definite pre-execution capability
  rejection; definite business failures such as insufficient currency retain
  their normal typed receipt.

Closeout verification (same dirty-worktree round):

- Focused Simulation `native_game_shop`: 6 passed / 0 failed, including hidden
  ledger stale-session union, conflict fail-closed, session A -> B -> delayed A,
  durable duplicate and 4,096-entry capacity fail-closed.
- Focused Gateway `native_game_shop`: 6 passed / 0 failed, including the real
  handler seam buy -> typed receipt -> duplicate -> real `CollectParcel` ->
  reload path with one debit and one visible purchase mail.
- Focused typed Zone RPC: 4 passed / 0 failed; old-host compatibility/rejection:
  2 passed / 0 failed. Tests use real loopback TCP fake hosts and assert V2
  roundtrip, pre-Execute fallback only, commit-then-response-loss Execute 1/0,
  legacy decode rejection and old-host typed Execute 0.
- Full Simulation library snapshot: 1,267 passed / 0 failed in 248.84 s. A
  concurrent worker added one unrelated test afterward (the later focused run
  reports 1,262 filtered + 6 selected), so the full result is explicitly tied
  to its start snapshot rather than claimed for the moving worktree.
- The current Gateway library full gate is green: 529 passed / 0 failed /
  1 environment-gated ignored test. The final run used a temporary empty Cargo
  feature only to force a distinct Windows test-executable hash because an
  unrelated running game process held the previous executable open; the feature
  was removed immediately afterward. A fresh default-feature
  `cargo +1.95.0 check -p mir2-gateway` passes on the final source.
- `git diff --check` exits 0 (line-ending warnings only). Protected Gateway PID
  11856 remains the original 2026-08-20 process; port 7110 `/health` returns
  HTTP 200. It was not stopped or restarted.
- [x] Authenticated Axum WebSocket black-box purchase/claim passes locally as
  described above. No live PostgreSQL transaction, deployed remote Zone owner,
  process-crash response-loss recovery or real finite-stock WS run is claimed.

Sol XHigh rejection follow-up (2026-08-21, scoped rework):

- [x] The transport's generic `execute` and the session's generic command entry
  can no longer bypass Native V2 policy. Native purchase is automatically forced
  through `nativeGameShopPurchaseV2`, and raw common-call economic Execute is
  rejected before network I/O.
- [x] Ordinary non-opted-in `ClientPacket::GameShopBuy` is now classified as an
  economic mutation too. It remains old-host compatible (no capability probe),
  but is attempted on one selected endpoint only; transport/response failure
  after Execute is unknown and never falls back. Unrelated commands retain the
  previous endpoint-fallback behavior.
- [x] Pre-execution native receipts clear `pending` only after successful send.
  A failed send closes unknown, retains the exact pending request and cannot
  create an automatic replay opportunity.
- [x] The 4,097th operation now has full transaction evidence for Gold/global
  stock and Credit/individual stock: currency, both stock scopes, visible mail,
  ordinary packets and durable store remain unchanged, while the oldest ledger
  key remains replayable. Hidden ledger mail is absent from player mail reads and
  cannot be collected or deleted through ordinary commands.
- Final focused evidence on the stable source snapshot: Simulation
  `native_game_shop` 8/8; Gateway typed Zone RPC 12/12; Gateway native handler
  7/7; Gateway session generic-bypass regression 1/1. Gateway full was not run
  in that intermediate follow-up because another worker owned `routing.rs`, as
  directed. This historical note is superseded by the final 529/0/1 aggregate
  Gateway result below.
- The integrity findings addressed in this scoped pass are P0=0/P1=0 by local
  code/test evidence, but this is not a fresh independent audit acceptance.
  Remaining P2 is the 4,096-entry availability ceiling/pragmatic hidden-mail
  carrier plus absent live PostgreSQL, deployed remote-Zone and crash-recovery
  E2E evidence.

### B3 Android adapter truth boundary

- [x] Shared Android input uses the same typed reducer as Windows.
- [x] Register a bounded GameShop receipt inbound resource in the real Bevy
  Plugin Update chain. Exact receipts release both correlation owners;
  malformed/wrong receipts do not, and overflow/Destroy fail closed to unknown
  without replay. This is an adapter/JNI-host handoff only; a real Android
  WebSocket transport remains explicitly unimplemented.
- [x] Consume the current shared `GatewayCommand` set into a bounded, FIFO,
  drainable Android queue; local-only effects remain explicitly non-sendable.
  SendMail plus all Group/Guild/Trade commands now have exact BrowserCommand
  shapes, over-five-attachment mail fails closed, and the Android host gate is
  36/36.
- [ ] Do not call Android online-playable until a real WebSocket transport,
  APK and device/emulator evidence exist.

## Round C — Shared world and ordinary-player loop

- [x] Add Gateway checkpoint-failure coverage for Zone NPC teleport rollback,
  including AOI/occupancy state and no observer half-packets.
- [x] Keep exact replay idempotent on the current ordered WebSocket path.
- [x] Verify movement intent cannot overwrite a successful Zone teleport.
- [x] Exercise fresh account -> create -> StartGame -> movement -> NPC -> quest
  -> combat -> reward -> Logout -> relog through ordinary protocol paths.
- [x] Prove saved gold, inventory, quest state and authoritative transform
  across Logout and a new SimulationSession login using only ordinary packets.
- [x] Do not claim live teleport availability while authoritative targets are 0.

## Round D — Build, soak and Candidate evidence

- [x] client-bevy native-ui tests pass (254/254).
- [x] platform-windows tests pass (237/237).
- [x] ui-core, runtime and Android host tests pass (30/30, 166/166 and 36/36).
- [x] Full Simulation passes 1,283/1,283 and the shared Zone integration suite
  passes 189/189 on the final non-visual source snapshot.
- [x] Gateway full library gate passes 529/0 with one existing
  environment-gated test ignored; the temporary no-op feature used to avoid a
  Windows executable lock was removed, and the final default-feature Gateway
  check passes.
- [x] Web typecheck passes (`next typegen` + `tsc --noEmit`).
- [x] A fresh Web production build passed end-to-end on the current source
  snapshot (2026-08-22): dual WASM backends, 9,650-frame/7-page entity atlas,
  40,808-entry asset manifest, 58-page map atlas, TypeScript and 13/13 static
  pages. `test:bevy-runtime-budget` passes for runtime
  `bevy-1813be587ef98bc1`: WebGPU 27,119,641 raw / 5,902,117 gzip and WebGL2
  28,489,677 raw / 6,342,038 gzip. Next BUILD_ID is
  `OXQE2c59Nd1B4bxoWcPQf` (SHA-256
  `2B7EF9CDFFD6A652EEADF085F40AE4CBFFCE5AAC8FEB60DD8F84FBAC9E1173D0`).
  This is a non-visual production-build gate; browser visual/interaction
  acceptance remains deliberately open.
- [x] `git diff --check` passes (line-ending warnings only).
- [ ] `cargo fmt --all --check` is not green: both pinned Rust 1.89 and client
  Rust 1.95 propose a pre-existing, repository-wide reformat of large legacy
  files (`web.rs`, `runtime/tests.rs`, and others). Do not bulk-reformat the
  dirty worktree merely to change this checkbox; keep touched/new files locally
  formatted and report the repository gate separately.
- [ ] Release package verifies required assets/config/audio and launches from
  outside the repository without developer-only environment variables. The
  v4 package and verifier script self-tests pass (ADS/reparse/signature-contract
  fixtures), but formal staging is correctly blocked: `Cert:\CurrentUser\My`
  contains no certificate with both a private key and Code Signing EKU, and the
  historical internal-playtest `dist/mir2-windows-candidate` lacks the required
  `BUILD-ATTESTATION.json`, `PACKAGE-MANIFEST.json`,
  `RELEASE-STATEMENT.json` and detached `RELEASE-STATEMENT.p7s`. The old package
  is not promoted or relabelled as a signed Candidate.
- [~] 30-minute ordinary-client soak and bounded resource report. The Windows
  native-client half remains open because its proxy file is only 5 minutes.
  The separate pre-seeded 64-client Gateway half passed on 2026-08-22 under the
  strict `candidate-64-active-30m` profile: `ready/peak/startedGames 64/64/64`,
  `errors/capacityRejected/unexpectedReadyClosures 0/0/0`, per-client hold
  KeepAlive minimum `360/360`, and all PID/resource bounds green. Evidence:
  `docs/generated/load/isolated-ws-soak/soak-30m-64-active-release.json`, SHA-256
  `3AA049B3541D7B9A105D7E1BB7DEAF7E3ED3388E947B6E60A5AC0751832360DA`.
  Security limits were not weakened; this closes WN-CANDIDATE Closing 4b only.
- [ ] Aggregate independent read-only review reports P0=0 and P1=0 for every
  checked slice.
  Mail/status transactions, finite GameShop stock and secure native resume
  each have independent P0=0/P1=0 reviews. The GameShop client receipt half now
  also has a scoped P0=0/P1=0 self-audit with 686/686 client tests. The server
  producer/handler seam has a scoped P0=0/P1=0 self-audit, but the aggregate
  gate remains open for an independent reviewer; the local authenticated Axum
  WebSocket E2E now passes.

## Independent non-visual P1 audit

Read-only review on 2026-08-21 found no source-proven P0 and six Windows-native
P1 gaps. A backend feature or Web implementation is not evidence that the
native typed intent/command/read-model chain exists.

- [x] Cash GameShop catalog/stock/wallet/purchase chain is **Automated
  Verified** across the client receipt and authoritative server transaction,
  including bounded queues, at-most-once replay, and the authenticated local
  Axum WebSocket black-box proof above.
- [x] Automatic reconnect and same-character session restoration after a short
  transport loss is **Automated Verified**: the bounded 5-attempt/14-second
  state machine preserves data on transient loss, quarantines the fresh
  socket until `sessionResumed`, emits one SceneReset before the restored
  snapshot, emits one DataReset on terminal failure/cancel, and never logs or
  persists the credential. Independent review reports P0=0/P1=0 after covering
  same-process and remote-Gateway revocation windows, terminal credential
  expiry and authorization-revision exhaustion. Gateway focused resume tests
  pass 21/21, Gateway library passes 497/497 with one PostgreSQL-environment
  test ignored, and Windows tests pass 205/205. Live dual-Gateway Redis/
  PostgreSQL and cable-pull acceptance runs remain unclaimed P2 evidence.
- [x] Group, Guild and Trade native typed commands/read models are
  **Automated Verified** in `docs/generated/player-qa/native-social/WN-SOCIAL-01-REPORT.md`.
  Live authenticated protocol acceptance is intentionally not claimed; the
  current ordinary protocol has no sender-correlated TradeGold/TradeConfirm
  ACK, and native Guild notice text editing remains a follow-up.
- [x] Native SendMail has bounded recipient/text/gold/item selection, exact
  instance IDs, one in-flight operation and durable server feedback. The
  authenticated transaction and failure/reload/duplicate gates are automated;
  no human UI claim is implied.
- [x] Quest Abandon uses a dedicated ordinary command and persists the server
  result; independent focused review passed.
- [x] Learned-skill selection/casting uses the authoritative learned skill
  model and rejects missing/zero/other-player object IDs; the hard-coded-only
  F1 FireBall path is no longer the command source.

The older `100% Candidate` statements in broad roadmap history refer to their
named Web/Stage5 or package gate. They do not mean Windows-native functional
parity, and must not be used to close this checklist.

## Acceptance labels

- `Implemented`: source exists but may not have complete evidence.
- `Automated Verified`: focused and adjacent deterministic gates pass.
- `Protocol Verified`: ordinary authenticated packet flow passes.
- `Candidate`: P0/P1 clear for the named slice; human acceptance still open.
- `Accepted`: reserved for the user/human visual, audible and feel decision.

## End-of-run report

- [x] Record completed checkboxes and exact commands/results.
- [x] List unresolved P0/P1/P2 separately.
- [x] Record files changed by each worker and independent-review findings.
- [x] Update `AGENT-TASK-QUEUE.md`, roadmap/progress/parity docs for backend or
  frontend changes that actually passed.
- [x] Do not mark the whole WN-WEB-PARITY-01 goal complete merely because the
  12-hour window ends.

## 2026-08-21 final non-visual closeout

Automated results on the final source snapshot:

- `cargo +1.95.0 test -p mir2-simulation`: 1,283 passed / 0 failed.
- `cargo +1.95.0 test -p mir2-simulation --test shared_zone`: 189 passed /
  0 failed.
- `cargo +1.95.0 test -p mir2-gateway`: 529 passed / 0 failed / 1 ignored;
  see the Windows executable-lock qualification above.
- `cargo +1.95.0 test --manifest-path apps/game-client/ui-core/Cargo.toml`:
  30 passed / 0 failed.
- `cargo +1.95.0 test --manifest-path apps/game-client/runtime/Cargo.toml`:
  166 passed / 0 failed.
- `cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml
  --features native-ui`: 254 passed / 0 failed.
- `cargo +1.95.0 test --locked --manifest-path
  apps/game-client/platform-windows/Cargo.toml`: 231 passed / 0 failed after
  restoring the complete native Gateway bridge.
- `cargo +1.95.0 check --locked --manifest-path
  apps/game-client/platform-android/Cargo.toml`: exit 0. This is a host compile
  gate, not an Android device acceptance claim.
- `cargo +1.95.0 check -p mir2-gateway`, Web `typecheck`, and
  `git diff --check`: exit 0. The diff check reports line-ending warnings only.
- `cargo +1.95.0 fmt --all -- --check`: not green because it proposes a broad
  reformat across pre-existing dirty files. No bulk formatting was applied.
- The final Gateway rerun passed 529 library tests with one environment-gated
  ignore, all Home Tunnel tests 4/4, and the complete Zone RPC integration
  suite 29/29. Windows WAL recovery now opens a truncatable handle and skips
  Unix-only directory `fsync`; the partial-tail repair regression passes.
- Windows packaging-script hardening and its PowerShell/Pwsh self-tests pass,
  but no artifact is promoted because this checkout has no valid v2
  attestation/private signing identity for a truthful Candidate package.
- Protected Gateway PID 11856 retained its 2026-08-20 start time and returned
  HTTP 200 from `127.0.0.1:7110/health`; unrelated game PID 67220 was untouched.

Resolved automated P0/P1 slices: typed UI ownership, reconnect state-machine
safety, GameShop client correlation, GameShop durable at-most-once execution,
fresh embedded item identities, explicit monster disposition, shared-Zone
combat/PVP authority, and bounded Android host queues.

Worker/change ledger for this execution window:

- Shared UI/client workers changed `apps/game-client/ui-core/`,
  `apps/game-client/client-bevy/`, `apps/game-client/runtime/`,
  `apps/game-client/platform-windows/` and
  `apps/game-client/platform-android/` for typed controls, reconnect, receipt
  correlation and bounded host adapters.
- Server transaction/RPC workers changed `apps/simulation/`,
  `apps/gateway/` and `apps/gateway/tests/zone_rpc.rs` for durable GameShop,
  typed V2 execution and resume safety.
- Final authority integration changed `packages/game-data/src/lib.rs`,
  `packages/game-data/data/starter_server_data.json`, Simulation monster/item/
  stage5/shared-Zone paths and Gateway projection/PVP routing. Sol/Terra review
  agents diagnosed fixture, authoritative-position, projection and PVP routing
  defects; those review agents did not own unrelated high-conflict edits.
- The final Luna documentation reviewer was read-only. Broad roadmap history
  was preserved; only new top-level truth-boundary notes were added.

Still open before calling the whole goal Candidate:

- P1 evidence: real self-contained Windows package with valid v2
  attestation/private signing identity and outside-repository launch. The fresh
  Web production build gate closed on 2026-08-22.
- P2/environment evidence: live PostgreSQL transaction, deployed remote-Zone
  and crash-response-loss recovery, real Android transport/APK/device, and the
  still-open Windows native-client 30-minute soak. The separate local 64-client
  Gateway soak has passed and does not substitute for these environment gates.
- Human-only acceptance: visual, audio and feel comparison. This run
  intentionally performed no screenshots, mouse automation or computer use.
