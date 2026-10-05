# Browser shared bag host: B2 contract

2026-09-30 implementation contract; not an acceptance record. The preceding
equipment106/106 and native99d/Web9D GUI checkpoint remains immutable evidence.

## Ownership and scope

Reuse `paint_crystal_inventory`, `InventoryModel`, exact instance resolution,
the existing portable UI window/camera/font and item hints. The first opt-in
desktop WebGPU slice supports pages, inspect, Use/Equip, bag Move, close, and
ordinary save/relogin. It must preserve working world input outside the bag.
Touch stays on the compatibility owner for unsupported presentations. The
bounded landscape geometry/action checkpoint is recorded below; complete
touch Inspect and physical-device acceptance remain separate.
Install the existing portable UI window for `bevyQuestUi=1` or `bevyBagUi=1`,
once only. Bag opt-in must request its font/skins independently of Quest input.
The Bag exports remain queryable when not installed, with ready=false.

The browser world and shared UI canvases use the stage's logical coordinates.
Winit owns physical resizing and input DPI together. Do not feed the canvas's
default backing attributes into CSS sizing, or force scale factor 1 on a real
high-DPI surface. Keep exact layout readiness checks: mismatched physical pixels
and DPR cannot grant shared input. Initial high-DPI startup and DPI transitions
require actual browser evidence; CDP emulation and physical devices remain
separate gates. Native windows retain their platform DPI behavior.
Tooltip action hints describe the host's available interaction: native retains
its Shift-click split shortcut, while this portable slice points to More actions.
Original item metadata and maximum stack count remain shared.

Page and its small-WASM `EquipmentSessionController` are the single sender and
equipment ledger. The render runtime has a separate read-only bag model and
must not invoke the legacy inventory-ingest reconciliation or create equipment
entries in its `PendingOperations`. Quest state and pending remain independent.

Storage, trade, mail, hero and other complex modes retain React. Unmigrated
visible controls cannot silently do nothing: Delete requests a compatibility
handoff into the existing mode; a More actions control gives access to the
remaining existing bag tools. A handoff is a UI action, not an item request.
Compatibility stays selected until this bag closes or another explicit open.
If a player starts a pointer or keyboard interaction in the React bag during
preparation, keep that compatibility owner until close. Hover alone does not
select it. Closing invalidates input synchronously, before the React state
update, so a later intent in the same renderer drain cannot act on a closed bag.

## ABI 1

Use independent exports: `getMir2BagUiAbiVersion`, `setMir2BagUiSnapshot`,
`getMir2BagUiStatus`, `setMir2BagUiIntentSink`, `clearMir2BagUiIntentSink`,
`setMir2BagUiPointerEdge`. Do not add Bag fields to the old Quest DTO.

All JSON fields use camelCase. `BagIdentity` has `runGeneration`,
`connectionGeneration`, `sessionGeneration`, `ownerRevision`. Integers are
nonnegative safe JS integers; live run/connection/session identities are positive.

Snapshot adds `revision`, `modelRevision`, `presentationRevision`, `bagOpen`,
`page` (`bag1|bag2|quest`), `inputEnabled`, nullable existing Quest presentation
metrics, `model` (existing BevyInventoryModel), `player` (PlayerStats wire shape),
and `blockedUniqueIds` (read-only current controller reservations).
Unknown player stats remain unknown. Validate the entire model, including
capacity, duplicate cells and duplicate identities, before replacing the model.

Status echoes BagIdentity plus `frame`, `ready`, `inputEnabled`,
`appliedRevision`, `appliedModelRevision`, `appliedPresentationRevision`,
`inputRegions` (`left,top,width,height` in logical stage units), and nullable
`error`. Ready means valid current model, loaded assets and applied layout;
it does not by itself grant input. Regions include the bag background, blank
cells and inspector/action panels, but not decorative hints.

The actual panel carries its owner, model and presentation revisions plus page
and visible-item count. Content readiness must observe that current subtree,
not a resource updated when a rebuild was merely queued. Verify the expected
cell identities, occupied markers, original image paths, visible nonzero icon
layout and exact stack-label text/layout. Empty pages and read-only Quest items
without instance IDs remain valid. Stale or incomplete content keeps readiness
false and leaves the compatibility owner available; do not use a fixed delay.
These ECS/layout checks are not a GPU presentation fence or proof that every
captured frame has drawn; actual browser screenshots remain a separate gate.

Intent carries the identity and revisions captured when the action was made:
`intentSequence`, `modelRevision`, `presentationRevision`, `type`.
Types are `useItem|equipItem|moveItem|close|selectPage|handoff`.
Item source is `{container:0,slot,uniqueId}` with global bag cells and UID0 valid;
Move adds target `{container:0,slot}`. SelectPage adds `page`. Handoff adds
`mode:fullInventory|delete`. Quest items can be inspected but grant no bag action.
The host translates global bag cells to existing bag1/bag2 local references,
then revalidates current identity, capacity and pending before the normal send.
`{accepted,error?}` reports submission/local UI handling only, never server ACK.

## Transition and input rules

Prepare with input disabled while React owns the visible bag. After exact
identity/model/layout status and a live frame are confirmed, increase the
owner revision synchronously, update the existing ledger's owner revision, and
request enabled input. Commit DOM ownership only after the enabled status echo.
Fallback invalidates the old owner synchronously before exposing React.
Neither transition releases pending or replays an item operation.

React callbacks capture their rendered owner token. Bevy intents retain their
original token through draining. Recheck it at submission and after synchronous
action listeners at the actual socket send; never attach the latest token to an
old event. Reject repeated intentSequence in the same owner epoch.

Pointer edges carry BagIdentity, `sequence`, `presentationRevision`, `pointerId`,
`phase` (`down|move|up|cancel|blur`), `origin` (`bag|world`), logical `x,y` and
`button` (0 primary, 2 secondary). Shell captures ordered PointerEvents on the
existing UI canvas. Down position determines the gesture owner through release
or cancellation. Forward world-origin gestures through the existing world
helpers and suppress duplicate compatibility mouse dispatch. Winit can prevent
compatibility mouse events, so mouse handlers alone are insufficient.

Rust likewise rejects world-origin input and stale pointer epochs, including a
world drag entering the bag. Do not infer origin only from final cursor/hover.
Cancel active gestures on layout/owner/focus changes and quarantine held input
until release. Temporary handoff keeps the open page, clears action selections
and drags, and preserves both equipment pending and separate Quest state.
Retired terminal pointer edges may clear their old gesture but cannot become a
click under a new owner. A terminal edge after host eligibility loss is cancel,
even if the renderer has not yet applied its disabled snapshot. Ordinary model
or reservation updates do not reset an unchanged presentation or hide the tree;
an item action still validates its current instance and source cell.

## Writers and checks

Rust worker owns new portable bag UI and runtime bag host modules, their focused
tests, and necessary shared UI registration/camera/font/hint wiring. Web worker
owns the new Bag ABI/hook, Page/shell/scene/props/CSS and their Node tests.
The coordinator owns this contract, integration, actual GUI and QA/queue docs.

Verify exact UID0, duplicate/capacity rejection, old callback/intent rejection,
pending-preserving owner changes, ordered pointer origin, layout and stopped
runtime fallback. Build portable WASM and exercise actual same-canvas hints,
Use/Equip/Move, close, inner clicks/drag without world movement, outer-world
clicks/keys, normal logout and restored save. Positive expanded Bag II, physical
mobile and full shared bag feature parity remain separately stated gates.

## Touch design boundary and current bounded evidence

Start with the existing wide-landscape stage at approximately640×360 and
844×390 CSS pixels. Derive all geometry from the actual stage presentation;
portrait, unsupported dimensions, WebGL2 and unavailable assets keep the
compatibility owner. Both hit-target size and readable content are required.
Every enabled cell, tab, close/delete control and inspector action must have
at least40×40 CSS pixels of non-overlapping usable area. Enlarging empty cells
around unchanged tiny icons/text is not completion.

Extend the shared painter's layout options with native desktop defaults.
Prefer real Node dimensions/positions for touch geometry over a scaled parent
Transform: the current action hit rectangles and published regions use
ComputedNode dimensions plus translation. Original item-image layout currently
restores each texture's natural dimensions, so icon scale needs an explicit,
bounded presentation path as well. Keep one icon/stack/tooltip source and
verify the native default independently of the touch variant.

Reuse the existing touch secondary-window exclusivity and the Page's Quest
eligibility guards. Prove one foreground interactive surface on the shared
canvas through Quest→Bag→Quest and More→React transitions. Preserve the
existing selected Quest/detail state and equipment ledger. Audit the mobile
controls' stacking/input interception before granting Bag ownership. Additional
fingers and held pointers across resize, portrait or ownership changes must
neither issue an item operation nor fall through into world movement.

The bounded gate requires layout and pointer regressions plus actual ordinary
touch-emulated page switching, inspect/use/equip/move, More handoff, pending
ACK during handoff, resize/portrait recovery and normal save/relogin. Capture
readable icons, text, controls and hints at both target sizes. Desktop Chrome
emulation remains distinct from physical Android/iOS lifecycle, memory, safe
areas and IME acceptance; do not enable an unsupported layout to fill that gap.

The2026-10-01 WebGPU7fca artifact satisfies the bounded landscape core-action,
pointer cancellation and normal-save/restoration portion in desktop Chrome
touch emulation at600/640/844. Requested390×844 portrait was clamped to actual
500×844. The original b4b readiness failure and later harness/observation gap
are retained in [player QA](../generated/player-qa/client-core-20260930/touch-bag/README.md).
Name/action selection is not complete Inspect. A separate detail slice must
read the exact current stamped cell's existing `CrystalItemHint` document,
provide readable text and complete paging, block underlying actions while open,
and invalidate on model/owner/page/presentation or input loss. It is local UI
state and must not add an item request, DTO schema or second operation ledger.

The later e275 Inspect slice satisfies that bounded existing-document gate:
actual600/640/844 paging, mixed-input cancellation, desktop hover/selection and
item actions, blur/resize/portrait invalidation, More/page handoff and normal
saves118/120. Its independent current-artifact touch Use_weapon/Equip/Move
supplement restores the baseline at save126.4fdd's real Next failure demonstrates that navigation target
geometry alone cannot grant input: status must also publish the complete
measured active detail region, including the footer and panel gap. e275 adds
that third region; missing/stale/unlaid-out detail fails ready, and closing
detail restores the original two regions. UID-less Quest and partial/long
documents have source regression evidence, not a nonempty actual Quest fixture.
[Exact builds, screenshots and limits](../generated/player-qa/client-core-20260930/touch-bag/inspect-detail/input-region-fix/README.md).

Independent short-landscape CSS entry repair leaves install/fullscreen and
More reachable and moves Approach away from HUD Quest. Actual600/640/844 hit
tests and a640 Diary→detail→Bag→Diary route are recorded separately from the
original7fca CSS. The task detail close returns to Diary before a second close
ends that surface. General HUD touch-target sizing remains an open gate.

The later opt-in WebGL2 prototype109d reuses the primary Window/UI camera above
the complete DOM world for the whole experimental page lifetime. Exact query
gates are specified in the [prototype contract](webgl2-shared-ui-prototype-contract.md).
It adds no Bag operation or ledger. Transparent primary UI, four ordinary mouse
and four touch success receipts,600/640/844 Inspect, input/lifecycle/More
handoff and normal saves136/138 are recorded with57 bounded source backups.
Default GL2 retains React/save140; GPU retains shared UI/equipment/save144.
The [actual round](../generated/player-qa/client-core-20260930/webgl2-shared-ui/README.md)
does not close B2, strict GL-context enumeration, final world composition,
payload performance, physical devices, DPR2 touch or remaining inventory variants.

The next host-only mobile More Diary entry adds no Bag state/ledger. The existing
Quest callback,40/44px panel targets, PWA spacing and touch menu dismissal have
current63-source English/Chinese/Portuguese600/640/844 and defaultGL2/GPU evidence
under the [entry contract](mobile-diary-entry-contract.md). Runtime109d stays
byte-identical.600 Diary remains React while shared Bag is usable;640/844 use
shared Diary. Normal passing saves through165 preserve the public baseline.
Failed Chinese resize emitted20 walks and normal facing restoration two; this
does not count as zero UI leakage for that failed epoch. Stable explicit native
CSS DOM touches supersede that QA helper. Compact Quest, full locale/HUD,
physical devices/performance and the earlier broad inventory gates stay open.
[Scoped routes and retained failures](../generated/player-qa/client-core-20260930/mobile-diary-entry/README.md).
