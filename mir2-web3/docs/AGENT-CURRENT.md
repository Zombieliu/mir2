# Agent Current Context

Short routing summary, updated 2026-10-27 (Asia/Shanghai).
This indexes recorded decisions and open work; it does not revalidate today's
working tree, services, release packages, source snapshots or parity percentages.

## Start with the user task

- A specific request defines the scope. Do not start unrelated gameplay or release
  work because a historical document says to continue a previous round.
- Small tasks read relevant source/tests only. For substantial implementation or
  general queue work, read [coordination](AGENT-ORCHESTRATION.md) and this summary,
  then only the relevant sections linked below.
- Inspect the current diff before editing; the shared workspace contains unrelated
  work. Confirm active configuration/code only where needed for the requested task.

## Last recorded decisions and open work

| Topic | Recorded state and next scope | Detailed reference |
| --- | --- | --- |
| Overall parity | Candidate100 remains false in the latest summarized records; final human acceptance remains open. Do not invent an overall percentage. | [Roadmap](CRYSTAL-1TO1-ROADMAP.md), relevant topic section |
| Cell preproduction P0 / P1 (2026-10-27) | P1 passed; P0 incomplete, P2 not started. Historical formal2/10 passed;25 combat frequency failed; final500 target retains original100 P0 gate. Old518ef24b Root519+301=820 checks passed. V3 partial100 bank freshly reused first4 stock12, then actor5 held658 consumed with ACK but exactHP stayed12/18 and original2s recovery timed out; prepared4/100, zero measured CCU, cleanup/PID71364/ports verified. Candidates: normalUseItem shortTick wake plus selfObjectHealth-as-raw-refresh-cue; no candidate live acceptance or new25/50/100/500/2h pass. | [Preparation/500 scope](generated/cell-p0-p1-completion-20261024/capacity-ordinary-economic-v1/README.md), [V13](generated/cell-p0-p1-completion-20261024/capacity-online-presence-publication-v1/README.md), [Plan](CELL-ZONING-PREPRODUCTION-PLAN.zh-CN.md) |
| Level 100 / SLG (2026-10-19) | User direction is a level-100 cap with new SLG after max level. Resource audit/planning is recorded; runtime cap, level 71-100 content, balancing, ordinary acquisition and server entry gates still require implementation/verification. Keep custom rules separate from Crystal's original baseline. | [Growth rules](LEVEL100-SLG-PROGRESSION.zh-CN.md), [equipment audit](EQUIPMENT-LEVEL100-SLG-AUDIT-20261019.zh-CN.md), [queue](AGENT-TASK-QUEUE.md) topic heading |
| Cross-chat integration (2026-10-14) | Code/push/validation scope was reconciled. Signed R26 paired release, event-site re-publication and player acceptance remained open in that record; confirm current status only for related tasks. | [Code progress](CODE-PROGRESS-20261014.zh-CN.md) |
| Web delivery (2026-10-12) | Source78 records a verified empty Spells bootstrap/reentry branch. It does not establish nonempty skills, every panel, combat/quest, mobile or final acceptance. Do not rebuild/retest that historical package for unrelated tasks. | [Frontend gaps](FRONTEND-1TO1-GAPS.md), [Windows QA](NATIVE-WINDOWS-PLAYER-QA.md), queue Source78 section |
| Shared world / authentication | Session owns personal state; Zone owns shared world authority. Enforce authentication, safe routing and final transform before save. Inspect present implementation rather than assuming an old MVP description is current. | [Zone rules](SHARED-ZONE-MVP-RULES.md), relevant runtime/gateway source |

## Read by task

| Task | Start here; widen only if needed |
| --- | --- |
| Simulation behavior | `apps/simulation/src/runtime/` and `apps/simulation/tests/`; matching backend/parity topic sections |
| Gateway/auth/session/routing | `apps/gateway/src/`, `apps/simulation/src/world_runtime.rs`; Zone rules when lifecycle/movement is involved |
| Player Web/UI | `apps/web/app/`, `apps/web/lib/`, `apps/web/scripts/`, `apps/web/package.json`; relevant gaps/QA section |
| Native/shared client | `apps/game-client/`; relevant native QA section |
| Data/assets | `packages/game-data/`, `packages/tooling/`, the relevant generator and manifest; use explicit asset/evidence paths |
| Architecture/integration | [Architecture reference](ARCHITECTURE-CURRENT.md) plus current source at affected boundaries; dated implementation claims require checking |
| Resume after context loss | This file and coordination, then task-specific saved evidence; [old handoff](AGENT-RESUME-HANDOFF.md) only for the relevant historical topic |

All paths above are relative to `mir2-web3`. Source/configuration manifests remain
visible in default search; binary media, `work/`, packaged copies and generated QA
evidence are excluded by root `.ignore` and remain available by explicit path.

## Bounded lookup examples

From the repository root:

```text
rg -n "<symbol>" mir2-web3/apps/simulation/src mir2-web3/apps/simulation/tests
rg -n "^##|<topic>" mir2-web3/docs/AGENT-TASK-QUEUE.md
rg --no-ignore -n "<topic>" mir2-web3/docs/generated/<relevant-evidence-directory>
rg --files --no-ignore mir2-web3/apps/web/public/<relevant-library> -g '*.png'
```

Read only the matching section. For queue selection, prefer the newest relevant
user decision over stale completed-round tables. Keep this summary below 120
lines; record a detailed round once in its topic document and link to it here.
