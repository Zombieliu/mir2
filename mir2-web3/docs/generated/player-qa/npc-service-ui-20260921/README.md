# NPC item service UI candidate — 2026-09-21

Source: Crystal `Client/MirScenes/Dialogs/NPCDialogs.cs`, `NPCDropDialog`
constructor and `NPCDropPanel_BeforeDraw`, `Confirm`, and `ItemCell_Click`.

The native sale/repair/special-repair surface now uses the active source
Prguse2/351 frame at its decoded 176×147 dimensions (the constructor's
Prguse/392 is overridden before drawing). Confirm uses Title/290–292 at
(114,62), 48×25; Hold uses Title/293–295 at (114,36), 48×25. The selected
original item draws at (38,72). NPC purchase Prguse/1000 is corrected from
242×330 to 244×334, and its Title/312 purchase button from 80×22 to 80×25.

Hold toggles selection-event auto-submit through the existing pending-intent
queue. It is scoped to the current service mode and cleared on close/new
NPC-open signals. Rendering never sends transactions. Normal confirmation
and Hold share a submit path with the same repair selection predicate and
authoritative server checks. Successful enqueue clears the target, while a
pending duplicate or full queue retains it. Sale count cannot exceed the live
stack, and the sale label uses checked multiplication of the authoritative
per-unit sell value and that count.

The old first-ten-only text list is replaced with a full 46-slot bag and
14-slot equipment icon picker, preserving concrete item identity and tooltips.
This adjacent picker has a dark backing, and its quantity controls, Close/Buy controls and repair
rate are native adapters, not source Crystal inventory drag/drop parity.
Source screen placement below the variable-height NPC dialog remains open.
Repair displays `Quote unavailable`: there is no authoritative repair quote
in the current model. A separate source/backend discrepancy prevents claiming
an exact quote: Crystal `Stats.Count` sums absolute stat values and doubles
repair cost for rental items, whereas simulation equipment pricing uses the
number of merged stat entries and does not apply that rental doubling.
This UI change does not alter backend prices or invent a local repair quote.

Validation:

- Rust 1.95.0, offline native-ui library filtered `npc_item_service`: 5/5.
  Tests cover authoritative action modes, separated bag/equipment identity,
  rendered final inventory/equipment slots and confirm geometry, and one
  Hold selection producing exactly one special-repair intent with no idle
  update re-submission and proper close reset. Follow-up coverage also checks
  clear-after-enqueue, pending-duplicate target retention and stack quantity clamping.
- Existing `overlay_buttons_shop_storage_flow_generates_intents`: 1/1,
  with its former retained-target assertion updated to the source's
  clear-after-successful-enqueue behavior after that assertion failed on the
  initial follow-up run.
- Scoped git diff whitespace check passes. Pre-existing compile warnings remain.
- Asset export/closure is coordinated separately by the UI asset worker.

No native screenshot, original/candidate paired visual check, live service
transaction, package or human frontend acceptance was performed here.
`visualAccepted=false`; `accepted=false`; no global parity claim.

## Source contract for remaining repair-quote gate

Verified against Shared/Data/ItemData.cs Price()/RepairPrice() and
Shared/Data/Stat.cs Count: added-stat weight is the sum of absolute values,
not the number of stat entries. RentalInformation != null doubles RepairPrice.
For source price1000, durability/max1000, current500, one added stat +5:
Price=floor(875*1.5)=1312; full-price1500; RepairPrice188 (rental376),
before ordinary/special NPC rate multiplication. Current simulation's .len()
uses1 rather than5 and produces138 for the same nonrental item.

Do not turn `Quote unavailable` into a client-invented number. This gate needs
shared source arithmetic and rental identity parity, then matching display and
authoritative deduction tests. No economy or save changes were made by this
source review. Public ordinary repair remains separate live acceptance.

## NPC service drag integration (2026-09-21)

Source review found two gaps behind otherwise passing service tests: the native
single-panel state hid the regular inventory when NPCDrop opened, and selection
retained only a bag slot rather than the source item identity. Crystal
NPCDialogs.cs NPCDropDialog.Show opens InventoryDialog; ItemCell_Click assigns
TargetItem, and its active sale path retains the full stack (the amount prompt
branch is commented out).

Candidate now routes ordinary bag drag release to the source NPCDrop target,
keeps the regular bag visible with the service, and removes the redundant bag
picker. Sale/repair/special-repair deferred confirmation revalidates the selected
instance. Hold submits through the same pending queue once. Full-stack sale
count and displayed price use the dragged quantity bounded by current quantity
and protocol width. Equipment selection remains an adapter; repair quotes still
have the separately documented source arithmetic gap.

Ordered gesture, stale-instance, and rendered-source-panel regressions cover
these changes. Final build/test receipts follow after the independent bag-close
behavior is checked. No live transaction or screenshot is claimed for this
candidate. Existing player session remains untouched while desktop handoff is
pending; the stationary memory blocker is still unresolved.

Final regression receipt: client native-ui 897 passed, 0 failed;
C:/Users/Administrator/AppData/Local/Temp/mir2-client-bevy-native-ui-npc-service.log.
Windows release build passes (77 s); log
C:/mir2-ui-repair-20260921/npc-service-drag-build.log.
Independent bag close/reopen preserves NPC service, hides/cancels its bag drag
source while closed, and reopens the bag on a new NPC service session.
Candidate package C:/numeron-legend-of-rebirth-20260921-npc-service-drag,
EXE SHA256 184A68FE091578BBCC95871684149394F0DB10EEAED6EB8117FEEFE004B2A1F2.
It includes image/font attribution telemetry but has NOT been launched.
Source, tests, and build do not establish live visual/service acceptance.

## Repair/sale pricing candidate correction

The earlier source-contract gate is now fixed in simulation source: merged
AddedStats uses the sum of absolute values, and rental identity doubles repair
cost using the existing wire-presence reconstruction (including Some(default)).
No carrier schema or saved player state changed. Fixed numeric vectors pass3/3:
base1000/max1000/current500/+5 yields current1312, repair188, special564;
rental repair376 and special at rate1.5 yields1692. Negative stats, stack count,
full durability and nondurable zero quotes are covered. Existing public packet
repair tests5/5 and sale tests2/2 pass. Logs under
C:/mir2-ui-repair-20260921/npc-price-tests.log,
npc-repair-packet-tests.log and npc-sale-packet-tests.log.
Running Gateway remains the old build; UI quotes must be deployed alongside
this correction before claiming displayed/deducted equality. Live acceptance
and source-equivalent equipment drag remain open.

## Matched quote package receipts

Client now computes ordinary/special repair display from concrete tooltip Info
and UserItem identity, live top-level quantity and current/max durability,
absolute stat-value weight, rental presence and validated NPC rate. Missing or
inconsistent source data retains Quote unavailable and cannot enqueue repair.
Authoritative simulation snapshots rebuild tooltip metadata; external partial
payloads have no revision guarantee for same-ID stat/rental changes. No full
external-server parity claim is made. Live quantity/durability updates are tested.

Initial full client suite found2 incomplete repair fixtures; those now provide
real source metadata and rate without weakening intent assertions. Final full
native-ui suite900/900 passes; log npc-quote-client-tests-rerun.log under
C:/mir2-ui-repair-20260921. Server release139s and native release68s pass.
Matched candidate directory C:/numeron-legend-of-rebirth-20260921-npc-quotes:
- mir2-platform-windows.exe SHA256 F4658793D053848295FFA2CAFBD01B3FAC7EA95C12E55C2B4082463EB62CB156
- mir2-gateway.exe SHA256 FED42F81FF2160C3A95583747C95EA7AC97F6AE52B06502D503D4F5E817C16FD
Both are staged, neither deployed/launched. Preserve current saved account store
and ports when switching after normal logout and desktop handoff. Do not use
a new client quote against the old server to claim exact pricing acceptance.
Affordability feedback, actual deduction, equipment drag, and visual acceptance
remain separate. Memory diagnostic instrumentation is included, not a leak fix.

## Corrected source scope for next service-layout pass

A deeper active-code read supersedes the earlier equipment-drag backlog:
Crystal NPCDialogs.cs ItemCell_Click at1699 requires MirGridType.Inventory for
Sell/Repair/SpecialRepair. Equipment cells are not a supported drop source for
these three services; the native equipment picker is an extra adapter to remove,
not a missing source-equivalent equipment drag to implement. Disassemble/reset
have separate preceding branches and do not change this repair/sale restriction.

BeforeDraw sets NPCDropDialog.Location=(264,NPCDialog.Size.Height), not x0.
Native OverlayShop currently uses(0,224). Earlier notes asserting source-faithful
overall frame position proved only relative cell/button geometry and were too
broad. Next layout pass must move the service frame/hit target consistently,
remove the equipment adjunct, and verify the ordinary bag remains visible and
usable. No source/client paired visual result has established this yet.

## Repair affordability feedback

Native repair/SRepair Confirm and Hold now preserve selection and report the
source LowGold text through one System-chat entry when funds are insufficient.
No request or pending operation is created on rejection. Confirm remains
clickable, matching source feedback rather than silently disabling it.
NpcRepairQuote now distinguishes displayed/compared f32 value from the server's
truncated u32 deduction:188*0.1 displays18.8, gold18 rejects and19 accepts.
Low/equal-gold normal/special, Hold-once, and fractional-rate regressions pass.
Full native-ui suite904/904; log
C:/mir2-ui-repair-20260921/npc-affordability-client-tests.log.
This patch is source/test verified only, not yet packaged or run in-game.

Sale gold-cap preflight remains open: native per-unit sell_value multiplication
can differ from original floor(total Price /2) for an odd-price stack. Do not
implement a supposedly exact capacity guard from that lossy unit value.
