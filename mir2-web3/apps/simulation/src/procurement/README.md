# Personal procurement V1: authoritative backend boundary

The simulation owns real game gold and complete equipment instances. The chain
owns the mutually exclusive order outcome. This requires a trusted game-server
custodian and a trusted finality verifier; a contract cannot inspect the game DB
or force an offline game server to deliver.

## Persistence and authority

`AccountRecord.procurement` is an optional account ledger in `accounts.raw_json`
(or the isolated development File store). Publication and delivery use the same
`commit_account_store_transaction` as the complete character save. They do not
debit a separate illustrative wallet. Only a current serial owner checkpoint may
remove gold or the exact bag item. A stale checkpoint fails closed. The live ECS
is updated after durable commit; unknown publication errors carry
`PROCUREMENT_COMMIT_OUTCOME_UNKNOWN` and require the gateway to retire the owner.

Order conditions commit the shared protocol's canonical BCS representation.
The target is original Crystal template **245 / DragonSword**, not a localized
name comparison. The original `ItemState` JSON, metadata, nested UIDs, durability
and added statistics survive custody and delivery. V1 refuses bound, rented,
equipped, expiring, sealed, shop-reference and otherwise locked items.

Delivery confirmation previews come from the eligible original inventory UID,
including actual durability, `refined_value` (`Ref`), `refine_added` (`Add`) and
the authoritative merged stat values. The single-line ASCII description is
bounded to 96 characters; long stat names use `S<stat_id>` and extra entries are
explicitly counted as `+Nmore`. This display limit never truncates or alters the
stored equipment instance or its numeric attribute values.

Account-store schema 12 validates the ledger before and after transactions.
Legitimate schema-11 orders and legacy quotes migrate forward; new optional quote
rule snapshots and default-empty quota quote/purchase collections preserve their
original formats. A legacy unconfirmed publication without a complete rule
snapshot must request a fresh quote, rather than silently accept a different fee.
Migration `0018_procurement_authority_guard` prevents an older Postgres writer
from deleting or omitting this authority or its economic history. The empty
existing billing ledger is a File compatibility fence: the old schema-10
`billing::validate_complete_store` rejects a future schema with `billing: Some`;
the same version predicate fences the previous schema-11 reader. Migration 0018
also preserves quota quotes and one-time grant receipts against older writers.
The migration is additive. Rollback means disabling **new** publication while
retaining the V1 recovery service and authority guard. Do not point an older
binary at V1 data. Removing historical authority requires an audited archive
migration after outstanding custody, delivery and fees are reconciled.

## State and recovery

- Publication reserves real gold, one quota unit and a durable publish job in
  one transaction. The Dubhe create phase is not public and does not consume the
  reserved unit. Confirmed activation changes the order to `Open` and consumes
  the reserved service unit once.
- A seller's explicitly signed, purpose-specific intent and original equipment
  are locked durably before an oracle proof is signed. Prepared signed Sui bytes
  and digest are committed before broadcast; uncertain submissions retain both
  the bytes and custody. Technical retries must rebroadcast those exact bytes.
  A trusted finalized failed-effects receipt may be recorded with
  `procurement_record_finalized_failure`. Only that digest may then be replaced
  with a newly prepared attempt for the same step. Its original envelope and
  failure checkpoint remain archived; failed effects alone never refund assets.
- An `Open` order owns a persistent `watch:<order_id>` job, allowing expiry and
  reconciliation while both players are offline. Watch polling leases can be
  released with delay without fabricating a chain event.
- A trusted finalized receipt atomically chooses `Filled`, `Cancelled` or
  `Expired`. Gold escrow becomes seller pending proceeds or buyer pending refund;
  the original item becomes buyer pending collection or seller pending return.
  The transaction writes receipts, deduplication and an optional cursor together.
- Owner-side collection moves those existing pending assets into the current
  wallet/bag in a transaction with the claim receipt. A full bag or a `u32` gold
  overflow retains the entire pending asset. UI must distinguish chain outcome,
  pending game delivery and completed game delivery.
- A local timeout never releases anything. An initialized order's terminal state
  invalidates competing proofs. An open order can release a losing proof only
  after a coherent finalized higher fence or a finalized chain Clock strictly
  beyond that proof's expiry, with the fresh order still open.
  The same rule retires an unsuccessful cancellation proof. A subsequent buyer
  confirmation creates a distinct `cancel:<order_id>:<intent_hash>` task, keeping
  every previous authorization and attempt. Cancellation proofs refer to the
  original publication's gold lock. Confirmation deadlines never outlive the
  order, and existing orders keep their frozen authorization rule.
- If creation never committed, the dedicated `ChainAbsenceReceipt` is required.
  It must bind the domain and order, Clock-first finalized observation, unchanged
  double registry read and registry previous-transaction effects. It is valid
  only for a Publishing order with no known object and after every issued publish
  proof/intent deadline. It refunds gold and releases reserved quota without
  inventing a cancellation or an object ID.

`procurement_apply_chain_receipt` and `procurement_apply_chain_absence` are trusted
backend seams, not player commands. The caller must verify coherent fullnode
effects, object/registry membership, the last mutation and finalized Clock before
constructing these records. An indexer row is insufficient. `ChainReceipt.checkpoint`
is the sampled finalized **Clock observation checkpoint**. Inbox identities bind
order, object transaction digest, event sequence and that checkpoint. Avoid writing
unchanged observations every poll; apply state/fence changes, proof expiry or a
created-order publication deadline, and otherwise release the watch lease.

Persist listener progress with `procurement_advance_cursor`; persist an applicable
asset receipt and its cursor atomically with
`procurement_apply_chain_receipt_with_cursor`. Reconciliation always rereads the
configured source. Postgres worker discovery and public-order lookup query the
source, rather than relying on one gateway's partial account cache. A development
File worker must share the gateway's process/authority; a second File writer is
deliberately refused.

## Charging and rollout

Quota batches distinguish `Test`, `Paid`, `Gift` and `Unknown`. Gift/unknown
batches cannot fund publication, and test batches require an explicit test-only
configuration. A rule snapshot fixes the quota cost and cancellation/expiry fee
policy for each order. Subsequent delivery, cancellation, expiry and retries do
not debit another publication unit. Rule upgrades or disabling new publication
must not block existing settlement, cancellation, refund or collection.

With explicit test credits and publication enabled, the authenticated NPC
`QuoteBuyQuota`/`Confirm` flow exposes a free Test package covering ten
publications at the frozen rule rate. It grants at most once per account/rule
version, including concurrent and restarted confirmations, and moves no gold,
points or cash. Full rules load through strict `MIR2_PROCUREMENT_RULE_JSON`;
publication and quota quotes freeze the complete rule before confirmation.

Production purchases remain closed. Existing game `credit` is a mixed balance;
it cannot prove that a spent point came from a real payment. No dollar conversion,
profitability, production charge or cash revenue is inferred from that balance.
Future paid quota issuance must reference the existing genuine recharge receipt
and an approved provenance policy. The game monthly card remains independent.

## Validation scope

The focused tests use deterministic signatures, mocked finalized chain records,
real isolated File transactions, deliberate commit faults and real serial-owner
wallet/inventory collection. They cover preservation, conservation, concurrency,
mutually exclusive outcomes, unknown commit fencing, crash recovery, full bags,
gold overflow and fixed SDK/Move/Rust BCS signatures. They are not real Sui
testnet transactions or PostgreSQL execution. This task does not start Docker,
deploy a production contract, generate production keys or activate real fees.
