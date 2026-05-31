# Integration / Merge-Order Assessment

> Owner: architect/review session. **Point-in-time** snapshot — 2026-05-31.
> Companion to `SYSTEM-OWNERSHIP-AND-INTERFACES.md` (who owns what) and
> `SCALABILITY-AND-CAPACITY.md` (the roadmap). This is the safe order to land the
> currently-open PRs from the parallel game-system sessions, with the conflicts
> and one compile hazard found by actually test-merging each branch into `main`.
>
> Method: for every open PR, `git merge-base` (staleness) + `git merge-tree
> --write-tree` into the current `origin/main` (real conflict set + materialized
> merged tree inspected for silent breakage). Nothing was merged or pushed to
> those branches — this is read-only analysis.

## Readiness matrix (vs `origin/main` @ `0a32e66a`)

| PR | Track / branch | Base behind main | Merge conflicts | Compile risk on merge | Verdict |
| --- | --- | ---: | --- | --- | --- |
| **#19** | CI gate · `ci-pr-gate-fix` | 6 | none | none (workflow only, +10/−2) | **Merge FIRST** — see decision below |
| **#22** | architect (this) · `optimistic-gates-hMc7O` | 0 | none | none — oracle **4/4 green** | merge anytime (docs + tests only) |
| **#21** | zone combat · `laughing-ride-D4Dkg` | 1 | none | none | merge clean |
| **#17** | persistence/ops · `trusting-faraday-1x8nx` | 10 | none | none | merge clean |
| **#8** | A* pathfinding · `optimistic-mayer-gswKV` | 36 | 2 (`tests.rs`, `zone/runtime.rs`) | — | rebase; low-urgency (module unwired) |
| **#13** | map system + world authority · `gifted-bardeen-Kg0cs` | 36 | 3 (`monster_ai.rs`, `tests.rs`, `zone/runtime.rs`) | **`E0428` duplicate fn** (see hazard) | rebase **and** drop stale crit helper |

## The one hard hazard: PR #13 duplicate `crystal_apply_player_critical`

`main` already carries the stat-engine `crystal_apply_player_critical` (landed via
#9/#12/#16). PR #13 was branched **36 commits earlier**, before that existed, and
carries its **own older copy** of the same function. The text test-merge is
deceptive: `combat.rs` is reported as *auto-merged* (not a conflict) because the
two definitions sit at **different line ranges** — so git keeps **both**. The
materialized merged tree has:

```
combat.rs:267  fn crystal_apply_player_critical(   // PR #13's stale copy  (4-arg call @3017)
combat.rs:387  fn crystal_apply_player_critical(   // main's stat-engine copy (5-arg call @425)
```

Two definitions, **different arity** → guaranteed `error[E0428]: the name
'crystal_apply_player_critical' is defined multiple times` (and semantically the
stale one is the inferior version). This is exactly what PR #13's own description
warned about ("prefer #9/#12's stat-engine versions and drop this branch's
`crystal_apply_player_critical` if it conflicts"). **Action on rebase:** delete
#13's stale copy + its call site, keep `main`'s; #13's real value is the map
system + world-authority work, not combat.

## Recommended order

1. **PR #19 (CI gate) first.** It's foundational: until it lands, the PR gate is
   *silently green* (the `changes` job 403s and skips the Rust build + web gate),
   so every "CI passing" below is currently meaningless. Landing it makes the
   signal real for everything after. **Decision point (yours):** it will turn the
   gate **red**, because it exposes the ~46 pre-existing `mir2-simulation` parity
   failures already on `main` (combat/stat/magic, e.g. `casting_summon_shinsu…`).
   That red is *honest*, not new breakage. Either (a) land it and accept
   red-but-honest CI while a gameplay-owned session triages the 46, or (b) land it
   together with a triage plan. Don't keep the false green.
2. **Clean batch, any order: #21, #17, #22.** Conflict-free, recent, each verified
   green on its own track (#21 zone-combat parity, #17 persistence on live PG, #22
   architect docs/tests). #21 should precede #13 (see below) so combat-crit
   converges on the stat-engine version before the stale branch rebases onto it.
3. **Rebase + re-verify the stale pair, last:**
   - **#13** — rebase onto post-#21 `main`; resolve the 3 conflicts
     (`monster_ai.rs`, `tests.rs`, `zone/runtime.rs`); **drop the stale crit
     helper** (hazard above); re-run `shared_zone`. Highest-value of the two (real
     map-system + world-authority gameplay).
   - **#8** — rebase; resolve 2 conflicts. **Low urgency**: the `pathfind.rs`
     module is not yet wired into movement, so it changes no behavior; can wait
     until the movement session integrates it.

## Cross-PR thread: combat-crit converges *if* ordered right

`main` (stat-engine crit) → **#21** extends it cleanly into the **zone** path
(1-behind, 0 conflicts) → **#13**, rebased *after*, drops its stale duplicate.
Land #13 before #21/main reconciliation and you reintroduce the inferior helper.
Order is the safeguard; the matrix above encodes it.

## Hot-file contention (confirms the ownership rule)

`apps/simulation/src/runtime/zone/runtime.rs` and `runtime/tests.rs` are the
**recurring** conflict center — *both* stale branches (#8, #13) collide there,
because the L2 zone work (PR #18) and the combat-parity work (#21) both moved
through `zone/runtime.rs`. This is the live evidence behind
`SYSTEM-OWNERSHIP-AND-INTERFACES.md`'s hot-file rule: edits to `zone/runtime.rs`
must be serialized through the zone/多人 owner and rebased promptly; a branch that
sits 36 commits back on this file is guaranteed pain. Keep PRs that touch it
small, current, and frequently rebased.

## What this assessment is not

Merge execution is the maintainer's call — this is analysis, not a merge. No
branch here was modified. Re-run the matrix after each merge (every merge shifts
`main` and can change the conflict set for the stale branches).
