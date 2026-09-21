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
