# UI Fidelity Assessment (Web Client vs Crystal)

Last updated: 2026-05-31

Purpose: quantify how faithfully the React/Next.js web client (`apps/web`)
reproduces the original Legend of Mir 2 (Crystal C# client) UI, dialog by
dialog. This complements `docs/FRONTEND-1TO1-GAPS.md` (which tracks gaps as a
`[~]` checklist) by attaching a per-dialog fidelity percentage and separating
the *visual shell* from the *interaction* layer.

## One-line conclusion

"Looks like Crystal" is already high (visual/asset shell ~75-80%, using real
extracted `Interface.Lib` frames at correct frame indices). "Behaves like
Crystal" lags (interaction fidelity ~45% at baseline). The three system-level
gaps were: no drag-and-drop, no skill hotbar (F1-F8), and the social dialogs
collapsed into one generic tab panel.

> Note: the per-dialog scorecard below is the **baseline** before the
> 2026-05-31 UI work. See "Update 2026-05-31" for what has since landed.

## Update 2026-05-31 (implemented)

Closed several of the largest interaction gaps (verified by `tsc --noEmit` +
`next build`; runtime/visual feel remains human-gated):

- **Drag-and-drop (system-level)** - a shared pointer-drag layer
  (`ItemDragProvider`) with a cursor item ghost. Wired across inventory bag,
  character paperdoll (equip/unequip by drag), storage (store/take-back/reorder),
  and belt, plus **drop-to-ground** by releasing over the map. Reuses the
  existing item commands; click/right-click paths are preserved. Inventory
  interaction ~62% -> ~80%; the click-staging paradigm gap is closed.
- **Skill hotbar (F1-F8)** - a real `SkillBar` above the HUD assigned from each
  skill's server `hotkey`, with cooldown/toggle state; F1-F8 cast the matching
  slot. ~5% -> ~70% (no MagIcon art, so slots are labelled not icon-art).
- **Buff row** - active buffs render as a labelled chip row with durations and
  bonus tooltips (BuffDialog stand-in). ~10% -> ~55%.
- **Floating combat numbers** - damage/heal numbers rise/fade over entities,
  derived from per-entity HP deltas. ~0% -> ~70%.
- **HUD truth** - HP/MP orbs now fill bottom-up (clip-path) like Crystal's
  liquid orbs; MP is driven by the server's authoritative mana percentage
  instead of a hardcoded `maxMp=100` + bad ratio. Main HUD ~70% -> ~82%.
  (A real numeric max MP with stat/gear scaling stays a backend stat-engine
  item: the server still reports mana as a percentage of a fixed scale, and
  `mana_percent` divides by a literal 100.)
- **Options dialog** - the HUD Option button opens a real in-game Options
  dialog (Sound + Game toggles) instead of jumping to the character stats tab.
  ~15% -> ~45%.
- **MirAmountBox** - split / drop-gold use a Crystal-style amount box (spinner
  with hold-repeat, drag slider, Max button) instead of a bare number input.
  ~30% -> ~70%.

## Update 2026-06-01 (round 4 - R2 fullcrystal + Crystal source: real dialog frames)

Two external unlocks closed the last big "asset-gated" item (per-dialog Crystal
bitmap frames):

1. The R2 asset release switched to `20260601-fullcrystal-a2f10be0`, a full
   transcode of the Crystal client (verified: the `mir2-r2-asset-cache` Worker
   maps `/original-ui/<lib>/<n>.png` straight to an R2 object key, and the
   `mir2-asset-worker.js` service worker routes all `/original-ui/` requests to
   that R2 base) - so any referenced frame resolves at runtime.
2. The open-source Crystal client (Suprcode/Crystal) provides the exact
   dialog->frame mapping that the compiled DLL hid.

Rebuilt with the genuine frame indices pulled from the C# source:

- **NPC dialog**: Prguse/995 background, close (413,3), body text at (8,34)
  420px wide on an 18px stride (NPCDialogs.cs).
- **Trade**: two 204x152 Prguse/389 frames; 5x2 cells at (x*36+10+x, y*32+39+y);
  gold (35,123); confirm Title/520-522 (135,120); close (181,3) (TradeDialogs.cs).
- **Social backdrops**: guild=Prguse/180, group=Prguse/120, friend=Title/199,
  relationship/marriage=Prguse/583 rendered as the real window art behind the
  data-driven panel (GuildDialog/GroupDialog/FriendDialog/RelationshipDialog.cs).

Each verified by `tsc` + `next build`; the export manifest was updated so a
local export reproduces the same frames. "Per-dialog Crystal bitmap frames" is
no longer gated - it is a mechanical source-lookup + wire job for any remaining
window.

## Update 2026-05-31 (round 3 - Crystal .Lib source available)

The Crystal client Debug build (incl. all `.Lib` graphics + Cursors) became
available, so several items previously written off as "asset-gated" were
actually doable. Landed (each `tsc`/`next build` green; backend ones also
`cargo check` + `world_snapshot` tests):

- **NPC vendor shop** (buy grid + sell drop-zone + repair), **mail compose /
  reply / item attachments** (real `SendMail` with `items_idx`).
- **Item tooltip is now full-field**: grade colour, attack/defence (base+bonus),
  bonus stat list (MAC/MC/SC/Accuracy/Agility/Luck/HP/MP/Haste), weight,
  **equip requirements** (level/AC/MAC/DC/MC/SC + class restriction), and
  **price** - all forwarded from the Crystal item template via WorldItemSnapshot.
- **Real skill & buff icons**: exported MagIcon.Lib (224 frames) + BuffIcon.Lib
  (265 frames) to PNG; the F1-F8 skill bar renders the real magic icon (greyed
  on cooldown) and the buff row renders real BuffType icons.
- **Custom cursors fixed**: the 7 Crystal `.CUR` cursors were committed but the
  CSS referenced mismatched-case filenames (never loaded on a case-sensitive
  host); normalized to lowercase `.cur` and wired default/attack/npc/text +
  trash (delete) / upgrade (repair) states.
- **Social dialog identity**: per-system Crystal menu-icon header + accent.

Truly remaining (still gated): per-dialog Crystal bitmap *frames* for the NPC /
social windows (the compiled `Client.dll` has no C# source, so the exact frame
indices can't be confirmed without guessing), the data-gated social
interactions (guild ranks/war, searchable market), and the backend-command-gated
Inspect (view other players' gear) + BigMap town teleport. Live two-client
trade/shop feel remains human-gated.

## Update 2026-05-31 (round 2)

Closed the round-1 "still open" headline items (verified by `tsc`/`next build`,
plus `cargo check` + the `world_snapshot` lib tests for the backend pieces):

- **Real max MP (backend)** - `mana_percent`/`zone_mana_percent` now divide by
  the player's real class/level max MP (`crystal_base_vitals`) instead of a
  literal 100, so a full-mana low-level character reads 100% not ~17%. Packet
  shape unchanged (percent-only, Crystal parity); the lib test suite shows the
  identical pass/fail set with and without the change.
- **Character paperdoll preview** - the player's standing sprite renders in the
  character dialog via the scene sprite pipeline.
- **Full stat block** - the session already computes the player's combat stats
  to send to the zone; forwarded into the world snapshot (serde auto-forward),
  so the Stats I tab shows real DC/MC/SC/AC/MAC ranges + Accuracy/Agility.
- **Rich item tooltips** - `WorldItemSnapshot` now forwards base attack/defence
  and weight (grade + added attack/defence were already serialized); tooltips
  show grade name colour, Attack/Defence (base +bonus), and Weight.
- **Player-to-player Trade dialog** - the full gateway trade protocol wired into
  a real two-grid dialog (drag-deposit/retrieve via the drag layer, partner grid
  resolved from `NewItemInfo`, gold, lock/confirm, cancel, request prompt). Live
  two-player feel is human-gated (cannot drive two clients in CI).
- **Social dialog identity** - each social system gets a Crystal menu-icon
  header + per-system accent, so they read as distinct dialogs rather than one
  generic tab panel.

Still deeper-gated: required-level/class + price tooltip fields (in ItemInfo,
not ItemState), per-dialog Crystal bitmap frames for the social windows, the
data-gated social interactions (guild ranks/war, searchable market), skill/buff
icon art (no MagIcon library extracted), NPC buy/sell/repair dialogs, and
BigMap teleport.

## Two dimensions

| Dimension | Fidelity | Notes |
| --- | --- | --- |
| Visual / asset shell | ~75-80% | Real Crystal frames by original frame index (Prguse/1 HUD, Prguse/4 orb, Title/196 inventory, Title/504 character, Title/567 menu...). Core window sizes and slot coords match. |
| Interaction / behaviour | ~45% | Click-staging instead of drag, no right-click menus, no skill hotbar, many stubbed buttons. |
| Composite "production 1:1 UI" | ~55% | Always-on HUD + entry flow + main windows are strong; social / skillbar / buffs / options / tooltips are the long tail. |

## Method and data sources

- Read `apps/web/app/components/original-client-*.tsx` (~16k lines of UI),
  `lib/original-ui.ts` (the asset/coordinate table), `app/globals.css`
  (4683 lines / 423 selectors), and the generated asset manifests
  (1440 source libraries catalogued; frames rasterized to PNG on demand).
- Four focused per-module audits (inventory/character, map/NPC/shop,
  social/menu, HUD/login/scene), cross-checked on the load-bearing claims
  (drag-drop absence, skillbar absence, `maxMp` hard-coding, social collapse).
- Limitation: the Crystal submodule is empty, so no pixel-level C# diff. Final
  visual "feel" remains human-gated, consistent with the project's own
  `Human-Only Acceptance Boundary`.

## Per-dialog scorecard

Legend: [done] faithful, [part] simplified, [gap] missing/stub.

### A. Always-on HUD + entry flow (strong)

| Module | Fidelity | Finding |
| --- | --- | --- |
| Login | ~90% [done] | Real frames (Prguse/1084 + Title/30-334 + 19-frame ChrSel animated bg), inputs, Enter submit, login music. `ChangePassword` repurposed as Quick-Enter; Web3 passkey added (beyond Crystal). |
| Select | ~85% [done] | Real frames, 5 class cards, animated ChrSel portraits, create flow (name/gender/class). Credits is placeholder text. |
| Main HUD | ~70% [part] | Real frames. HP/MP orbs use a CSS vertical-height clip, not Crystal's radial/masked bitmap fill. `maxMp` hard-coded to 100 (orb + text inaccurate). 7 action buttons present; Option button mis-wired to the stats2 tab instead of a settings dialog. |
| Chat + control bar | ~75% [part] | Real frames; channel prefixes exact (`!` shout, `/` whisper, `:)` lover, `!#` mentor, `!!` group, `!~` guild); 4-line scrollable feed. |
| Belt | ~80% [done] | Real frames, horizontal/vertical rotate, 6 slots, number-key 1-6 item use. |
| Dura panel | ~80% [done] | Real frames, per-slot durability icons with correct 0.66/0.33 colour thresholds. |
| MiniMap | ~70% [part] | Radar projection is real (player white / NPC green / monster red). Light icon static; no new-mail indicator. |

### B. Main windows (mixed)

| Module | Fidelity | Finding |
| --- | --- | --- |
| MenuDialog (system menu) | ~80% [done] | Real Title/567 frame + 13 sprite icons at exact Crystal coords + correct routing. Missing the trade icon slot; adds a non-Crystal QA panel. |
| GameShop | ~80% [done] | Real frames, 4x2 grid, class/section tabs, search, pagination, gold/credit payment, working buy. Preview character render is an empty div. |
| Storage password | ~65% [part] | Full set/change/remove/unlock logic wired to the real backend; but a plain HTML form with zero Crystal frame art. |
| Inventory | ~62% [part] | Real frames + accurate 40-cell grid. But click-staging (no drag), no right-click context menu, no hotkeys; weight is a static image not a proportional gauge; the storage "page 2" is an invention. |
| Mail | ~60% [part] | Real frames, claim/delete work; compose/reply/pagination are stubs (only first page shows). |
| BigMap | ~55% [part] | Renders (base map + radar). Teleport button / scrollbar / search are no-op stubs; town-link rows are not clickable. |
| NPC dialog | ~45% [part] | Link click + input round-trip end-to-end (server-driven). But no original dialog frame, no buy/sell/repair service buttons, no text pagination. |
| Character | ~35% [gap] | Real frames/tabs, but NO paperdoll character preview; stat block gutted (no AC/MAC/DC/MC/SC ranges; DC/MC/SC summed into one "attack"; `maxMp` = 100); equipment slots rely on a manual `+8/+90` offset. |
| Amount box (split/drop/sell) | ~30% [gap] | Plain HTML number input; no Crystal `MirAmountBox` sprite frame, slider, or +/- spinner. |
| Item tooltip | ~20% [gap] | Shows ~4 of 15+ fields (name/desc/qty/durability/attack/defence). No grade colour, type, required stats, bonus stats, weight, price. Capped by the `DisplayItem` data model. |

### C. Social systems (weak; overall ~22%)

All eight individually-framed Crystal dialogs are collapsed into one generic
CSS tab panel (`SocialSystemPanel`); `original-ui.ts` has no frame definitions
for any of them.

| System | Fidelity | Finding |
| --- | --- | --- |
| Ranking | ~45% | Real packet + 7 tabs + live rows, but generic skin. |
| Friend | ~25% | Add/block/refresh packets work, but operate only on the selected row (no free-text add). |
| Creature | ~20% | Summon/dismiss packets work; CSS gauges + default payload, not the real management dialog. |
| Guild | ~15% | Member list + last-3 chat lines; no ranks/notice/storage/war/buff/tax; Notice button is a no-op. |
| Trade | ~15% | Missing the defining two-side item grids + per-side lock + confirm handshake; only a session row + hard-coded "offer 1 gold". |
| Group / Marriage / Mentor | ~15% | Status rows + canned-arg commands (e.g. Mentor "Add" defaults to literal "Master"). |
| Market (TrustMerchant) | ~12% | List + refresh; list/buy/cancel are hard-coded-arg stubs; not searchable / not freely buyable. |

### D. Essentially missing

| Element | Fidelity | Finding |
| --- | --- | --- |
| SkillBar (F1-F8 + number-key skills) | ~5% [gap] | Largest HUD gap. Strip asset (Prguse/2034) defined but never rendered; number keys 1-6 bound to belt *items*; skills castable only from the character spells tab. |
| BuffDialog (icon row) | ~10% [gap] | `activeBuffs` shown only as truncated HUD text; no icon row / timers / tooltips. |
| Floating combat numbers | ~0% [gap] | No damage/heal popups; no over-entity speech bubbles. |
| Options dialog | ~15% [gap] | Only music/effects on-off toggles; no volume sliders, no video/game tabs; no in-game settings panel. |
| Inspect / Mount / Fishing / Help / Keyboard / Conquest / Refine / Mining dialogs | low/none | Missing, or reduced to menu no-ops / custom mini-panels. |
| Custom cursors | 0% | Crystal's Cursors library not extracted. |

### Beyond Crystal (additions)

Mobile nipplejs joystick + action wheel, Web3 wallet login, multi-viewport
responsive scaling. Crystal is desktop-only; these are net-new.

## Priority roadmap (by leverage)

1. **Drag-and-drop** - system-level; covers inventory / character / storage /
   trade / belt at once. Highest single-point feel gap (replaces click-staging).
2. **Combat-feel trio** - SkillBar F1-F8 hotkeys + Buff icon row + floating
   damage numbers.
3. **Un-collapse social dialogs** - real per-dialog Crystal frames + real
   interactions (trade two-grid lock/confirm first).
4. **Character stat engine + paperdoll preview + full tooltip** (needs added
   `DisplayItem` fields: grade/type/required/bonus/weight/price).
5. **HUD truth fixes** - real `maxMp`, radial orb fill, Option-button wiring,
   full Options dialog.
6. **Stub fills** - NPC buy/sell/repair dialogs, BigMap teleport, mail compose.

## Status note

Like `docs/FRONTEND-1TO1-GAPS.md`, the percentages above reflect code/asset
fidelity, not human visual acceptance. Automation should not flip a dialog to
"done" without a direct Crystal screenshot/feel comparison.
