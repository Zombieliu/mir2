# Project Status Snapshot — 2026-06-21

Current state of `mir2-web3` (Crystal → web port) on `main`. Evidence-focused.
Supersedes `PROJECT-STATUS-2026-06-15.md`. The 06-15 → 06-21 window was a
**verification/hardening pass** (a full automated QA-loop suite + 2 fixes), not a
feature-percentage pass — so the headline completion numbers are unchanged and are
now *better-verified* rather than higher.

## Headline (overall completion)

| Layer | Completion | Bounded by |
|---|---|---|
| Whole-project **Accepted Crystal 1:1** | **≈ 90%** (estimate, not final) | human visual/feel acceptance gate (`PARITY-TRUTH-AUDIT.md`) |
| Automated parity evidence | **100% Candidate** | — |
| Protocol · **ServerPacket** (inbound) | **98.6%** (278/282) | 4 vestigial variants |
| Protocol · **ClientPacket** (outbound) | **44.4%** literal / **~72.5%** via gateway bridge (111/153) | endgame/social protocol |
| Frontend · **visual client** | **≈ 90.5%** | committed assets + real-GPU sign-off |
| Frontend · **playable game** (end-to-end loop) | **≈ 74.0%** | protocol breadth + asset bytes |
| Backend · **gameplay depth** (strict prod口径) | **≈ 85%** | per-monster AI breadth |

- **Backend tests:** ~**1272** `#[test]`/`#[tokio::test]` in `mir2-simulation`.
- **Deploy:** Player Web live at `https://mir2.obelisk.build` (Vercel + Cloudflare
  Worker); Gateway on UCloud; active asset release
  `mir2/v/20260601-fullcrystal-a2f10be0` (complete full-Crystal upload, 0 missing).

The ~10–16 pt gap to 1:1 is **not "main features missing"** — it is (a) gameplay
*depth* (per-monster AI), (b) externally-supplied *asset bytes* (audio/VFX),
(c) production *architecture* (Zone sharding / persistence normalization), and
(d) the final *human acceptance* pass.

## Landed since 2026-06-15

This window built a **comprehensive automated QA harness** over the *built*
surfaces, plus two real cross-layer fixes:

**QA loops (CDP-driven, drive the real client + verify against WS truth):**

| Loop | PR | Covers |
|---|---|---|
| `qa-playthrough.mjs` | #124/#130 | register→login→quest→move + combat/cross-map/inventory beats + camera A/B |
| `qa-persistence.mjs` | #131 | logout/login + WS reconnect-resume state survival |
| `qa-render-sweep.mjs` | #132 | 17-map render robustness + WebGPU→WebGL2 fallback guard |
| `qa-load-stress.mjs` | #133 | movement load-stress (self-move overshoot/snap repro) |
| `qa-items.mjs` | #134 | inventory→pickup→use→equip→NPC buy/sell |
| `qa-combat-survival.mjs` | #135 | combat/survival from WS truth |
| `qa-web3.mjs` | #138 | Sui passkey/wallet login + on-chain mine |
| `qa-quests.mjs` | #139 | full quest arc (accept→turn-in→reward) |
| `qa-economy.mjs` | #140 | storage/mail/market economy |
| `qa-social.mjs` | #141 | two-client party/trade/whisper/friends/marriage |
| `qa-magic-skills.mjs` | #142 | magic/skill book→bind→cast self/monster/ground |

**Fixes:** death → town-revive wired across sim/gateway/client (#137);
over-cap self-move prediction clamped instead of discarded — closes the
overshoot/snap drift (#136); quest-reward verification corrected to check
belt + WS truth, not just React state (#143).

Net: the surfaces a player exercises (login, move, fight, loot, talk, quest,
trade, economy, magic, persistence, web3) now each have a re-runnable
verification loop. **What's built is now proven; what's unbuilt is the gap list
below.**

## Remaining gaps (by nature — what closes them)

| Gap | Nature | Path to close | Parallelizable? |
|---|---|---|:---:|
| **Per-monster AI breadth** — *reassessed 2026-06-21, see note ↓; the dominant gap was one systemic bug, now fixed* | code (Rust) | mostly DONE; low-value long tail remains | n/a |
| Cross-process Zone sharding / single-owner handoff | design + infra | durable Zone snapshot/log + real RPC | no (serial design) |
| Persistence normalization (inventory/mail/economy/auction) | code | per-account JSON blobs → normalized tables | partly |
| VFX real atlases + audio *bytes* | asset-gated | extract Crystal `.Lib`/`Sound` on a real machine → R2 publish | no (hardware/credential) |
| Real-GPU / mobile actor-render sign-off | hardware-gated | headed device QA (sandbox lacks GPU) | no |
| A few unwirable window actions (conquest gate/tax, hero dismiss/recall) | protocol-gated | new packet or NPC-script flow | no |

### The monster-AI gap, reassessed (2026-06-21)

A parallel-agent investigation (classify → validation-wave → gap-analysis →
respawn-manifest cross-reference) found the "35 of 212" framing was misleading,
and that the gap was dominated by **one systemic bug**:

1. **The sim already covers far more than 35.** Beyond the per-monster modules,
   `monsters.rs` has data-driven AI-id-keyed tables — `monster_player_attack_damage`
   (~43 ids), `monster_player_status_effect`, `monster_attack_range`,
   `monster_can_attack` — plus summon/line branches. Monsters not in any of these
   run the generic default (chase + melee), which is *faithful* for plain melee mobs.

2. **Only 87 of 212 AI families are actually spawned** (`crystal_respawn_manifest.json`,
   6,341 spawn groups); the other 125 are data-only. Of the 87 spawned, **64 already
   have dedicated handlers**. The elaborate boss subclasses (EvilMir, Behemoth, the
   Oma*/Horned*/Flame* families) are **not in timed respawns** — they're event/quest
   spawned, an endgame-fidelity long tail.

3. **The real bug: `ai = 0` (base `MonsterObject`) — 3,588 of 6,341 spawns / 251
   distinct monster names — had no arm in `monster_player_attack_damage` and fell
   through to a `_ => 7` stub.** The bulk of the live world hit the player for a flat
   7 regardless of its real DC. **Fixed** (catch-all → imported DC, matching the
   already-correct zone path; +regression test; full suite 1277/0).

**Net:** the high-impact monster-combat gap is closed. The `monster_ai.rs` →
per-monster-module refactor (committed) makes the remaining low-value long tail
(event-spawned bosses) safe to port on demand: a new monster = new
`monster_ai/<name>.rs` + one dispatch arm; workers write disjoint files and only the
coordinator edits `mod.rs` (no N-way collision), per the project's isolated-worktree /
integrate-via-PR rule (`CLAUDE.md`, `AGENT-ORCHESTRATION.md`).

## Verify / reproduce

```bash
# completion numbers in this doc
cd mir2-web3/apps/web && node ./scripts/measure-frontend-coverage.mjs

# backend tests (toolchain pin matters — default 1.87 cannot build bevy)
cd mir2-web3 && cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1

# any QA loop (needs a running gateway + next dev; see each script header)
cd mir2-web3/apps/web && node ./scripts/qa-<name>.mjs

# live health
curl https://mir2.obelisk.build/api/asset-manifest   # remoteAssets.assetBaseUrl non-null
```

See also: `PARITY-TRUTH-AUDIT.md` (status wording authority),
`FRONTEND-COMPLETENESS-AUDIT.md` (per-module %), `CRYSTAL-1TO1-ROADMAP.md`
(backend roadmap), `AGENT-ORCHESTRATION.md` (parallel working rules).
