# System Ownership & Interface Contracts (Architect Coordination)

> Owner: the **architect/review** session. Updated: 2026-05-30.
> Companion to `docs/AGENT-ORCHESTRATION.md` (orchestration) and
> `docs/PRODUCTION-GAP-ASSESSMENT.md` (the parity bar each system targets).

## Role model

- **This (architect) session**: architecture + review only. Owns the shared
  contracts below; does not implement individual game systems.
- **Each game-system session**: implements one system on its own branch,
  opens a PR to `main`, and is reviewed here.
- **Base branch**: `main` (currently `5f47c3d9`). Always branch off the latest
  `main` and rebase before merge.

## Session → module ownership

Each system **owns** its primary files (only that session edits them per round).
Files marked *shared* require coordination through the architect.

| System (session) | Owns (primary) | Shared — coordinate, don't edit alone |
| --- | --- | --- |
| 人物状态 Character State | `runtime/components.rs`, `runtime/save.rs`, `runtime/buffs.rs`, `runtime/equipment.rs` | StatBlock contract; `crystal_compat.rs` stat tables |
| 地图 Map | `runtime/map.rs`, `packages/game-data/src/lib.rs`, `apps/web/lib/crystal-map-loader.ts`, `apps/web/scripts/export-*` | `runtime/zone/collision.rs` (shared w/ Movement) |
| 移动 Movement | `runtime/movement.rs`, `runtime/zone/movement.rs` | `runtime/zone/collision.rs` (shared w/ Map); `runtime/zone/runtime.rs` (Multiplayer) |
| 技能魔法 Skills/Magic | `runtime/skills.rs` | Combat damage API; StatBlock |
| 怪物AI Monster AI | `runtime/monster_ai.rs`, `runtime/monsters.rs`, `runtime/hero_ai.rs` | Combat damage API; Zone authority |
| 战斗 Combat | `runtime/combat.rs`, `runtime/drops.rs` | StatBlock contract (architect); Zone authority |
| 多人 Multiplayer/Zone | `runtime/zone/runtime.rs`, `zone/manager.rs`, `zone/aoi.rs`, `zone/packets.rs`, `world_runtime.rs`, gateway `routing.rs`/`cache.rs`/`session.rs` | the *logic* inside combat/AI (those systems own it; Multiplayer owns the tick/AOI host) |
| 可玩 Playable (vertical slice) | `apps/web/*` client, smoke scripts, scene render, integration tests | does not change core sim logic — files bugs/PRs to the owning system |

## Architect-owned shared contracts (single source of truth)

1. **StatBlock & damage/mitigation** — the Mir2 stat sheet (AC/AMC, DC/MC/SC as
   `min..max`, Accuracy/Agility/Luck) derived from `base(class, level) +
   equipment + buffs`; damage = `Random(minX, maxX)`; AC/MAC absorb a random
   share; crit/accuracy-vs-agility. One definition; nobody forks it.
   *(Directly fixes the audit finding: combat currently uses flat
   `18 + level/2 + equip` with no AC/MAC mitigation — see
   `PRODUCTION-GAP-ASSESSMENT.md`.)*
2. **Shared Zone authority interfaces** — how combat / monster-AI / NPC mutation
   / pickup run **once** in the authoritative Zone tick instead of per-session.
   Movement, Combat, Monster AI, and Multiplayer all depend on this seam.
3. **ECS component schema** — vitals / position / equipment / buff components
   (co-owned with Character State; architect arbitrates breaking changes).
4. **Parity acceptance bar** — per-system "done = Crystal parity" criteria.
   No silent `fallback` / `simplified` / `stub` vs Crystal: it must be called
   out in the PR description so review can accept or reject it.

## Hot files — one editor per round (coordinate through architect)

- `runtime/zone/runtime.rs` (Multiplayer) — **the hottest**; combat/AI/movement
  land here only through agreed seams.
- `runtime/combat.rs` (Combat) — Skills/AI/Char-State call its API, never edit it.
- `runtime/zone/collision.rs` (Map + Movement) — shared; coordinate.
  *(PR #4 just modified this — Movement/Map sessions must rebase on it.)*
- `runtime/components.rs` (Char State) — schema changes ripple everywhere.
- `packages/protocol/*`, `runtime/packets.rs` — wire format; architect sign-off.

## Inter-system interface contracts (the seams)

- **Character State** → `effective_stats(entity) -> StatBlock`.
- **Combat** → `roll_attack(attacker, defender, kind) -> DamageResult`;
  `apply_damage(world, target, DamageSpec)`. Skills & AI call these — they
  never reimplement damage or mitigation.
- **Skills** → `cast(world, caster, Spell, target) -> SkillOutcome`, built on
  Combat's `apply_damage` + StatBlock; shared preflight (MP/cooldown/LOS/safezone).
- **Monster AI** → emits `Intent { Move | Attack | Cast | Flee }`; Movement and
  Combat execute it. AI never mutates vitals directly.
- **Map** → `cell_blocked(p)`, `transfer_source(p)`, `safe_zone(p)`; Movement consumes.
- **Multiplayer/Zone** → owns the tick loop + AOI broadcast; systems register
  their per-tick systems against the single shared `World`.

## Integration order

1. Architect lands **StatBlock** + **Zone-authority interface** stubs (small,
   reviewed) — unblocks everyone to deepen in parallel against stable seams.
2. Character State implements stat derivation; Combat consumes it (replaces the
   flat formula, adds AC/MAC mitigation + min..max rolls).
3. Skills and Monster AI build on Combat's damage API.
4. Movement + Map coordinate on `collision.rs`; Multiplayer promotes combat/AI
   ticks into the shared Zone (per the authority interface).
5. Playable continuously integrates the **Bichon vertical slice** as the smoke gate.

## Branch / PR / review protocol

- Branch off latest `main`; **one system per branch**; PR → `main`.
- Keep PRs small and single-system; do not edit another system's owned files.
- The architect session **subscribes to every PR** and reviews for: correctness,
  parity shortcuts (any `fallback`/`simplified`/`stub` vs Crystal must be
  flagged), contract adherence, hot-file conflicts, and test coverage.
  Ambiguous calls are escalated to the human via `AskUserQuestion`.
- Rebase on `main` before merge; resolve hot-file conflicts through the architect.

## Current review queue

| PR | Branch | Area | Status |
| --- | --- | --- | --- |
| #4 | `claude/keen-hawking-ALdqD` | Movement/Map — collision transfer (Bichon Library door) | architect review |
| #2 | `fix/vercel-scene-bundle` | Web infra — Turbopack bundle size | triage |
| #3 | `fix/candidate-gate-submodule` | CI — checkout unblock | triage |
