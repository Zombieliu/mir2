# Compact landscape shared Quest: bounded contract

2026-10-01, CP-03 follow-up to the sealed mobile More Diary entry. At actual
600×320 the current335px panel-height gate keeps a tiny React Diary. The next
slice must provide a readable shared single-sheet Quest surface at that size,
with the same existing quest state, intents and authoritative receipts.

## Supported geometry and local pages

Keep the existing desktop/native Crystal geometry and mobile panel heights
at least335 CSS px unchanged. A new compact branch may support panel heights
296–334 CSS px, width at least400 CSS px and landscape stage aspect at least1.5.
Keep12px stage margins and the560px panel-width cap. Actual600×320 therefore
uses560×296. Portrait, smaller/shorter or mismatched window metrics continue
to fail closed to the compatibility owner; do not just lower the old minimum.

Reuse the existing single-sheet priority: Confirmation, NPC list, Dialog,
Detail, Diary. Compact Diary/basic dialog can use existing rows/paging after
layout verification. Detail and NPC quest list get local Text and Rewards
subpages. One subpage's action nodes exist at a time. Local tabs, text scroll,
reward paging and selection are not server state or new packet operations.

At the296px minimum, a viable CSS budget is tabs/close at4–48, Detail text
at54–222 (seven24px lines), or NPC navigation54–98 and message102–222 (five
lines), and text pager247–291. Compact tabs may replace only the Detail/NPC
chrome title band; keep an identifiable quest title in their content. Rewards
may use currency54–74, fixed rewards82–126, selectable132–176, reward paging
190–234 and existing decision actions247–291. Other safe arrangements are
allowed if measured evidence proves readable, nonoverlapping content. Keep
body/reward text at least14 CSS px and enabled controls at least40×40; existing
44px controls are preferred. Do not shrink fonts or silently truncate rewards.
Paginate overflow fixed/selectable rewards with the existing authoritative
selection indices. Keep readable names/quantities and the current hint source.
Long confirmation/alert content must paginate locally or retain compatibility
ownership if it cannot fit; clipped text is not a successful compact layout.

Preserve selected quest, text reading position and valid local subpage through
temporary visibility/resize handoff. Clamp scroll/reward pages when content
changes; reset compact local pages on a different selected quest/NPC, close,
invalidated selection and logout/new generation. Confirmation returns to the
same valid underlying page. Existing pending/ACK/NACK/endpoint/profile guards,
QuestUiIntentQueue and the shared operation ledger remain the only submit path.

## Measured ownership and input

Compact support is provisional until the current tree has completed Bevy UI
layout. Measure the active current sheet after UiSystems::Layout and global
transform propagation; inspect computed bounds rather than declared Node sizes.
Verify root/sheet identity, stage/panel containment, enabled action minimum
sizes, readable text and no overlapping enabled actions. Old/hidden/unlaid-out
subpages cannot satisfy readiness or receive input. A newly rebuilt page must
not inherit readiness from prior sheet geometry. Publish inspectable bounded
diagnostics through local UI status if needed; these are not protocol DTOs.

Avoid the preparation deadlock in the present runtime: host visibility currently
depends on previous status.ready, and hidden roots have no usable layout. Permit
a compact tree to prepare and lay out without granting input/intent authority;
only the validated current tree may capture pointer or submit. Do not make an
invisible tree ready by inventing bounds or keep stale ready during transitions.
The ordinary React owner remains available until shared geometry is validated.
Keep existing WebGL2 primary/DOM-world prototype and GPU canvas arrangements.
When the browser pauses Bevy in background, raw status getters can retain the
last ready/current values. The host must require an advancing frame plus current
visibility/owner/canvas gates; that cached status cannot grant input. On resume,
freshly observed geometry and local state must again satisfy the same contract.

## Bounded ownership and verification

Root owns architecture, docs/queue, release builds and actual browser clients.
One Sol/high worker may edit only:

- `apps/game-client/client-bevy/src/quest_ui.rs`
- `apps/game-client/client-bevy/src/portable_quest_ui.rs` (needed preparation/lifecycle only)
- `apps/game-client/runtime/src/quest_ui_host.rs`

Tests belong in these existing scoped modules unless a dedicated adjacent
test module is required; request an expanded write set before editing other
product files. Back up the complete current write set before mutation into
the new compact-quest QA directory. No Web Page/Shell/CSS, Cargo dependency,
wire schema, auth, server, saved player state or runtime package publication
is assigned to the worker. Preserve all unrelated staged work.

Run meaningful compact layout/interaction/lifecycle tests, existing640/844
mobile and native geometry regressions, then relevant native/portable library
and both runtime WASM checks. Actual Bevy-computed layout tests must cover
600×320, a minimum supported landscape stage,640×359.298,844×390 and rejected
sub-minimum/portrait/window-mismatch cases; fake declared geometry alone is
not layout proof. Exercise long text, multiple fixed/selectable reward pages,
hidden page input rejection, confirmation return, pending finish/ACK, temporary
host hide/resize and new session. Root reviews source and builds immutable
GL2/GPU packages before ordinary touch/UI/save/relogin verification at600/640/844.
Retain failed artifacts and source variants separately. Physical Android/iOS,
DPR2/IME/safe-area, all HUD/locale, performance and full CP-02–04 stay open.

The first ba90 real600 route exposed logical-to-CSS rounding absent in scale1
fixtures. Final f7 uses compact-only LayoutConfig without rounding, retains the
400×296 minimum and measures panelCssBounds. Its bounded English-entry actual
Text/Rewards/Cancel/lifecycle/save routes are recorded in
[compact QA](../generated/player-qa/client-core-20260930/compact-quest/README.md);
multiple rewards/NPC/Finish ACK/long confirmation remain fixture-only here.
