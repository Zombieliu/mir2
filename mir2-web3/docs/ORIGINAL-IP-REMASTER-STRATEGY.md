# Original IP Remaster And Global Launch Strategy

Last updated: 2026-06-13

Status: strategy draft.

Purpose: define how to turn the current Crystal/Mir2 1:1 foundation into an
**original-IP, Mir2-like, web-first cross-platform game for global release** —
without licensing the Mir2 IP and without shipping the original copyrighted
assets. This is the legal/branding execution layer for the direction already
stated in `docs/POST-1TO1-EVOLUTION-PLAN.md` ("moving from pure 1:1
reconstruction toward a custom MMORPG built on the proven Mir2-style
foundation"). It complements:

- `docs/POST-1TO1-EVOLUTION-PLAN.md` — when divergence from Crystal is allowed.
- `docs/PLATFORM-CLIENT-STRATEGY.md` — platform coverage stance.
- `docs/FRONTEND-COMPLETENESS-AUDIT.md` — per-module completeness.
- `docs/ASSET-RELEASE-RUNBOOK.md` — how assets are published to R2/CDN.

> **Not legal advice.** The legal section below states general principles only.
> Before any commercial launch, retain qualified IP counsel in **each** target
> market (the Mir2 / "Legend of Mir" / 传奇 IP is co-owned and heavily
> litigated; see the Wemade/Actoz/Shanda disputes).

---

## TL;DR

1. **The engine is ~90% done and already cross-platform and i18n-ready.** Server
   logic, data-driven content, localization, and mobile/touch input already
   exist (citations in the Appendix).
2. **The original-IP remaster is a *data + art swap*, not an engine rewrite.**
   Content names travel the wire as strings sourced from JSON manifests; the
   asset CDN is a single env var; UI strings are fully externalized.
3. **The real cost is content production, not engineering:** ~15,000 art frames
   to redraw and ~2,700 proprietary names to replace. That is an **art + design**
   project sitting on top of a finished engine.
4. **The one code-level coupling** (internal `Spell` enum identifiers) **does not
   need to change** for rebranding — those identifiers are never shown to players.
5. Beyond the reskin lies the standard **live-game operations layer** (accounts,
   persistence, payments, hosting/scale, anti-cheat, store presence). The
   server-authoritative design already gives us a head start.

---

## 1. Scope And Positioning

**What we are building:** an original-IP MMORPG in the same *genre and feel* as
Mir2 (2.5D top-down, level/loot/PK/guild-siege loop), running primarily in the
browser, packaged to desktop and mobile, released globally.

**What we are explicitly NOT building:**

- Not a licensed Mir2 / 传奇 product (the license path is a legal minefield —
  co-owned copyright, frozen license agreements; out of reach without
  simultaneously satisfying both co-owners).
- Not a "private server" distributing the original game's extracted art, audio,
  names, or maps. Genre is free; the original *expression* is not.

**Design invariant:** the **mechanics** are ours to reproduce (game rules are not
copyrightable); the **expression** (sprites, maps, audio, proper nouns, story)
and the **marks** (传奇 / Legend of Mir / Mir2) must be 100% replaced.

---

## 2. Legal Posture (principles, not advice)

What is **not** protected and free to reproduce:

- The genre and gameplay systems: isometric grid movement, level/EXP curves,
  loot tables, PK/karma, guild sieges, the three-class triangle, etc.
- The **server engine logic** reimplemented from the open-source Crystal
  emulator. Rules and mechanics are not copyrightable subject matter.

What **is** protected and must be replaced:

- **Art**: every sprite/tile/UI frame, animations, particle/effect art.
- **Audio**: BGM and SFX.
- **Proper nouns**: map names (Sabuk, Bichon...), monster/item/NPC/skill names,
  lore and quest text.
- **Trademarks/trade dress**: the names 传奇 / Legend of Mir / Mir2, logos, and
  any look-and-feel that would cause players to believe this *is* Mir2.

### Three routes (and why we pick #2)

| Route | Description | Verdict |
|---|---|---|
| 1. Personal / open-source, non-commercial | Current repo posture. Lowest risk, but publicly redistributing extracted original assets is still infringement. | Fine for R&D; not a business. |
| **2. Original-IP "Mir2-like" (this doc)** | Keep the engine; replace 100% of protected expression and marks; ship globally. | **Chosen.** Sidesteps the IP fight; commercial-viable. |
| 3. Licensed Mir2 | Obtain a real license. | Impractical: co-owned IP, litigation, frozen deals. |

### Clean-rebrand red lines

To make "original" actually hold:

- **No homophone/near-miss names** (e.g. Sabuk → "Sabuck" is not enough).
- **No tracing or palette-swapping** original sprites; new art must be
  independently authored from an original art-direction brief.
- **No confusingly similar trade dress** (UI layout that is *functionally*
  similar is OK; pixel-identical skins/logos are not).
- **Per-market review**: trademark and copyright tests differ by jurisdiction;
  clear names and key art with counsel for each launch region.

---

## 3. Current Decoupling State (audited)

The codebase is already structured so that the reskin is mostly data + assets.
Verified findings (full `file:line` evidence in the Appendix):

| Layer | State | Implication for rebrand |
|---|---|---|
| Server logic (sim/protocol/gateway) | 1:1, pure mechanics | Use as-is; not IP-bearing. |
| Content names (monster/item/map/NPC) | **Data-driven** JSON manifests; names sent over the wire as **strings** | Rename = edit data; **zero** engine/protocol/client code change. |
| Skill/magic names | User-facing names data-driven + localized; only **internal** `Spell` enum ids hardcoded | Rebrand needs **no** code change (internal ids are never shown). |
| Asset CDN base | **Centralized** behind `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` | Swap art set by pointing the env var at a new bucket. |
| Asset path layout | ~220 `/original-ui` path literals in `lib/original-ui.ts` | Don't rename them — **mirror the directory structure** on the new bucket; literals become opaque keys. |
| UI strings (i18n) | **Fully externalized**: `localization_bundle.json` (en/zh-CN/es), 415 `t()` calls, zero inline strings | Multi-language is ready; swap/extend the bundle. |
| Mobile/touch | **Implemented**: virtual joystick (Nipple.js), responsive `@media`, pointer events | Cross-platform input already works with new art. |
| Art assets | ~6,900 UI PNGs + ~8,100 map PNGs ≈ **15,000 frames** | The actual long pole of the project. |

---

## 4. The Rebrand / Naming Pipeline (the keystone)

**Goal:** make "rename everything to our IP" a repeatable data operation, not a
hand-edit of thousands of strings across ~20 manifests.

**Design (additive, non-destructive to Crystal source data):**

1. Author a single override map, e.g.
   `packages/game-data/data/overrides/brand_overrides.json`:
   `{ "monster": { "ArcherGuard": "WardenSentinel", ... }, "item": {...}, "map": {...}, "npc": {...}, "magic": {...} }`.
2. A generation step consumes the **read-only** Crystal manifests
   (`packages/game-data/data/generated/crystal_*_manifest.json`) plus the
   override map and emits **branded** manifests the runtime loads. Crystal source
   data stays untouched as the parity oracle.
3. The branded display names are wired into `localization_bundle.json` keys, so
   renaming and translation become one pipeline (good for global launch).
4. Anything without an explicit override falls back to a generated placeholder
   (so nothing ships under an original Mir2 name by accident — fail closed).

**Volume to cover** (measured): ~555 monster names, ~1,600 item names, ~100+
maps (463 respawn entries incl. repeats), the magic table (`crystal_magic_manifest.json`),
plus quest/NPC script text. Scriptable; the work is *creative naming*, not typing.

**Explicit non-goal:** renaming the internal `Spell` enum variants
(`FireBall`, `Healing`, ...). They are numeric over the wire and never shown to
players; touching them is a cross-protocol change with no rebrand benefit. Leave
them; localize the player-facing spell names via data only.

---

## 5. The Art Pipeline (the real long pole)

~15,000 frames is the schedule driver. Treat it as a production line, not a
code task.

**Inventory** (Appendix has the directory shape):

- `public/original-ui/` ≈ 6,900 PNG: class equipment (armour/hair/weapon
  variants), `Monster/<id>/`, `NPC/<id>/`, `Items/`, UI frames (`Prguse*`),
  cursors, minimap, character select.
- `public/original-map/` ≈ 8,100 PNG: tiles + objects across map variants
  (`ShandaMir2`, `WemadeMir2`, `WemadeMir3`).

**Mechanics of swapping (trivial engineering):**

- Stand up a **separate asset bucket** (own R2/CDN) and point
  `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` at it (per `docs/ASSET-RELEASE-RUNBOOK.md`).
- **Mirror the existing directory structure** with new art at the **same
  resolution and frame anchors/offsets** so the renderer needs no change.
- Keep `meta.json` per sprite folder (frame metadata) compatible.

**Phasing — do NOT redraw all 15k at once:**

- **Wave 0 (vertical slice):** one starting zone + ~30 core monsters + 4 classes'
  starter equipment + core UI/cursor/minimap. Enough for an end-to-end playable
  demo under the new brand.
- **Wave 1+:** roll outward by zone/level band, retiring content we don't ship.

**Sourcing options (mix them):**

- Commissioned artists (highest originality/quality, slowest, costliest).
- Licensed isometric/MMO sprite packs (fast, but verify license + check no
  pack is itself a Mir2 rip).
- AI-assisted batch generation against an original art-direction brief
  (fastest at volume) — **must** produce independent expression, never
  near-copies of specific original frames.

**QA gates:** resolution/anchor match (no rendering drift), visual-distinctness
review vs the original (legal), and style consistency across waves.

---

## 6. Globalization

Already strong: `lib/localization.ts` + `localization_bundle.json` ship en /
zh-CN / es with full-coverage `t()` usage and inline fallbacks.

To launch globally:

- Extend the bundle with target launch languages; fold branded names in (§4).
- Verify locale-sensitive formatting (numbers/dates/currency) at the
  `languageLocale()` boundary.
- Plan font coverage (CJK, Cyrillic, etc.) for the canvas/HTML UI.

---

## 7. Cross-Platform Packaging

Web is primary and already mobile-capable (touch + responsive). Per
`docs/PLATFORM-CLIENT-STRATEGY.md`:

- **Browser (desktop + mobile):** done; the long pole is art, not platform code.
- **PWA:** add manifest + service worker for "install to home screen".
- **App stores (iOS/Android):** wrap the web client with **Capacitor** (or
  **Tauri** for desktop) once a vertical slice is stable. Keep the server
  protocol frontend-agnostic so the wrapper stays thin.

---

## 8. Live-Game Operations Layer

Shipping a global game is more than the reskin. Largely a separate workstream;
the server-authoritative design helps. Track against existing runbooks where
they exist:

- **Accounts/auth, persistence (Postgres), sessions (Redis):** see
  `docs/PERSISTENCE-OPERATIONS-RUNBOOK.md`, `docs/ARCHITECTURE-CURRENT.md`.
- **Payments/monetization:** store + in-game economy; sandbox first.
- **Hosting/scale/zoning:** see `docs/SCALABILITY-AND-CAPACITY.md`,
  `docs/L2-ECS-ZONE-DESIGN.md`.
- **Anti-cheat:** server authority is the foundation; add validation + telemetry.
- **Compliance:** ToS/privacy, age ratings, and per-region game regulations
  (note: a mainland-China launch additionally requires a 版号/ISBN approval — a
  different and much heavier track; global-first sidesteps this initially).

---

## 9. Phased Roadmap

| Milestone | Goal | Dominant effort |
|---|---|---|
| **M0 — Vertical Slice** | Rebrand pipeline (§4) + Wave-0 art (§5) → an end-to-end playable demo fully under the new brand and names. Proves the whole reskin loop. | Small eng + focused art |
| **M1 — Content Reskin Fill** | Roll naming + art across the full content set in priority waves; complete launch-language localization. | Art + design (long pole) |
| **M2 — Closed Beta / Soft Launch** | Harden the ops layer (accounts, persistence, payments sandbox, anti-cheat, load tests); soft-launch one region with telemetry + live-ops. | Backend + ops |
| **M3 — Global Launch** | Multi-region hosting, PWA + store wrappers, monetization live, marketing, per-market legal sign-off. | Ops + business |

---

## 10. Risk Register

| Risk | Impact | Mitigation |
|---|---|---|
| Rebrand not "clean enough" (derivative/trademark exposure) | Legal takedown, suit | Original art-direction brief; no tracing/homophones; per-market counsel review of names + key art |
| Art volume (~15k) drives schedule | Slips launch | Vertical-slice + waves; pack/AI hybrid; cut unused content |
| Scope creep vs parity oracle | Lose testability | Per `POST-1TO1-EVOLUTION-PLAN.md`: brand/data may diverge; keep mechanics testable against Crystal |
| Live-game ops underestimated | Unstable launch | Treat as its own workstream; reuse server authority + ops runbooks |
| Tempting internal-id renames | Needless protocol churn | Don't rename `Spell` enum ids; localize display names only |

---

## 11. Immediate Next Steps

1. **(eng)** Build the rebrand/override pipeline (§4): `brand_overrides.json` +
   a generation step emitting branded manifests, wired into
   `localization_bundle.json`. Additive; Crystal source data stays read-only.
2. **(art)** Stand up a separate asset bucket + mirror-structure convention;
   draft the Wave-0 redraw list (starting zone + ~30 monsters + 4 classes + core UI).
3. **(legal)** Draft the original brand: name + art-direction brief; line up IP
   counsel for the first target markets.
4. **(product)** Choose launch languages and the first soft-launch region.

---

## Appendix — Audit Evidence

Findings backing §3 (cited so future work can verify, per the repo's
`file:line` convention).

**Content names are data-driven and travel as strings:**

- Manifests: `packages/game-data/data/generated/crystal_{monster,item,respawn,npc_info,magic}_manifest.json`,
  loaded via `packages/game-data/src/lib.rs` (~`:1644`–`:1727`).
- `MonsterInfo { name: String }` — `packages/protocol/src/types.rs:3020-3041`;
  populated `apps/simulation/src/runtime/packets.rs:3911-3932`.
- `ItemInfo { name: String }` — `packages/protocol/src/types.rs:2731-2764`.
- `NpcInfo { name: String }` — `packages/protocol/src/types.rs:3096-3105`;
  populated `apps/simulation/src/runtime/packets.rs:3951-3961`.
- `MapInformation { title, file_name: String }` — `packages/protocol/src/types.rs:3339-3350`;
  populated `apps/simulation/src/runtime/npc_script.rs:2169-2191`.

**Only internal spell ids are hardcoded (not player-visible):**

- `Spell` enum — `packages/protocol/src/types.rs:209-373`.
- `canonical_spell_name()` — `apps/simulation/src/runtime/skills.rs:168-283`.

**Asset CDN is centralized:**

- `createRemoteAssetConfig()` — `apps/web/app/api/asset-manifest/route.ts:295-310`.
- Env `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` / `MIR2_ASSET_BASE_URL`; `.env.example:19`.
- ~220 `/original-ui` literals — `apps/web/lib/original-ui.ts`.
- Static prefixes — `apps/web/app/components/asset-cache-registrar.tsx:122-128`.
- Rewrite worker — `public/mir2-asset-worker.js`.

**i18n fully externalized:**

- `apps/web/lib/localization.ts` + `apps/web/lib/generated/localization_bundle.json` (en/zh-CN/es).
- Translator built `apps/web/app/page.tsx:1538-1539`; 415 `t()` usages in `page.tsx`.

**Mobile/touch implemented:**

- `apps/web/app/components/original-client-mobile-input.ts:59-117` (joystick → direction/move).
- `apps/web/app/components/original-client-mobile-controls.tsx` (Nipple.js, action wheel).
- `apps/web/app/globals.css` — 8 responsive `@media` queries.

**Art inventory (rough):**

- `public/original-ui/` ≈ 6,900 PNG across ~21 top dirs (class equipment,
  `Monster/<id>/`, `NPC/<id>/`, `Items/`, `Prguse*` UI, cursors, minimap, char-select).
- `public/original-map/` ≈ 8,100 PNG across `ShandaMir2` / `WemadeMir2` /
  `WemadeMir3` (Tiles/SmTiles/Objects layers).
- `public/original-effects/effects.generated.json` (effects metadata).

**Manifest sizes (rename/translation surface):** item 1.5 MB, npc 5.1 MB,
respawn 6.2 MB, drop 23 MB, magic 50 KB, **localization_bundle 466 KB**.
