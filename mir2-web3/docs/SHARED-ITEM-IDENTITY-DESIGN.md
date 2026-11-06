# Shared item identity prerequisite for Hero custody

Status: source audit and proposed implementation contract, 2026-09-10. This document does not enable an allocator, migrate live saves, or establish production Hero grants. The registry containment scanner is not a reason to reject every unrelated legacy item with a duplicated UID.

## Source and current boundary

Crystal `Server/MirEnvir/Envir.cs` declares the world-wide `ulong NextUserItemID` at line 127, saves it at 2592 and restores it at 2981. `CreateFreshItem` (4396) and `CreateDropItem` (4413) both increment it. `CreateShopItem` (4432) instead accepts a display ID; catalog/shop preview IDs must not be counted as minted physical assets.

Our `runtime/inventory.rs::allocate_item_unique_id_avoiding` (1268) chooses a container/slot default or a maximum from one personal inventory and its reservations. This is not a shared allocator. The ordinary starter inventory contains unresolved UID zero, while equipment can retain a missing wire UID and derive a display alias. Existing characters may therefore legitimately contain identical ordinary numeric UIDs. Rejecting them globally or silently changing retained `UserItem` identities is not an acceptable migration.

A Hero-only reserved high-bit formula is insufficient: an unchanged personal allocator can eventually select the same number. Neither an in-memory counter copied into a session checkpoint nor an untracked reserved range proves non-reuse across rollback, fork, disconnect and multiple gateway processes.

## Minimum mint and transfer surface

Paths below are implementation locations, not claims that every legacy/debug path is enabled for ordinary clients.

| Surface | Existing locations | Required treatment |
| --- | --- | --- |
| Common fresh roots | `inventory.rs::add_or_increment_item_with_random_metadata`, root construction around 2307 | Plan stack merges and new roots separately; merges preserve destination UID. Every newly created physical root uses the authority. |
| Common callers | `npc.rs` 1005; `npc_script.rs` 2396/3142; `quests.rs` 866/980/1038; `mining.rs` 411; `fishing.rs` 703; `items.rs` 2682; `onchain.rs` 201; `packets.rs` 5957 | Carry a trusted mint operation through their existing account/currency/quest settlement. Verify ordinary routing individually; do not expose debug commands. |
| Stage5 mail/shop/craft fallback | `stage5.rs` 721/3949/4983/5269 | Distinguish a blueprint reward from an already owned exact carrier. Never remint a custody transfer because its legacy fallback previously reconstructed by item key. |
| Split stacks | `inventory.rs` 3205/3245 | Preserve old root for remainder; mint exactly one new root per resulting new stack; no mint for rejected split. |
| Pet box | `intelligent_creatures.rs` 641 | Existing staged reward inventory must receive authoritative IDs within its single-consumption transaction. Preserve random instance stats. |
| New Hero seal | ordinary creation/sealing still pending | Mint seal UID and Hero record/attachment transition in one fixed-scope transaction, preserving source bind flags. No allocation from personal slot aliases. |
| Generated ground loot | `drops.rs::ground_drop_item_payload_from_resolved` | Currently clears all root/socket UIDs and emits `uid_assigned=false`; this is a blueprint, not an owned exact item. Source parity requires world creation to mint before publishing the persisted physical drop. |
| Shared pickup | `drops.rs` shared pickup; `inventory.rs::plan_exact_ground_drop_item` | Keep claim-ticket/commit settlement. An already minted drop transfers identity; an explicitly legacy blueprint may be materialized once under its durable claim receipt. The latter compatibility path is not source spawn-time parity. |
| Existing exact carriers | player drops, equipment, mail, auction, trade, rental, Guild bank, Hero inventory, nested sockets | Preserve their identity and full state. Reject an ambiguous destination collision without changing either carrier. |
| Existing rekey helpers | `inventory.rs` 2563/2628; `session.rs` 1657/2055; `rental.rs` 552/621; personal Guild fallback in `packets.rs` | Do not route new authoritative carriers through `normalize_incoming_item_tree_unique_ids`. These call sites need an explicit legacy-vs-minted custody rule; changing only the allocator leaves renumbering holes. |
| QA/admin | `stage5.rs` 5583; `gm_commands.rs` 558 | Keep ordinary-client authorization unchanged. If enabled for an authorized operator, actual created assets still use authority. |

Line numbers are audit references and can move with concurrent work. Nested `UserItem.slots` contain physical identities too; empty socket slots do not consume an ID. Bounds must match the existing carrier validator (depth 8, 256 nodes, 64 slots and 256 stats per node), before traversal or allocation.

## Proposed authority contract

1. Introduce a dedicated item identity domain, not permission to mutate the global shop. Store a checked `u64` high watermark, source version and durable operation receipts. PostgreSQL representation must preserve the full unsigned range (validated numeric/text representation, not signed BIGINT casts). Allocation is checked; exhaustion rejects without wrap or falling back to local IDs.
2. A server-created prepared item plan identifies stable source operation, affected accounts/Hero records, ordered new physical nodes, exact input carrier revisions and the account/currency changes. Clients cannot provide mint capabilities or provenance. Repeated identical source operations return the same committed result; altered payload under the same key rejects.
3. Reserve and apply IDs inside the same durable transaction as the business change. Publish inventory/ground packets only after known committed adoption. Known rollback publishes nothing; an unknown commit freezes/reconciles via its receipt rather than allocating another sequence. A committed-but-compensated allocation remains retired: its high watermark never decreases. Uncommitted candidates are not published identities.
4. Extend File and PG/Mirror plans with an exact item-allocator scope and source receipt. Preserve existing Guild/Hero/Account scope checks. Establish one universal lock order before implementation: clock, Guild IDs, item allocator, Hero allocator/IDs, Account IDs. All combined operations must acquire it consistently; do not insert an allocator lock after an Account lock in an existing transaction.
5. Preparation must be explicit and fallible. Do not hide database calls inside `allocate_item_unique_id(&InventoryResource)` or return provisional IDs that ordinary code immediately serializes. Refactor central gain/split planning first, then supply committed mint assignments at the outer existing persistence boundary. A synchronous simulation remains synchronous; the gateway owns transport/async coordination.
6. Track authoritative minted provenance in server-controlled durable state. A JSON field or client `uid_assigned=true` alone cannot certify ownership. Legacy aliases remain scoped legacy identities until an explicitly authorized conversion. Hero seals and registry-owned new assets must be certified, and cross-account receipt/physical custody must agree.

## Bootstrap and legacy policy

The initial high watermark must exceed every resolvable legacy physical UID in the complete authoritative data set, including nested items, live account storage, unclaimed mail, unsold auctions, Guild storage, active/Sealed Hero records and persisted external ground/outbox items. Trade/rental reference records and historical sold/claimed projections are not extra physical copies. Inventory projections mirrored by an explicit Hero attachment are counted once. Released Hero audit state is not a live asset root.

This census computes a ceiling and ambiguity report; it does not require old ordinary UIDs to be globally unique and does not rewrite them. Unknown ordinary templates can retain raw IDs. Exact zero, malformed nested carriers and ambiguous seal references cannot be promoted into registry authority by inference. If a legacy UID is `u64::MAX`, or external physical stores cannot be inventoried under a stable snapshot, bootstrap must stop with an actionable reason instead of inventing a safe range.

Bootstrap needs an exclusive, explicit maintenance/migration transaction with a versioned completion marker. An implicit startup scan while another server still mints locally is unsafe. Cutover must cover all production mint entrances atomically at the deployment boundary. Earlier phase tests can use isolated fixtures, but cannot certify live global uniqueness.

The previously implemented narrow Hero legacy-zero transfer exception does not grant global mint authority. It is limited to unresolved root-zero ItemState with no retained wire metadata or nested identities. Its eventual migration must use the same global authority; exact wire zero is rejected, not repaired silently.

## Bounded implementation sequence and write sets

1. Add independent typed identity plan/receipt and File/PG allocator modules, migration and domain tests. Root leads schema/lock-order review; Config remains one writer. No ordinary gates change yet.
2. Refactor common gain and split into fallible preparation plus assignment/application. Add an explicit source context at real account transaction boundaries; port pet box, NPC/quest rewards and other ordinary callers. Preserve metadata and rollback in each closure.
3. Integrate external Zone drop creation and durable outbox/claim materialization with the same authority. Audit transfer normalizers and certify exact carriers. Complete inventory bootstrap tooling and explicit deployment cutover tests.
4. Mint the ordinary SealedHero carrier with the registry mutation, then enable strict seal containment validation and actual attachment grants. Remove the interim conservative rejection of every seal inside Hero inventory only when the full bounded graph validator is active. Mirror compensation must update the explicit attachment cache revision as well as the registry record.
5. Connect authoritative grants to shared Zone Hero actors; personal Hero tests alone never enable shared casting or AI.

Proposed new files: `config_item_identity.rs`, `config_item_identity_postgres.rs`, isolated source/migration tests, `runtime/item_creation_plan.rs`, and explicit bootstrap tooling. Existing narrow edits necessarily include Config mutation plan/source versions, inventory gain/split, participating runtime transaction boundaries, and Zone/outbox mint settlement. This is broader than a one-line Hero UID fix.

## Acceptance tests required before enabling

- Two accounts and two independently connected PG clients mint concurrently: disjoint IDs; stale source version fails without account change.
- Existing unrelated duplicate ordinary legacy UIDs remain byte-identical and loadable; an ambiguous legacy seal cannot become a grant.
- Bootstrap includes a nested high UID and external ground/outbox high UID; an omitted/unavailable authority refuses completion; exhaustion never wraps.
- Stack-only merge allocates no new root; split preserves remainder; nested new roots allocate exactly once; capacity/weight/source refusal changes neither item state nor published identity.
- NPC reward, quest reward, shop/craft, mining/fishing, pet box and permitted admin creation all exercise the real common mint boundary; no personal allocator fallback remains on enabled production paths.
- Drop spawn assigns before appearance; disconnect/restart before pickup preserves UID. Pickup claim retry, duplicate receipt and failed persistence cannot duplicate or renumber the drop.
- Trade/mail/auction/rental/Hero transfer preserve a certified carrier and nested IDs; collision rejects atomically. Default SealedHero bind restrictions remain negative tests.
- Hero creation/sealing commits exact seal UID and registry state together; lost-COMMIT probe, process restart and Mirror compensation retain high watermark and prevent reuse.
- Same operation key plus different inputs rejects; ordinary client-supplied UID/provenance cannot mint or grant.
- Seal B inside Hero A is legal if externally rooted; A contains its own seal or a rootless A/B cycle rejects without save mutation. Legacy caches without exact attachment reference never masquerade as registry mirrors.

## Alternative: committed bounded reservations

Root authorized investigation of bounded range reservation after the initial design. This can replace per-business allocation transactions, while preserving the same global high watermark and all-entry cutover requirement. The source first CAS-commits a finite range. Only after that known receipt may a single non-Clone, non-serializable process authority hand out IDs. The cursor must live outside rollbackable World resources. Failed previews may waste IDs; aborted business operations never rewind the cursor. Process restart discards every unused prior range and reserves a new one. No receipt-replay API may recreate a cursor that was already handed out. A lost reservation COMMIT response must be reconciled or its entire possible range abandoned before obtaining another; it must never yield a cursor on an assumed result.

Reservations reduce source traffic, but do not make business settlement idempotent: an unknown reward/box/Hero commit still needs its exact business receipt and freeze/reconciliation. A retry must not award the same reward with a different newly minted UID. The earlier no-allocation-on-failed-business test becomes no-published-asset on failure; burning an already committed reservation is allowed. A capacity-only pure preview should request no IDs, while a materialized speculative preview is allowed to consume and discard IDs.

The independent, unregistered `apps/simulation/src/item_identity_reservation.rs` now models bounded arithmetic, receipt matching and a consuming cursor. It deliberately does not claim database receipt authenticity, bootstrap, source CAS concurrency, restart fencing or production caller integration. Its constructor is crate-private and must remain behind a trusted source adapter; trusted call sites must not manufacture receipts from saved/client fields. Four standalone Rust tests passed on Windows: aborted preview/restart burn, all-or-nothing bulk exhaustion, full-u64 boundary, and source receipt mismatch. Command: `rustup run 1.95.0 rustc --edition 2021 --test apps/simulation/src/item_identity_reservation.rs -o C:/mir2-build/item-identity-reservation-tests.exe`, then run that executable. No shared Cargo cache was used.

Existing registry/PG/domain tests establish their own bounded prerequisites and do not cover the global mint cutover described here.

## PostgreSQL reservation candidate evidence

`item_identity_postgres.rs`, its independent tests and migration candidate `0015_shared_item_identity.sql` are implemented but **not registered**. The table is seeded blocked: bootstrap false, high zero, version one. `NUMERIC(20,0)` plus explicit bounds covers the full unsigned UID range. Versions use the existing signed `i64` domain; reservation rejects the maximum version before increment. The controlled census capability has private fields, no serde and no production constructor. Thus ordinary startup, client JSON and a partial scan cannot activate this candidate.

Each reservation uses a separate source transaction with exact version, high watermark and bootstrap digest in its UPDATE predicate. It locks only the allocator row and returns a non-Clone cursor only after successful COMMIT. Its future caller must reserve before entering account/Guild/Hero business locks; it must not call this separate transaction while holding those locks. A source error before COMMIT returns no cursor. Any COMMIT error is conservatively unknown and returns no cursor. There is no API to reconstruct an old range from a receipt; refreshing source and committing a new range can abandon an uncertain one.

Four actual isolated PostgreSQL tests passed on 2026-09-10 (`C:/mir2-build/item-identity-postgres-tests.log`): bootstrap gate and Int64-exceeding integer precision; simultaneous two-connection CAS race with exactly one winning range; rollback and committed-response-loss with no returned cursor; full `u64::MAX` and `i64::MAX` exhaustion with source unchanged. The lost-response case commits to real PostgreSQL then uses a deterministic adapter fault to suppress success; it is not a real TCP-failure test. Each case creates and drops a unique schema in the explicit local test database. The updated four pure arithmetic tests also passed. Standalone rustc linked existing postgres dependencies and did not run or lock the shared Cargo cache.

Lead schema/lock-order review, actual production source registration, File/Mirror adapters, exclusive bootstrap scanner and all mint/transfer caller cutover remain pending. These tests do not change that activation gate.

## File/Mirror candidate interface and tests

Root approved an independent canonical `accounts-path.item-identity.json` authority. It is a required durable component, not a cache. Ordinary AccountStore restore/compensation does not rewrite it. Backups and operator restore must include it; missing or damaged authority blocks allocation rather than inventing zero. Explicit reinitialization cannot be inferred from the restored account JSON. The production census capability still has no constructor, so bootstrap is not enabled by this candidate.

New `config_item_identity.rs` is designed as a Config child. `load`, `reserve_file`, `reserve_mirror`, `bootstrap_file` and `bootstrap_mirror` acquire the canonical persist mutex and validate the live FileAuthority binding. The only new FileAuthority helper derives the fixed sidecar path after `owns` validates the current Config's path/store/locks. A detached/replay Config cannot borrow the original authority to mint. The sidecar uses a bounded strict JSON schema and decimal-string high watermark; read failure preserves existing bytes.

File reservation: exact source comparison, checked plan, durable publication marker, existing atomic file writer/sync, marker settlement, then cursor. Mirror: validate a non-regressing PG source with identical census digest, durable marker, strict PG reserve COMMIT, publish its new high/version to File, settle, then cursor. Any PG/file uncertainty or failed publication returns no cursor and preserves the freeze; no Mirror compensation lowers the PG high watermark. Discarding the local cursor burns its committed range. File-only allocation refuses configurations with a PG source so it cannot bypass the configured authority.

Mirror bootstrap only installs a missing sidecar from an already controlled, bootstrapped PG source matching the consumed census capability; it cannot activate PG itself or overwrite existing identity bytes. The adapter connects to the Config's validated database URL, not a client-supplied source. Test-only internals accept an isolated PG connection to avoid modifying the real configured schema.

Source files and tests are candidates, with production registration intentionally absent (Config mounts these modules only under `cfg(test)`). File tests cover missing/corrupt authority, non-overwriting bootstrap, stale snapshot/reopen retirement, failed publication with release/reopen freeze, and detached authority/database bypass. An isolated PG test covers normal Mirror allocation followed by committed PG/File failure without high-watermark compensation.

Actual crate evidence on 2026-09-10: `item_identity_` passed 29/29 (new File four, pure arithmetic four, existing carrier regressions 21; five PG tests ignored by that ordinary run). All five PG cases then ran explicitly and passed 5/5 in isolated schemas, including the new Mirror case. Existing FileAuthority tests passed 10/10, including its real child-process reopen checks; the new File adapter itself tests Arc release/reacquire. Root's separate shared-social-buff regressions passed 3/3 on the same executable. Logs: `C:/mir2-build/item-identity-file-tests.log`, `item-identity-mirror-postgres-tests.log`, `item-identity-fileauthority-regression.log`, and `item-identity-social-buff-regression.log` in that directory. No live migration, production registration, ordinary mint cutover or client visual acceptance is established by these tests.
