# Level-9 NPC shop lifetime investigation — 2026-10-01

Status: the passive snapshot text-clear cause is reproduced; bounded production UI/ECS, final FIFO Exit, explicit NPCResponse, complete client regression and offscreen GPU checks pass. Earlier closing-boundary failures are retained. Clean 180b01d46 r9 installer and signed sequence5 are verified and published. Actual installed-client proof and human GUI acceptance have separate scopes: [r9 delivery](release-r9/README.md).

The user reports that, after choosing **View shop** at Alchemist Samuel at level 9, the shop appears for about half a second, then the inventory moves to the upper left and covers/replaces the goods. The two small user images and the reported time sequence are a bug report, not a measured animation capture. Static bounds alone cannot establish what happened between those moments.

## Ordinary cohort chronology

The sanitized [before evidence](cohort-r2-shop-lifecycle-before.json) contains only seven public events for each normal caster, with SHA256 hashes of the private raw traces. Complete traces and account/character/storage data are not copied.

| Cohort | Open NPC text | Select service | Goods / Sell | Passive text clear | Gap from Goods |
| --- | --- | --- | --- | --- | --- |
| Wizard, level 9 | snapshot 2954 | select `@BuySell` 2955 | `NPCGoods` 2956 / `NPCSell` 2957 | snapshot 2958, `activeNpcDialog: null` | 11 ms |
| Taoist, level 9 | snapshot 3276 | select `@BuySell` 3277 | `NPCGoods` 3278 / `NPCSell` 3279 | snapshot 3280, `activeNpcDialog: null` | 11 ms |

Both services contain eighteen potion rows. Neither selected interval contains an `NPCResponse` packet. This is an ordinary server service response followed by a world snapshot, not a client-created service fixture. The `activeNpcDialog: null` snapshot arrives only **11 ms** after Goods: it clears optional NPC text but is not an explicit Exit/Hide instruction. Treating it as one closes the goods and restores the bag to the upper left, matching the reported time sequence.

## Before-fix native source chain

The line references in this section describe the source at reproduction time; later lifetime and FIFO edits can move them.

`apps/game-client/platform-windows/src/gameplay_bridge.rs:3832` transforms an absent/null `activeNpcDialog` into a default closed `NpcDialogModel`; `drain_gameplay_events` replaces the current dialog with the latest snapshot at line 2118.

`apps/game-client/client-bevy/src/crystal_ui/overlays.rs:6546` watches only the parent dialog's `is_open` boolean. On `true → false`, it moves the bag to `(0,0)` (lines 6568–6569), turns `NpcShop` into ordinary `Inventory` (6577–6581), and closes the packet service mode (6594). A snapshot with no dialogue text therefore has the same UI effect as a genuine parent close.

The separate service-placement function at line 6603 cannot preserve a service already closed by that earlier transition. In a split-frame delivery the service first appears and is removed on the following snapshot; coalescing the service and empty-text snapshot in one update is a second scheduling case. The initial repair checks below cover both, while genuine close/reopen ordering needs its own final boundary gate.

The existing forty-two offscreen GPU caster shop checks cover production rendering/input with synthetic catalogs and static bounds, but their caster setup starts with `NpcDialogModel::default()` (`npc_shop_layout_tests.rs:195`). Their open/reopen sequence never first opens authoritative NPC text and then clears it. Thus those passing captures do not disprove this dynamic failure and must remain separate from ordinary journey evidence.

## Crystal reference and closing boundaries

Original local reference checkout: `E:/mir2/Crystal`.

- `Client/MirScenes/GameScene.cs:4190–4208`: `NPCGoods` requires the parent dialogue to be visible, then opens the goods panel.
- `Client/MirScenes/GameScene.cs:4240–4244`: `NPCSell` opens the drop/sell panel beneath the visible parent.
- `Client/MirScenes/GameScene.cs:3891–3912`: an actual `NPCResponse` updates the page; an empty page hides the parent, and both empty and nonempty pages close the previous service children.
- `Client/MirScenes/Dialogs/NPCDialogs.cs:1023–1045`: `NPCDialog.Hide` closes service children and restores inventory to `(0,0)`; `Show` places it beside the 440-pixel parent at `(445,0)`.

The Rust behavior copied the effect of a real `Hide` but used recurring snapshot text absence as its trigger. The service packet and its explicit lifetime must be distinguished from the optional dialogue-text projection.

Local closing boundaries include `CloseNpcDialog`, NPC quest-list close paths, `SelectNpcDialog` with an enabled `@exit`/`@Exit` target, and Escape in `quest_ui.rs`. They also include general close-all, session/logout reset, map boundary and switching to another NPC. A passive null snapshot following Goods must retain the active service; a real boundary must retire the previous service and its capabilities. Outbound acceptance/failure and batched FIFO order require explicit controls rather than treating any hidden text page as Exit. The final bounded checks below cover ordered Exit and homogeneous send outcomes; mixed partial failure remains unmeasured.

## Initial dynamic repair checkpoint

The candidate separates NPC text visibility from the packet service lifetime. The
[delayed-hide reproduction](checks/shop-lifecycle-before.log) failed before the
change (0 passed / 1 failed). The
[initial shop regression](checks/shop-lifecycle-after.log) then passed 8/8,
including three-class delayed hides, coalesced Goods/text-hide delivery,
explicit exit with hidden text and the earlier layout/Buy/Sell controls.

The [complete shared-client run at that checkpoint](checks/shared-shop-lifecycle-full-after.log)
passed 1191 tests / 10 ignored. This predates the final FIFO Exit review and is
not a final-source or installed-package acceptance result.

The first GPU replay also failed its purchase-intent assertion (`[]` instead of
one BuyItem); its [fixture failure log](checks/shop-lifecycle-gpu-fixture-before-reset.log)
is retained separately. The subsequent
[GPU run](checks/shop-lifecycle-gpu-after.log) and
[manifest](shop-lifecycle-gpu/shop-lifecycle-layouts.json) record six actual
offscreen captures: Warrior, Wizard and Taoist, Buy/Sell, zh-TW, 1024x768,
**560 ms after NPC text hides**. All retain the bag at `(445,0)` beside the shop,
with usable production widget controls. The manifest explicitly has
`liveAcceptance=false`.

Further review exposed four boundary failures in the
[first extended run](checks/shop-lifecycle-boundaries-before.log)
(8 passed / 4 failed): scene reset, Escape followed by a sell-only service,
new NPC text replacing the previous merchant, and late Goods following Exit.
The [first boundary repair run](checks/shop-lifecycle-boundaries-after.log)
still had two close/reopen failures (10 passed / 2 failed). A later
[FIFO regression](checks/shop-lifecycle-fifo-before.log) also failed before
correction (0 passed / 1 failed) when a request batch ended in Exit. These logs
remain retained; the final after results are recorded separately below.
The earlier 8/8 and GPU captures do not override those boundary findings.

## Final bounded closing checks

The final ordered service-Exit correction passes the
[complete shared-client suite](checks/shared-shop-lifecycle-fifo-final.log):
**1197 passed / 10 ignored**. The
[native Windows suite](checks/native-shop-lifecycle-fifo-final.log) passes
**808 passed / 5 ignored**. [Focused native FIFO checks](checks/shop-lifecycle-fifo-after.log)
pass **3/3**, comprising two
lifecycle tests and one existing input guard. The native batch control exercises
four cases with all sends succeeding or all sends failing. It does not establish
behavior for a partially successful mixed batch. Before/after logs remain
separate; the original 0/1 FIFO failure is not relabelled as passing.

The final [GPU03 1/1 replay](checks/shop-lifecycle-gpu-after-03.log) produces
six level-9 Warrior/Wizard/Taoist Buy/Sell
captures using real production widgets, after the same **560 ms** delayed text
hide. Buy and Sell frames were inspected with the image viewer. This remains
an offscreen synthetic-state check with no authenticated GUI or real mouse
operation; `liveAcceptance=false` is retained. The copied public logs preserve
each before, intermediate and final result separately.

## Replay structure and remaining boundaries

### Explicit server response follow-up — October 2 (+08:00)

The final packet review identified a separate closing gap: native routing did not
forward real `NPCResponse` pages as service boundaries. The
[boundary evidence](npc-response-service-boundary.json) preserves two actual
pre-fix assertion failures (native 0/1 and runtime 0/1), as well as earlier
compile-cache failures without counting those as assertions.

Native routing now accepts only an actual array of page strings and retires the
old service for empty or nonempty pages. Malformed responses and passive snapshot
text absence do not retire it. Runtime ingestion applies the ordered service
signals: Closed cancels a previous opening in the same frame, never opens an
empty bootstrap shop, and a later Buy legitimately requests opening again.

Root source `b369b07e7` passes the complete Windows suite **809 / 5 ignored** and
runtime suite **276 / 1 ignored**. The client-only release backport is
`180b01d46f35a5e97f4c60ee2ae033c7b53717e5`; its clean attested package and updater
validation are tracked separately. This packet/ECS verification does not create
authenticated GUI or whole-game acceptance.

1. Begin in game at level 9 with Samuel's open authoritative text, and update the real UI.
2. Deliver real-template potion catalog plus Buy and Sell service signals, then update; assert goods, Buy input, and a separate bag at `(445,0)`.
3. Apply the next authoritative dialog projection with `activeNpcDialog: null`, then update for multiple frames.
4. Assert the service and goods remain open, the bag has not returned to `(0,0)`, and selecting a potion still reaches the production Buy handler exactly once.
5. Repeat with Goods/Sell and null-text snapshot arriving in the same update. Complete explicit Exit/Close/Escape, service reopen, map/logout, another-NPC, late-Goods and FIFO request controls so the fix does not retain stale shop state.

This is bounded production ECS/input replay with synthetic service state.
Ordinary protocol purchases independently succeed for both casters, as linked
in the [journey supply evidence](README.md#ordinary-purchases-and-supplies), but
those receipts cannot verify how the native panels render or receive mouse input.

## Backport checks and remaining human gate

The initial client-only backport source was
`715078944ac7a84a61b6d64901b80568a8e8d63b`, based on clean r8 `3f5e61533`.
Its separate complete shared suite passes
[1197/10 ignored](checks/r9-shared-backport-full.log), Windows host
[803/5 ignored](checks/r9-native-backport-full-final.log), and
[six delayed-hide captures](checks/r9-shop-lifecycle-gpu.log).
Initial missing-input failures remain recorded: [169 failures](checks/r9-native-backport-full.log),
[157 on the premature rerun](checks/r9-native-backport-full-inputs-restored.log),
then [one missing pet sound](checks/r9-native-backport-full-environment-fixed.log).
The fresh worktree had no ignored map derivatives/dependencies and lacked
[15 local WAV inputs](r9-missing-local-sound-inputs.json). Reusing known existing
map/dependency inputs and copying only absent sounds restores the environment;
tracked source remains clean. These backport results are separate from the
integration branch's 808 native tests.

- Preserve the failed dynamic, GPU fixture, closing-boundary and FIFO records;
  retain their separate public logs alongside the final results.
- Bind the passing final checks to the exact source used by the package.
  Initial checkpoint totals must not be reused for a later changed source;
  mixed partial-send behavior has no measured acceptance claim.
- Final clean 180b01d46 package, installer and signed updater are verified;
  the actual release receipts preserve source/package identity. Earlier 715078944
  test totals are historical and are not reused as a full-suite run of 180b01d46.
- Verify ordinary native login, Samuel Buy/Sell, one real mouse purchase and
  panel close/reopen on the installed client, including the affected laptop.

The r9 installer and sequence5 update publication are recorded in the separate
[release evidence](release-r9/README.md). Native mouse/GUI acceptance remains
open. Game services and human stores are preserved; capacity remains paused.
