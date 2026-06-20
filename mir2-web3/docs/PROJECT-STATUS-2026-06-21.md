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
| **Per-monster AI breadth** — 35 specialized behaviors vs Crystal's **212** | code (Rust) | port from `Crystal/Server/MirObjects/Monsters/*.cs` | **yes, but** see structural note ↓ |
| Cross-process Zone sharding / single-owner handoff | design + infra | durable Zone snapshot/log + real RPC | no (serial design) |
| Persistence normalization (inventory/mail/economy/auction) | code | per-account JSON blobs → normalized tables | partly |
| VFX real atlases + audio *bytes* | asset-gated | extract Crystal `.Lib`/`Sound` on a real machine → R2 publish | no (hardware/credential) |
| Real-GPU / mobile actor-render sign-off | hardware-gated | headed device QA (sandbox lacks GPU) | no |
| A few unwirable window actions (conquest gate/tax, hero dismiss/recall) | protocol-gated | new packet or NPC-script flow | no |

### Structural note on the monster-AI gap (the #1 lane)

The current monster AI is **one file** — `apps/simulation/src/runtime/monster_ai.rs`
(6,139 lines) — with a **central `match agent.ai { … }`** numeric dispatch
(20 AI ids handled) calling 35 `update_<monster>_state` functions. Crystal's 212
monster subclasses each override `ProcessAI`/`Attack`/`MoveTo` etc.

This means a **naive 212-wide parallel fan-out would collide** — every agent
would edit the same `monster_ai.rs` + the same dispatch. Closing this lane in
parallel requires one of:

1. **Per-monster modules first** (serial refactor): split `monster_ai.rs` into
   `monster_ai/<name>.rs`, each registering its AI id, so agents add *new files*
   (disjoint write set) + one tiny dispatch edit. Then fan out safely.
2. **Batched additive ports**: each agent (isolated worktree) owns a disjoint
   group of monsters, produces additive behavior fns, and integrates via PR with
   a single owner resolving the central-dispatch merge. Accepts merge friction.

Either way, the project's parallel rule holds: **isolated git worktrees, disjoint
file domains, integrate via PR — never shared trees** (`CLAUDE.md`,
`AGENT-ORCHESTRATION.md`).

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
