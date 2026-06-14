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

---

## Launch Regions & Regulatory (LATAM / SEA)

Last researched: 2026-06-14. Status: research synthesis — **NOT legal advice**;
retain local IP/gaming + tax counsel per market before any commercial launch.

> **Methodology / confidence.** Compiled from a multi-agent web-research fan-out over
> authoritative sources (Tilleke & Gibbins, Baker McKenzie, DLA Piper, Mayer Brown,
> Hogan Lovells, SSEK, ABNR, Niko Partners, Xsolla, Antom, USTR, WIPO, national
> regulators). Direct page fetches were frequently bot-blocked (HTTP 403), so most
> claims are search-summarised from those sources rather than primary-text reads, and
> part of the fan-out was rate-limited and re-run. Each claim is tagged
> **[high]/[med]/[low]**; time-sensitive 2024–2026 licensing / child-safety / tax
> rules **must be re-verified** before relying.

### Headline

1. **Geography is not an IP shield.** All five markets are Berne Convention + WTO/TRIPS
   members, so the original Mir2's protected expression is automatically protected in
   each. **[high]**
2. **But there is no local Mir licensee to fear in any of the five.** Wemade
   **self-publishes** MIR4 / MIR M / Legend of YMIR globally via WEMIX PLAY, using only
   service partners (Xsolla payments, Razer promo, Coins.ph token) — **no local
   game-publishing licensee found in BR/ID/PH/TH/VN**. The "most-motivated local
   litigant" essentially does not exist; the rights-holder of record everywhere is
   Wemade (Korea). **[med-high; absence actively searched]**
3. **Wemade is extremely litigious — but over the NAME, not the mechanics.** ~65 suits
   across China/Korea/Singapore; wins vs Chinese "传奇类" clones; ₩-billions in
   arbitration; a live ~US$600M royalty fight with Shengqu (Apr 2025). Its cases target
   use of the **"传奇 / Legend of Mir / MIR" mark or false license claims** — not generic
   isometric-MMO mechanics, and not (on the record) in Brazil/SEA. **An original-name,
   original-asset clone specifically sidesteps Wemade's actual litigation trigger** —
   exactly the strategy in this doc. **[med]**
4. **The real, country-independent risk is platform takedown.** A single rights-holder
   IP complaint to Apple/Google can pull a title from all stores fast, regardless of
   where you incorporate; payment rails run through the same gatekeepers. **Clean
   originality matters more than launch geography.** **[high]**
5. **Wemade is strongest in Brazil + Philippines** (Portuguese + South-America servers;
   a Manila server + WEMIX on Coins.ph) — its brand is most actively in-market exactly
   where this game most wants to launch, so visible differentiation matters most there.
   **[high]**
6. **Regulatory entry barrier (easy→hard): Brazil ≈ Philippines < Thailand < Indonesia
   < Vietnam.** The SEA markets often assumed "easy" (Vietnam, Indonesia) are the
   heaviest lifts; Brazil and the Philippines are the genuinely open ones. **[high]**

### Decision-critical: IP & enforcement (cross-country)

- **Berne + WTO/TRIPS:** Brazil (Berne 1922 / WTO 1995), Indonesia (1997 / 1995 / +WCT),
  Philippines (1951 / 1995), Thailand (1931 / 1995), Vietnam (Berne 2004 / WTO 2007 /
  WCT 2022). All auto-protect foreign copyright. **[high]**
- **Trademarks:** all five are in the Madrid Protocol, so Wemade/Actoz could extend
  MIR/传奇 marks cheaply; **whether the marks are actually registered in each country,
  and by whom, is UNVERIFIED** (registers not queryable this pass) — keep name/logo
  clearly distinct. **[low]**
- **USTR Special 301 (2024):** Indonesia = **Priority Watch List** (weakest rep);
  Brazil / Thailand / Vietnam = Watch List; **Philippines = not listed** (cleanest).
  **[high]**
- **Vietnam is the structural outlier:** Decree 147/2024 requires any online game (clone
  or not) to run through a licensed local entity, and the state can block unlicensed
  titles — an enforcement gate independent of Wemade. **[high]**

### Per-country

#### Brazil — recommended first beachhead

- **Market / fit [high]:** Largest LatAm market (~US$2.2–2.7B; ~100–115M gamers);
  **Tibia's single largest country** (~29% of tibia.com traffic) — canonical proof a
  top-down open-PK MMO can be carried by Brazil; MU/RO huge; MIR4 live & monetising
  (Portuguese, South-America servers).
- **Licensing [high]:** **No game-operating license** for non-gambling F2P (the heavy
  "Brazil gaming license" is real-money-gambling only, Lei 14.790/2023). Marco Legal dos
  Games (Lei 14.852/2024) is an industry-development law, not a license. Age rating
  mandatory but **self-declared via IARC** (Portaria 368/2014; overhauled by Portaria
  MJSP 1.048/2025, phased Nov-2025 / Mar-2026).
- **⚠️ ECA Digital (Lei 15.211/2025, in force 17 Mar 2026):** for games accessible to
  minors — **bans paid loot boxes**, **requires real age verification (no
  self-declaration)**, parental consent + account-linking under-16, parental controls,
  no dark patterns, and **Art. 40 mandates a legal representative in Brazil**. Penalties
  up to R$50M or 10% of group's BR revenue. *This forces a Brazil legal footprint even
  without a "game license."* **[high]**
- **Data (LGPD) [high]:** extraterritorial; **no local-rep requirement** (contrast
  GDPR); DPO required (small-biz exemption, Res. 18/2024); no data localization;
  cross-border via ANPD SCCs (mandatory since 2024). Fines up to 2% BR revenue / R$50M.
- **Tax / payments [high]:** stores withhold ~25% on non-BRL payouts → use a
  **merchant-of-record (PagBrasil / EBANX / Xsolla)** settling in BRL / **Pix** (57% of
  BR payments; 160M users) + **Boleto** (68.9% use cash). Avoids a local entity/bank for
  monetisation (separate from the ECA-Digital legal-rep duty).
- **Hosting:** São Paulo (AWS `sa-east-1` / GCP / Azure), Cloudflare SP+Rio;
  single-digit–~20ms in-metro.

#### Philippines — recommended second (cleanest SEA entry)

- **Market / fit [high]:** SEA's #2 by players (~67.7M); mobile-first with strong
  PC-café MMO legacy (RO/MU); **Wemade's MIR4/YMIR a top market here** (Manila server
  ~20–40ms; WEMIX on Coins.ph 2026); English-friendly.
- **Licensing [high]:** **No game license** for non-gambling games; **PAGCOR is
  gambling-only** (POGO banned 2024) — does not apply. Software/online services **100%
  foreign-ownable** (not on the Negative List); a purely offshore operator can serve PH
  players with no local entity.
- **Entity / capital [high]:** a domestic-serving entity >40% foreign needs FIA min
  paid-up **US$200k** (→ **US$100k** with advanced tech or ≥50 (or ≥15 startup) Filipino
  staff; export-enterprise exempt).
- **Data (DPA 2012) [high]:** extraterritorial; DPO + NPC system registration above
  thresholds (250 staff / 1,000 sensitive records / risk); admin fines 0.25–3% of gross
  income, ₱5M/act cap.
- **Tax [high]:** **VAT on Digital Services (RA 12023): 12% VAT**, register with BIR if
  >**₱3M/yr** to PH consumers (games in scope; live ~Jun 2025); plus Internet
  Transactions Act 2023 (extraterritorial consumer rules).
- **Payments [high]:** **GCash + Maya**; carrier billing >25% of micro-txns. Hosting:
  **Singapore** (no local region); Manila→SG ~35–60ms (OK for tab-target/grind PvP);
  Cloudflare MNL edge.

#### Thailand — second-wave (open now, watch the draft Game Act)

- **Market / fit [high]:** Largest SEA market by revenue (~$2.2B → $2.4B by 2029);
  **highest spend per payer in SEA** (~$393; 49% pay); RO's strongest SEA market; mobile
  69%; highest internet penetration of the five (88%).
- **Licensing [med-high]:** **No dedicated online-game license today**; online/cloud/
  mobile games are **outside the Film & Video Act rating regime** (which catches only
  physical media) → no mandatory pre-release rating. A purely offshore operator can
  currently sell to Thai users with no Thai license. **⚠️ Watch:** DEPA's draft **Game
  Industry Act** (advancing 2024–25, not yet enacted) would add registration/licensing
  (refs a "G1" license), target loot-box/gacha "hidden gambling," and could bind
  offshore operators + end the online-rating exemption.
- **Entity (FBA) [high]:** game services fall in **List 3** → foreign-majority needs a
  **Foreign Business License**; routes: Thai-majority (≤49% foreign), **BOI promotion
  (100% + tax breaks for digital/software)**, or **US Treaty of Amity (100% US-owned)**.
  FBA bites only if you "carry on business in Thailand" — pure offshore web-serving
  generally isn't.
- **Data (PDPA) [high]:** extraterritorial; **foreign operator MUST appoint a Thailand
  representative (s.37(5))**; DPO in defined cases; no localization; cross-border via
  adequacy/SCC routes (since Mar 2024); admin fines to ฿5M + criminal/civil; enforcement
  active (฿7M fine Nov 2024).
- **Tax [high]:** **e-Service VAT 7%**, register if >**฿1.8M/yr** B2C (games in scope);
  **CCA** log-retention ≥90 days + 24h takedown. Payments: TrueMoney / PromptPay /
  ShopeePay / K PLUS / AIS. Hosting: **AWS Thailand live Jan 2025**, **GCP Thailand Jan
  2026** → in-country single-digit ms; else Singapore.

#### Indonesia — big but heavy (needs local entity/partner)

- **Market / fit [high]:** SEA's largest by revenue; "mobile-only" (83% smartphone);
  RO/MU/Seal heritage; MIR4 + Legend of YMIR localised for Indonesia; low ARPU (~$4.8),
  huge base.
- **Licensing (heavy) [high→med]:** (1) **PSE registration** (PP71/2019 + MR5/2020) —
  offshore games in scope; non-registration → **access blocking** (2022 precedent:
  Steam/Epic/PayPal). (2) **IGRS rating** (MR 2/2024; 3+/7+/13+/15+/18+/RC; enforced
  from Jan 2026 — but **IGRS suspended ~Apr 2026 after a data breach**, status fluid).
  (3) **Perpres 19/2024: game publishers must be an Indonesian PT** → foreign needs **PT
  PMA (up to 100%, verify KBLI) or local JV** + taxable entity (binding lex specialis
  vs the general PSE "no entity needed"). (4) **PP TUNAS (PP 17/2025)** child-protection
  — games in scope, age-verification/parental-consent/age-tiering, enforced **28 Mar
  2026**.
- **Data (PDP Law UU 27/2022) [high]:** in force (transition ended Oct 2024);
  private-sector data **may stay offshore** (PP71/2019 Art. 21) but must remain
  accessible to regulators; DPO mandate broadened by Constitutional Court (Jul 2025);
  fines up to 2% revenue + criminal.
- **Payments [high]:** GoPay (88%) / DANA (83%) / OVO / ShopeePay + carrier +
  convenience-store cash. Hosting: **Jakarta** (AWS `ap-southeast-3` / GCP / Azure 2025)
  or Singapore (~9–10ms hop).

#### Vietnam — heaviest + a business-model conflict (do last / via local publisher)

- **Market / fit [high]:** ~58.5M players, **86.6% mobile**; charts favour **Chinese-
  style ARPGs** → a 传奇-lineage game fits taste; ~$0.65–0.8B domestic.
- **Licensing (heaviest) [high]:** **Decree 147/2024** — a Mir2-like is a **G1** game
  (multiplayer via server) = **G1 License + per-game content/release Decision**, and
  **cross-border operation is prohibited** → you **must** run through a **licensed
  Vietnamese entity** (own or local publisher). Foreign ownership **49% baseline / up to
  100% for CPTPP-country investors (since 14 Jan 2024)** — investor's home country is
  decisive; residual local-JV expectation unsettled. Plus minor playtime caps,
  **mandatory VN-phone identity/age verification**, ban on casino/card-image games.
- **⚠️ Business-model conflict [high]:** Decree 147 says in-game **virtual items cannot
  be exchanged for cash/cards/real value AND player-to-player item trading is
  prohibited** — directly at odds with a 传奇-like "loot, trade, grind-economy" loop.
  Vietnam needs a reworked economy or it's a non-starter.
- **Data (PDPL Law 91/2025, eff 1 Jan 2026 + Decree 13/2023) [high]:** DPIA +
  cross-border-transfer dossiers to MPS/A05 within 60 days; **Decree 53/2022**
  localization is conditional for foreigners (online games are an enumerated in-scope
  service; triggered only by an MPS order) — but a local licensed entity pulls you
  toward domestic-storage duties anyway. Fines up to 5% revenue for transfer breaches.
- **Payments:** MoMo / ZaloPay (36M users) + prepaid cards + Mobile Money. Hosting:
  **Singapore** (no local region; watch subsea-cable cuts); Cloudflare HAN/SGN edge.

### Comparative table

| | BR | PH | TH | ID | VN |
|---|---|---|---|---|---|
| Entry barrier | Low | Low | Medium | High | **Highest** |
| Local entity to launch? | No\* | No | No\** | **Yes** (PT) | **Yes** (licensed) |
| Game license? | No | No | No (draft pending) | PSE + IGRS | **G1 + per-game** |
| Mandatory data rep? | No | DPO/NPC reg | **Yes (s.37(5))** | local rep (PSE) | conditional (MPS) |
| Digital-goods tax | MoR / WHT ~25% | **12% VAT >₱3M** | **7% VAT >฿1.8M** | local PT taxable | local entity |
| Genre fit | **Highest** (Tibia) | Strong | Strong (top spend) | Strong | Strong (CN-style) |
| Payments | Pix / Boleto | GCash / Maya | TrueMoney / PromptPay | GoPay / DANA | MoMo / ZaloPay |
| Item-economy legal? | Yes | Yes | Yes | Yes | **No (no trade/cash-out)** |
| Wemade in-market | Strong | **Strongest** | Yes | Yes (localized) | None licensed |
| USTR 301 (2024) | Watch | **None** | Watch | **Priority** | Watch |
| Hosting | São Paulo | Singapore | AWS/GCP TH | Jakarta | Singapore |

\* Brazil: no entity for licensing, but ECA Digital Art. 40 forces a **legal
representative** from 17 Mar 2026. \** Thailand: offshore OK today, but appoint a PDPA
s.37(5) representative + register 7% VAT.

### Recommended launch sequencing

1. **Brazil** — open, single-language (pt-BR), strongest genre fit, simplest payments.
   Prove the whole original-IP + cross-platform pipeline here. (Budget for ECA Digital:
   real age-gating + BR legal rep + no minor loot boxes by Mar 2026.)
2. **Philippines → Thailand** — Philippines is the cleanest SEA entry (no game license,
   100% foreign-ownable, English, GCash; just 12% VAT + DPA). Thailand: offshore-feasible
   today (PDPA rep + 7% VAT), highest payer spend; watch the draft Game Act.
3. **Indonesia** — big, but a real market-entry project: PT/PT-PMA or JV + PSE + IGRS +
   PP TUNAS.
4. **Vietnam — last, or only via a licensed local publisher**, and only after reworking
   the item economy to fit Decree 147's no-trade / no-cash-out rule.

### Cross-cutting: a configurable compliance layer

Every market is rolling out **2025–2026 minor-protection regimes** — Brazil **ECA
Digital** (loot-box ban for minors, real age verification, parental consent <16),
Indonesia **PP TUNAS** (age-tiering, parental consent), Vietnam **Decree 147** (playtime
caps, phone-based age verification), Thailand/Philippines parental-consent-for-minors
under PDPA/DPA. Build **age verification + parental consent + minor purchase limits as a
per-region toggle in the client** — one engineering effort reused across all five (same
"compliance/skin layer" as the rebrand pipeline). **i18n TODO:** the bundle is
en/zh-CN/es today; add **pt-BR (≠ es), id, th, vi**.

### Time-sensitive — re-verify with counsel

Vietnam CPTPP-100% / JV nuance + item-economy rules (Decree 147 text); Indonesia IGRS
post-breach status + Perpres 19/2024 entity enforcement + PT-PMA KBLI cap; Brazil ECA
Digital lead enforcer + Art. 40 threshold for small operators; Thailand draft Game
Industry Act status + offshore applicability; per-country actual MIR/传奇 trademark
registrations (unverified). Most figures are search-summarised (fetch 403-blocked) —
confirm primary texts.

### Sources (representative)

- IP / Wemade: koreaherald.com/article/2346151; linklaters.com (SICC 2023);
  kedglobal.com (2025-04 royalties); clarivate.com/blog/game-ip-china-korea/;
  ustr.gov 2024 Special 301; wipo.int WIPOLex (Berne); support.google.com/googleplay
  (IP complaints).
- Brazil: planalto/camara.leg.br (Lei 14.852/2024); machadomeyer.com.br &
  migalhas.com.br (Lei 15.211/2025 ECA Digital, Art. 40); dejus Portaria 368/2014 &
  MJSP 1.048/2025; iapp.org/news/a/an-overview-of-brazils-lgpd; mayerbrown.com (ANPD
  SCCs 2024/2025); segpay.com & pagbrasil.com (Pix/Boleto).
- Philippines: lawyer-philippines.com & chambers (no game license; PAGCOR=gambling);
  EO 175 Negative List (lexology/forvismazars); privacy.gov.ph (DPA, NPC 2022-04);
  RA 12023 12% VAT (pwc.com.ph, ey.com, grantthornton.com.ph); RA 11967 ITA.
- Thailand: tilleke.com (draft Game Industry Act; CCA); nagashima.com (FVA online-game
  exemption); boi.go.th & unctad (FBA); nortonrosefulbright.com (PDPA s.37(5));
  rd.go.th (e-Service VAT 7%).
- Indonesia: niko/abnr/ssek/nagashima (Perpres 19/2024 PT requirement); MR5/2020 PSE
  (norton rose; theregister 2022 blocking); MR2/2024 IGRS (kk-advocates, makarim);
  PP 17/2025 PP TUNAS (hoganlovells, jurist.org); itif.org (PP71/2019 Art. 21).
- Vietnam: tilleke.com & dfdl.com & nikopartners.com (Decree 147/2024 G1, virtual-item
  & cross-border ban); bakermckenzie (CPTPP 100% from 14 Jan 2024); kpmg & freshfields
  (Decree 53/2022 localization); tilleke.com (PDPL Law 91/2025).
- Market / hosting: nikopartners.com (SEA-6 $5.37B/285M); tibiaqa.com (Brazil = Tibia
  #1); datareportal.com (penetration); aws/gcp/azure region docs (São Paulo, Jakarta,
  Thailand 2025/2026, Singapore); knowledge.antom.com & xsolla.com (payments).
