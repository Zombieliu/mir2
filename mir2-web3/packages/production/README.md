# Composable production rules

`mir2-production` is the synchronous first implementation of the [production design](../../docs/SLG-CRAFTING-DESIGN.zh-CN.md). It has no network loop, personal World copy, implicit inventory grants or gateway command route. The bundled catalog remains disabled and its symbolic material IDs are not real item templates.

Implemented code:

- Validated recipe graphs and canonical SHA256 catalog revisions; transitive raw sources, source gates, facilities and material bindings.
- Read-only batch plans that consume existing intermediate stock first and report raw deficits, full upstream batches, surplus, fees and sequential time.
- Input plans using actual nonzero unique instance IDs, exact template bindings, checked quantities and preserved custody carriers.
- Server-clock offline jobs with immutable recipes, one unresolved job per owner/facility, collection deadlines and direct-material cancellation refunds.
- Owner-scoped exact request replay, different-payload rejection, restore validation and bounded encoded checkpoints.

`ProductionContext` must be built by an authenticated server. Its owner uses a persisted, non-reusable character incarnation, not a character slot or display name. Access must authorize the exact current action at the exact facility. Inventory eligibility, material/template bindings and time are never client assertions.

`ProductionLedger::prepare` returns a proposal. Its receipt is not evidence of a successful payment or delivery. The caller must atomically commit **both** `next_ledger` and the economic changes, using the expected ledger and inventory revisions. A stale version, capacity failure or failed save leaves the old ledger and inventory intact. Unknown publication must freeze further mutations until the original request is recovered; do not invent another request ID to retry payment.

The normal inventory adapter still needs to:

1. Derive and validate each custody carrier against authoritative ItemState UID, template, count, binding and protected metadata. Preserve ore purity separately from stack count; exclude protected refining, upgraded, socketed, sealed, rented, equipped and held items.
2. Plan the entire output/refund capacity and weight before committing. Keep output in its job when delivery does not fit.
3. Allocate new output and split-refund UIDs through the existing durable `UserItemUidAllocator` with `Crafting` reason. A refund origin describes metadata provenance; its old UID is **not** a license to reuse a live partial stack's identity.
4. Persist the job, input/gold debit or output delivery and request receipt through the original account transaction. Update World and send successful game packets only after durable success.

Guild production is not implemented by this personal-owner ledger. It needs territory-owned assets, the shared warehouse and budget transaction, and separate start/cancel/withdraw permissions. Do not copy guild assets into the initiating Session.

The ledger refuses further state changes at 1,024 retained jobs, 4,096 receipts or a 16 MiB encoded checkpoint. Every unresolved job reserves one maximum-size terminal receipt before new work is accepted, so new jobs cannot consume the space required to collect or cancel existing work. These are safe bounds for this first slice, not lifetime limits suitable for a released economy. Persistent archival must preserve exact replay and asset custody before enabling long-lived gameplay; records must not be silently evicted.

Original NPC instantaneous crafting and weapon refinement retain their existing runtime paths. Gathering, planting, dedicated salt mining, new item templates, food effects, buildings, public workshop NPCs, client UI and ordinary gameplay acceptance remain subsequent work.
