# Shared Hero identity and sealed-item custody

Status: source-reviewed design, not implemented or visually accepted. This is required before claiming ordinary NewHero/SealHero/SealedHero acquisition matches Crystal.

## Source behavior

Crystal PlayerObject.NewHero (9624 onward) validates the request through Envir.CanCreateHero, creates a globally indexed HeroInfo, then gives the configured SealedHero item with AddedStats[Hero] equal to the global index. With the current real item catalog, item 1349 exists, so successful creation does not directly spawn the actor. Success is NewHero result 10. Only a missing configured seal item follows AddHero directly. The NPC HeroCreate entry checks level 22; production must retain server-side permission and eligibility checks rather than reproduce the source's missing NewHero permission guard.

PlayerObject.SealHero (14616 onward) removes the attached hero and produces another indexed seal, with a configured maximum of five seals. AddHero consumes a globally looked-up identity; current configured maximum is one attached hero. The source checks character names, not uniqueness among all Hero names; global identity must not be derived from a display name.

The current actual item 1349 has Bind=28703 (0x701f): no death drop, drop, sale, storage, trade, rental, disassembly or mail. Default-source acceptance must verify those denials; it must not unlock them to demonstrate a transfer. A separately permitted template/fixture can exercise generic transferable-seal custody, which transfers the seal identity rather than copying Hero state.

## Authority model

- Persist a shared registry keyed by a non-reused positive Int32 Hero ID, with CAS revision, identity, full owned inventory/equipment carriers, capacity, current vitals, learned magic, experience and seal count.
- Track exactly one custody state: attached to a stable account/character/slot, sealed to one exact seal carrier UID, or released/tombstoned. Moving a sealed carrier between ordinary inventories/trade/storage does not duplicate or reset registry state.
- Treat current per-character Hero fields as a compatibility projection. They must never be independently authoritative while a shared identity exists. A save updates the registry only with a matching attached owner and expected revision.
- Legacy character-owned Hero saves can be migrated once inside the same transaction that assigns the global ID and attachment. Preserve full carriers and the documented legacy capacity policy. Do not merge by name or import a second authority for an already mapped character. Ambiguous imported ownership must fail validation with original bytes preserved.
- Seal binding is checked using the actual held item type, Hero stat, exact carrier UID and registry custody/revision. An arbitrary stat value in an unrelated item cannot mint or claim a Hero. Duplicate seals and active-plus-sealed conflicts are rejected during load and before mutation.

## Transaction boundary

Use exact account plus Hero-ID transaction scopes and ordered CAS locks, analogous to shared guild records. Do not authorize unrelated market/global changes merely to write one Hero. FullRestore must validate all references and tombstones and cannot reissue consumed seals. File, PostgreSQL source, and Mirror publication/failure rules must provide the same authority; a second post-account write is not atomic acquisition.

Create commits ID allocation, fresh full Hero state, seal creation and source character inventory together. Seal commits removal of attachment, current Hero save, incremented seal count and fresh seal carrier together. Acquire commits seal consumption, attachment and full Hero state together. Release removes the attachment and tombstones its registry state; it must not reset ID allocation. Failure leaves both inventories and registry unchanged; an unknown commit result freezes the relevant mutation until reconciled rather than guessing a compensation.

Trade of a sealed carrier uses the existing exact-carrier trade transaction. The registry stays bound to that carrier while sealed; acquire validates the recipient's actual custody. Registry binding must be part of the domain validation for imports, mailbox/global market/guild bank and nested carriers. Never hydrate a transferable Hero solely from a client's item payload.

## Required tests before enabling the ordinary flow

1. NPC permission, level/class/name/full-bag failures do not allocate a Hero or consume/create items; success 10 creates one seal and no actor.
2. Acquire once, duplicate/replayed seal rejection, complete equipment/magic/vitals preservation, logout/relogin and empty/full Hero capacity checks.
3. Seal/acquire cycles retain identity and state, increment only successful seals, reject the configured limit without losing the attached Hero.
4. The actual source seal rejects trade/storage/mail/drop without changing custody. With an explicitly permitted test template, two clients transfer a seal and the recipient acquires the same Hero; the previous owner cannot acquire or save it. Include duplicate old-session saves and two concurrent acquisitions. Never alter the production seal bindings for this fixture.
5. Inject File rename failure, PostgreSQL rollback/CAS conflict and Mirror publication failure at each boundary; assert asset and owner conservation. Test restart/replay with exact stable receipts.
6. Legacy migration and FullRestore cannot duplicate a live Hero, revive consumed seals or reuse tombstoned IDs.

The existing independent Hero inventory/equipment/stat work is a prerequisite, not a substitute for this registry. Paired client screenshots and ordinary create/seal/trade/acquire interaction remain open.


Source type/lifecycle refinement: Hero experience is Int64; the existing Stage5 UInt32 projection must not truncate and overwrite registry experience. HeroInfo saves magic cast state, so its time domain requires explicit restart handling rather than persisting a process-local Instant. HeroInfo overrides serialization and does not persist Buffs: independent Hero buffs must follow source in-memory summon/despawn/death rules and must not be copied into the registry as player buff state. New ID allocation happens only after complete legacy carrier validation and never rolls back into reusing a published identity.

Further source read of HeroInfo.Load (73–76) confirms that each loaded magic's CastTime is explicitly reset to int.MinValue. Persist spell/level/key/experience, reset cast deadlines on actual restart, and retain original monotonic deadlines only for same-process transaction rollback.

### Persistent carrier scan inventory

The complete scanner must distinguish real carriers from references, projections and history:

- `CharacterSaveRecord.inventory_items_json`, `belt_items_json`, `storage_items_json`: full ItemState roots; inspect recursive socket carriers without renumbering any UID.
- `equipment_items_json`: EquipmentState roots, with retained `user_item_unique_id` and `user_item_metadata`; use the existing checked equipment-to-carrier conversion rather than treating the equipment slot as a UID.
- `npc_buy_back_items_json`: NpcBuyBackState records, whose `items[].item` are UserItem carriers. `npc_used_goods_items_json`: NpcUsedGoodsState, `items[]` UserItems. Both have recursive UserItem slots and already have checked save decoders.
- `stage5_systems_json.auction`: `item_state_json` is a live carrier only while not sold; sold rows are history. Refine has one `oven_item_state_json` plus `item_states` by slot, paired with their key fields. Reuse existing key/carrier consistency checks.
- Mail `item_states_json` contains retained ItemStates while attachments are unclaimed; `items` alone contains legacy names, not sufficient identity. Claimed/deleted semantics must follow the actual claim/delete implementation, with historical retained carriers excluded from active ownership.
- Trade `offered_slots`/`offered_unique_ids` refer to the live source inventory; do not count a second physical carrier. Rental `item_rental_records_json` is only an item ID/name/borrower/return-date record; active rented items remain in a real inventory/equipment carrier. Active rental `deposited_item` is transient, outside AccountStore, and needs its own transactional boundary if a permitted seal type ever enters it.
- SharedGuildRecord storage holds `SharedGuildStoredItem.item_state_json`; legacy per-character guild storage is a compatibility projection and must not be blindly counted as another canonical copy.
- Character `hero_inventory_items_json`/`hero_equipment_items_json` are compatibility mirrors once a real registry attachment exists. Compare the exact mirror to the attached record; count registry Attached/Sealed carriers once. Released records are historical tombstones and cannot recreate items.

AccountStore does not contain the shared ground-drop or economy-outbox authority. A generic transferable/drop-enabled seal would require those checkpoint/receipt boundaries too; the default SealedHero bindings forbid entering those paths. A whole-store scan cannot by itself prove conservation across an external outbox. Unknown or conflicting carrier ownership must reject with original bytes retained, not silently deduplicate by matching UID.

### Legacy zero-root transfer compatibility (2026-09-10)

Only a legacy slot-zero ItemState without a retained UserItem sidecar and without socket children can receive a fresh nonzero root identity when first transferred into Hero custody. This is a one-time migration; exact wire identities are otherwise preserved. An exact zero UID or a zero root with nested identity is rejected rather than renumbered. Allocation, carrier conversion, capacity checks and custody publication are staged together; a rejected carrier leaves both inventories and reserved identities unchanged. Logout/login retains the successfully migrated identity. Three dedicated regressions passed within the 56/56 Hero suite (`C:/mir2-build/hero-after-clock-fencing-tests.log`). The failure test exercises invalid stack carrier validation after staged allocation, not a weight rejection. This does not validate global sealed-Hero acquisition or visual acceptance.
