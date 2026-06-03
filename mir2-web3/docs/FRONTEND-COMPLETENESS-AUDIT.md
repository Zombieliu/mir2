# Frontend Completeness Audit

**Scope:** the mir2-web3 **player web client** (`apps/web`) — the Next.js shell,
its React UI surface, the wire-protocol handling in `app/page.tsx`, the
committed Crystal asset inventory it ships, and the Bevy WASM / WebGL2 render
runtime it drives. This audit is about the **client**, not the server-side game
simulation (that is tracked in `BACKEND-1TO1-PROGRESS.md` /
`CRYSTAL-SERVER-PARITY.md`).

**Date measured:** 2026-06-01
**Re-runnable metrics:** `apps/web/scripts/measure-frontend-coverage.mjs`

> Every number in the "Measured numbers" table below is produced by the script,
> not hand-counted. Re-run it any time the client, protocol, or assets change:
>
> ```sh
> cd apps/web
> npm run measure:frontend-coverage
> # or: node ./scripts/measure-frontend-coverage.mjs
> ```
>
> The script is dependency-free, offline, and resolves all paths from its own
> directory, so it prints identical numbers from any working directory.

---

## 1. Methodology

The script derives each signal purely from committed source so the result is
deterministic and auditable.

| Signal | Numerator | Denominator | How |
|---|---|---|---|
| **ServerPacket handlers** | distinct `case "X"` kinds in `app/page.tsx` that name a defined variant | variants in `pub enum ServerPacket` | parse the enum body by brace depth (no hard-coded line numbers); intersect with the PascalCase `case` labels in `page.tsx`. Direction (`Up`/`Down`/…) and chat-channel (`whisperin`, `shout`, equipment-slot) `case` labels belong to unrelated switches and are correctly **excluded** because they do not name a `ServerPacket` variant. |
| **ClientPacket senders** | distinct outbound `command.type === "X"` / `type: "X"` tokens that name a defined variant (case-insensitive) | variants in `pub enum ClientPacket` | union the two outbound token forms in `page.tsx`, lowercase-match against variant names (the client emits camelCase `clientVersion` for Rust `ClientVersion`). Tokens with no matching variant are higher-level UI/transport messages and are **excluded** from the numerator (see §3). |
| **UI components** | `app/components/*.tsx` files | — | `readdirSync` filtered to `.tsx`. |
| **UI dialog/window components** | the windowing subset of those files | total components | filename matches `/(dialog\|window\|panel\|overlay\|tooltip\|menu)/i`. |
| **original-ui / original-map asset libraries** | committed top-level dirs | — | `git ls-files <prefix>` → distinct first path segment. Top-level manifest `.json` files are not counted as libraries. |
| **original-ui / original-map PNG frames** | committed `*.png` | — | `git ls-files <prefix>` → `.png` count. **Committed files only** (uncommitted scratch is ignored). |
| **crystal map pack** | committed `*.map.gz` | — | `git ls-files lib/generated/crystal-map-pack` → `.map.gz` count. |

Protocol source: `packages/protocol/src/packets.rs`
Client source: `apps/web/app/page.tsx`

---

## 2. Measured numbers

Verbatim from `npm run measure:frontend-coverage` on 2026-06-01:

```
+------------------------------+-------------------------+-------+
| Signal                       | Numerator / Denominator | %     |
+------------------------------+-------------------------+-------+
| ServerPacket handlers        | 80 / 280                | 28.6% |
| ClientPacket senders         | 34 / 153                | 22.2% |
| UI dialog/window components  | 11 / 20                 | 55.0% |
| UI components (total .tsx)   | 20 components           |       |
| original-ui asset libraries  | 20 dirs                 |       |
| original-ui PNG frames       | 6,857 png               |       |
| original-map asset libraries | 3 dirs                  |       |
| original-map PNG frames      | 8,148 png               |       |
| crystal map pack             | 1,620 .map.gz           |       |
+------------------------------+-------------------------+-------+

Summary
  ServerPacket coverage : 80/280 (28.6%) variants handled by a case branch in page.tsx
  ClientPacket coverage : 34/153 (22.2%) variants emitted from page.tsx (47 distinct outbound tokens total)
  UI components         : 20 .tsx files, 11 dialog/window/panel surfaces
  original-ui assets    : 20 libraries, 6,857 committed PNG frames
  original-map assets   : 3 libraries, 8,148 committed PNG frames
  crystal map pack      : 1,620 committed .map.gz tiles
```

**Headline totals**

- **Protocol:** 80/280 ServerPacket kinds handled (28.6%); 34/153 ClientPacket
  kinds emitted directly (22.2%).
- **UI:** 20 component modules, 11 of them dialog/window/panel surfaces.
- **Assets:** 23 committed Crystal libraries (20 UI + 3 map) totalling **15,005
  committed PNG frames** (6,857 UI + 8,148 map), plus **1,620** committed
  `.map.gz` map tiles.

Supplementary render-coverage of the converted asset set (from
`docs/generated/assets/latest-asset-coverage-summary.json`): map-sprite
renderable **99.76%**, mini-map **100%**, combined render coverage **99.88%**.
Sound coverage is **0.89%** (raw `.wav` bytes are not committed — see §5).

---

## 3. Interpreting the protocol percentages

The raw protocol ratios (28.6% / 22.2%) are **floors, not the experienced
completeness**, for three concrete reasons:

1. **The enums are a superset of the live MVP wire surface.** `ServerPacket`
   (280) and `ClientPacket` (153) enumerate the *entire* historical Mir2
   protocol — guild war, mentor, hero, awakening, refine, marriage, trade,
   storage variants, etc. The Crystal MVP the web client targets
   (`packages/protocol/crystal-mvp-v1.md`) is a much smaller slice. The handled
   80 server kinds are the **gameplay-critical core**: object lifecycle
   (`ObjectPlayer/Monster/Npc/Item/Hero`), movement
   (`ObjectWalk/Run/Turn/Dash/BackStep/Pushed`), combat
   (`ObjectAttack/RangeAttack/Struck/Died`, `Magic*`), state
   (`UserInformation/Location`, health/mana/gold, buffs), and login/character
   flow. The 200 unhandled kinds are overwhelmingly endgame/social systems not
   in the MVP.

2. **Many outbound actions map *indirectly* to ClientPackets.** The script
   counts only tokens that *literally name* a `ClientPacket` variant. The client
   also emits **13 higher-level tokens** that are intentionally *not* 1:1 with a
   variant: `moveTo`, `castSkill`, `interact`, `selectNpcDialog`,
   `submitNpcInput`, `transferMap`, `specialRepairItem` (UI commands that the
   movement/skill/NPC layer translates into `walk`/`run`/`magic`/NPC-call
   packets server-side), plus internal transport messages `clientVersion`'s
   peers `error`, `packet`, `tick`, `worldSnapshot`, `stage5Command`,
   `setLanguage`. Counting *behaviour* rather than *literal variant names* would
   raise the effective client→server coverage well above 22%.

3. **A handled `case` is a real, wired UI reaction**, not a stub — these branches
   drive the scene graph, entity store, combat text, buffs, inventory, chat, and
   minimap. The denominator is inflated by dead protocol; the numerator is all
   live.

This is why §4 scores the client on weighted *experienced* subsystems, not on
the bare enum ratio.

---

## 4. Weighted scoring model

Two completeness axes are scored separately because they are bounded by
different things (this mirrors the framing in
`RESOURCE-LOADING-COMPLETION.md`):

- **Visual client** — "does the browser render and behave like the Crystal
  client?" Bounded by *client code + committed assets*.
- **Playable game** — "can a player actually log in, move, fight, loot, talk to
  NPCs, and progress end-to-end?" Bounded additionally by *protocol breadth*,
  *server parity*, and *externally-supplied asset bytes*.

Each subsystem is scored 0–100 with the rationale tied to the measured numbers
and the existing parity docs.

### 4a. "Visual client" scale

| # | Subsystem | Weight | Score | Rationale |
|---|---|:---:|:---:|---|
| V1 | Scene / map rendering (WebGL2 over committed PNGs) | 22% | 95 | 1,620 map-tile packs + 8,148 map PNGs committed; combined render coverage **99.88%**; graceful per-frame/per-library degradation. Holdback: a few source-missing frames + real-GPU sign-off. |
| V2 | Entity / actor rendering (players, monsters, NPCs, projectiles) | 18% | 88 | `ObjectPlayer/Monster/Npc/Item/Hero/Projectile` all handled; DOM fallback makes entity rendering *functionally* 100%. Holdback: GPU entity atlas is a budget-bound **starter set** (one 4096² atlas, 2,631 sprites) — full breadth needs multi-atlas runtime (§5). |
| V3 | Movement & animation feel | 15% | 90 | Crystal-authoritative walk/run/turn/dash cadence, render-only prediction, ACK reconciliation; movement-jitter QA captures clean. Holdback: last-mile real-device feel. |
| V4 | UI windows / HUD / dialogs | 16% | 90 | 20 component modules, 11 dialog/window/panel surfaces (inventory, character, storage, shop, NPC dialogs, social, system menu, map panels, tooltips, mobile controls). Covers the MVP HUD; some endgame windows (guild war, mentor) absent. |
| V5 | Map presentation (minimap / big map) | 8% | 95 | Minimap **294/294** maps, projection verified 1:1 with Crystal; big map 227/229 with minimap fallback for the 2 un-exported frames (`MAP-RENDERING-MINIMAP-GAP.md`). |
| V6 | Audio subsystem (events, fallback, settings, telemetry) | 8% | 80 | Audio *code* is ~100% (semantic event registry, presence-aware resolution, volume settings, telemetry). Score capped by committed sound **bytes** (0.89% coverage; §5). |
| V7 | Asset pipeline / caching / service worker | 8% | 92 | Versioned manifest + sha256 integrity, tiered service-worker cache with R2/CDN remote fallback, offline build mode, numeric coverage report + release preflight. |
| V8 | Renderer fallback chain & device gating | 5% | 90 | Bevy → WebGL2 → DOM auto-fallback, capability-based mobile gating. Holdback: real-GPU/mobile verification. |

**Weighted "visual client" score:**
`0.22·95 + 0.18·88 + 0.15·90 + 0.16·90 + 0.08·95 + 0.08·80 + 0.08·92 + 0.05·90`
= `20.90 + 15.84 + 13.50 + 14.40 + 7.60 + 6.40 + 7.36 + 4.50`
= **≈ 90.5% ("visual client" completeness).**

### 4b. "Playable game" scale

Adds protocol breadth and server-coupled progression, which the visual scale
does not penalise.

| # | Subsystem | Weight | Score | Rationale |
|---|---|:---:|:---:|---|
| P1 | Core loop: login → character → enter world | 14% | 92 | Full login/account/character-create/select/start handled; `LoginSuccess/StartGame*` wired. |
| P2 | Movement & navigation | 12% | 90 | As V3; map transfer handled via `transferMap` → server routing. |
| P3 | Combat (melee, ranged, magic) | 16% | 82 | Attack/range/magic cast modes (target/ground/direction/self/toggle) routed per Crystal cast mode; `Magic*`, `ObjectAttack/RangeAttack/Struck/Died` handled. Holdback: full spell-effect atlas + numeric edge cases vs server. |
| P4 | Inventory / items / equipment | 12% | 80 | Move/store/take/merge/split/equip/remove/drop/use item packets emitted and `UserSlotsRefresh`/`DuraChanged` handled. Endgame item ops (refine/awakening/disassemble) not wired. |
| P5 | NPC interaction / shops / trade | 10% | 70 | NPC dialog select + input submit + sell/repair emitted; `NPCStorage` handled. Trade/consign/market mostly server-side + unhandled client side. |
| P6 | Player progression (level, stats, buffs, gold) | 10% | 85 | `UserInformation`, base stats, `GainedGold/LoseGold/ObjectGold`, `AddBuff/RemoveBuff/PauseBuff`, level/struck handled. |
| P7 | Protocol breadth vs full Mir2 surface | 12% | 40 | Bare enum coverage 80/280 server + 34/153 client. Most of the gap is endgame/social protocol outside the MVP, but it *is* unbuilt client surface and is scored honestly low here. |
| P8 | Social systems (guild, party, mentor, relationship) | 8% | 35 | Chat channels (`whisper/shout/group/guild/system`) render; structured guild/party/mentor packets largely unhandled. |
| P9 | End-to-end asset/audio availability for play | 6% | 75 | Visuals 99.88% renderable; audio byte-limited; depends on R2 republish + raw-byte imports (§5). |

**Weighted "playable game" score:**
`0.14·92 + 0.12·90 + 0.16·82 + 0.12·80 + 0.10·70 + 0.10·85 + 0.12·40 + 0.08·35 + 0.06·75`
= `12.88 + 10.80 + 13.12 + 9.60 + 7.00 + 8.50 + 4.80 + 2.80 + 4.50`
= **≈ 74.0% ("playable game" completeness).**

### Overall

| Scale | Question | Overall % |
|---|---|:---:|
| **Visual client** | Does it render & behave like the Crystal client? | **≈ 90.5%** |
| **Playable game** | Can a player complete the full game loop end-to-end? | **≈ 74.0%** |

The ~16-point gap is almost entirely (a) unbuilt endgame/social **protocol
breadth** (P7/P8) which is outside the Crystal MVP, and (b) externally-supplied
**asset bytes** (P9) — neither is a "visual fidelity of what's shipped" problem.

---

## 5. Known external-asset blockers

These are the things that cannot be closed from client code alone in this
environment — they need original WeMade/Shanda Crystal client bytes, a real GPU,
or an out-of-band republish. They are the reason the weighted numbers are not
higher.

### B1 — R2 map-tile frame republish
The browser serves first cache-misses from an R2-backed CDN
(`NEXT_PUBLIC_MIR2_ASSET_BASE_URL`, production base
`https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c`), keying the browser
cache to the local same-origin path. Some active-scene map-object frames (e.g.
`original-map/WemadeMir2/Objects/2652..2661`, `Objects23/1418..1429`,
`Objects/289.png`) were missing from earlier immutable R2 prefixes, producing
console 404 storms until republished. **Blocker:** keeping the R2/CDN object set
in lockstep with `original-asset-manifest.generated.json` for each immutable
asset version requires re-running the remote-asset release+upload+HEAD-verify
workflow (`npm run assets:remote:build` → R2 upload) — an out-of-band publish
step, not a code change. A few frames (e.g. `Objects/289.png`) are also absent
from the local source tree, so they need the original `.Lib` export first.

### B2 — Effect / entity atlas export (GPU breadth)
Entity rendering is *functionally* 100% via the hardened DOM fallback, but the
**GPU** path packs a single 4096² atlas (`build-bevy-entity-atlas-pack.mjs`,
starter roots: `CArmour/CHair/CWeapon`, `AArmour/AHair/AWeapon`, `NPC`,
`Monster/000…139`) holding the ~2,631-sprite playable starter set — it is at its
texture budget. **Blocker:** covering *every* entity/effect on the GPU path
(full monster roster + magic/spell effect frames) needs a **multi-atlas runtime**
in `webgl2-entity-atlas-layer.tsx` plus the effect frames exported from the
Crystal `.Lib` files. Until then, atlas breadth is the starter set and overflow
renders via DOM.

### B3 — Audio bytes in R2
The audio **system** is code-complete (semantic event registry, presence-aware
resolution, fallback chains, volume settings, telemetry), but **446 of 450**
`.wav` files are not committed — they are proprietary WeMade/Shanda client data
(the public `Suprcode/Crystal` repo is C# *source*, not game assets), so measured
sound coverage is **0.89%**. **Blocker:** importing the real `Sound/` set
(`CRYSTAL_CLIENT_ROOT=<path>/Debug npm run export:crystal-sounds &&
npm run generate:present-sounds`, per `SOUND-IMPORT-RUNBOOK.md`) and uploading
them to R2 under the asset version. Fabricating placeholder audio was
deliberately rejected as false completeness. Until then the client resolves only
present sounds and degrades the rest silently (no 404s).

### B4 — Bevy WASM actor fidelity (real-GPU sign-off)
The Bevy WASM runtime (WebGPU + WebGL2 backends, version-pinned via
`lib/generated/bevy_runtime_version.json`) drives the scene, with a WebGL2→DOM
auto-fallback and capability-based mobile gating. These paths are verified by
type-check, unit logic, and review, but **pixel/behaviour parity of actor
rendering needs a real browser/GPU**, which this sandbox lacks; WebGPU/WebGL
support also varies across platforms/WebViews (`PLATFORM-CLIENT-STRATEGY.md`).
**Blocker:** a real-device GPU/mobile QA pass (and, for full actor fidelity, the
multi-atlas work in B2) is the last mile — it cannot be honestly signed off
without hardware.

---

## 6. Re-running this audit

```sh
cd apps/web
npm run measure:frontend-coverage          # prints the table in §2
# equivalently:
node ./scripts/measure-frontend-coverage.mjs
```

The script exits 0, has no dependencies, makes no network calls, and resolves
all inputs relative to its own location, so it is safe to run from anywhere and
on CI. When the protocol, `page.tsx`, components, or committed assets change,
re-run it and update §2 (and revisit the §4 scores).

---

## §6. Re-index snapshot — 2026-06-02 (branch `claude/fe-integration`)

State after the three consolidation batches (packets / windows / VFX / SW / sim-parity / outbound / tests). **Not yet merged to `main` / not deployed.**

### Hard metrics (from `measure:frontend-coverage`)

| Signal | Value |
|---|---|
| ServerPacket handlers (inbound) | 276 / 280 (98.6%) |
| ClientPacket senders (outbound) | 51 / 153 (33.3%; ~65 distinct true) |
| UI dialog/window components | 27 / 36 (75%) |
| Total components (.tsx) | 36 |
| original-ui assets | 20 libs / 6,857 PNG |
| original-map assets | 3 libs / 8,148 PNG |
| crystal map pack | 1,620 .map.gz |

### Per-module completion (estimates w/ evidence)

| Module | % | Notes |
|---|---|---|
| Login / account / char-select (passkey) | 90% | mature |
| Core HUD (HP/MP orbs, quick/skill bar, gold) | 85% | quickslot/belt present |
| Scene / map-tile render (back/mid/front/light) | 88% | 1,620-tile pack, real tiles |
| Mini-map / big-map | 85% | present |
| Actor / monster sprite render (Bevy WASM) | ~60% | 8-dir+anim; **name tags / health bars / chat bubbles missing**; sprite frames currently 404 from R2 |
| Chat (channels / input / log) | 75% | over-head bubbles missing |
| Inventory / storage | 85% | |
| Character / stats / equipment | 85% | |
| Skill / magic book + skill bar | 80% | |
| NPC dialog / shop / repair | 80% | |
| Trade | 78% | actions partly wired |
| Social: guild / group / friends / mentor-marriage | 80% | real data, most actions wired |
| Quest log | 72% | track/abandon disabled (no packet) |
| Hero / pet / intelligent creatures | 70% | real data; summon etc. disabled |
| Mail | 75% | claim/delete wired |
| Ranking / Market-auction | 80% | data + requests wired |
| Conquest / guild-war | 70% | start-war wired; gate/tax via NPC |
| Buff display | 75% | real data |
| Help / hotkeys / world-map / chat-settings | 78% | static/config |
| Options + audio settings | 70% | |
| Mobile controls / input | 80% | SW + chunk + joystick hardened |
| VFX / effects layer | 60% | 11 elements × 11 archetypes procedural; **real atlases pending** |

### Weighted overall (visual/UI client): **≈ 75% (range 72–78%)**

### Caps that block a blanket ">95% everywhere" from the sandbox
- **VFX real atlases** + **audio** + **live sprite serving** → need the real Crystal client `.Lib` files + R2 credentials + deploy (the `/Crystal` C# submodule carries source, **0 `.Lib` assets**).
- **Actor sprite fidelity** in the Bevy WASM runtime → heavy `wasm-bindgen` rebuild.
- Code-doable toward ~95% (in-sandbox, cargo + Crystal C# source now available): outbound/window actions (via new gateway BrowserCommands), HUD/chat/inventory/character polish, entity name-tag/health-bar overlays, and deeper backend Crystal 1:1.

## §7. Batch 4 results — 2026-06-02 (4 parallel agents → `claude/fe-integration`)

Goal for this batch was **"push every code-doable module ≥95%."** Verdict: the code-doable
surfaces advanced strongly (several modules from the 70–78% band into 82–90%), but a
**blanket ≥95% everywhere was NOT reached and cannot be from the sandbox** — the ceilings on
actor-sprite render (≈68%) and VFX (60%) are asset/deploy-gated (see §5 / §6 caps), not code.

Branches merged (all verified, disjoint domains): `fe4-ui-polish` (6 components),
`fe4-scene-hud` (shell + new `scene-overlays.tsx`), `fe4-outbound` (gateway `web.rs` +
`page.tsx`), `fe4-sim-parity` (`apps/simulation`).

### Verification gates (integration context, post-merge)
- `npx tsc --noEmit` → **exit 0**
- `cargo check -p mir2-gateway` → **exit 0**
- `cargo test -p mir2-simulation` → **1207 passed / 0 failed** (was 1205; +2 crit/magic-parity fixes, 0 regressions)

### Hard-metric deltas (`measure:frontend-coverage`)
| Signal | §6 (before) | §7 (after) |
|---|---|---|
| ServerPacket handlers | 276/280 (98.6%) | 276/280 (98.6%) — at practical max |
| ClientPacket senders (page.tsx-local) | 51/153 (~65 true) | 63/153 (77 distinct page.tsx-visible) |
| Outbound bridge (gateway `browser_command_to_action`) | — | **111 distinct ClientPacket** of 153 (≈72.5%) |
| UI dialog/window components | 27/36 | 28/37 (+`scene-overlays.tsx`) |

### Per-module deltas (only modules this batch moved)
| Module | §6 → §7 | What changed |
|---|---|---|
| Actor/monster sprite render | ~60% → **~68%** | DOM overlays added (health bars, selection ring, target readout); **sprite frames still 404 from R2 — asset-gated, the real cap** |
| Chat | 75% → **82%** | over-head chat bubbles now derived from chat log + rendered |
| Quest log | 72% → **85%** | track→`ShareQuest`, abandon→`AbandonQuest` wired (4 quest BrowserCommands added) |
| Hero / pet / creatures | 70% → **82%** | summon hero→`ChangeHero`, creature summon/release/cycle→`UpdateIntelligentCreature`; dismiss/recall still blocked (no protocol packet) |
| Social: guild/group/friends | 80% → **88%** | guild notice/invite/kick, group invite/kick/leave, friend whisper wired |
| Trade | 78% → **85%** | accept/confirm/cancel → `TradeReply`/`TradeConfirm`/`TradeCancel` |
| Inventory / storage | 85% → **90%** | stack-split steppers + slider + Min/Half/Max, gold-drop quick-amounts, locale gold |
| Character / stats / equipment | 85% → **90%** | stat pages in Crystal row order, AC/DC ranges, paperdoll badge, richer equip tooltips |
| Item tooltip (shared) | — | rebuilt: grade colors, AC/MAC/DC/MC/SC ranges, flat bonuses, requirements, bind/seal lines |
| Options + audio | 70% → **80%** | master-volume slider + mute-all; system-menu Help/Keyboard overlays wired |
| Combat / AI / magic (backend) | — | crit weights (Rate×5/Dmg×50) + magic `GetDamage` multiplier/truncation fixed to Crystal 1:1 |

### Weighted overall (visual/UI client): **≈ 78% (range 76–81%)** — up from ≈75%

### Still NOT ≥95% — honest gaps that remain (no code path from the sandbox)
1. **Live sprite serving** — `/original-ui/Monster/...` frames exist in git but are pruned from the Vercel output and served only from R2; the R2 release is stale/partial → 404. Fix is an **R2 republish** (creds + deploy), not on these branches.
2. **VFX real atlases** (60%) and **audio bytes** (in R2) — need real Crystal `.Lib` extraction on a real machine.
3. **Actor sprite fidelity** — Bevy WASM `wasm-bindgen` rebuild.
4. **Unwirable window actions** — hero dismiss/recall, conquest gate/tax, market consign (no item/price on button), mail compose (no mail component): each needs a **new protocol packet or sim handler or UI component**, deliberately out of this batch's additive scope.
5. **Not merged to `main` / not deployed** — `claude/fe-integration` is 34 commits ahead of `origin/main`.

